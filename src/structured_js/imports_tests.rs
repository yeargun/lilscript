//! Authored ESM contracts exercise the same target/verifier/namer/printer owners.
//! Rust tests are staged outside the repository until coordinated integration.
use super::naming::{Plan, Style};
use super::*;
use crate::compilation_contract::{JavaScriptExecution, JavaScriptWorld};
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const MODES: [analysis::Mode; 5] = [
    analysis::Mode::Tree,
    analysis::Mode::Indexed,
    analysis::Mode::Memoized,
    analysis::Mode::Regions,
    analysis::Mode::Values,
];
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "lilscript-esm-imports-{}-{}",
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
            eprintln!("ESM import fixture retained: {}", self.0.display());
        } else {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn execute(consumer: &str, files: &[(&str, &str)], caught: bool) -> String {
    let dir = Directory::new();
    std::fs::write(dir.0.join("consumer.mjs"), consumer).unwrap();
    for (name, code) in files {
        std::fs::write(dir.0.join(name), code).unwrap();
    }
    let path = serde_json::to_string(dir.0.join("consumer.mjs").to_str().unwrap()).unwrap();
    let script = if caught {
        format!("import{{pathToFileURL}}from'node:url';try{{await import(pathToFileURL({path}));console.log('loaded')}}catch(e){{console.log(e.name)}}")
    } else {
        format!("import{{pathToFileURL}}from'node:url';await import(pathToFileURL({path}));")
    };
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for structural import execution tests");
    assert!(
        result.status.success(),
        "{}\n{consumer}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}
fn binding(module: &Module, name: &str) -> BindingId {
    BindingId::new(
        module
            .bindings
            .iter()
            .position(|b| b.spelling == name)
            .unwrap(),
    )
}
fn replace_declaration(module: &mut Module, name: &str, source: &str, imported: &str) {
    let id = binding(module, name);
    module.regions[module.root.index()].statements.retain(|s| {
        !matches!(s,
        Statement::Let { binding, .. } | Statement::Function { binding, .. } if *binding == id)
    });
    module.import(source, imported, id);
}
fn policy(module: bool) -> crate::compilation_policy::ResolvedPolicy {
    crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: module,
        })
        .unwrap()
}

#[test]
fn imported_live_cells_keep_snapshots_across_calls_in_every_analysis_and_name_plan() {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena,
        "int longCounter=0;void advance(){}int prior=longCounter;advance();print(prior);print(longCounter);int a=7;int nested(int prior){return prior+longCounter;}print(nested(a));").unwrap();
    let semantics = crate::analyze(&source).unwrap();
    for mode in MODES {
        let mut tree = lower::lower_slice(&source, &semantics).unwrap();
        tree.edit(|module| {
            replace_declaration(module, "longCounter", "./producer.mjs", "publicCounter");
            replace_declaration(module, "advance", "./producer.mjs", "publicAdvance");
        })
        .unwrap();
        optimize::optimize_in_execution(
            &mut tree,
            mode,
            JavaScriptWorld::ClosedApplication,
            JavaScriptExecution::Module,
        )
        .unwrap();
        assert_eq!(tree.target().imports.len(), 2);
        let view = extract::JavaScriptView::prepare_in_execution(
            &tree,
            mode,
            JavaScriptWorld::ClosedApplication,
            JavaScriptExecution::Module,
        );
        let output = view.output().unwrap();
        for style in STYLES {
            let js = output.render(&Plan::new(style)).unwrap();
            assert_eq!(execute(&js, &[("producer.mjs",
                "console.log('init');export let publicCounter=1;export function publicAdvance(){publicCounter+=1}")], false), "init\n1\n2\n9\n", "{mode:?}/{style:?}\n{js}");
            assert!(js.contains("publicCounter") && js.contains("publicAdvance"));
        }
    }
}

