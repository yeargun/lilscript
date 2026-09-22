use super::*;
use crate::compilation_policy::{BaselineFirstPlan, BudgetError, ResourceLimits};
use crate::parser::admitted_arena_activity_for_test;
use crate::semantic::with_analyzed_modules;
use crate::semantic_program::from_checked_modules_admitted;
use crate::semantic_program::publication::{CheckpointLimit, Compilation};
use std::sync::atomic::{AtomicU64, Ordering};

const WORK: u64 = 10_000_000;
const MEMORY: u64 = 10_000_000;
const SENTINEL: u64 = 37;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-parse-once-{}-{}",
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
    let mut ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: work,
            retained_bytes: memory + SENTINEL,
            terminal_work: 0,
        },
    )
    .unwrap();
    ledger.retain(WorkDomain::Baseline, SENTINEL).unwrap();
    ledger
}

fn assert_same_graph(actual: &ModuleSet<&str>, expected: &ModuleSet) {
    assert_eq!(actual.root, expected.root);
    assert_eq!(actual.eager, expected.eager);
    assert_eq!(actual.dependency_order, expected.dependency_order);
    assert_eq!(
        actual.for_of_specialize_family,
        expected.for_of_specialize_family
    );
    assert_eq!(actual.modules.len(), expected.modules.len());
    for (actual, expected) in actual.modules.iter().zip(&expected.modules) {
        assert_eq!(actual.path, expected.path);
        assert_eq!(actual.source, expected.source);
        assert_eq!(actual.dependencies, expected.dependencies);
        assert_eq!(actual.foreign_dependencies, expected.foreign_dependencies);
        assert_eq!(actual.dynamic_dependencies, expected.dynamic_dependencies);
        assert_eq!(actual.offset, expected.offset);
    }
}

fn discover_and_discard(
    root: &Path,
    ledger: &mut BudgetLedger,
) -> Result<(), ModuleDiscoveryError> {
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let result = {
        let syntax = AdmittedArena::new(ledger, WorkDomain::Baseline);
        discover_parsed_modules_admitted(root, &ProjectConfig::default(), &sources, &syntax).map(
            |(modules, programs)| {
                assert_eq!(modules.modules.len(), programs.len());
                drop(programs);
                drop(modules);
            },
        )
    };
    sources.discard(ledger).unwrap();
    result
}

