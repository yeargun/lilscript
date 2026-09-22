//! Inspect the actual selected target, with the unchanged executed oracle as a
//! source input. Existing public runtime cohorts measure all names and codecs.
use super::demand::DemandMode;
use super::helper_family::FamilyOutcome;
use super::implementations::ImplementationMap;
use super::javascript;
use super::product_family;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{AnalysisAttempt, CompilationRequest, WorkDomain};
use crate::output_budget::{AllocationBudget, AllocationClass};
use crate::structured_js as js;

const ORIGINAL: &str = include_str!("fixtures/search-reference-runtime/opaque-int-field.lil");

fn with_targets(
    source: &str,
    fields: bool,
    inline_overwrite: bool,
    mut inspect: impl FnMut(&js::Module, DemandMode),
) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let mut ledger = super::helper_family_tests::ledger();
    let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
    let mut map = ImplementationMap::direct();
    if fields {
        let root = CellId::from_index(
            program
                .cells()
                .iter()
                .position(|cell| cell.name == "state")
                .unwrap(),
        )
        .unwrap();
        let analyzed = product_family::analyze_published(
            &program,
            &uses,
            root,
            product_family::FamilyRequest {
                execution: crate::compilation_contract::JavaScriptExecution::Module,
                attempt: AnalysisAttempt {
                    plan: product_family::PRODUCT_FAMILY_PLAN,
                    algorithm_version: product_family::PRODUCT_FAMILY_VERSION,
                    work_quota: 1_000_000,
                },
                scratch_bytes: 1_000_000,
                output_bytes: 1_000_000,
            },
            &mut ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        let product_family::FamilyOutcome::Complete(family) = analyzed.outcome else {
            panic!("{analyzed:?}");
        };
        let next = map
            .with_product(family, &mut ledger, WorkDomain::Optional)
            .unwrap();
        map.discard(&mut ledger).unwrap();
        map = next;
    }
    if inline_overwrite {
        let mut cache = super::helper_family_tests::cache(&mut ledger);
        let (outcome, _) = super::helper_family_tests::analyze(
            &program,
            &uses,
            super::helper_family_tests::root(&program, "overwrite"),
            &mut ledger,
            &mut cache,
        );
        let FamilyOutcome::Complete(family) = outcome else {
            panic!("{outcome:?}");
        };
        let next = map
            .with_inline_helper(family, &mut ledger, WorkDomain::Optional)
            .unwrap();
        map.discard(&mut ledger).unwrap();
        map = next;
        cache.discard(&mut ledger).unwrap();
    }
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    for mode in [DemandMode::Prune, DemandMode::Preserve] {
        for compact in [false, true] {
            let retained = ledger.retained_bytes();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                let (module, literals) = javascript::lower_admitted(
                    &program,
                    &uses,
                    &map,
                    policy.javascript_contract().unwrap(),
                    mode,
                    compact,
                    &mut budget,
                )
                .unwrap();
                module.verify().unwrap();
                assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
                inspect(&module, mode);
                drop(literals);
                drop(module);
            }
            assert_eq!(ledger.retained_bytes(), retained);
        }
    }
    map.discard(&mut ledger).unwrap();
    uses.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}

fn function(module: &js::Module, spelling: &str) -> Option<js::FunctionId> {
    module.regions[module.root.index()]
        .statements
        .iter()
        .find_map(|statement| {
            let js::Statement::Function { binding, function } = *statement else {
                return None;
            };
            let binding = &module.bindings[binding.index()];
            (binding.source_symbol.is_none() && binding.spelling == spelling).then_some(function)
        })
}

fn paths(module: &js::Module, rebuild: bool) -> usize {
    let mut count = 0;
    for statement in &module.regions[module.root.index()].statements {
        let js::Statement::Let {
            binding,
            value: Some(mut value),
        } = *statement
        else {
            continue;
        };
        let binding = &module.bindings[binding.index()];
        if binding.source_symbol.is_some() || binding.spelling != "ref_path" {
            continue;
        }
        count += 1;
        loop {
            let js::Expr::Array(elements) = &module.expressions[value.index()] else {
                assert!(matches!(
                    module.expressions[value.index()],
                    js::Expr::Literal(js::Literal::Null)
                ));
                break;
            };
            assert_eq!(elements.len(), 3);
            if rebuild {
                assert!(matches!(
                    module.expressions[elements[1].index()],
                    js::Expr::Binding(_)
                ));
            } else {
                assert!(matches!(
                    module.expressions[elements[1].index()],
                    js::Expr::Literal(js::Literal::Null)
                ));
            }
            value = elements[2];
        }
    }
    count
}

#[test]
fn selected_inline_writer_refreshes_static_path_support_without_target_rewrites() {
    for fields in [false, true] {
        for inline in [false, true] {
            with_targets(ORIGINAL, fields, inline, |module, mode| {
                let replacement = function(module, "ref_replace").is_some();
                assert_eq!(function(module, "ref_product").is_some(), replacement);
                assert!(paths(module, replacement) > 0);
                if mode == DemandMode::Prune {
                    assert_eq!(replacement, !inline);
                }
                if let Some(write) = function(module, "ref_write") {
                    let body = &module.regions[module.functions[write.index()].body.index()];
                    assert!(matches!(
                        body.statements.last(),
                        Some(js::Statement::Evaluate(_))
                    ));
                }
            });
        }
    }
}

#[test]
fn readonly_forwarding_keeps_private_path_slots_and_empty_roots_need_no_schema_support() {
    let nested = r#"
struct Pair {int x;int y;}struct Outer {Pair pair;}
int read(ref int value){return value;}
int forward(ref Pair value){return read(ref value.x);}
Outer state=Outer{Pair{5,7}};print(forward(ref state.pair));
"#;
    with_targets(nested, false, false, |module, _| {
        assert!(function(module, "ref_append").is_some());
        assert!(function(module, "ref_replace").is_none());
        assert!(function(module, "ref_product").is_none());
        assert_eq!(paths(module, false), 2);
        let javascript = module
            .render(js::PrintPolicy {
                mangle_bindings: false,
            })
            .unwrap();
        let result = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &javascript])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{javascript}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_eq!(result.stdout, b"5\n");
    });
    let empty = "struct Empty{}void ignore(ref Empty value){}Empty state=Empty{};ignore(ref state);print(5);";
    with_targets(empty, false, false, |module, _| {
        assert!(function(module, "ref_product").is_none());
        assert_eq!(paths(module, false), 0);
    });
}