#[test]
fn unused_import_rows_keep_authored_module_order_link_failure_and_initialization() {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, "int first=0;int second=0;print(99);").unwrap();
    let semantics = crate::analyze(&source).unwrap();
    for mode in MODES {
        let mut tree = lower::lower_slice(&source, &semantics).unwrap();
        tree.edit(|module| {
            replace_declaration(module, "first", "./first.mjs", "unused");
            replace_declaration(module, "second", "./second.mjs", "unused");
        })
        .unwrap();
        optimize::optimize_in_execution(
            &mut tree,
            mode,
            JavaScriptWorld::ClosedApplication,
            JavaScriptExecution::Module,
        )
        .unwrap();
        assert_eq!(tree.target().imports.len(), 2);
        let js = tree
            .target()
            .prepare_output_with_policy(&policy(true))
            .unwrap()
            .render(&Plan::new(Style::Global))
            .unwrap();
        assert_eq!(
            execute(
                &js,
                &[
                    ("first.mjs", "console.log('first');export const unused=0;"),
                    (
                        "second.mjs",
                        "console.log('second');throw Error('stop');export const unused=0;"
                    )
                ],
                true
            ),
            "first\nsecond\nError\n"
        );
        assert_eq!(
            execute(
                &js,
                &[
                    (
                        "first.mjs",
                        "console.log('must-not-run');export const unused=0;"
                    ),
                    ("second.mjs", "export const different=0;")
                ],
                true
            ),
            "SyntaxError\n"
        );
    }
}

#[test]
fn cyclic_import_read_keeps_tdz_even_when_its_payload_is_discarded() {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, "int count=0;int probe(){count;return 7;}").unwrap();
    let semantics = crate::analyze(&source).unwrap();
    for mode in MODES {
        let mut tree = lower::lower_slice(&source, &semantics).unwrap();
        tree.edit(|module| {
            replace_declaration(module, "count", "./producer.mjs", "count");
            module.exports.push(Export {
                binding: binding(module, "probe"),
                name: "probe".into(),
            });
        })
        .unwrap();
        optimize::optimize_in_execution(
            &mut tree,
            mode,
            JavaScriptWorld::ClosedApplication,
            JavaScriptExecution::Module,
        )
        .unwrap();
        let js = extract::JavaScriptView::prepare_in_execution(
            &tree,
            mode,
            JavaScriptWorld::ClosedApplication,
            JavaScriptExecution::Module,
        )
        .output()
        .unwrap()
        .render(&Plan::new(Style::Scoped))
        .unwrap();
        assert_eq!(execute(&js, &[("producer.mjs", "import{probe}from'./consumer.mjs';console.log('before');probe();export let count=1;")], true), "before\nReferenceError\n", "{mode:?}\n{js}");
        assert_eq!(execute(&js, &[("producer.mjs", "import{probe}from'./consumer.mjs';export let count=1;console.log('before');console.log(probe());")], true), "before\n7\nloaded\n");
    }
}

#[test]
fn fixed_import_export_names_and_escaped_specifier_survive_local_alias_mangling() {
    let mut module = Module::default();
    let imported = module.binding(Binding {
        source_symbol: Some(SymbolId(0)),
        scope: ScopeId::new(0),
        spelling: "descriptiveAlias".into(),
        pinned: false,
    });
    module.import("./producer\"quoted.mjs", "default", imported);
    module.exports.push(Export {
        binding: imported,
        name: "publicAlias".into(),
    });
    let local = module.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "a".into(),
        pinned: true,
    });
    let value = module.expression(Expr::Literal(Literal::Number(7.0)), None);
    module.regions[0].statements.push(Statement::Let {
        binding: local,
        value: Some(value),
    });
    let output = module.prepare_output_with_policy(&policy(true)).unwrap();
    for style in STYLES {
        let js = output.render(&Plan::new(style)).unwrap();
        assert!(js.contains("import{default as ") && js.contains(" as publicAlias"));
        assert!(js.contains("producer\\\"quoted.mjs"));
        let dir = Directory::new();
        std::fs::write(dir.0.join("producer\"quoted.mjs"), "export default 17;").unwrap();
        std::fs::write(dir.0.join("consumer.mjs"), &js).unwrap();
        let path = serde_json::to_string(dir.0.join("consumer.mjs").to_str().unwrap()).unwrap();
        let script = format!("import{{pathToFileURL}}from'node:url';const m=await import(pathToFileURL({path}));console.log(JSON.stringify(Object.keys(m)),m.publicAlias)");
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{js}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "[\"publicAlias\"] 17\n"
        );
    }
}

