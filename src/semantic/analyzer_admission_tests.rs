use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use std::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};

const SENTINEL: u64 = 29;
const WORK: u64 = 1_000_000;

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

#[test]
fn refused_analyzer_construction_releases_each_partial_backing() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    let scope = size_of::<AHashMap<&str, SymbolId>>() as u64;
    let narrowing = size_of::<Narrowing<'_>>() as u64;
    for (work, memory, error, peak) in [
        (
            WORK,
            scope - 1,
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
            0,
        ),
        (
            WORK,
            scope + narrowing - 1,
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
            scope,
        ),
        (
            0,
            100_000,
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            0,
        ),
        (
            1,
            100_000,
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            scope,
        ),
        (
            2,
            100_000,
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            scope + narrowing,
        ),
    ] {
        let mut ledger = ledger(work, SENTINEL + memory);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut facts = ModuleFacts::new(program.source_identity());
        let mut declarations = DeclarationTables::default();
        let mut initialization = ModuleInitialization::default();
        let failure = Analyzer::new(
            &mut facts,
            &mut declarations,
            &mut initialization,
            None,
            &mut budget,
        )
        .err()
        .expect("construction must refuse");
        assert_eq!(failure, AllocationError::Budget(error));
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        assert!(declarations.symbols.is_empty());
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak);
    }
}

#[test]
fn lexical_and_narrowing_scope_publication_is_atomic_through_growth_and_refusal() {
    let scope = size_of::<AHashMap<&str, SymbolId>>() as u64;
    let narrowing = size_of::<Narrowing<'_>>() as u64;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    for (work, memory, expired, capacities, error, peak) in [
        (
            WORK,
            5 * scope + narrowing - 1,
            false,
            (1, 1),
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
            scope + narrowing,
        ),
        (
            WORK,
            4 * scope + 5 * narrowing - 1,
            false,
            (4, 1),
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
            5 * scope + narrowing,
        ),
        (
            4,
            100_000,
            false,
            (1, 1),
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            scope + narrowing,
        ),
        (
            6,
            100_000,
            false,
            (4, 1),
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            5 * scope + narrowing,
        ),
        (
            8,
            100_000,
            false,
            (4, 4),
            BudgetError::WorkExhausted(WorkDomain::Baseline),
            (5 * scope + narrowing).max(4 * scope + 5 * narrowing),
        ),
        (
            WORK,
            100_000,
            true,
            (1, 1),
            BudgetError::DeadlineExceeded,
            scope + narrowing,
        ),
    ] {
        let mut ledger = ledger(work, SENTINEL + memory);
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
        analyzer.narrowings[0].insert(SymbolId(3), Type::Array(Box::new(Type::Int)));
        let Type::Array(value) = &analyzer.narrowings[0][&SymbolId(3)] else {
            unreachable!()
        };
        let payload = &**value as *const Type<'_>;
        if expired {
            analyzer.budget.with_ledger(|owner| {
                owner
                    .unwrap()
                    .0
                    .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000))
            });
        }
        assert_eq!(analyzer.push_scope(), Err(AllocationError::Budget(error)));
        assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
        assert_eq!(
            (analyzer.scopes.capacity(), analyzer.narrowings.capacity()),
            capacities
        );
        assert_eq!(analyzer.scopes[0]["retained"], SymbolId(3));
        let Type::Array(value) = &analyzer.narrowings[0][&SymbolId(3)] else {
            unreachable!()
        };
        assert_eq!(&**value as *const Type<'_>, payload);
        drop(analyzer);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak);
    }

    let peak = (12 * scope + 4 * narrowing).max(8 * scope + 12 * narrowing);
    let mut ledger = ledger(26, SENTINEL + peak);
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
    for _ in 0..4 {
        analyzer.push_scope().unwrap();
    }
    assert_eq!(
        (analyzer.scopes.capacity(), analyzer.narrowings.capacity()),
        (8, 8)
    );
    assert_eq!(
        analyzer.budget.retained_bytes(AllocationClass::Scratch),
        8 * (scope + narrowing)
    );
    drop(analyzer);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak);
    assert_eq!(ledger.work_used(WorkDomain::Baseline), 26);
}

