//! Independent observations and target-shape obligations for effective demand.
//! These tests execute the compiler's formation path, without an AST
//! optimizer or a family-specific cleanup pass.
use super::facts::{CacheLimits, FactRequest, RetainedFactsCache};
use super::helper_family::{self, FamilyOutcome as HelperOutcome, PreparationOutcome};
use super::implementations::ImplementationMap;
use super::record_family::{self, FamilyOutcome as RecordOutcome};
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::{
    self,
    selection::{Plan, Style},
};
use std::process::Command;

struct Case {
    name: &'static str,
    source: &'static str,
    host: &'static str,
    expected: &'static str,
    helper: Option<&'static str>,
    scalar: bool,
}
macro_rules! case {
    ($name:literal, $helper:expr, $scalar:expr) => {
        Case {
            name: $name,
            source: include_str!(concat!("fixtures/demand/", $name, ".lil")),
            host: include_str!(concat!("fixtures/demand/", $name, ".setup.js")),
            expected: include_str!(concat!("fixtures/demand/", $name, ".expected.out")),
            helper: $helper,
            scalar: $scalar,
        }
    };
}
fn cell(program: &Program<'_>, name: &str) -> CellId {
    let mut cells = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name);
    let id = CellId::from_index(cells.next().expect("fixture cell").0).unwrap();
    assert!(cells.next().is_none(), "ambiguous fixture cell {name}");
    id
}
fn run(case: &Case, mut inspect: impl FnMut(&js::Module, bool, bool)) {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, case.source)
        .unwrap_or_else(|error| panic!("{}: {error:?}", case.name));
    let semantics = crate::analyze(&source).unwrap();
    let program = from_checked_source(&source, &semantics).unwrap();
    let mut ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: 100_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap();
    let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
    let mut cache = RetainedFactsCache::new(
        CacheLimits {
            entries: 8,
            bytes: 1_000_000,
            result_bytes: 100_000,
        },
        &mut ledger,
        WorkDomain::Baseline,
    )
    .unwrap();
    let mut maps = vec![(false, false, ImplementationMap::direct())];
    if case.scalar {
        let analyzed = record_family::analyze(
            &program,
            &uses,
            cell(&program, "state"),
            record_family::FamilyRequest {
                attempt: AnalysisAttempt {
                    plan: record_family::RECORD_FAMILY_PLAN,
                    algorithm_version: record_family::RECORD_FAMILY_VERSION,
                    work_quota: 1_000_000,
                },
                scratch_bytes: 1_000_000,
                output_bytes: 1_000_000,
            },
            &mut ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        let family = match analyzed.outcome {
            RecordOutcome::Complete(family) => family,
            other => panic!("{}: scalar prerequisite: {other:?}", case.name),
        };
        maps.push((
            false,
            true,
            maps[0]
                .2
                .with_scalar(family, &mut ledger, WorkDomain::Optional)
                .unwrap(),
        ));
    }
    if let Some(name) = case.helper {
        for index in 0..maps.len() {
            let prepared = helper_family::prepare(
                &program,
                &uses,
                cell(&program, name),
                helper_family::FamilyRequest {
                    execution: crate::compilation_contract::JavaScriptExecution::Module,
                    attempt: AnalysisAttempt {
                        plan: helper_family::HELPER_FAMILY_PLAN,
                        algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                        work_quota: 1_000_000,
                    },
                    scratch_bytes: 1_000_000,
                    output_bytes: 1_000_000,
                },
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            let mut prepared = match prepared.outcome {
                PreparationOutcome::Ready(prepared) => prepared,
                other => panic!("{}: helper prerequisite: {other:?}", case.name),
            };
            {
                let mut session = cache.session(&mut ledger, WorkDomain::Optional, 1).unwrap();
                let facts = session
                    .query(
                        &program,
                        prepared.root().body,
                        FactRequest {
                            attempt: AnalysisAttempt {
                                plan: super::facts::LOCAL_FACTS_PLAN,
                                algorithm_version: super::facts::LOCAL_FACTS_VERSION,
                                work_quota: 100_000,
                            },
                            result_bytes: 100_000,
                        },
                    )
                    .unwrap();
                prepared
                    .check_body(&program, facts.facts, facts.receipt)
                    .unwrap();
            }
            let family = match prepared.finish(&mut ledger).unwrap() {
                HelperOutcome::Complete(family) => family,
                other => panic!("{}: helper proof: {other:?}", case.name),
            };
            let map = maps[index]
                .2
                .with_inline_helper(family, &mut ledger, WorkDomain::Optional)
                .unwrap();
            maps.push((true, maps[index].1, map));
        }
    }
    // Exercise both modern and lowered lazy-control syntax with discarded
    // results; the older target needs scratch storage solely for its recipe.
    for edition in ["es2022", "es2018"] {
        let text = format!("[javascript]\nstrip_console=false\necmascript=\"{edition}\"\n");
        let config: crate::config::ProjectConfig = toml::from_str(&text).unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        for (mode, (inline, scalar, map)) in [
            super::demand::DemandMode::Prune,
            super::demand::DemandMode::Preserve,
        ]
        .into_iter()
        .flat_map(|mode| maps.iter().map(move |map| (mode, map)))
        {
            let before = ledger.retained_bytes();
            let target = super::javascript::lower_with_implementations(
                &program,
                &uses,
                map,
                policy.javascript_contract().unwrap(),
                mode,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap_or_else(|error| {
                panic!("{} inline={inline} scalar={scalar}: {error:?}", case.name)
            });
            assert_eq!(
                ledger.retained_bytes(),
                before,
                "demand must release after formation"
            );
            if mode == super::demand::DemandMode::Prune {
                inspect(&target, *inline, *scalar);
            }
            let output = target.prepare_output_with_policy(&policy).unwrap();
            for style in [Style::Source, Style::Scoped, Style::Global] {
                let javascript = output.render(&Plan::new(style)).unwrap();
                let script = format!("{}\n{javascript}", case.host);
                let result = Command::new("node")
                    .args(["--input-type=module", "-e", &script])
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{} {edition} {style:?} {mode:?} inline={inline} scalar={scalar}: {}\n{script}",
                    case.name,
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(
                    String::from_utf8(result.stdout).unwrap(),
                    case.expected,
                    "{} {edition} {style:?} {mode:?} inline={inline} scalar={scalar}\n{script}",
                    case.name
                );
            }
        }
    }
    for (_, _, map) in maps {
        map.discard(&mut ledger).unwrap();
    }
    cache.discard(&mut ledger).unwrap();
    uses.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn dead_arithmetic_and_private_local_storage_disappear_in_direct_and_inline_output() {
    run(
        &case!("unused-pure-local", Some("compute"), false),
        |target, inline, _| {
            assert!(
                !target
                    .expressions
                    .iter()
                    .any(|expr| matches!(expr, js::Expr::IntBinary { .. })),
                "unused arithmetic survived"
            );
            assert!(!target
                .bindings
                .iter()
                .any(|binding| binding.spelling == "unused"));
            assert_eq!(target.functions.len(), usize::from(!inline));
        },
    );
}

#[test]
fn unused_inline_parameters_disappear_but_argument_throw_and_finally_remain() {
    run(
        &case!("unused-argument-throw", Some("first"), false),
        |target, inline, _| {
            if inline {
                assert!(!target
                    .bindings
                    .iter()
                    .any(|binding| matches!(binding.spelling.as_str(), "unused" | "last")));
            } else {
                assert_eq!(
                    target.functions[0].parameters.len(),
                    3,
                    "direct ABI must retain its arity"
                );
            }
        },
    );
}

#[test]
fn discarded_helper_calls_preserve_lazy_argument_gating_and_can_have_no_body() {
    for case in [
        case!("lazy-discard-false", Some("helper"), false),
        case!("lazy-discard-true", Some("helper"), false),
    ] {
        run(&case, |target, inline, _| {
            if inline {
                assert!(target.functions.is_empty());
                assert!(!target
                    .bindings
                    .iter()
                    .any(|binding| binding.spelling == "unused"));
            }
        });
    }
}

#[test]
fn dead_scalar_slots_and_writes_disappear_without_erasing_initializer_or_rhs_effects() {
    for case in [
        case!("scalar-unused-slot-effects", None, true),
        case!("scalar-unused-initializer-throw", None, true),
    ] {
        run(&case, |target, _, scalar| {
            if scalar {
                assert_eq!(
                    target
                        .bindings
                        .iter()
                        .filter(|binding| binding.spelling.starts_with("record_"))
                        .count(),
                    1
                );
                assert!(!target
                    .expressions
                    .iter()
                    .any(|expr| matches!(expr, js::Expr::Object(_))));
            }
        });
    }
}

#[test]
fn demand_keeps_reference_preparation_tdz_captures_loop_control_and_required_effects() {
    for case in [
        case!("discarded-reference-call", None, false),
        case!("captured-tdz", None, false),
        case!("unused-closure-body", None, false),
        case!("public-captured-state", Some("helper"), false),
        case!("finite-loop-control", None, false),
        case!("known-string-required-effect", None, false),
    ] {
        run(&case, |target, _, _| {
            if case.name == "unused-closure-body" {
                assert!(
                    target.functions.is_empty(),
                    "unused callable body/support must disappear"
                );
            }
        });
    }
}

#[test]
fn public_parameter_abi_does_not_make_unused_parameter_assignments_live() {
    run(&Case {
        name: "public unused parameter assignment",
        source: "extern int arity(func(int,int)->int f);export int visible(int used,int unused){unused=42;return used;}print(arity(visible));print(visible(3,4));",
        host: "function arity(f){return f.length}", expected: "2\n3\n", helper: None, scalar: false,
    }, |target, _, _| {
        assert_eq!(target.functions[0].parameters.len(), 2);
        assert!(!target.expressions.iter().any(|expr| matches!(expr, js::Expr::Literal(js::Literal::Number(n)) if *n == 42.0)));
    });
}

#[test]
fn captured_slot_and_parameter_demand_compose_across_helper_and_record_choices() {
    run(&Case {
        name: "captured record with write-only slot and unused effectful parameter",
        source: "extern int argument();export func()->int make(int seed){Record<int> state=record{count:seed,dead:0};auto helper=(int used,int unused)=>{state.dead=used;state.count=(state.count??0)+used;return state.count??0;};return ()=>helper(1,argument());}auto a=make(3);auto b=make(10);print(a());print(b());print(a());",
        host: "function argument(){console.log('argument');return 99}",
        expected: "argument\n4\nargument\n11\nargument\n5\n",
        helper: Some("helper"), scalar: true,
    }, |target, inline, scalar| {
        if scalar {
            assert_eq!(target.bindings.iter().filter(|binding| binding.spelling.starts_with("record_")).count(), 1);
        }
        if inline {
            assert!(!target.bindings.iter().any(|binding| binding.spelling == "unused"));
        }
    });
}

#[test]
fn discarded_nullish_and_select_results_retain_only_their_controlled_effects() {
    for enabled in [false, true] {
        run(&Case {
            name: "discarded select and nullable result",
            source: "extern bool enabled();extern int? nullable();extern int argument(int n);int helper(int unused){return 1;}int discarded=if(enabled()){helper(argument(1))}else{helper(argument(2))};nullable()??helper(argument(3));print(9);",
            host: if enabled {"function enabled(){return true}function nullable(){return null}function argument(n){console.log('argument:'+n);return n}"}
                  else {"function enabled(){return false}function nullable(){return 7}function argument(n){console.log('argument:'+n);return n}"},
            expected: if enabled {"argument:1\nargument:3\n9\n"} else {"argument:2\n9\n"},
            helper: Some("helper"), scalar: false,
        }, |target, inline, _| {
            if inline { assert!(target.functions.is_empty()); }
        });
    }
}

#[test]
fn resource_only_string_construction_can_disappear_without_materializing_its_value() {
    run(
        &Case {
            name: "discarded primitive string construction under resource contract",
            source: "string unused=\"left\"+\"right\";print(1);",
            host: "",
            expected: "1\n",
            helper: None,
            scalar: false,
        },
        |target, _, _| {
            assert!(!target
                .bindings
                .iter()
                .any(|binding| binding.spelling == "unused"));
            assert_eq!(
                target
                    .expressions
                    .iter()
                    .filter(|expr| matches!(
                        expr,
                        js::Expr::Binary {
                            op: js::Binary::Add,
                            ..
                        }
                    ))
                    .count(),
                0,
                "resource-only construction with discarded result must disappear"
            );
        },
    );
}

#[test]
fn string_helper_knowledge_keeps_computation_while_discarded_expansion_disappears() {
    run(&Case {
        name: "string helper computation and discarded expansion",
        source: "string helper(string unused){return \"left\"+\"right\";}helper(\"ignored\");print(helper(\"needed\"));",
        host: "", expected: "leftright\n", helper: Some("helper"), scalar: false,
    }, |target, inline, _| {
        if inline {
            assert!(target.functions.is_empty());
            assert!(!target.bindings.iter().any(|binding| binding.spelling == "unused"));
        }
        assert_eq!(target.expressions.iter().filter(|expr| matches!(expr, js::Expr::Binary {op: js::Binary::Add, ..})).count(), 1);
        assert!(!target.expressions.iter().any(|expr| matches!(expr, js::Expr::Literal(js::Literal::String(value)) if value == &crate::literal::StringValue::from("leftright"))),
            "exact fact query must not choose literal emission");
    });
}