#[test]
fn imports_reject_missing_duplicate_nested_writable_and_invalid_name_declarations() {
    let mut module = Module::default();
    module.import("./producer.mjs", "x", BindingId::new(0));
    assert!(module
        .verify()
        .unwrap_err()
        .contains("unknown import binding"));
    let id = module.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "value".into(),
        pinned: false,
    });
    module.verify().unwrap();
    module.import("./producer.mjs", "other", id);
    assert!(module.verify().unwrap_err().contains("duplicate import"));
    module.imports.pop();
    module.regions[0].statements.push(Statement::Let {
        binding: id,
        value: None,
    });
    assert!(module
        .verify()
        .unwrap_err()
        .contains("duplicate binding declaration"));
    module.regions[0].statements.clear();
    let nested = module.region(ScopeId::new(0));
    module.bindings[id.index()].scope = module.regions[nested.index()].scope;
    assert!(module.verify().unwrap_err().contains("binding declaration"));
    module.bindings[id.index()].scope = ScopeId::new(0);
    module.imports[0].imported = "not-an-IdentifierName".into();
    assert!(module
        .verify()
        .unwrap_err()
        .contains("imported export name"));
    module.imports[0].imported = "default".into();
    let target = module.expression(Expr::Binding(id), None);
    let value = module.expression(Expr::Literal(Literal::Number(2.0)), None);
    let assign = module.expression(Expr::Assign { target, value }, None);
    module.regions[0]
        .statements
        .push(Statement::Evaluate(assign));
    assert!(module.verify().unwrap_err().contains("readonly import"));
    // Readonly is the import binding itself, not properties of its value.
    module.regions[0].statements.clear();
    module.expressions.clear();
    module.origins.clear();
    let object = module.expression(Expr::Binding(id), None);
    let target = module.expression(
        Expr::Member {
            object,
            property: Property::Named("payload".into()),
        },
        None,
    );
    let value = module.expression(Expr::Literal(Literal::Number(3.0)), None);
    let write = module.expression(Expr::Assign { target, value }, None);
    module.regions[0]
        .statements
        .push(Statement::Evaluate(write));
    module.verify().unwrap();
}

fn ledger(memory: u64, work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}
#[test]
fn import_payload_and_row_admission_release_refusals_without_partial_declarations() {
    let source = "./long-producer-name.mjs";
    let imported = "publicFunction";
    let payload = (source.len() + imported.len()) as u64;
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        for (memory, work) in [
            (0, 100_000),
            (payload - 1, 100_000),
            (
                payload + 4 * std::mem::size_of::<Import>() as u64 - 1,
                100_000,
            ),
            (100_000, 0),
        ] {
            let mut module = Module::default();
            let mut ledger = ledger(memory, work);
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                assert!(module
                    .import_in(source, imported, BindingId::new(0), &mut budget)
                    .is_err());
                assert!(module.imports.is_empty());
                assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
                assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
                drop(module);
            }
            assert_eq!(ledger.retained_bytes(), 0);
        }
        let mut module = Module::default();
        let mut ledger = ledger(100_000, 100_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
            module
                .import_in(source, imported, BindingId::new(0), &mut budget)
                .unwrap();
            let retained = module.imports.capacity() * std::mem::size_of::<Import>()
                + module.imports[0].source.capacity_bytes()
                + module.imports[0].imported.capacity();
            assert_eq!(
                budget.retained_bytes(AllocationClass::Retained),
                retained as u64
            );
            drop(module);
            budget
                .release(AllocationClass::Retained, retained as u64)
                .unwrap();
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn imports_require_module_policy_and_printer_limits_never_return_partial_imports() {
    let mut module = Module::default();
    let id = module.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "value".into(),
        pinned: true,
    });
    module.import("./producer.mjs", "value", id);
    assert!(module
        .prepare_output_with_policy(&policy(false))
        .err()
        .unwrap()
        .contains("module execution"));
    let output = module.prepare_output_with_policy(&policy(true)).unwrap();
    let plan = Plan::new(Style::Source);
    let text = output.render(&plan).unwrap();
    assert_eq!(text, "import{value}from\"./producer.mjs\";");
    assert!(output.render_bounded(&plan, text.len() - 1).is_err());
    assert_eq!(output.render_bounded(&plan, text.len()).unwrap(), text);
    // An unused request still pays its source payload verification work.
    let mut expensive = Module::default();
    let id = expensive.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "value".into(),
        pinned: false,
    });
    expensive.import(&"x".repeat(10_000), "value", id);
    let mut ledger = ledger(100_000, 100);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        assert!(expensive
            .prepare_output_admitted(&policy(true), &mut budget)
            .is_err());
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    let empty = Module::default();
    assert_eq!(empty.render(PrintPolicy::default()).unwrap(), "");
    assert!(empty.imports.is_empty());
}
