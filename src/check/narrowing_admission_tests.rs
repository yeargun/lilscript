use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind,
};
use std::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};

const SENTINEL: u64 = 29;
const WORK: u64 = 1_000_000;
const MEMORY: u64 = 1_000_000;

fn ledger(work: u64, memory: u64) -> BudgetLedger {
    let mut ledger = BudgetLedger::new(
        ResourceLimits {
            wall_time_ms: Some(100_000),
            ..ResourceLimits::default()
        },
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap();
    ledger.retain(WorkDomain::Baseline, SENTINEL).unwrap();
    ledger
}

fn step_bytes() -> u64 {
    size_of::<NarrowingStep<'_, '_>>() as u64
}

fn answer_bytes() -> u64 {
    size_of::<(Narrowing<'_>, Narrowing<'_>)>() as u64
}

fn scope_bytes() -> u64 {
    (size_of::<AHashMap<&str, SymbolId>>() + size_of::<Narrowing<'_>>()) as u64
}

fn expression<'ast, 'src>(program: &Program<'ast, 'src>, item: usize) -> &'ast Expr<'ast, 'src> {
    let Item::Stmt(Stmt::Expr(expression)) = &program.items[item] else {
        panic!("condition fixture")
    };
    expression
}

#[test]
fn narrowing_leaf_and_compositions_have_exact_work_and_overlapping_backing() {
    let s = step_bytes();
    let a = answer_bytes();
    for (source, analysis_work, render_work, peak) in [
        ("true;", 1, 0, 0),
        ("!true;", 4, 7, 4 * s + 4 * a),
        ("true&&false;", 5, 9, 4 * s + 4 * a),
        ("true&&false&&true;", 8, 19, (12 * s).max(8 * s + 4 * a)),
        (
            "true&&(true&&(true&&(true&&true)));",
            14,
            34,
            (12 * s + 4 * a).max(8 * s + 12 * a),
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let mut ledger = ledger(
            4 + 2 * (analysis_work + render_work),
            SENTINEL + scope_bytes() + peak,
        );
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
        for _ in 0..2 {
            assert_eq!(
                analyzer.condition_narrowing(expression(&program, 0)),
                Ok((Narrowing::default(), Narrowing::default()))
            );
            assert_eq!(
                analyzer.budget.retained_bytes(AllocationClass::Scratch),
                scope_bytes()
            );
            assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
        }
        drop(analyzer);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(
            ledger.peak_retained_bytes(),
            SENTINEL + scope_bytes() + peak,
            "{source}"
        );
        assert_eq!(
            ledger.work_by_kind(WorkKind::Analysis),
            2 * analysis_work,
            "{source}"
        );
        assert_eq!(
            ledger.work_by_kind(WorkKind::Render),
            4 + 2 * render_work,
            "{source}"
        );
    }
}

#[test]
fn narrowing_refuses_each_worklist_allocation_and_growth_before_publication() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "true&&(true&&(true&&(true&&true)));").unwrap();
    let s = step_bytes();
    let a = answer_bytes();
    for (required, peak) in [
        (4 * s, 0),
        (4 * s + 4 * a, 4 * s),
        (12 * s + 4 * a, 4 * s + 4 * a),
        (8 * s + 12 * a, 12 * s + 4 * a),
    ] {
        let mut ledger = ledger(WORK, SENTINEL + scope_bytes() + required - 1);
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
            analyzer.condition_narrowing(expression(&program, 0)),
            Err(AdmittedCheckError::Resources(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Baseline)
            )))
        );
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            scope_bytes()
        );
        assert!(analyzer.narrowings[0].is_empty());
        assert!(analyzer.facts.expression_types.iter().all(Option::is_none));
        drop(analyzer);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(
            ledger.peak_retained_bytes(),
            SENTINEL + scope_bytes() + peak
        );
    }
}

#[test]
fn every_narrowing_work_cutoff_refuses_without_leaving_temporary_storage() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "true&&(true&&(true&&(true&&true)));").unwrap();
    // Four constructor units, fourteen probes, twenty-two pushes and twelve
    // allocation/movement units for two worklists growing from four to eight.
    for limit in 4..=52 {
        let mut ledger = ledger(limit, MEMORY);
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
        let result = analyzer.condition_narrowing(expression(&program, 0));
        if limit == 52 {
            assert_eq!(result, Ok((Narrowing::default(), Narrowing::default())));
        } else {
            assert_eq!(
                result,
                Err(AdmittedCheckError::Resources(AllocationError::Budget(
                    BudgetError::WorkExhausted(WorkDomain::Baseline)
                ))),
                "work={limit}"
            );
        }
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            scope_bytes()
        );
        assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
        drop(analyzer);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert!(ledger.work_used(WorkDomain::Baseline) <= limit);
        if limit == 52 {
            assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 14);
            assert_eq!(ledger.work_by_kind(WorkKind::Render), 38);
        }
    }
}

