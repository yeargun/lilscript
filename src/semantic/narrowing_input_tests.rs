use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind,
};
use std::mem::size_of;

const SENTINEL: u64 = 29;
const WORK: u64 = 1_000_000;

fn ledger(work: u64) -> BudgetLedger {
    let mut ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: 1_000_000,
        },
    )
    .unwrap();
    ledger.retain(WorkDomain::Baseline, SENTINEL).unwrap();
    ledger
}

fn expression<'ast, 'src>(program: &Program<'ast, 'src>, item: usize) -> &'ast Expr<'ast, 'src> {
    let Item::Stmt(Stmt::Expr(expression)) = &program.items[item] else {
        panic!("condition fixture")
    };
    expression
}

// Small syntax oracle for the summary carried by the production worklist.
// Compare its queries against the unchanged complete condition traversal.
fn input<'ast, 'src>(expression: &'ast Expr<'ast, 'src>) -> NarrowingInput<'ast, 'src> {
    match &expression.kind {
        ExprKind::Binary {
            op: op @ (BinaryOp::And | BinaryOp::Or),
            lhs,
            rhs,
            ..
        } => input(lhs).join(input(rhs), expression, *op),
        _ => NarrowingInput::leaf(expression),
    }
}

#[test]
fn narrowed_inputs_match_full_traversal_across_boolean_shapes_and_live_contexts() {
    let atoms = ["true", "value!=null", "value==null", "null!=value"];
    let mut comparisons = 0;
    for left in atoms {
        for middle in atoms {
            for right in atoms {
                for outer in ["&&", "||"] {
                    for inner in ["&&", "||"] {
                        for left_nested in [false, true] {
                            let source = if left_nested {
                                format!("({left}{inner}{middle}){outer}{right};value=null;")
                            } else {
                                format!("{left}{outer}({middle}{inner}{right});value=null;")
                            };
                            let arena = bumpalo::Bump::new();
                            let program = crate::parse_source(&arena, &source).unwrap();
                            let guard = expression(&program, 0);
                            let input = input(guard);
                            let mut ledger = ledger(WORK);
                            let mut budget =
                                AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                            let mut facts = ModuleFacts::new(program.source_identity());
                            let mut declarations = DeclarationTables::default();
                            let mut initialization = ModuleInitialization::default();
                            let mut analyzer = Analyzer::new(
                                &mut facts,
                                &mut declarations,
                                &mut initialization,
                                None,
                                &mut budget,
                            )
                            .unwrap();
                            let value = analyzer
                                .declare(
                                    Ident {
                                        name: "value",
                                        span: Span::empty(10_000),
                                    },
                                    Type::Nullable(Box::new(Type::String)),
                                )
                                .unwrap();
                            for context in 0..5 {
                                match context {
                                    1 => {
                                        analyzer.push_scope().unwrap();
                                        analyzer.apply_narrowing(
                                            [(value, Type::String)].into_iter().collect(),
                                        );
                                    }
                                    2 => {
                                        analyzer
                                            .analyze_expr(expression(&program, 1), None)
                                            .unwrap();
                                    }
                                    3 => {
                                        analyzer
                                            .declare(
                                                Ident {
                                                    name: "value",
                                                    span: Span::empty(10_001),
                                                },
                                                Type::Nullable(Box::new(Type::Bool)),
                                            )
                                            .unwrap();
                                    }
                                    4 => analyzer.pop_scope(),
                                    _ => {}
                                }
                                let live = analyzer.budget.retained_bytes(AllocationClass::Scratch);
                                let full = analyzer.condition_narrowing(guard).unwrap();
                                assert_eq!(
                                    analyzer.narrowing_from_input(input).unwrap(),
                                    full,
                                    "{source}, context {context}"
                                );
                                assert_eq!(
                                    analyzer.budget.retained_bytes(AllocationClass::Scratch),
                                    live
                                );
                                comparisons += 1;
                            }
                            drop(analyzer);
                            drop(declarations);
                            drop(budget);
                            assert_eq!(ledger.retained_bytes(), SENTINEL);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 2_560);
}

#[test]
fn discarded_projections_still_resolve_guards_and_report_the_first_diagnostic() {
    for source in [
        "(missing==null&&true)||false;",
        "(missingLeft!=null&&true)&&(missingRight!=null||false);",
        "((missing is string)&&true)||false;",
        "(!(missing!=null)&&true)||false;",
    ] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let guard = expression(&program, 0);
        let input = input(guard);
        assert!(input.expression.is_some());
        let expected = analyze(&program).unwrap_err();
        let mut ledger = ledger(WORK);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut facts = ModuleFacts::new(program.source_identity());
        let mut declarations = DeclarationTables::default();
        let mut initialization = ModuleInitialization::default();
        let mut analyzer = Analyzer::new(
            &mut facts,
            &mut declarations,
            &mut initialization,
            None,
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            analyzer.condition_narrowing(guard),
            Err(AdmittedSemanticError::Semantic(expected.clone()))
        );
        assert_eq!(
            analyzer.narrowing_from_input(input),
            Err(AdmittedSemanticError::Semantic(expected))
        );
        drop(analyzer);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
    }

    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "((value is string)&&true)||false;").unwrap();
    let mut ledger = ledger(WORK);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let mut facts = ModuleFacts::new(program.source_identity());
    let mut declarations = DeclarationTables::default();
    let mut initialization = ModuleInitialization::default();
    let mut analyzer = Analyzer::new(
        &mut facts,
        &mut declarations,
        &mut initialization,
        None,
        &mut budget,
    )
    .unwrap();
    analyzer
        .declare(
            Ident {
                name: "value",
                span: Span::empty(10_000),
            },
            Type::Nullable(Box::new(Type::String)),
        )
        .unwrap();
    let guard = expression(&program, 0);
    let input = input(guard);
    assert!(!input.when_true && !input.when_false);
    let expected = analyzer.condition_narrowing(guard).unwrap_err();
    let AdmittedSemanticError::Semantic(error) = &expected else {
        panic!("semantic error")
    };
    assert_eq!(
        error.message,
        "type guard was not analyzed before narrowing"
    );
    assert_eq!(analyzer.narrowing_from_input(input), Err(expected));
    drop(analyzer);
    drop(declarations);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn sparse_guard_prefixes_recheck_only_retained_guards_with_linear_work() {
    for operators in [1_u64, 4, 16, 64] {
        for two_guards in [false, true] {
            let source = format!(
                "value!=null{}{};",
                if two_guards { "&&other!=null" } else { "" },
                "&&true".repeat(operators as usize)
            );
            let arena = bumpalo::Bump::new();
            let program = crate::parse_source(&arena, &source).unwrap();
            let mut ledger = ledger(WORK);
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let mut facts = ModuleFacts::new(program.source_identity());
            let mut declarations = DeclarationTables::default();
            let mut initialization = ModuleInitialization::default();
            let mut analyzer = Analyzer::new(
                &mut facts,
                &mut declarations,
                &mut initialization,
                None,
                &mut budget,
            )
            .unwrap();
            analyzer
                .declare(
                    Ident {
                        name: "value",
                        span: Span::empty(10_000),
                    },
                    Type::Nullable(Box::new(Type::String)),
                )
                .unwrap();
            if two_guards {
                analyzer
                    .declare(
                        Ident {
                            name: "other",
                            span: Span::empty(10_001),
                        },
                        Type::Nullable(Box::new(Type::Int)),
                    )
                    .unwrap();
            }
            assert_eq!(
                analyzer.analyze_binary_expression(expression(&program, 0), None),
                Ok(Type::Bool)
            );
            assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
            assert!(analyzer.narrowings[0].is_empty());
            drop(analyzer);
            drop(declarations);
            drop(budget);
            assert_eq!(ledger.retained_bytes(), SENTINEL);
            assert_eq!(
                ledger.work_by_kind(WorkKind::Analysis),
                if two_guards {
                    9 * operators + 15
                } else {
                    5 * operators + 6
                }
            );
        }
    }
}

#[test]
fn guard_input_resource_refusals_preserve_declarations_and_restore_scopes() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "(value!=null&&true)&&true;").unwrap();
    // Constructor/declaration: 8 Render units. Three binary operators:
    // 14 probes, two guard queries, seven push/allocation units, eight scope units.
    for limit in 8..=39 {
        let mut ledger = ledger(limit);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut facts = ModuleFacts::new(program.source_identity());
        let mut declarations = DeclarationTables::default();
        let mut initialization = ModuleInitialization::default();
        let mut analyzer = Analyzer::new(
            &mut facts,
            &mut declarations,
            &mut initialization,
            None,
            &mut budget,
        )
        .unwrap();
        let value = analyzer
            .declare(
                Ident {
                    name: "value",
                    span: Span::empty(10_000),
                },
                Type::Nullable(Box::new(Type::String)),
            )
            .unwrap();
        let result = analyzer.analyze_binary_expression(expression(&program, 0), None);
        assert_eq!(
            result,
            if limit == 39 {
                Ok(Type::Bool)
            } else {
                Err(AdmittedSemanticError::Resources(AllocationError::Budget(
                    BudgetError::WorkExhausted(WorkDomain::Baseline),
                )))
            },
            "work={limit}"
        );
        assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
        assert_eq!(analyzer.scopes[0]["value"], value);
        assert_eq!(
            analyzer.declarations.symbols[value.0 as usize].ty,
            Type::Nullable(Box::new(Type::String))
        );
        let shared = (analyzer.declarations.symbols.capacity() * size_of::<Symbol<'_>>()
            + analyzer.declarations.symbol_modules.capacity()
                * size_of::<Option<crate::module::ModuleId>>()) as u64;
        let frames = (analyzer.scopes.capacity() * size_of::<AHashMap<&str, SymbolId>>()
            + analyzer.narrowings.capacity() * size_of::<Narrowing<'_>>())
            as u64;
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            shared + frames
        );
        drop(analyzer);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), shared);
        drop(declarations);
        budget.release(AllocationClass::Scratch, shared).unwrap();
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        if limit == 39 {
            assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 16);
            assert_eq!(ledger.work_by_kind(WorkKind::Render), 23);
        }
    }
}