#[test]
fn callable_context_refusals_release_partial_frames_but_keep_declarations() {
    let s = size_of::<AHashMap<&str, SymbolId>>() as u64;
    let n = size_of::<Narrowing<'_>>() as u64;
    let parameters = 4 * size_of::<AHashSet<&str>>() as u64;
    let returns = 4 * size_of::<ReturnContext<'_>>() as u64;
    let generators = 4 * size_of::<Option<Type<'_>>>() as u64;
    let constructors = 4 * size_of::<Option<&str>>() as u64;
    let declarations =
        4 * (size_of::<Symbol<'_>>() + size_of::<Option<crate::module::ModuleId>>()) as u64;
    let scoped = declarations + 4 * (s + n) + parameters;
    let scope_peak = declarations + parameters + (5 * s + n).max(4 * s + 5 * n);
    for (source, required, peak, shared, capacities) in [
        (
            "int f(){return 1;}",
            s + n + parameters,
            s + n,
            0,
            (1, 0, 0, 0, 0),
        ),
        (
            "int f(){return 1;}",
            scoped + returns,
            scope_peak,
            declarations,
            (4, 4, 0, 0, 0),
        ),
        (
            "int f(){return 1;}",
            scoped + returns + generators,
            scoped + returns,
            declarations,
            (4, 4, 4, 0, 0),
        ),
        (
            "class Box{init(){}}",
            scoped + returns + constructors,
            scoped + returns,
            declarations,
            (4, 4, 4, 0, 0),
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let mut ledger = ledger(WORK, SENTINEL + required - 1);
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
            analyzer.analyze_program(&program),
            Err(AdmittedSemanticError::Resources(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Baseline)
            )))
        );
        assert_eq!(
            (
                analyzer.scopes.capacity(),
                analyzer.type_parameter_scopes.capacity(),
                analyzer.return_contexts.capacity(),
                analyzer.generator_contexts.capacity(),
                analyzer.constructor_classes.capacity()
            ),
            capacities,
            "{source}"
        );
        drop(analyzer);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), shared);
        drop(declarations);
        budget.release(AllocationClass::Scratch, shared).unwrap();
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak, "{source}");
    }
}

#[test]
fn all_context_backings_drop_before_release_without_releasing_shared_declarations() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        r#"
class Box<T> {
    init() {
        auto a = () => {
            auto b = () => {
                auto c = () => {
                    auto d = () => {
                        auto e = () => { return 1; };
                        return e();
                    };
                    return d();
                };
                return c();
            };
            return b();
        };
        a();
    }
}
"#,
    )
    .unwrap();
    let frames = (8
        * (size_of::<AHashMap<&str, SymbolId>>()
            + size_of::<Narrowing<'_>>()
            + size_of::<ReturnContext<'_>>()
            + size_of::<Option<&str>>()
            + size_of::<Option<Type<'_>>>())
        + 4 * size_of::<AHashSet<&str>>()) as u64;
    let shared =
        (8 * (size_of::<Symbol<'_>>() + size_of::<Option<crate::module::ModuleId>>())) as u64;
    for unwind in [false, true] {
        let mut ledger = ledger(WORK, 1_000_000);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let parent = budget.string(AllocationClass::Retained, "parent").unwrap();
        let mut facts = ModuleFacts::new(program.source_identity());
        let mut declarations = DeclarationTables::default();
        let mut initialization = ModuleInitialization::default();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut analyzer = Analyzer::new(
                &mut facts,
                &mut declarations,
                &mut initialization,
                None,
                &mut budget,
            )
            .unwrap();
            analyzer.analyze_program(&program).unwrap();
            assert_eq!(
                (
                    analyzer.scopes.capacity(),
                    analyzer.narrowings.capacity(),
                    analyzer.return_contexts.capacity(),
                    analyzer.type_parameter_scopes.capacity(),
                    analyzer.constructor_classes.capacity(),
                    analyzer.generator_contexts.capacity()
                ),
                (8, 8, 8, 4, 8, 8)
            );
            assert_eq!((analyzer.scopes.len(), analyzer.narrowings.len()), (1, 1));
            assert!(
                analyzer.return_contexts.is_empty()
                    && analyzer.type_parameter_scopes.is_empty()
                    && analyzer.constructor_classes.is_empty()
                    && analyzer.generator_contexts.is_empty()
            );
            assert_eq!(analyzer.declarations.symbols.len(), 7);
            assert_eq!(
                (
                    analyzer.declarations.symbols.capacity(),
                    analyzer.declarations.symbol_modules.capacity()
                ),
                (8, 8)
            );
            assert_eq!(
                analyzer.budget.retained_bytes(AllocationClass::Scratch),
                shared + frames
            );
            analyzer.budget.with_ledger(|owner| {
                owner
                    .unwrap()
                    .0
                    .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000))
            });
            if unwind {
                panic!("injected analyzer unwind");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), shared);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 6);
        assert_eq!(parent, "parent");
        drop(declarations);
        budget.release(AllocationClass::Scratch, shared).unwrap();
        drop(parent);
        budget.release(AllocationClass::Retained, 6).unwrap();
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
    }
}
