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

fn frame_bytes() -> u64 {
    size_of::<BinaryContinuation<'_, '_>>() as u64
}

fn scope_bytes() -> u64 {
    (size_of::<AHashMap<&str, SymbolId>>() + size_of::<Narrowing<'_>>()) as u64
}

fn expression<'ast, 'src>(program: &Program<'ast, 'src>, item: usize) -> &'ast Expr<'ast, 'src> {
    let Item::Stmt(Stmt::Expr(expression)) = &program.items[item] else {
        panic!("expression fixture")
    };
    expression
}

#[test]
fn both_binary_tree_directions_charge_exact_growth_and_release_between_expressions() {
    // Allocation tariff: one allocation at capacity 4, then each old length
    // moved plus one allocation. Peak includes simultaneous old/new buffers.
    for (operators, growth_work, peak_frames) in [(1, 1, 4), (4, 1, 4), (5, 6, 12), (9, 15, 24)] {
        for nested_on_left in [true, false] {
            let source = if nested_on_left {
                format!("1{};", "+1".repeat(operators))
            } else {
                format!("{}1{};", "1+(".repeat(operators), ")".repeat(operators))
            };
            let arena = bumpalo::Bump::new();
            let program = crate::parse_source(&arena, &source).unwrap();
            let expected = analyze(&program).unwrap();
            let mut ledger = ledger(WORK, SENTINEL + scope_bytes() + peak_frames * frame_bytes());
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
                    analyzer.analyze_binary_expression(expression(&program, 0), None),
                    Ok(Type::Int)
                );
                assert_eq!(
                    analyzer.facts.expression_types,
                    expected.facts.expression_types
                );
                for (actual, expected) in analyzer
                    .facts
                    .source_info
                    .iter()
                    .zip(&expected.facts.source_info)
                {
                    assert_eq!(
                        actual.expression.map(|value| value.id),
                        expected.expression.map(|value| value.id)
                    );
                    assert_eq!(actual.resolution, expected.resolution);
                }
                assert_eq!(
                    analyzer.budget.retained_bytes(AllocationClass::Scratch),
                    scope_bytes()
                );
            }
            drop(analyzer);
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            drop(budget);
            assert_eq!(ledger.retained_bytes(), SENTINEL);
            assert_eq!(
                ledger.peak_retained_bytes(),
                SENTINEL + scope_bytes() + peak_frames * frame_bytes()
            );
            assert_eq!(
                ledger.work_by_kind(WorkKind::Analysis),
                2 * (4 * operators as u64 + 2)
            );
            assert_eq!(
                ledger.work_by_kind(WorkKind::Render),
                4 + 2 * (2 * operators as u64 + growth_work)
            );
        }
    }
}

#[test]
fn binary_memory_refusals_cover_first_and_overlapping_growth_without_publishing_types() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "1+1+1+1+1+1+1+1+1+1;").unwrap();
    for (required_frames, peak_frames) in [(4, 0), (12, 4), (24, 12)] {
        let mut ledger = ledger(
            WORK,
            SENTINEL + scope_bytes() + required_frames * frame_bytes() - 1,
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
        assert_eq!(
            analyzer.analyze_binary_expression(expression(&program, 0), None),
            Err(AdmittedSemanticError::Resources(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Baseline)
            )))
        );
        assert!(analyzer.facts.expression_types.iter().all(Option::is_none));
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            scope_bytes()
        );
        assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
        drop(analyzer);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(
            ledger.peak_retained_bytes(),
            SENTINEL + scope_bytes() + peak_frames * frame_bytes()
        );
    }
}

