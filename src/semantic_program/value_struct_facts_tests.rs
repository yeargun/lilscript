use super::facts::*;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantic = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantic).unwrap();
    program.verify().unwrap();
    inspect(program);
}

fn with_facts(program: &Program<'_>, inspect: impl FnOnce(&UnitData, &UnitFacts)) {
    let mut ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 1_000_000,
            optional_work: 1_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 4_000_000,
        },
    )
    .unwrap();
    let mut cache = FactsCache::new(CacheLimits {
        entries: 1,
        bytes: 1_000_000,
        result_bytes: 100_000,
    })
    .unwrap();
    let unit = program.initialization[0];
    let mut session = FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
    let answer = session
        .query(
            program,
            unit,
            FactRequest {
                attempt: AnalysisAttempt {
                    plan: LOCAL_FACTS_PLAN,
                    algorithm_version: LOCAL_FACTS_VERSION,
                    work_quota: 100_000,
                },
                result_bytes: 100_000,
            },
        )
        .unwrap();
    assert!(answer.facts.dependencies().valid_for(program));
    inspect(program.unit(unit).unwrap(), answer.facts);
}

#[test]
fn logical_struct_values_have_resource_cost_without_reference_identity_or_host_effects() {
    checked("struct Pair{int x;}extern int host();Pair original=Pair{host()};Pair copy=original;copy.x=3;print(copy.x);",
        |program| with_facts(&program, |unit, facts| {
            let mut construction = 0;
            let mut copies = 0;
            let mut host_calls = 0;
            let mut field_accesses = 0;
            for (index, operation) in unit.operations.iter().enumerate() {
                let id = OpId::from_index(index).unwrap();
                match operation.kind {
                    OperationKind::Allocate {kind: AllocationKind::Struct(_), ..}
                    | OperationKind::CopyValue => {
                        if matches!(operation.kind, OperationKind::CopyValue) {copies += 1;}
                        else {construction += 1;}
                        assert_eq!(facts.effects(id), EvaluationBehavior {
                            may_exhaust_resources: true, ..EvaluationBehavior::TOTAL
                        });
                        assert_eq!(facts.can_drop(id, ObservationDemand::Discarded), Legality::PermittedUnderContext);
                        assert_eq!(facts.can_duplicate(id), Legality::PermittedUnderContext);
                        assert_eq!(facts.can_speculate(id, SpeculationContext {operands_available:true}), Legality::PermittedUnderContext);
                        assert_ne!(facts.can_speculate(id, SpeculationContext {operands_available:false}), Legality::PermittedUnderContext);
                        assert!(facts.exact(operation.result.unwrap()).is_none(),
                            "value-copy legality does not invent an exact aggregate payload");
                    }
                    OperationKind::Call(_) => {
                        host_calls += 1;
                        assert!(facts.effects(id).requires_evaluation(),
                            "constructor operands and external observations keep their own effects");
                    }
                    OperationKind::Load(place) | OperationKind::Store(place)
                        if matches!(unit.places[place.index()], Place::Field {..}) => {
                        field_accesses += 1;
                        assert_eq!(facts.effects(id), EvaluationBehavior::UNKNOWN,
                            "value identity is not an initialized product-domain/host-access proof");
                    }
                    _ => {}
                }
            }
            assert!(construction > 0 && copies > 0 && host_calls >= 2 && field_accesses >= 2);
        }));
}

#[test]
fn reference_handle_copy_is_total_without_proving_a_primitive_runtime_domain() {
    checked(
        "int[] first=[1];int[] second=first;print(second.length);",
        |mut program| {
            let unit_id = program.initialization[0];
            let first = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "first")
                    .unwrap(),
            )
            .unwrap();
            let unit = program.unit(unit_id).unwrap();
            let initializer = unit.operations.iter().find(|operation|
            matches!(operation.kind, OperationKind::Initialize(cell) if cell == first)).unwrap().operands;
            let load = unit
                .operations
                .iter()
                .position(|operation| {
                    matches!(operation.kind, OperationKind::Load(place)
                if matches!(unit.places[place.index()], Place::Cell(cell) if cell == first))
                })
                .unwrap();
            let mut working = program.units[unit_id.index()].clone().into_working();
            working.get_mut().operations[load].kind = OperationKind::CopyValue;
            working.get_mut().operations[load].operands = initializer;
            program.units[unit_id.index()] = working.freeze();
            program.verify().unwrap();
            with_facts(&program, |unit, facts| {
                let operation = &unit.operations[load];
                assert_eq!(
                    facts.effects(OpId::from_index(load).unwrap()),
                    EvaluationBehavior::TOTAL
                );
                assert!(!primitive_result_domain(&program, unit, operation, &[]));
                assert!(facts.exact(operation.result.unwrap()).is_none());
            });
        },
    );
}

#[test]
fn read_only_value_root_transfers_actual_domain_without_a_cell_initialization_assumption() {
    checked("int first=7;print(first);", |mut program| {
        let unit_id = program.initialization[0];
        let first = CellId::from_index(
            program
                .cells
                .iter()
                .position(|cell| cell.name == "first")
                .unwrap(),
        )
        .unwrap();
        let unit = program.unit(unit_id).unwrap();
        let initializer = unit.operations.iter().find(|operation|
            matches!(operation.kind, OperationKind::Initialize(cell) if cell == first)).unwrap();
        let input = unit.operands(initializer.operands).unwrap()[0];
        let load = unit
            .operations
            .iter()
            .position(|operation| {
                matches!(operation.kind, OperationKind::Load(place)
                if matches!(unit.places[place.index()], Place::Cell(cell) if cell == first))
            })
            .unwrap();
        let mut working = program.units[unit_id.index()].clone().into_working();
        let place = PlaceId::from_index(working.data().places.len()).unwrap();
        working.get_mut().places.push(Place::Value(input));
        working.get_mut().operations[load].kind = OperationKind::Load(place);
        program.units[unit_id.index()] = working.freeze();
        program.verify().unwrap();
        with_facts(&program, |unit, facts| {
            let operation = &unit.operations[load];
            assert_eq!(
                facts.effects(OpId::from_index(load).unwrap()),
                EvaluationBehavior::TOTAL
            );
            assert!(!primitive_result_domain(&program, unit, operation, &[]));
            let mut domains = vec![false; unit.values.len()];
            domains[input.index()] = true;
            assert!(primitive_result_domain(&program, unit, operation, &domains));
        });
    });
}