#[test]
fn retained_discovery_parses_each_canonical_source_once_and_preserves_checked_identity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/modules-javascript/entry.lil");
    let expected = discover_modules_configured(&root, &ProjectConfig::default()).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let before = admitted_arena_activity_for_test();
    let syntax = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let (modules, programs) =
        discover_parsed_modules_admitted(&root, &ProjectConfig::default(), &sources, &syntax)
            .unwrap();
    assert_same_graph(&modules, &expected);
    assert_eq!(programs.len(), modules.modules.len());
    assert_eq!(
        admitted_arena_activity_for_test(),
        (before.0 + 1, before.1 + programs.len())
    );
    let identities: Vec<_> = programs
        .iter()
        .map(|program| program.source_identity().clone())
        .collect();
    let prepared = syntax.with_ledger(|ledger, domain| {
        let mut budget = AllocationBudget::new(Some((ledger, domain)));
        with_analyzed_modules(&programs, &modules, &mut budget, |checked, budget| {
            for (module, program) in programs.iter().enumerate() {
                assert_eq!(checked.source(module), Some(program.source_identity()));
            }
            from_checked_modules_admitted(&programs, checked, budget)
        })
        .unwrap()
        .unwrap()
    });
    prepared.program().verify().unwrap();
    let unit_count = prepared.program().units().len();
    assert_eq!(
        admitted_arena_activity_for_test().1,
        before.1 + programs.len()
    );
    let source_bytes = sources.allocated_bytes() as u64;
    assert!(source_bytes > 0);
    drop(programs);
    drop(syntax);
    for (module, identity) in prepared.program().modules().iter().zip(&identities) {
        assert!(module.source.same(identity));
    }
    assert_eq!(admitted_arena_activity_for_test().0, before.0);
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let semantic = compilation.adopt_prepared(prepared).unwrap();
    assert_eq!(compilation.view(semantic).unwrap().unit_count(), unit_count);
    let mut ledger = compilation.finish();
    assert_eq!(ledger.retained_bytes(), SENTINEL + source_bytes);
    for (actual, expected) in modules.modules.iter().zip(&expected.modules) {
        assert_eq!(actual.source, expected.source);
    }
    drop(modules);
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn cycle_and_repeated_canonical_edges_keep_one_program_per_module() {
    let scratch = Scratch::new();
    let root = scratch.0.join("entry.lil");
    fs::write(&root, "import {a} from \"./a\";import {a as again} from \"./a.lil\";export int entry(){return 1;}").unwrap();
    fs::write(
        scratch.0.join("a.lil"),
        "import {entry} from \"./entry.lil\";export int a(){return 2;}",
    )
    .unwrap();
    let expected = discover_modules(&root).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let before = admitted_arena_activity_for_test();
    {
        let syntax = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let (modules, programs) =
            discover_parsed_modules_admitted(&root, &ProjectConfig::default(), &sources, &syntax)
                .unwrap();
        assert_same_graph(&modules, &expected);
        assert_eq!(programs.len(), 2);
        assert_eq!(modules.modules[0].dependencies, [1, 1]);
        assert_eq!(modules.modules[1].dependencies, [0]);
        assert_eq!(modules.dependency_order, [1, 0]);
        assert_eq!(admitted_arena_activity_for_test().1, before.1 + 2);
        for (module, program) in modules.modules.iter().zip(programs.iter()) {
            for import in program.imports {
                let source_start = module.source.as_ptr() as usize;
                let borrowed_start = import.source.as_ptr() as usize;
                assert!(borrowed_start >= source_start);
                assert!(borrowed_start + import.source.len() <= source_start + module.source.len());
            }
        }
    }
    assert_eq!(
        ledger.retained_bytes(),
        SENTINEL + sources.allocated_bytes() as u64
    );
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn static_foreign_and_nested_dynamic_imports_share_the_original_collector() {
    let scratch = Scratch::new();
    let root = scratch.0.join("entry.lil");
    fs::write(&root, "import {a} from \"./a\";import extern \"./setup.js\";void load(){auto task=import(\"./lazy\");}").unwrap();
    fs::write(scratch.0.join("a.lil"), "export int a(){return 1;}").unwrap();
    fs::write(scratch.0.join("lazy.lil"), "export int value(){return 2;}").unwrap();
    fs::write(scratch.0.join("setup.js"), "globalThis.ready=true;").unwrap();
    let expected = discover_modules(&root).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let before = admitted_arena_activity_for_test();
    {
        let syntax = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let (modules, programs) =
            discover_parsed_modules_admitted(&root, &ProjectConfig::default(), &sources, &syntax)
                .unwrap();
        assert_same_graph(&modules, &expected);
        assert_eq!(programs.len(), 3);
        assert_eq!(modules.eager, [true, true, false]);
        assert_eq!(modules.modules[0].foreign_dependencies.len(), 1);
        assert_eq!(modules.modules[0].dynamic_dependencies, [2]);
        assert_eq!(admitted_arena_activity_for_test().1, before.1 + 3);
        let expected_error = crate::semantic::analyze_modules(&programs, &expected)
            .err()
            .unwrap();
        let error = syntax.with_ledger(|ledger, domain| {
            let mut budget = AllocationBudget::new(Some((ledger, domain)));
            with_analyzed_modules(&programs, &modules, &mut budget, |_, _| ()).unwrap_err()
        });
        let crate::semantic::AdmittedSemanticError::Semantic(actual) = error.error else {
            panic!("expected the same unsupported module diagnostic");
        };
        assert_eq!(error.module, expected_error.module);
        assert_eq!(actual, expected_error.error);
    }
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn discovery_failures_keep_original_diagnostics_and_release_both_arenas() {
    let scratch = Scratch::new();
    let root = scratch.0.join("entry.lil");
    let dependency = scratch.0.join("dependency.lil");
    fs::write(&root, "import {value} from \"./dependency\";print(1);").unwrap();
    for dependency_source in [
        None,
        Some(&b"export int bad( {"[..]),
        Some(&b"\xff\xfe"[..]),
    ] {
        if let Some(source) = dependency_source {
            fs::write(&dependency, source).unwrap();
        }
        let expected = discover_modules(&root).unwrap_err();
        let mut ledger = ledger(WORK, MEMORY);
        let before = admitted_arena_activity_for_test();
        let error = discover_and_discard(&root, &mut ledger).unwrap_err();
        let ModuleDiscoveryError::Module(actual) = error else {
            panic!("original module diagnostic");
        };
        assert_eq!(actual, expected);
        assert!(ledger.peak_retained_bytes() > SENTINEL);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(admitted_arena_activity_for_test().0, before.0);
    }
}

#[test]
fn retained_discovery_refusals_keep_typed_errors_and_release_partial_owners() {
    let scratch = Scratch::new();
    let root = scratch.0.join("entry.lil");
    fs::write(&root, "import {answer} from \"./answer\";print(answer());").unwrap();
    fs::write(
        scratch.0.join("answer.lil"),
        "export int answer(){return 42;}",
    )
    .unwrap();
    let mut full = ledger(WORK, MEMORY);
    discover_and_discard(&root, &mut full).unwrap();
    let work = full.work_used(WorkDomain::Baseline);
    let memory = full.peak_retained_bytes() - SENTINEL;
    assert!(work > 4);
    assert!(memory > 4);
    for allowance in [0, work / 2, work - 1] {
        let mut limited = ledger(allowance, MEMORY);
        assert!(matches!(
            discover_and_discard(&root, &mut limited),
            Err(ModuleDiscoveryError::Resources(AllocationError::Budget(
                BudgetError::WorkExhausted(WorkDomain::Baseline)
            )))
        ));
        assert_eq!(limited.retained_bytes(), SENTINEL);
    }
    for allowance in [0, memory / 2, memory - 1] {
        let mut limited = ledger(WORK, allowance);
        assert!(matches!(
            discover_and_discard(&root, &mut limited),
            Err(ModuleDiscoveryError::Resources(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Baseline)
            )))
        ));
        assert_eq!(limited.retained_bytes(), SENTINEL);
    }
    let mut expired = BudgetLedger::new_baseline_first(
        ResourceLimits {
            wall_time_ms: Some(1),
            ..ResourceLimits::default()
        },
        BaselineFirstPlan {
            logical_work: WORK,
            retained_bytes: MEMORY,
            terminal_work: 0,
        },
    )
    .unwrap();
    expired.retain(WorkDomain::Baseline, SENTINEL).unwrap();
    expired.set_deadline_elapsed_for_test(std::time::Duration::from_millis(1));
    assert!(matches!(
        discover_and_discard(&scratch.0.join("absent.lil"), &mut expired),
        Err(ModuleDiscoveryError::Resources(AllocationError::Budget(
            BudgetError::DeadlineExceeded
        )))
    ));
    assert_eq!(expired.retained_bytes(), SENTINEL);
    assert_eq!(expired.peak_retained_bytes(), SENTINEL);
}

#[test]
fn retained_root_override_uses_the_same_loader_without_reading_old_text() {
    let scratch = Scratch::new();
    let root = scratch.0.join("entry.lil");
    fs::write(&root, "invalid old source").unwrap();
    let text = "export int answer(){return 17;}";
    let expected = discover_modules_with_source(&root, text).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let before = admitted_arena_activity_for_test();
    {
        let syntax = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let (modules, programs) = discover_with_storage(
            &root,
            Some(text),
            None,
            RetainedSources {
                sources: &sources,
                syntax: &syntax,
                parsed: syntax.parsed_sources(),
            },
        )
        .unwrap();
        assert_same_graph(&modules, &expected);
        assert_eq!(programs.len(), 1);
        assert_eq!(admitted_arena_activity_for_test().1, before.1 + 1);
        assert_ne!(modules.modules[0].source.as_ptr(), text.as_ptr());
    }
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}
