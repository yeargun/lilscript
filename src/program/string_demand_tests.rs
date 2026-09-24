//! Real retained string recipes exercise the common demand owner. These checks
//! inspect semantic identities and work/storage demand, never emitted spelling.
use super::demand::{DemandError, DemandMode, DemandPlan};
use super::facts::{
    CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};
use super::helper_family;
use super::implementations::ImplementationMap;
use super::string_family::{self, StringChoice, StringFamily, ValueRef};
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::JavaScriptCompilationContract;
use crate::compilation_policy::{
    AnalysisAttempt, BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits,
    WorkDomain, WorkKind,
};

const MEMORY: u64 = 10_000_000;
const WORK: u64 = 10_000_000;

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    inspect(&from_checked_source(&syntax, &semantics).unwrap());
}
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work: WORK,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap()
}
fn contract() -> JavaScriptCompilationContract {
    let mut configuration = crate::config::ProjectConfig::default();
    configuration.javascript.strip_console = false;
    *configuration
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .javascript_contract()
        .unwrap()
}
fn cache(ledger: &mut BudgetLedger) -> RetainedFactsCache {
    RetainedFactsCache::new(
        CacheLimits {
            entries: 8,
            bytes: 1_000_000,
            result_bytes: 100_000,
        },
        ledger,
        WorkDomain::Baseline,
    )
    .unwrap()
}
fn facts_request() -> FactRequest {
    FactRequest {
        attempt: AnalysisAttempt {
            plan: LOCAL_FACTS_PLAN,
            algorithm_version: LOCAL_FACTS_VERSION,
            work_quota: 100_000,
        },
        result_bytes: 100_000,
    }
}
fn definitions(program: &Program<'_>) -> Vec<ValueRef> {
    program
        .units
        .iter()
        .enumerate()
        .flat_map(|(unit, data)| {
            data.data().operations.iter().filter_map(move |operation| {
                matches!(operation.kind, OperationKind::Binary(BinaryOp::Add)).then(|| ValueRef {
                    unit: UnitId::from_index(unit).unwrap(),
                    value: operation.result.unwrap(),
                })
            })
        })
        .collect()
}
fn string(
    program: &Program<'_>,
    definitions: &[ValueRef],
    choice: StringChoice,
    cache: &mut RetainedFactsCache,
    ledger: &mut BudgetLedger,
) -> StringFamily {
    let preparation = string_family::prepare(
        program,
        definitions,
        choice,
        string_family::FamilyRequest {
            attempt: AnalysisAttempt {
                plan: string_family::STRING_FAMILY_PLAN,
                algorithm_version: string_family::STRING_FAMILY_VERSION,
                work_quota: 100_000,
            },
            scratch_bytes: 100_000,
            output_bytes: 100_000,
            local_facts: facts_request(),
        },
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    let mut ready = match preparation.outcome {
        string_family::PreparationOutcome::Ready(ready) => ready,
        other => panic!("string preparation: {other:?}"),
    };
    {
        let mut session = cache.session(ledger, WorkDomain::Optional, 1).unwrap();
        let view = session
            .query(program, ready.unit(), facts_request())
            .unwrap();
        ready
            .check_facts(program, view.facts, view.receipt)
            .unwrap();
    }
    match ready.finish(ledger).unwrap().outcome {
        string_family::FamilyOutcome::Complete(family) => family,
        other => panic!("string proof: {other:?}"),
    }
}
fn inline_helper(
    program: &Program<'_>,
    uses: &UseIndex,
    cache: &mut RetainedFactsCache,
    ledger: &mut BudgetLedger,
) -> helper_family::HelperFamily {
    let cell = CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "label")
            .unwrap(),
    )
    .unwrap();
    let preparation = helper_family::prepare(
        program,
        uses,
        cell,
        helper_family::FamilyRequest {
            execution: crate::compilation_contract::JavaScriptExecution::Module,
            attempt: AnalysisAttempt {
                plan: helper_family::HELPER_FAMILY_PLAN,
                algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                work_quota: 100_000,
            },
            scratch_bytes: 100_000,
            output_bytes: 100_000,
        },
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    let mut ready = match preparation.outcome {
        helper_family::PreparationOutcome::Ready(ready) => ready,
        other => panic!("helper preparation: {other:?}"),
    };
    {
        let mut session = cache.session(ledger, WorkDomain::Optional, 1).unwrap();
        let view = session
            .query(program, ready.root().body, facts_request())
            .unwrap();
        ready.check_body(program, view.facts, view.receipt).unwrap();
    }
    match ready.finish(ledger).unwrap() {
        helper_family::FamilyOutcome::Complete(family) => family,
        other => panic!("helper proof: {other:?}"),
    }
}

