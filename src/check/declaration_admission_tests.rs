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

fn symbol(index: usize) -> Symbol<'static> {
    Symbol {
        id: SymbolId(index as u32),
        name: ["a", "b", "c", "d", "e"][index],
        ty: Type::Array(Box::new(Type::Int)),
        span: Span::new(index, index + 1),
        origin: DeclarationOrigin::Source,
        identifier_occurrences: 0,
    }
}

fn payload<'src>(ty: &Type<'src>) -> *const Type<'src> {
    let Type::Array(element) = ty else {
        panic!("array payload")
    };
    &**element
}

#[test]
fn symbol_pair_growth_admits_overlap_and_never_publishes_half_a_row() {
    let symbols = 4 * size_of::<Symbol<'_>>() as u64;
    let modules = 4 * size_of::<Option<crate::module::ModuleId>>() as u64;
    for (seed, memory, expected_capacities, peak) in [
        (0, 0, (0, 0), 0),
        (0, symbols, (4, 0), symbols),
        (4, 3 * symbols + modules - 1, (4, 4), symbols + modules),
    ] {
        let mut ledger = ledger(WORK, SENTINEL + memory);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut tables = DeclarationTables::default();
        for index in 0..seed {
            tables
                .add_symbol(symbol(index), Some(7), &mut budget)
                .unwrap();
        }
        let pointers: Vec<_> = tables
            .symbols
            .iter()
            .map(|symbol| payload(&symbol.ty))
            .collect();
        assert_eq!(
            tables.add_symbol(symbol(seed), Some(7), &mut budget),
            Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(tables.symbols.len(), seed);
        assert_eq!(tables.symbol_modules, vec![Some(7); seed]);
        assert_eq!(
            (tables.symbols.capacity(), tables.symbol_modules.capacity()),
            expected_capacities
        );
        assert_eq!(
            tables
                .symbols
                .iter()
                .map(|symbol| payload(&symbol.ty))
                .collect::<Vec<_>>(),
            pointers
        );
        drop(tables);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak);
    }

    let peak = (3 * symbols + modules).max(2 * symbols + 3 * modules);
    let mut ledger = ledger(22, SENTINEL + peak);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let mut tables = DeclarationTables::default();
    let mut pointers = Vec::new();
    for index in 0..5 {
        let value = symbol(index);
        pointers.push(payload(&value.ty));
        assert_eq!(
            tables.add_symbol(value, Some(7), &mut budget).unwrap(),
            SymbolId(index as u32)
        );
    }
    assert_eq!(
        (tables.symbols.capacity(), tables.symbol_modules.capacity()),
        (8, 8)
    );
    assert_eq!(
        tables
            .symbols
            .iter()
            .map(|symbol| payload(&symbol.ty))
            .collect::<Vec<_>>(),
        pointers
    );
    assert_eq!(
        budget.retained_bytes(AllocationClass::Scratch),
        2 * (symbols + modules)
    );
    drop(tables);
    drop(budget);
    assert_eq!(ledger.work_used(WorkDomain::Baseline), 22);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak);
}

#[test]
fn symbol_pair_work_and_deadline_refusals_preserve_rows_after_partial_reservation() {
    let symbols = 4 * size_of::<Symbol<'_>>() as u64;
    let modules = 4 * size_of::<Option<crate::module::ModuleId>>() as u64;
    for (work, expired, capacities, live, peak) in [
        (10, false, (4, 4), symbols + modules, symbols + modules),
        (
            15,
            false,
            (8, 4),
            2 * symbols + modules,
            3 * symbols + modules,
        ),
        (
            20,
            false,
            (8, 8),
            2 * (symbols + modules),
            (3 * symbols + modules).max(2 * symbols + 3 * modules),
        ),
        (WORK, true, (4, 4), symbols + modules, symbols + modules),
    ] {
        let mut ledger = ledger(work, 1_000_000);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut tables = DeclarationTables::default();
        for index in 0..4 {
            tables.add_symbol(symbol(index), None, &mut budget).unwrap();
        }
        let before = tables.symbols.clone();
        if expired {
            budget.with_ledger(|owner| {
                owner
                    .unwrap()
                    .0
                    .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000))
            });
        }
        let expected = if expired {
            BudgetError::DeadlineExceeded
        } else {
            BudgetError::WorkExhausted(WorkDomain::Baseline)
        };
        assert_eq!(
            tables.add_symbol(symbol(4), None, &mut budget),
            Err(AllocationError::Budget(expected))
        );
        assert_eq!(tables.symbols, before);
        assert_eq!(tables.symbol_modules, vec![None; 4]);
        assert_eq!(
            (tables.symbols.capacity(), tables.symbol_modules.capacity()),
            capacities
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), live);
        drop(tables);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + peak);
    }
}