#[test]
fn sparse_guard_checking_preserves_runtime_short_circuit_and_statement_boundaries() {
    let source = format!(
        r#"
int calls=0;
bool mark(bool answer){{calls+=1;return answer;}}
bool present(string? value){{return value!=null{}&&mark(true)&&value.length>0;}}
bool absent(string? value){{return value==null{}||mark(false)||value.length>0;}}
print(present(null));print(present("x"));print(absent(null));print(absent(""));print(calls);
"#,
        "&&true".repeat(64),
        "||false".repeat(64)
    );
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    for preserve_root_exports in [false, true] {
        let options = crate::compiler_service::ServiceOptions {
            preserve_root_exports,
            ..Default::default()
        };
        let compiled = crate::compiler_service::compile_source_semantic(&source, &config, options)
            .unwrap();
        let javascript = compiled
            .javascript(config.javascript.cost_model)
            .unwrap()
            .javascript()
            .to_string();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &javascript])
            .output()
            .expect("Node is required for sparse guard observations");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "false\ntrue\ntrue\nfalse\n2\n"
        );
    }
    let invalid = format!(
        "int size(string? value){{bool ready=value!=null{};return value.length;}}",
        "&&true".repeat(64)
    );
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, &invalid).unwrap();
    let error = analyze(&program).unwrap_err();
    assert!(
        error
            .message
            .contains("type `string?` has no member `length`"),
        "{error}"
    );
    assert!(error.span.start > invalid.find("return").unwrap());
}
