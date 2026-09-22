use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};

const WORK: u64 = 10_000_000;
const MEMORY: u64 = 10_000_000;
const SOURCE: &str = "struct Pair{int x;int y;}enum Label{A,B}int global=3;int identity(int x){return global+x;}export int result=identity(4);export int other=5;Pair pair=Pair{1,2};print(pair.x+result);";

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    inspect(&program);
}

fn ledger(memory: u64, work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 0,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}

fn run(
    program: &Program<'_>,
    memory: u64,
    work: u64,
) -> (Result<VerificationReceipt, VerificationError>, BudgetLedger) {
    let mut ledger = ledger(memory, work);
    let result = {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let result = verify_admitted(program, &mut budget);
        assert_eq!(budget.retained_bytes(Scratch), 0);
        result
    };
    assert_eq!(ledger.retained_bytes(), 0);
    (result, ledger)
}

#[test]
fn admitted_full_verification_matches_inspection_and_releases_all_scratch() {
    checked(SOURCE, |program| {
        program.verify().unwrap();
        let (first, first_ledger) = run(program, MEMORY, WORK);
        let receipt = first.unwrap();
        assert_eq!(receipt.units, program.units.len());
        assert_eq!(
            receipt.operations,
            program
                .units
                .iter()
                .map(|unit| unit.data().operations.len())
                .sum::<usize>()
        );
        assert!(first_ledger.peak_retained_bytes() > 0);
        assert!(first_ledger.work_used(WorkDomain::Optional) > receipt.operations as u64);
        let (again, again_ledger) = run(program, MEMORY, WORK);
        assert_eq!(again.unwrap(), receipt);
        assert_eq!(
            again_ledger.peak_retained_bytes(),
            first_ledger.peak_retained_bytes()
        );
        assert_eq!(
            again_ledger.work_used(WorkDomain::Optional),
            first_ledger.work_used(WorkDomain::Optional)
        );
    });
}

#[test]
fn work_memory_and_deadline_refusal_stay_typed_without_mutating_program() {
    checked(SOURCE, |program| {
        let (_, success) = run(program, MEMORY, WORK);
        let used = success.work_used(WorkDomain::Optional);
        let peak = success.peak_retained_bytes();
        let revisions: Vec<_> = program.units.iter().map(FrozenUnit::revision).collect();
        for limit in [0, 1, used / 3, used - 1] {
            let (result, ledger) = run(program, MEMORY, limit);
            assert!(matches!(
                result,
                Err(VerificationError::Allocation(AllocationError::Budget(_)))
            ));
            assert!(ledger.work_used(WorkDomain::Optional) <= limit);
        }
        for limit in [0, 1, peak / 2, peak - 1] {
            let (result, ledger) = run(program, limit, WORK);
            assert!(matches!(
                result,
                Err(VerificationError::Allocation(AllocationError::Budget(_)))
            ));
            assert!(ledger.peak_retained_bytes() <= limit);
        }
        let mut ledger = BudgetLedger::new(
            ResourceLimits {
                wall_time_ms: Some(10),
                ..ResourceLimits::default()
            },
            BudgetPlan {
                baseline_work: 0,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
        .unwrap();
        ledger.set_deadline_elapsed_for_test(std::time::Duration::from_millis(10));
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            assert!(matches!(
                verify_admitted(program, &mut budget),
                Err(VerificationError::Allocation(AllocationError::Budget(
                    BudgetError::DeadlineExceeded
                )))
            ));
        }
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.work_used(WorkDomain::Optional), 0);
        assert_eq!(
            program
                .units
                .iter()
                .map(FrozenUnit::revision)
                .collect::<Vec<_>>(),
            revisions
        );
        program.verify().unwrap();
    });
}

#[test]
fn scratch_indexes_keep_duplicate_contracts_and_admit_compared_name_bytes() {
    checked(SOURCE, |program| {
        let (_, ordinary) = run(program, MEMORY, WORK);
        let mut long = program.clone();
        std::sync::Arc::make_mut(&mut long.exports)[0].name = "public".repeat(1024);
        let (result, larger) = run(&long, MEMORY, WORK);
        result.unwrap();
        assert!(larger.work_used(WorkDomain::Optional) > ordinary.work_used(WorkDomain::Optional));
        let mut repeated_enum = program.clone();
        let definition = repeated_enum.enums[0].clone();
        std::sync::Arc::make_mut(&mut repeated_enum.enums).push(definition);
        let (result, _) = run(&repeated_enum, MEMORY, WORK);
        assert!(matches!(
            result,
            Err(VerificationError::Invalid("duplicate enum declaration"))
        ));
        let mut repeated_variant = program.clone();
        let definition = &mut std::sync::Arc::make_mut(&mut repeated_variant.enums)[0];
        definition.variants[1].value = definition.variants[0].value;
        let (result, _) = run(&repeated_variant, MEMORY, WORK);
        assert!(matches!(
            result,
            Err(VerificationError::Invalid("duplicate enum variant"))
        ));
        let mut repeated_export = program.clone();
        let export = repeated_export.exports[0].clone();
        std::sync::Arc::make_mut(&mut repeated_export.exports).push(export);
        std::sync::Arc::make_mut(&mut repeated_export.modules)[0]
            .exports
            .end += 1;
        let (result, _) = run(&repeated_export, MEMORY, WORK);
        assert!(matches!(
            result,
            Err(VerificationError::Invalid(
                "duplicate public export within a module"
            ))
        ));
    });
}