#[test]
fn selected_production_cuts_obsolete_operands_without_cutting_execute_or_host_roots() {
    checked(
        "extern int effect();effect();print((\"left\"+\"middle\")+\"right\");",
        |program| {
            let mut ledger = ledger();
            let mut cache = cache(&mut ledger);
            let values = definitions(program);
            assert_eq!(values.len(), 2);
            let selected = values[1];
            let family = string(
                program,
                &[selected],
                StringChoice::LiteralAtDefinition,
                &mut cache,
                &mut ledger,
            );
            let map = ImplementationMap::direct()
                .with_string(family, &mut ledger, WorkDomain::Optional)
                .unwrap();
            let data = program.unit(selected.unit).unwrap();
            let operation = data.values[selected.value.index()].definition;
            let inner = data.values[values[0].value.index()].definition;
            let operands = data
                .operands(data.operations[operation.index()].operands)
                .unwrap();
            let host = OpId::from_index(data.operations.iter().position(|op| matches!(op.kind, OperationKind::Call(call) if matches!(data.calls[call.index()].target, CallTarget::Value{..}))).unwrap()).unwrap();
            for mode in [DemandMode::Prune, DemandMode::Preserve] {
                let before = ledger.retained_bytes();
                let plan = DemandPlan::build(
                    program,
                    None,
                    Some(&map),
                    &contract(),
                    mode,
                    Some((&mut ledger, WorkDomain::Optional)),
                )
                .unwrap();
                let root = plan.root();
                assert_eq!(plan.string_operation(selected.unit, operation), Some(0));
                assert_eq!(plan.string_value(selected.unit, selected.value), Some(0));
                assert!(plan.needs_production(root, operation));
                assert_eq!(
                    plan.needs_execution(root, operation),
                    mode == DemandMode::Preserve
                );
                assert_eq!(
                    plan.needs_operation(root, inner),
                    mode == DemandMode::Preserve
                );
                for &operand in operands {
                    assert_eq!(
                        plan.needs_value(root, operand),
                        mode == DemandMode::Preserve
                    );
                }
                assert!(plan.needs_execution(root, host));
                assert!(plan.shared_strings(root).next().is_none());
                plan.discard(Some(&mut ledger)).unwrap();
                assert_eq!(ledger.retained_bytes(), before);
            }
            map.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn shared_storage_is_rooted_by_selected_values_only_and_dead_selections_remain_dead() {
    for observed in [false, true] {
        let source = if observed {
            "string unused=\"left\"+\"right\";print(\"left\"+\"right\");"
        } else {
            "string unused=\"left\"+\"right\";print(9);"
        };
        checked(source, |program| {
            let mut ledger = ledger();
            let mut cache = cache(&mut ledger);
            let values = definitions(program);
            let family = string(
                program,
                &values,
                StringChoice::SharedLiteral {
                    activation: values[0].unit,
                },
                &mut cache,
                &mut ledger,
            );
            let map = ImplementationMap::direct()
                .with_string(family, &mut ledger, WorkDomain::Optional)
                .unwrap();
            for mode in [DemandMode::Prune, DemandMode::Preserve] {
                let before = ledger.retained_bytes();
                let plan = DemandPlan::build(
                    program,
                    None,
                    Some(&map),
                    &contract(),
                    mode,
                    Some((&mut ledger, WorkDomain::Optional)),
                )
                .unwrap();
                let root = plan.root();
                let needed = observed || mode == DemandMode::Preserve;
                assert_eq!(plan.needs_shared_string(root, 0), needed);
                assert_eq!(
                    plan.shared_strings(root).collect::<Vec<_>>(),
                    if needed { vec![0] } else { vec![] }
                );
                assert_eq!(
                    plan.needs_value(root, values[0].value),
                    mode == DemandMode::Preserve
                );
                plan.discard(Some(&mut ledger)).unwrap();
                assert_eq!(ledger.retained_bytes(), before);
            }
            map.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn shared_data_demand_belongs_to_each_inline_occurrence_in_either_composition_order() {
    checked(
        "string label(){return \"left\"+\"right\";}label();print(label());print(label());",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let values = definitions(program);
            assert_eq!(values.len(), 1);
            for string_first in [false, true] {
                let helper = inline_helper(program, &uses, &mut cache, &mut ledger);
                let calls = helper
                    .calls()
                    .iter()
                    .map(|call| call.call)
                    .collect::<Vec<_>>();
                assert_eq!(calls.len(), 3);
                let family = string(
                    program,
                    &values,
                    StringChoice::SharedLiteral {
                        activation: values[0].unit,
                    },
                    &mut cache,
                    &mut ledger,
                );
                let (first, map) = if string_first {
                    let first = ImplementationMap::direct()
                        .with_string(family, &mut ledger, WorkDomain::Optional)
                        .unwrap();
                    let map = first
                        .with_inline_helper(helper, &mut ledger, WorkDomain::Optional)
                        .unwrap();
                    (first, map)
                } else {
                    let first = ImplementationMap::direct()
                        .with_inline_helper(helper, &mut ledger, WorkDomain::Optional)
                        .unwrap();
                    let map = first
                        .with_string(family, &mut ledger, WorkDomain::Optional)
                        .unwrap();
                    (first, map)
                };
                first.discard(&mut ledger).unwrap();
                for mode in [DemandMode::Prune, DemandMode::Preserve] {
                    let before = ledger.retained_bytes();
                    let plan = DemandPlan::build(
                        program,
                        Some(&uses),
                        Some(&map),
                        &contract(),
                        mode,
                        Some((&mut ledger, WorkDomain::Optional)),
                    )
                    .unwrap();
                    let root = plan.root();
                    assert!(plan.shared_strings(root).next().is_none());
                    let mut observed = 0;
                    let mut discarded = 0;
                    let mut occurrences = Vec::new();
                    let data = program.unit(program.initialization[0]).unwrap();
                    for call in &calls {
                        let child = plan.child(root, *call).unwrap();
                        assert!(!occurrences.contains(&child));
                        occurrences.push(child);
                        let result = data.operations[call.index()].result.unwrap();
                        let needed = plan.needs_value(root, result);
                        assert_eq!(plan.needs_shared_string(child, 0), needed);
                        assert_eq!(plan.needs_value(child, values[0].value), needed);
                        if needed {
                            observed += 1;
                        } else {
                            discarded += 1;
                        }
                    }
                    assert_eq!(
                        (observed, discarded),
                        if mode == DemandMode::Prune {
                            (2, 1)
                        } else {
                            (3, 0)
                        }
                    );
                    plan.discard(Some(&mut ledger)).unwrap();
                    assert_eq!(ledger.retained_bytes(), before);
                }
                map.discard(&mut ledger).unwrap();
            }
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn string_demand_memory_and_preservation_work_failures_restore_existing_owners() {
    checked("print((\"left\"+\"middle\")+\"right\");", |program| {
        let mut ledger = ledger();
        let mut cache = cache(&mut ledger);
        let values = definitions(program);
        let family = string(
            program,
            &values[1..],
            StringChoice::SharedLiteral {
                activation: values[1].unit,
            },
            &mut cache,
            &mut ledger,
        );
        let map = ImplementationMap::direct()
            .with_string(family, &mut ledger, WorkDomain::Optional)
            .unwrap();
        let before = ledger.retained_bytes();
        let plan = DemandPlan::build(
            program,
            None,
            Some(&map),
            &contract(),
            DemandMode::Preserve,
            Some((&mut ledger, WorkDomain::Optional)),
        )
        .unwrap();
        let work = plan.work();
        plan.discard(Some(&mut ledger)).unwrap();
        assert_eq!(ledger.retained_bytes(), before);
        let filler = MEMORY - before - (work.peak_bytes - 1);
        ledger.retain(WorkDomain::Optional, filler).unwrap();
        let occupied = ledger.retained_bytes();
        assert!(matches!(
            DemandPlan::build(
                program,
                None,
                Some(&map),
                &contract(),
                DemandMode::Preserve,
                Some((&mut ledger, WorkDomain::Optional))
            ),
            Err(DemandError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(ledger.retained_bytes(), occupied);
        ledger.release(WorkDomain::Optional, filler).unwrap();
        let remaining = WORK - ledger.work_used(WorkDomain::Optional);
        ledger
            .charge(
                WorkDomain::Optional,
                WorkKind::Analysis,
                remaining - (work.steps - 1),
            )
            .unwrap();
        assert!(matches!(
            DemandPlan::build(
                program,
                None,
                Some(&map),
                &contract(),
                DemandMode::Preserve,
                Some((&mut ledger, WorkDomain::Optional))
            ),
            Err(DemandError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(ledger.retained_bytes(), before);
        map.discard(&mut ledger).unwrap();
        cache.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
