//! Original-file module checking and the existing direct native target share
//! one initialization contract. Fixed traces precede artifact measurements.
use super::native_tests::{compile_and_execute, execute};
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain, WorkKind,
};
use crate::js::selection::{Objective, Plan, Style};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const FILES: &[(&str, &str)] = &[
    (
        "entry.lil",
        include_str!("fixtures/modules-native/entry.lil"),
    ),
    ("boot.lil", include_str!("fixtures/modules-native/boot.lil")),
    (
        "cycle-a.lil",
        include_str!("fixtures/modules-native/cycle-a.lil"),
    ),
    (
        "cycle-b.lil",
        include_str!("fixtures/modules-native/cycle-b.lil"),
    ),
    (
        "barrel.lil",
        include_str!("fixtures/modules-native/barrel.lil"),
    ),
];
const EXPECTED: &str = include_str!("fixtures/modules-native/expected.out");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-native-modules-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("module native failure inputs: {}", self.0.display());
        } else {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn with_modules<R>(
    files: &[(&str, &str)],
    inspect: impl FnOnce(Program<'_>, &crate::module::ModuleSet) -> R,
) -> R {
    let directory = Directory::new();
    for &(name, source) in files {
        std::fs::write(directory.0.join(name), source).unwrap();
    }
    let modules = crate::module::discover_modules(&directory.0.join("entry.lil")).unwrap();
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let checked = crate::check::analyze_modules(&syntax, &modules).unwrap();
    let program = from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    inspect(program, &modules)
}

fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap()
}

fn digest(value: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(value.as_ref()))
}

#[test]
fn native_module_cycles_instantiate_all_functions_before_ordered_evaluation() {
    with_modules(FILES, |program, modules| {
        assert!(program.exports().is_empty(), "executable has no public ABI");
        assert_eq!(program.modules().len(), 5);
        assert_eq!(
            program
                .modules()
                .iter()
                .map(|m| m.exports.len())
                .sum::<usize>(),
            3,
            "dependency exports remain checked internal interfaces"
        );
        let initialization: Vec<_> = program
            .initialization()
            .iter()
            .map(|&id| {
                modules.modules[program.unit(id).unwrap().module.index()]
                    .path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            initialization,
            [
                "boot.lil",
                "cycle-b.lil",
                "cycle-a.lil",
                "barrel.lil",
                "entry.lil"
            ]
        );
        let private_owners: Vec<_> = program
            .cells()
            .iter()
            .filter(|cell| cell.name == "privateValue")
            .map(|cell| program.unit(cell.owner).unwrap().module)
            .collect();
        assert_eq!(private_owners.len(), 2);
        assert_ne!(private_owners[0], private_owners[1]);

        let mut compilation = compilation();
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let native_policy = crate::config::ProjectConfig::default()
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        let before = compilation.ledger().retained_bytes();
        let codecs_before = compilation.ledger().work_by_kind(WorkKind::Codec);
        let c = compilation
            .with_native_c(source, &native_policy, WorkDomain::Baseline, |output| {
                output.take_c()
            })
            .unwrap();
        assert_eq!(compilation.ledger().retained_bytes(), before);
        assert_eq!(
            compilation.ledger().work_by_kind(WorkKind::Codec),
            codecs_before
        );
        assert_eq!(compilation.checkpoint_count(), 1);
        let executions = compile_and_execute(&c, EXPECTED, "module-instantiation-cycle");

        let config: crate::config::ProjectConfig =
            toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: false,
            })
            .unwrap();
        let candidate = compilation
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let javascript = compilation
            .with_javascript_output(candidate, &policy, |output| {
                let mut rows = Vec::new();
                for style in [Style::Global, Style::Scoped, Style::Source] {
                    let artifact = output.render(&Plan::new(style))?;
                    output.with_artifact(artifact, |view| {
                        let ran = execute(Command::new("node").args([
                            "--input-type=module",
                            "-e",
                            view.javascript,
                        ]));
                        assert!(
                            ran.status.success(),
                            "{}",
                            String::from_utf8_lossy(&ran.stderr)
                        );
                        assert!(ran.stderr.is_empty());
                        assert_eq!(String::from_utf8_lossy(&ran.stdout), EXPECTED, "{style:?}");
                    })?;
                    let raw = output.measure(artifact, Objective::Raw)?;
                    let gzip9 = output.measure(artifact, Objective::Gzip)?;
                    let brotli11 = output.measure(artifact, Objective::Brotli)?;
                    let text = output.take_artifact(artifact)?;
                    rows.push(serde_json::json!({
                        "style": format!("{style:?}"), "javascript_sha256": digest(&text),
                        "javascript": text, "raw": raw, "gzip9": gzip9, "brotli11": brotli11,
                    }));
                }
                Ok::<_, CandidateError>(rows)
            })
            .unwrap()
            .unwrap();
        let sources: Vec<_> = FILES
            .iter()
            .map(|(name, source)| {
                serde_json::json!({
                    "file": name, "source": source, "source_sha256": digest(source),
                })
            })
            .collect();
        eprintln!(
            "module-native-artifact {}",
            serde_json::json!({
                "case": "module-instantiation-cycle", "sources": sources,
                "initialization": initialization, "expected": EXPECTED,
                "c_sha256": digest(&c), "c": c, "executions": executions,
                "javascript": javascript,
                "qualification": "original per-file ASTs; direct semantic-core native client; fixed trace; two C compilers and Node; no old linker/backend",
            })
        );
        assert_eq!(compilation.finish().retained_bytes(), 0);
    });
}