#[test]
fn narrowing_preserves_branch_maps_active_scope_assignment_and_shadowing() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "value!=null;value==null;!(value!=null);(value!=null)&&(other!=null);(value==null)||(other==null);(value!=null)||(other!=null);!(value is string);value=null;",
    )
    .unwrap();
    let mut ledger = ledger(WORK, MEMORY);
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
    let other = analyzer
        .declare(
            Ident {
                name: "other",
                span: Span::empty(10_001),
            },
            Type::Nullable(Box::new(Type::Int)),
        )
        .unwrap();
    let map = |pairs: Vec<_>| pairs.into_iter().collect::<Narrowing<'_>>();
    let expected = [
        (map(vec![(value, Type::String)]), map(vec![])),
        (map(vec![]), map(vec![(value, Type::String)])),
        (map(vec![]), map(vec![(value, Type::String)])),
        (
            map(vec![(value, Type::String), (other, Type::Int)]),
            map(vec![]),
        ),
        (
            map(vec![]),
            map(vec![(value, Type::String), (other, Type::Int)]),
        ),
        (map(vec![]), map(vec![])),
        (
            map(vec![(value, Type::Null)]),
            map(vec![(value, Type::String)]),
        ),
    ];
    for (item, expected) in expected.into_iter().enumerate() {
        assert_eq!(
            analyzer.analyze_expr(expression(&program, item), None),
            Ok(Type::Bool)
        );
        let live = analyzer.budget.retained_bytes(AllocationClass::Scratch);
        assert_eq!(
            analyzer.condition_narrowing(expression(&program, item)),
            Ok(expected),
            "item {item}"
        );
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            live
        );
    }
    // The identical source occurrence has different answers in each context.
    let guard = expression(&program, 0);
    analyzer.push_scope().unwrap();
    analyzer.apply_narrowing(map(vec![(value, Type::String)]));
    assert_eq!(
        analyzer.condition_narrowing(guard),
        Ok((map(vec![]), map(vec![])))
    );
    analyzer
        .analyze_expr(expression(&program, 7), None)
        .unwrap();
    assert_eq!(
        analyzer.condition_narrowing(guard),
        Ok((map(vec![(value, Type::String)]), map(vec![])))
    );
    let shadow = analyzer
        .declare(
            Ident {
                name: "value",
                span: Span::empty(10_002),
            },
            Type::Nullable(Box::new(Type::Bool)),
        )
        .unwrap();
    assert_ne!(value, shadow);
    assert_eq!(
        analyzer.condition_narrowing(guard),
        Ok((map(vec![(shadow, Type::Bool)]), map(vec![])))
    );
    analyzer.pop_scope();
    assert_eq!(
        analyzer.condition_narrowing(guard),
        Ok((map(vec![(value, Type::String)]), map(vec![])))
    );
    let shared = (analyzer.declarations.symbols.capacity() * size_of::<Symbol<'_>>()
        + analyzer.declarations.symbol_modules.capacity()
            * size_of::<Option<crate::module::ModuleId>>()) as u64;
    drop(analyzer);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), shared);
    drop(declarations);
    budget.release(AllocationClass::Scratch, shared).unwrap();
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn narrowing_semantic_failure_and_deadline_keep_original_diagnostics_and_cleanup() {
    let arena = bumpalo::Bump::new();
    let source = "!(missingLeft!=null&&(missingRight!=null));";
    let program = crate::parse_source(&arena, source).unwrap();
    let expected = analyze(&program).unwrap_err();
    assert_eq!(
        &source[expected.span.start..expected.span.end],
        "missingLeft"
    );
    for expired in [false, true] {
        let mut ledger = ledger(WORK, MEMORY);
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
        if expired {
            analyzer.budget.with_ledger(|owner| {
                owner
                    .unwrap()
                    .0
                    .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000))
            });
        }
        assert_eq!(
            analyzer.condition_narrowing(expression(&program, 0)),
            Err(if expired {
                AdmittedCheckError::Resources(AllocationError::Budget(
                    BudgetError::DeadlineExceeded,
                ))
            } else {
                AdmittedCheckError::Semantic(expected.clone())
            })
        );
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            scope_bytes()
        );
        drop(analyzer);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        if expired {
            assert_eq!(ledger.peak_retained_bytes(), SENTINEL + scope_bytes());
            assert_eq!(ledger.work_used(WorkDomain::Baseline), 4);
        }
    }
}

#[test]
fn narrowing_unwind_drops_both_live_worklists_before_callback_scope_rollback() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "true&&(missing!=null);").unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut scope = budget.scope();
        let mut facts = ModuleFacts::new(program.source_identity());
        let mut declarations = DeclarationTables::default();
        let mut initialization = ModuleInitialization::default();
        let mut analyzer = Analyzer::new(
            &mut facts,
            &mut declarations,
            &mut initialization,
            None,
            &mut scope,
        )
        .unwrap();
        // Deliberately corrupt a private binding so resolution panics after
        // the first answer is stored, without a production fault-injection hook.
        analyzer.scopes[0].insert("missing", SymbolId(999));
        let _ = analyzer.condition_narrowing(expression(&program, 0));
    }));
    assert!(result.is_err());
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(
        ledger.peak_retained_bytes(),
        SENTINEL + scope_bytes() + 4 * step_bytes() + 4 * answer_bytes()
    );
}

#[test]
fn guard_free_boolean_trees_skip_only_empty_narrowing_queries() {
    for operators in [1_u64, 4, 16, 64] {
        for nested_on_left in [true, false] {
            let source = if nested_on_left {
                format!("true{};", "&&true".repeat(operators as usize))
            } else {
                format!(
                    "{}true{};",
                    "true&&(".repeat(operators as usize),
                    ")".repeat(operators as usize)
                )
            };
            let arena = bumpalo::Bump::new();
            let program = crate::parse_source(&arena, &source).unwrap();
            let mut ledger = ledger(WORK, MEMORY);
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
                analyzer.analyze_binary_expression(expression(&program, 0), None),
                Ok(Type::Bool)
            );
            assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
            drop(analyzer);
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            drop(budget);
            assert_eq!(ledger.retained_bytes(), SENTINEL);
            assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 4 * operators + 2);
        }
    }
}
