//! Effective occurrences are checked against semantic identities and selected
//! families. Counts are physical recipe inputs, not demand-worklist visits.
use super::demand::{
    ContextId, DemandMode, DemandPlan, EffectiveUseRole as Role, EffectiveUseSite as Site,
    EffectiveValueUse,
};
use super::facts::{
    CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};
use super::implementations::ImplementationMap;
use super::uses::UseIndex;
use super::*;
use super::{helper_family, record_family, string_family};
use crate::compilation_contract::JavaScriptCompilationContract;
use crate::compilation_policy::{
    AnalysisAttempt, BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits,
    WorkDomain, WorkKind,
};

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
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}
fn contract(strip: bool) -> JavaScriptCompilationContract {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = strip;
    *config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .javascript_contract()
        .unwrap()
}
fn all(plan: &DemandPlan<'_, '_>, context: ContextId) -> Vec<EffectiveValueUse> {
    let mut result = Vec::new();
    plan.visit_effective_value_uses(
        context,
        |_| Ok::<_, ()>(()),
        |input| {
            result.push(input);
            Ok(())
        },
    )
    .unwrap();
    result
}
fn at(plan: &DemandPlan<'_, '_>, context: ContextId, site: Site) -> Vec<EffectiveValueUse> {
    let mut result = Vec::new();
    plan.visit_effective_site_uses(
        context,
        site,
        |_| Ok::<_, ()>(()),
        |input| {
            result.push(input);
            Ok(())
        },
    )
    .unwrap();
    result
}
fn cell(program: &Program<'_>, name: &str) -> CellId {
    CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == name)
            .unwrap(),
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
fn helper(
    program: &Program<'_>,
    uses: &UseIndex,
    root: CellId,
    cache: &mut RetainedFactsCache,
    ledger: &mut BudgetLedger,
) -> helper_family::HelperFamily {
    let prepared = helper_family::prepare(
        program,
        uses,
        root,
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
    let mut ready = match prepared.outcome {
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
fn repeated_operands_remain_distinct_and_execute_produce_do_not_double_count() {
    checked("print(7+8);", |original| {
        // A valid local edit makes two operand positions use the same SSA value.
        let mut program = original.clone();
        let unit = program.initialization[0];
        let mut working = program.units[unit.index()].clone().into_working();
        let data = working.get_mut();
        let operation = OpId::from_index(
            data.operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::IntBinary(_)))
                .unwrap(),
        )
        .unwrap();
        let range = data.operations[operation.index()].operands;
        let value = data.operands[range.start as usize];
        data.operands[range.start as usize + 1] = value;
        program.units[unit.index()] = working.freeze();
        program.verify().unwrap();
        for mode in [DemandMode::Prune, DemandMode::Preserve] {
            let plan =
                DemandPlan::build(&program, None, None, &contract(false), mode, None).unwrap();
            let inputs = at(&plan, plan.root(), Site::Operation(operation));
            assert_eq!(
                inputs
                    .iter()
                    .map(|input| (input.value, input.role))
                    .collect::<Vec<_>>(),
                vec![(value, Role::Operand(0)), (value, Role::Operand(1))]
            );
            if mode == DemandMode::Preserve {
                assert!(plan.needs_execution(plan.root(), operation));
                assert!(plan.needs_production(plan.root(), operation));
            }
            plan.discard(None).unwrap();
        }
    });
}

#[test]
fn prepared_receiver_and_key_precede_arguments_and_site_inventory_agrees() {
    checked("extern (func(int,int)->int)[] object();extern int key();extern int argument(int value);print(object()[key()](argument(2),argument(3)));", |program| {
        let plan = DemandPlan::build(program, None, None, &contract(false), DemandMode::Prune, None).unwrap();
        let root = plan.root();
        let data = program.unit(plan.context(root).unit).unwrap();
        let (index, place) = data.calls.iter().enumerate().find_map(|(index, call)| match &call.target { CallTarget::Reference { place } => Some((index,*place)), _ => None }).unwrap();
        let call = CallId::from_index(index).unwrap();
        let prepare = plan.call_prepare(root, call);
        let invoke = plan.call_invocation(root, call);
        let Place::Index { receiver,key } = data.places[place.index()] else { panic!("indexed callable") };
        let before = at(&plan, root, Site::Operation(prepare));
        assert_eq!(before.iter().map(|input| (input.value,input.role)).collect::<Vec<_>>(), vec![(receiver,Role::PlaceReceiver(place)),(key,Role::PlaceKey(place))]);
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap().iter().map(|argument| { let CallArgument::Value(value)=*argument else { panic!("value argument fixture") }; value }).collect::<Vec<_>>();
        let after = at(&plan, root, Site::Operation(invoke));
        assert_eq!(after.iter().map(|input| input.value).collect::<Vec<_>>(), arguments);
        assert_eq!(after.iter().map(|input| input.role).collect::<Vec<_>>(), vec![Role::Operand(0),Role::Operand(1)]);
        for value in arguments {
            let mut enclosing = plan.call_envelope(root,data.values[value.index()].definition);
            while enclosing != Some(prepare) {
                enclosing = plan.call_envelope(root,enclosing.expect("argument stays inside the outer prepared call"));
            }
        }
        let inventory = all(&plan,root);
        for input in before.iter().chain(&after) { assert_eq!(inventory.iter().filter(|found| *found == input).count(),1); }
        plan.discard(None).unwrap();
    });
}

#[test]
fn stores_use_receiver_then_key_then_rhs_and_lazy_regions_keep_their_sites() {
    checked("extern int[] array();extern int key();extern int rhs();array()[key()]=rhs();extern bool condition();extern int a();extern int b();int chosen=if(condition()){a()}else{b()};print(chosen);", |program| {
        let plan = DemandPlan::build(program, None, None, &contract(false), DemandMode::Prune, None).unwrap();
        let root = plan.root();
        let data = program.unit(plan.context(root).unit).unwrap();
        let (index,place) = data.operations.iter().enumerate().find_map(|(index,op)| match op.kind { OperationKind::Store(place) if matches!(data.places[place.index()],Place::Index{..})=>Some((index,place)),_=>None }).unwrap();
        let operation=OpId::from_index(index).unwrap();
        let inputs=at(&plan,root,Site::Operation(operation));
        assert_eq!(inputs.iter().map(|input|input.role).collect::<Vec<_>>(), vec![Role::PlaceReceiver(place),Role::PlaceKey(place),Role::Operand(0)]);
        let select = data.operations.iter().find_map(|op| match op.kind { OperationKind::Select{yes,no}=>Some((yes,no)),_=>None }).unwrap();
        for region in [select.0,select.1] {
            let inputs=at(&plan,root,Site::RegionResult(region));
            assert_eq!(inputs.len(),1);
            assert_eq!(inputs[0].value,data.regions[region.index()].result.unwrap());
            assert_eq!(inputs[0].role,Role::RegionResult);
        }
        plan.discard(None).unwrap();
    });
}

#[test]
fn inlined_helper_arguments_follow_storage_demand_and_tail_return_is_a_use() {
    checked(
        include_str!("fixtures/helper/ordered-repeated-unused.lil"),
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let family = helper(
                program,
                &uses,
                cell(program, "mix"),
                &mut cache,
                &mut ledger,
            );
            let body = family.root().body;
            let tail = family.tail_return().expect("value-returning fixture");
            let map = ImplementationMap::direct()
                .with_inline_helper(family, &mut ledger, WorkDomain::Optional)
                .unwrap();
            for mode in [DemandMode::Prune, DemandMode::Preserve] {
                let plan = DemandPlan::build(
                    program,
                    Some(&uses),
                    Some(&map),
                    &contract(false),
                    mode,
                    Some((&mut ledger, WorkDomain::Optional)),
                )
                .unwrap();
                let root = plan.root();
                let caller = program.unit(plan.context(root).unit).unwrap();
                let (call, context) = caller
                    .operations
                    .iter()
                    .enumerate()
                    .find_map(|(index, _)| {
                        let call = OpId::from_index(index).unwrap();
                        plan.child(root, call)
                            .filter(|&child| {
                                plan.context(child).unit == body
                                    && plan.context(child).kind.is_inline()
                            })
                            .map(|child| (call, child))
                    })
                    .unwrap();
                let arguments = at(&plan, root, Site::Operation(call));
                let positions = arguments
                    .iter()
                    .map(|input| match input.role {
                        Role::InlineParameter {
                            context: owner,
                            position,
                        } => {
                            assert_eq!(owner, context);
                            position
                        }
                        other => panic!("unexpected helper input {other:?}"),
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    positions,
                    if mode == DemandMode::Prune {
                        vec![0, 2]
                    } else {
                        vec![0, 1, 2]
                    }
                );
                let returned = at(&plan, context, Site::Operation(tail));
                assert_eq!(returned.len(), 1);
                assert_eq!(returned[0].role, Role::Return);
                assert!(all(&plan, context).contains(&returned[0]));
                // Discarding the unused parameter never discards argument(2)'s invocation.
                let caller = program.unit(plan.context(root).unit).unwrap();
                let OperationKind::Call(target) = caller.operations[call.index()].kind else {
                    panic!("helper call")
                };
                let CallArgument::Value(actual) = caller
                    .arguments(caller.calls[target.index()].arguments)
                    .unwrap()[1]
                else {
                    panic!("value argument")
                };
                assert!(plan.needs_execution(root, caller.values[actual.index()].definition));
                plan.discard(Some(&mut ledger)).unwrap();
            }
            map.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn scalar_initialization_uses_only_retained_slots_and_writes_only_rhs() {
    checked("Record<int> state=record{live:1+2,dead:3+4};state.live=(state.live??0)+5;print(state.live);", |program| {
        let mut ledger=ledger();
        let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let analyzed=record_family::analyze(program,&uses,cell(program,"state"),record_family::FamilyRequest {attempt:AnalysisAttempt{plan:record_family::RECORD_FAMILY_PLAN,algorithm_version:record_family::RECORD_FAMILY_VERSION,work_quota:100_000},scratch_bytes:100_000,output_bytes:100_000},&mut ledger,WorkDomain::Optional).unwrap();
        let family=match analyzed.outcome {record_family::FamilyOutcome::Complete(family)=>family,other=>panic!("record proof {other:?}")};
        let initialize=family.initialize().operation;
        let expected=family.slots().iter().enumerate().find(|(_,slot)|program.strings[slot.key.index()].as_unicode()==Some("live")).map(|(slot,entry)|(slot as u32,entry.initial_value.unwrap())).unwrap();
        let map=ImplementationMap::direct().with_scalar(family,&mut ledger,WorkDomain::Optional).unwrap();
        let plan=DemandPlan::build(program,Some(&uses),Some(&map),&contract(false),DemandMode::Prune,Some((&mut ledger,WorkDomain::Optional))).unwrap();
        let inputs=at(&plan,plan.root(),Site::Operation(initialize));
        assert_eq!(inputs.len(),1);assert_eq!(inputs[0].value,expected.1);assert_eq!(inputs[0].role,Role::RecordSlot{record:0,slot:expected.0});
        let data=program.unit(plan.context(plan.root()).unit).unwrap();
        for index in 0..data.operations.len() {let operation=OpId::from_index(index).unwrap();if matches!(plan.record_operation(plan.context(plan.root()).unit,operation),Some(super::demand::RecordOperation::Write{..})) {let inputs=at(&plan,plan.root(),Site::Operation(operation));assert_eq!(inputs.len(),1);assert_eq!(inputs[0].role,Role::Operand(0));}}
        plan.discard(Some(&mut ledger)).unwrap();map.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn selected_string_production_cuts_inputs_but_preserve_execution_counts_them_once() {
    checked("print((\"left\"+\"middle\")+\"right\");", |program| {
        let mut ledger = ledger();
        let mut cache = cache(&mut ledger);
        let unit = program.initialization[0];
        let data = program.unit(unit).unwrap();
        let operation = OpId::from_index(
            data.operations
                .iter()
                .rposition(|op| matches!(op.kind, OperationKind::Binary(BinaryOp::Add)))
                .unwrap(),
        )
        .unwrap();
        let definition = string_family::ValueRef {
            unit,
            value: data.operations[operation.index()].result.unwrap(),
        };
        let prepared = string_family::prepare(
            program,
            &[definition],
            string_family::StringChoice::LiteralAtDefinition,
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
            &mut ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        let mut ready = match prepared.outcome {
            string_family::PreparationOutcome::Ready(ready) => ready,
            other => panic!("string preparation {other:?}"),
        };
        {
            let mut session = cache.session(&mut ledger, WorkDomain::Optional, 1).unwrap();
            let view = session.query(program, unit, facts_request()).unwrap();
            ready
                .check_facts(program, view.facts, view.receipt)
                .unwrap();
        }
        let family = match ready.finish(&mut ledger).unwrap().outcome {
            string_family::FamilyOutcome::Complete(family) => family,
            other => panic!("string proof {other:?}"),
        };
        let map = ImplementationMap::direct()
            .with_string(family, &mut ledger, WorkDomain::Optional)
            .unwrap();
        for mode in [DemandMode::Prune, DemandMode::Preserve] {
            let plan = DemandPlan::build(
                program,
                None,
                Some(&map),
                &contract(false),
                mode,
                Some((&mut ledger, WorkDomain::Optional)),
            )
            .unwrap();
            let inputs = at(&plan, plan.root(), Site::Operation(operation));
            assert_eq!(inputs.len(), if mode == DemandMode::Prune { 0 } else { 2 });
            if mode == DemandMode::Preserve {
                assert_eq!(
                    inputs.iter().map(|input| input.value).collect::<Vec<_>>(),
                    data.operands(data.operations[operation.index()].operands)
                        .unwrap()
                );
            }
            plan.discard(Some(&mut ledger)).unwrap();
        }
        map.discard(&mut ledger).unwrap();
        cache.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn occurrence_walk_admits_work_before_visiting_and_never_allocates_or_mutates_demand() {
    checked("print(1+2);", |program| {
        let mut ledger = ledger();
        let plan = DemandPlan::build(
            program,
            None,
            None,
            &contract(false),
            DemandMode::Prune,
            Some((&mut ledger, WorkDomain::Baseline)),
        )
        .unwrap();
        let before = ledger.retained_bytes();
        let expected = all(&plan, plan.root());
        let mut observed = Vec::new();
        plan.visit_effective_value_uses(
            plan.root(),
            |work| ledger.charge(WorkDomain::Optional, WorkKind::Analysis, work as u64),
            |input| {
                observed.push(input);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(observed, expected);
        assert_eq!(ledger.retained_bytes(), before);
        let used = ledger.work_used(WorkDomain::Optional);
        ledger
            .charge(WorkDomain::Optional, WorkKind::Analysis, WORK - used)
            .unwrap();
        let mut visits = 0;
        assert_eq!(
            plan.visit_effective_value_uses(
                plan.root(),
                |work| ledger.charge(WorkDomain::Optional, WorkKind::Analysis, work as u64),
                |_| {
                    visits += 1;
                    Ok(())
                }
            ),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        );
        assert_eq!(visits, 0);
        assert_eq!(all(&plan, plan.root()), expected);
        assert_eq!(ledger.retained_bytes(), before);
        plan.discard(Some(&mut ledger)).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