#[test]
fn native_modules_share_imported_and_function_captured_scalar_storage() {
    for (case, entry, dependency) in [
        (
            "imported scalar",
            "import { value } from \"./state\";print(value);",
            "export int value=4;",
        ),
        (
            "function captures scalar",
            "import { read } from \"./state\";print(read());",
            "int value=4;export int read(){return value;}",
        ),
    ] {
        with_modules(
            &[("entry.lil", entry), ("state.lil", dependency)],
            |program, _| {
                let mut compilation = compilation();
                let source = compilation
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let policy = crate::config::ProjectConfig::default()
                    .resolve_policy(CompilationRequest::Native)
                    .unwrap();
                let before = compilation.ledger().retained_bytes();
                // A module binding another module or a function reads is one
                // file-scope slot, guarded until its initializer runs.
                let c = compilation
                    .with_native_c(source, &policy, WorkDomain::Baseline, |output| output.take_c())
                    .unwrap_or_else(|error| panic!("{case}: {error:?}"));
                assert!(c.contains("ls_native_unbound"), "{case}");
                compile_and_execute(&c, "4\n", "module-global-scalar");
                assert_eq!(compilation.ledger().retained_bytes(), before);
                assert_eq!(compilation.checkpoint_count(), 1);
                assert_eq!(compilation.finish().retained_bytes(), 0);
            },
        );
    }
}

#[test]
fn module_instantiation_prefix_cannot_include_an_evaluation_operation() {
    with_modules(FILES, |mut program, _| {
        let root = program
            .initialization()
            .iter()
            .copied()
            .find(|&id| {
                let data = program.unit(id).unwrap();
                data.instantiation_prefix > 0
                    && (data.instantiation_prefix as usize)
                        < data.regions[data.entry.index()].operations.len()
            })
            .unwrap();
        let mut working = program.units.remove(root.index()).into_working();
        working.get_mut().instantiation_prefix += 1;
        program.units.insert(root.index(), working.freeze());
        assert!(
            program.verify().is_err(),
            "an explicit boundary cannot promote ordinary evaluation into instantiation"
        );
    });
}

#[test]
fn module_evaluation_schedule_cannot_duplicate_or_omit_a_root() {
    for duplicate in [false, true] {
        with_modules(FILES, |mut program, _| {
            let order = std::sync::Arc::get_mut(&mut program.initialization).unwrap();
            if duplicate {
                order.push(order[0]);
            } else {
                order.remove(0);
            }
            assert!(
                program.verify().is_err(),
                "all required module evaluations execute exactly once; duplicate={duplicate}"
            );
        });
    }
}

#[test]
fn module_function_body_cannot_claim_another_source_owner() {
    with_modules(FILES, |mut program, _| {
        let function = program
            .units()
            .iter()
            .find(|unit| unit.data().kind == UnitKind::Function)
            .unwrap();
        let unit = function.id();
        let original = function.data().module;
        let other = (0..program.modules().len())
            .map(|index| ModuleId::from_index(index).unwrap())
            .find(|&module| module != original)
            .unwrap();
        let mut working = program.units.remove(unit.index()).into_working();
        working.get_mut().module = other;
        program.units.insert(unit.index(), working.freeze());
        assert!(
            program.verify().is_err(),
            "source-local spans and identities cannot acquire another module owner"
        );
    });
}