#[test]
fn every_binary_work_cutoff_restores_scopes_and_preserves_the_exact_success_tariff() {
    for (source, analysis_work, render_work, ty) in [
        ("1+1+1+1+1+1;", 22, 20, Type::Int),
        ("true&&(true&&(true&&true));", 14, 21, Type::Bool),
    ] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let total = analysis_work + render_work;
        // The Analyzer constructor consumes four units before traversal starts.
        for limit in 4..=total {
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
            analyzer.scopes[0].insert("retained", SymbolId(3));
            analyzer.narrowings[0].insert(SymbolId(3), Type::Int);
            let result = analyzer.analyze_binary_expression(expression(&program, 0), None);
            if limit == total {
                assert_eq!(result, Ok(ty.clone()));
            } else {
                assert_eq!(
                    result,
                    Err(AdmittedSemanticError::Resources(AllocationError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Baseline)
                    ))),
                    "{source}, work={limit}"
                );
            }
            assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
            assert_eq!(analyzer.scopes[0]["retained"], SymbolId(3));
            assert_eq!(analyzer.narrowings[0][&SymbolId(3)], Type::Int);
            let backing = analyzer.scopes.capacity() * size_of::<AHashMap<&str, SymbolId>>()
                + analyzer.narrowings.capacity() * size_of::<Narrowing<'_>>();
            assert_eq!(
                analyzer.budget.retained_bytes(AllocationClass::Scratch),
                backing as u64
            );
            drop(analyzer);
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            drop(budget);
            assert_eq!(ledger.retained_bytes(), SENTINEL);
            assert!(ledger.work_used(WorkDomain::Baseline) <= limit);
            if limit == total {
                assert_eq!(ledger.work_by_kind(WorkKind::Analysis), analysis_work);
                assert_eq!(ledger.work_by_kind(WorkKind::Render), render_work);
            }
        }
    }
}

#[test]
fn nested_binary_calls_admit_simultaneously_live_worklists_and_release_for_reuse() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "1+-(2+3);4+5;").unwrap();
    for enough in [false, true] {
        let peak = scope_bytes() + 8 * frame_bytes();
        let mut ledger = ledger(WORK, SENTINEL + peak - u64::from(!enough));
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
        let result = analyzer.analyze_binary_expression(expression(&program, 0), None);
        assert_eq!(
            result,
            if enough {
                Ok(Type::Int)
            } else {
                Err(AdmittedSemanticError::Resources(AllocationError::Budget(
                    BudgetError::MemoryExhausted(WorkDomain::Baseline),
                )))
            }
        );
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            scope_bytes()
        );
        assert_eq!(
            analyzer.analyze_binary_expression(expression(&program, 1), None),
            Ok(Type::Int)
        );
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            scope_bytes()
        );
        drop(analyzer);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(
            ledger.peak_retained_bytes(),
            SENTINEL + scope_bytes() + if enough { 8 } else { 4 } * frame_bytes()
        );
        if enough {
            assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 18);
            assert_eq!(ledger.work_by_kind(WorkKind::Render), 13);
        }
    }
}

#[test]
fn binary_semantic_errors_and_deadlines_preserve_diagnostics_and_release_scratch() {
    for source in ["true&&(true&&missingRight);", "(1+true)+(missingRight+1);"] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let expected = analyze(&program).unwrap_err();
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
            Err(AdmittedSemanticError::Semantic(expected))
        );
        assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
        let remaining = analyzer.budget.retained_bytes(AllocationClass::Scratch);
        analyzer.budget.with_ledger(|owner| {
            owner
                .unwrap()
                .0
                .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000))
        });
        assert_eq!(
            analyzer.analyze_binary_expression(expression(&program, 0), None),
            Err(AdmittedSemanticError::Resources(AllocationError::Budget(
                BudgetError::DeadlineExceeded
            )))
        );
        assert_eq!(
            analyzer.budget.retained_bytes(AllocationClass::Scratch),
            remaining
        );
        drop(analyzer);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
    }
}

#[test]
fn binary_panic_drops_live_worklist_before_the_existing_callback_scope_rolls_back() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "1+2;").unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut scope = budget.scope();
        let mut facts = ModuleFacts::new(program.source_identity());
        // Deliberately corrupt private test storage to panic at the first leaf,
        // while the admitted continuation is live, without a production hook.
        facts.expression_types.clear();
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
        let _ = analyzer.analyze_binary_expression(expression(&program, 0), None);
    }));
    assert!(result.is_err());
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(
        ledger.peak_retained_bytes(),
        SENTINEL + scope_bytes() + 4 * frame_bytes()
    );
}