#[test]
fn rejected_symbol_publication_does_not_install_scope_or_source_bindings() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int value=7;").unwrap();
    let Item::Stmt(Stmt::VarDecl(declaration)) = &syntax.items[0] else {
        unreachable!()
    };
    for detached in [false, true] {
        let mut ledger = ledger(4 + 2, 1_000_000);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut facts = ModuleFacts::new(syntax.source_identity());
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
        let result = if detached {
            analyzer.record_detached(declaration.name, Type::Int)
        } else {
            analyzer.declare(declaration.name, Type::Int)
        };
        assert_eq!(
            result,
            Err(AdmittedCheckError::Resources(AllocationError::Budget(
                BudgetError::WorkExhausted(WorkDomain::Baseline)
            )))
        );
        assert!(analyzer.scopes.iter().all(|scope| scope.is_empty()));
        assert!(analyzer.facts.identifier_symbols.is_empty());
        assert!(analyzer.facts.binding_types.is_empty());
        assert!(analyzer.declarations.symbols.is_empty());
        assert!(analyzer.declarations.symbol_modules.is_empty());
        assert_eq!(
            (
                analyzer.declarations.symbols.capacity(),
                analyzer.declarations.symbol_modules.capacity()
            ),
            (4, 4)
        );
        drop(analyzer);
        drop(initialization);
        drop(declarations);
        drop(facts);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
    }
}

#[test]
fn struct_and_member_backings_refuse_at_initial_and_later_growth() {
    let structs = 4 * size_of::<StructInfo<'_>>() as u64;
    let members = 4 * size_of::<MemberDefinition>() as u64;
    let base = (size_of::<AHashMap<&str, SymbolId>>() + size_of::<Narrowing<'_>>()) as u64;
    let parameters = 4 * size_of::<AHashSet<&str>>() as u64;
    for (source, frames, required, peak) in [
        ("struct A{}", base, structs, 0),
        (
            "struct A{}struct B{}struct C{}struct D{}struct E{}",
            base,
            3 * structs,
            structs,
        ),
        (
            "struct A{int a;}",
            base + parameters,
            structs + members,
            structs,
        ),
        (
            "struct A{int a;int b;int c;int d;int e;}",
            base + parameters,
            structs + 3 * members,
            structs + members,
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let facts = (syntax.source_identity().len()
            * (size_of::<Option<Type<'_>>>() + size_of::<SourceInfo<'_, '_>>()))
            as u64;
        let mut ledger = ledger(WORK, SENTINEL + facts + frames + required - 1);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let error = with_analyzed_source(&syntax, &mut budget, |_, _| {
            panic!("refused declaration must not reach callback")
        })
        .unwrap_err();
        assert_eq!(
            error,
            AdmittedCheckError::Resources(AllocationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(
            ledger.peak_retained_bytes(),
            SENTINEL + facts + frames + peak
        );
    }
}

#[test]
fn all_four_backings_and_detached_symbols_release_on_callback_error_and_unwind() {
    let source = "struct A{int a;int b;int c;int d;int e;}struct B{}struct C{}struct D{}struct E{}extern void consume(int left,int right);int first=1;int second=2;";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let expected = analyze(&syntax).unwrap();
    let nodes = syntax.source_identity().len() as u64;
    let facts = nodes * (size_of::<Option<Type<'_>>>() + size_of::<SourceInfo<'_, '_>>()) as u64;
    let s = size_of::<Symbol<'_>>() as u64;
    let m = size_of::<Option<crate::module::ModuleId>>() as u64;
    let t = size_of::<StructInfo<'_>>() as u64;
    let n = size_of::<MemberDefinition>() as u64;
    let base = (size_of::<AHashMap<&str, SymbolId>>() + size_of::<Narrowing<'_>>()) as u64;
    let parameters = 4 * size_of::<AHashSet<&str>>() as u64;
    let live = facts + 8 * (s + m + t + n);
    let peak = facts
        + base
        + (12 * t)
            .max(8 * t + parameters + 12 * n)
            .max(8 * (t + n) + parameters + (12 * s + 4 * m).max(8 * s + 12 * m));
    for unwind in [false, true] {
        let mut ledger = ledger(WORK, SENTINEL + 6 + peak);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let parent = budget.string(AllocationClass::Retained, "parent").unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_analyzed_source(&syntax, &mut budget, |model, scope| {
                assert_eq!(format!("{model:?}"), format!("{expected:?}"));
                assert_eq!(
                    (
                        model.declarations.symbols.len(),
                        model.declarations.symbol_modules.len(),
                        model.declarations.structs.len(),
                        model.declarations.nominal_members.len()
                    ),
                    (5, 5, 5, 5)
                );
                assert_eq!(
                    (
                        model.declarations.symbols.capacity(),
                        model.declarations.symbol_modules.capacity(),
                        model.declarations.structs.capacity(),
                        model.declarations.nominal_members.capacity()
                    ),
                    (8, 8, 8, 8)
                );
                for name in ["left", "right"] {
                    assert!(model.symbols().iter().any(|symbol| symbol.name == name));
                }
                assert_eq!(scope.retained_bytes(AllocationClass::Scratch), live);
                if unwind {
                    panic!("injected declaration callback panic");
                }
                Err::<(), _>("client refusal")
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().unwrap(), Err("client refusal"));
        }
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 6);
        assert_eq!(parent, "parent");
        drop(parent);
        budget.release(AllocationClass::Retained, 6).unwrap();
        drop(budget);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        assert_eq!(ledger.peak_retained_bytes(), SENTINEL + 6 + peak);
        // Parent string: one allocation and 6 copied bytes. Each five-row single
        // vector costs 11 units; the five paired symbol rows cost 22 units.
        // Analyzer creation costs 4; seven type scopes share one allocation.
        assert_eq!(
            ledger.work_used(WorkDomain::Baseline),
            7 + 2 * nodes + 2 + 11 + 11 + 22 + 4 + 8
        );
    }
}
