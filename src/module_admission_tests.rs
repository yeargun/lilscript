use super::*;
use crate::compilation_policy::{BaselineFirstPlan, BudgetError, ResourceLimits};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-source-admission-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn ledger(work: u64, memory: u64) -> BudgetLedger {
    BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: work,
            retained_bytes: memory,
            terminal_work: 0,
        },
    )
    .unwrap()
}

#[test]
fn admitted_source_graph_matches_legacy_and_releases_exact_capacities() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/modules-javascript/entry.lil");
    let config = ProjectConfig::default();
    let legacy = discover_modules_configured(&root, &config).unwrap();
    let mut ledger = ledger(1_000_000, 1_000_000);
    let (modules, charge) =
        discover_modules_configured_admitted(&root, &config, &mut ledger).unwrap();
    assert_eq!(modules.root, legacy.root);
    assert_eq!(modules.eager, legacy.eager);
    assert_eq!(modules.dependency_order, legacy.dependency_order);
    assert_eq!(modules.modules.len(), legacy.modules.len());
    for (actual, expected) in modules.modules.iter().zip(&legacy.modules) {
        assert_eq!(actual.path, expected.path);
        assert_eq!(actual.source, expected.source);
        assert_eq!(actual.dependencies, expected.dependencies);
        assert_eq!(actual.foreign_dependencies, expected.foreign_dependencies);
        assert_eq!(actual.dynamic_dependencies, expected.dynamic_dependencies);
        assert_eq!(actual.offset, expected.offset);
    }
    let capacity = modules
        .modules
        .iter()
        .map(|module| module.source.capacity() as u64)
        .sum::<u64>();
    assert!(capacity > 0);
    assert_eq!(charge.bytes(), capacity);
    assert_eq!(ledger.retained_bytes(), capacity);
    drop(modules);
    assert_eq!(ledger.retained_bytes(), capacity);
    charge.release(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn source_growth_admits_old_and_new_capacities_before_allocating() {
    let scratch = Scratch::new();
    let path = scratch.0.join("large.lil");
    let text = format!("{}print(1);", " ".repeat(20_000));
    fs::write(&path, &text).unwrap();
    let mut refused = ledger(1_000_000, 16_384);
    {
        let mut budget = AllocationBudget::new(Some((&mut refused, WorkDomain::Baseline)));
        assert!(matches!(
            read_module_source(&path, &mut budget),
            Err(ModuleDiscoveryError::Resources(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Baseline)
            )))
        ));
    }
    assert_eq!(refused.peak_retained_bytes(), 8192);
    assert_eq!(refused.retained_bytes(), 0);
    let mut admitted = ledger(1_000_000, 49_152);
    {
        let mut budget = AllocationBudget::new(Some((&mut admitted, WorkDomain::Baseline)));
        let source = read_module_source(&path, &mut budget).unwrap();
        assert_eq!(source, text);
        assert_eq!(source.capacity(), 32_768);
        assert_eq!(budget.retained_bytes(Retained), source.capacity() as u64);
        drop(source);
    }
    assert_eq!(admitted.peak_retained_bytes(), 49_152);
    assert_eq!(admitted.retained_bytes(), 0);
}

#[test]
fn source_work_and_deadline_refusals_are_typed_and_release_partial_reads() {
    let scratch = Scratch::new();
    let path = scratch.0.join("large.lil");
    fs::write(&path, " ".repeat(20_000)).unwrap();
    let mut limited = ledger(20_000, 100_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut limited, WorkDomain::Baseline)));
        assert!(matches!(
            read_module_source(&path, &mut budget),
            Err(ModuleDiscoveryError::Resources(AllocationError::Budget(
                BudgetError::WorkExhausted(WorkDomain::Baseline)
            )))
        ));
    }
    assert_eq!(limited.peak_retained_bytes(), 8192);
    assert_eq!(limited.retained_bytes(), 0);
    let mut expired = BudgetLedger::new_baseline_first(
        ResourceLimits {
            wall_time_ms: Some(1),
            ..ResourceLimits::default()
        },
        BaselineFirstPlan {
            logical_work: 1_000_000,
            retained_bytes: 100_000,
            terminal_work: 0,
        },
    )
    .unwrap();
    expired.set_deadline_elapsed_for_test(std::time::Duration::from_millis(1));
    let error = discover_modules_configured_admitted(
        &scratch.0.join("missing.lil"),
        &ProjectConfig::default(),
        &mut expired,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ModuleDiscoveryError::Resources(AllocationError::Budget(BudgetError::DeadlineExceeded))
    ));
    assert_eq!(expired.retained_bytes(), 0);
    assert_eq!(expired.peak_retained_bytes(), 0);
}

#[test]
fn malformed_and_non_utf8_sources_release_admitted_storage() {
    let scratch = Scratch::new();
    let path = scratch.0.join("bad.lil");
    for source in [&b"export int bad( {"[..], &b"\xff\xfe"[..]] {
        fs::write(&path, source).unwrap();
        let mut ledger = ledger(1_000_000, 1_000_000);
        let error =
            discover_modules_configured_admitted(&path, &ProjectConfig::default(), &mut ledger)
                .unwrap_err();
        let ModuleDiscoveryError::Module(error) = error else {
            panic!("source diagnostic");
        };
        assert_eq!(error.path, fs::canonicalize(&path).unwrap());
        if source[0] == 0xff {
            assert!(error.message.contains("stream did not contain valid UTF-8"));
            assert!(error.source.is_empty());
        } else {
            assert_eq!(error.source.as_bytes(), source);
        }
        assert!(ledger.peak_retained_bytes() > 0);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn missing_import_preserves_parent_diagnostic_after_admitted_cleanup() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    let source = "import {missing} from \"./missing.lil\";print(1);";
    fs::write(&path, source).unwrap();
    let config = ProjectConfig::default();
    let expected = discover_modules_configured(&path, &config).unwrap_err();
    let mut ledger = ledger(1_000_000, 1_000_000);
    let error = discover_modules_configured_admitted(&path, &config, &mut ledger).unwrap_err();
    let ModuleDiscoveryError::Module(actual) = error else {
        panic!("source diagnostic");
    };
    assert_eq!(actual, expected);
    assert_eq!(actual.path, fs::canonicalize(&path).unwrap());
    assert_eq!(actual.source, source);
    assert!(actual.message.contains("missing.lil"));
    assert!(ledger.peak_retained_bytes() > source.len() as u64);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn root_override_transfers_one_admitted_buffer_into_the_module() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    fs::write(&path, "invalid old source").unwrap();
    let source = "export int answer(){return 17;}";
    let mut ledger = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let modules = discover_modules_inner(&path, Some(source), None, &mut budget).unwrap();
        assert_eq!(modules.modules.len(), 1);
        assert_eq!(modules.modules[0].source, source);
        assert_eq!(modules.modules[0].source.capacity(), source.len());
        assert_eq!(budget.retained_bytes(Retained), source.len() as u64);
        drop(modules);
    }
    assert!(ledger.peak_retained_bytes() > source.len() as u64);
    assert_eq!(ledger.retained_bytes(), 0);
}