#[test]
fn unused_nested_types_are_admitted_before_branch_workspace_growth() {
    checked("print(1);", |program| {
        let mut nested = program.clone();
        std::sync::Arc::make_mut(&mut nested.types).push(Type::Union(vec![Type::Int; 128]));
        let mut success = ledger(MEMORY, WORK);
        {
            let mut budget = AllocationBudget::new(Some((&mut success, WorkDomain::Optional)));
            verify_type_contracts(&nested, &mut budget).unwrap();
            assert_eq!(budget.retained_bytes(Scratch), 0);
        }
        assert!(success.peak_retained_bytes() >= 127 * std::mem::size_of::<&Type<'_>>() as u64);
        let mut denied = ledger(success.peak_retained_bytes() - 1, WORK);
        {
            let mut budget = AllocationBudget::new(Some((&mut denied, WorkDomain::Optional)));
            assert!(matches!(
                verify_type_contracts(&nested, &mut budget),
                Err(VerificationError::Allocation(_))
            ));
        }
        assert_eq!(denied.retained_bytes(), 0);
        assert!(denied.peak_retained_bytes() < success.peak_retained_bytes());
        nested.verify().unwrap();
    });
}

#[test]
fn shared_static_schedule_admits_cycles_scratch_and_edge_visits() {
    use crate::module::{
        static_evaluation_order, static_evaluation_order_admitted, StaticOrderError,
    };
    let graph = [vec![1, 2], vec![0, 2], vec![1]];
    let expected = static_evaluation_order(0, graph.len(), |id| graph[id].iter().copied()).unwrap();
    let mut success = ledger(MEMORY, WORK);
    {
        let mut budget = AllocationBudget::new(Some((&mut success, WorkDomain::Optional)));
        let order = static_evaluation_order_admitted(
            0,
            graph.len(),
            |id| graph[id].iter().copied(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(order, expected);
        assert_eq!(
            budget.retained_bytes(Scratch),
            (order.capacity() * std::mem::size_of::<usize>()) as u64
        );
        drop(order);
    }
    assert_eq!(success.retained_bytes(), 0);
    assert!(
        success.work_used(WorkDomain::Optional) >= graph.iter().map(Vec::len).sum::<usize>() as u64
    );
    for memory in [0, success.peak_retained_bytes() - 1] {
        let mut denied = ledger(memory, WORK);
        {
            let mut budget = AllocationBudget::new(Some((&mut denied, WorkDomain::Optional)));
            assert!(matches!(
                static_evaluation_order_admitted(
                    0,
                    graph.len(),
                    |id| graph[id].iter().copied(),
                    &mut budget
                ),
                Err(StaticOrderError::Resources(_))
            ));
        }
        assert_eq!(denied.retained_bytes(), 0);
    }
}

#[test]
fn source_conversion_routes_admitted_verification_and_preserves_refusal_kind() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let ordinary = from_checked_source(&syntax, &semantics).unwrap();
    let (_, standalone) = run(&ordinary, MEMORY, WORK);
    let mut admitted = ledger(MEMORY, WORK);
    let prepared = {
        let mut budget = AllocationBudget::new(Some((&mut admitted, WorkDomain::Optional)));
        let prepared = from_checked_source_admitted(&syntax, &semantics, &mut budget).unwrap();
        assert_eq!(prepared.program().units.len(), ordinary.units.len());
        assert_eq!(budget.retained_bytes(Scratch), 0);
        prepared
    };
    assert!(admitted.work_used(WorkDomain::Optional) > standalone.work_used(WorkDomain::Optional));
    assert!(admitted.peak_retained_bytes() > standalone.peak_retained_bytes());
    prepared.discard(&mut admitted);
    assert_eq!(admitted.retained_bytes(), 0);
    let mut denied = ledger(0, WORK);
    {
        let mut budget = AllocationBudget::new(Some((&mut denied, WorkDomain::Optional)));
        assert!(matches!(
            from_checked_source_admitted(&syntax, &semantics, &mut budget),
            Err(ConversionError::Resources(AllocationError::Budget(_)))
        ));
    }
    assert_eq!(denied.retained_bytes(), 0);
    ordinary.verify().unwrap();
}
