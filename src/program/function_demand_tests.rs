//! Transport occurrences share the semantic argument owner and placement walk.
use super::demand::{DemandMode, DemandPlan, EffectiveUseRole};
use super::function_layout::{self, FamilyOutcome, FunctionLayout};
use super::implementations::ImplementationMap;
use super::uses::UseIndex;
use super::value_placement::{self, PlacementDepth, ValueStorage};
use super::*;
use crate::compilation_contract::{JavaScriptCompilationContract, JavaScriptExecution};
use crate::compilation_policy::{
    AnalysisAttempt, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::output_budget::AllocationBudget;

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program);
}
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 20_000_000,
            optional_work: 20_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 20_000_000,
        },
    )
    .unwrap()
}
fn contract() -> JavaScriptCompilationContract {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    *config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .javascript_contract()
        .unwrap()
}
fn body(program: &Program<'_>) -> UnitId {
    program
        .cells()
        .iter()
        .find_map(|cell| match cell.binding {
            CellBinding::Function(body) if cell.name == "step" => Some(body),
            _ => None,
        })
        .unwrap()
}
fn layout(program: &Program<'_>, uses: &UseIndex, ledger: &mut BudgetLedger) -> FunctionLayout {
    let result = function_layout::analyze(
        program,
        uses,
        body(program),
        function_layout::FamilyRequest {
            execution: JavaScriptExecution::Module,
            attempt: AnalysisAttempt {
                plan: function_layout::FUNCTION_LAYOUT_PLAN,
                algorithm_version: function_layout::FUNCTION_LAYOUT_VERSION,
                work_quota: 500_000,
            },
            scratch_bytes: 800_000,
            output_bytes: 800_000,
        },
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    let FamilyOutcome::Complete(layout) = result.outcome else {
        panic!("{result:?}")
    };
    layout
}
fn storage(
    program: &Program<'_>,
    demand: &DemandPlan<'_, '_>,
    ledger: &mut BudgetLedger,
) -> Vec<ValueStorage> {
    let context = demand.root();
    let data = program.unit(demand.context(context).unit).unwrap();
    let mut storage = data
        .values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if demand.needs_value(context, ValueId::from_index(index).unwrap())
                && !matches!(
                    data.operations[value.definition.index()].kind,
                    OperationKind::Constant(_)
                )
            {
                ValueStorage::Candidate
            } else {
                ValueStorage::Absent
            }
        })
        .collect::<Vec<_>>();
    let mut budget = AllocationBudget::new(Some((ledger, WorkDomain::Optional)));
    value_placement::plan(
        data,
        demand,
        context,
        &mut storage,
        PlacementDepth {
            enclosing: 1,
            limit: crate::js::MAX_NESTING,
        },
        &mut budget,
    )
    .unwrap();
    storage
}

#[test]
fn shared_field_transport_records_each_raw_projection_and_captures_the_packed_actual() {
    checked("struct P{int x;int y;}int step(P parameter){return parameter.x;}P source=P{1,2};print(step(source));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let layout=layout(program,&uses,&mut ledger);let call=layout.inputs().calls()[0];
        let data=program.unit(call.caller).unwrap();
        let CallArgument::Value(actual)=data.arguments(data.calls[call.target.index()].arguments).unwrap()[0] else {panic!("value")};
        let map=ImplementationMap::direct().with_function(layout,&mut ledger,WorkDomain::Optional).unwrap();let before=ledger.retained_bytes();
        let demand=DemandPlan::build(program,Some(&uses),Some(&map),&contract(),DemandMode::Prune,Some((&mut ledger,WorkDomain::Optional))).unwrap();
        let root=demand.root();assert_eq!(demand.context(root).unit,call.caller);
        let mut slots=Vec::new();
        demand.visit_effective_value_uses(root,|_|Ok::<_,()>(()),|input|{if input.value==actual {if let EffectiveUseRole::ProductArgument{call:found,position,slot}=input.role {assert_eq!(found,call.target);assert_eq!(position,0);slots.push(slot);}}Ok(())}).unwrap();
        assert_eq!(slots,[0,1],"a shared fixed-width ABI keeps even the body-unused field; no synthetic single use");
        let storage=storage(program,&demand,&mut ledger);
        assert!(matches!(storage[actual.index()],ValueStorage::Required),"two raw projections must read one original frozen actual");
        drop(storage);demand.discard(Some(&mut ledger)).unwrap();assert_eq!(ledger.retained_bytes(),before);
        map.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn zero_width_transport_captures_the_original_callee_and_retains_argument_control_execution() {
    checked("struct Empty{}extern bool mark();extern void note();void step(Empty value){note();}step(if(mark()){Empty{}}else{Empty{}});",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let layout=layout(program,&uses,&mut ledger);let call=layout.inputs().calls()[0];let data=program.unit(call.caller).unwrap();
        let CallTarget::Value{callee,..}=data.calls[call.target.index()].target else {panic!("value call")};
        let mark=program.cells().iter().position(|cell|cell.name=="mark").map(|index|CellId::from_index(index).unwrap()).unwrap();
        let mark_call=data.calls.iter().enumerate().find_map(|(index,site)|{
            let CallTarget::Value{callee,..}=site.target else {return None};
            let OperationKind::Load(place)=data.operations[data.values[callee.index()].definition.index()].kind else {return None};
            (data.places[place.index()]==Place::Cell(mark)).then(||CallId::from_index(index).unwrap())
        }).unwrap();
        let mark_operation=data.operations.iter().position(|operation|matches!(operation.kind,OperationKind::Call(target) if target==mark_call)).map(|index|OpId::from_index(index).unwrap()).unwrap();
        let map=ImplementationMap::direct().with_function(layout,&mut ledger,WorkDomain::Optional).unwrap();let before=ledger.retained_bytes();
        let demand=DemandPlan::build(program,Some(&uses),Some(&map),&contract(),DemandMode::Prune,Some((&mut ledger,WorkDomain::Optional))).unwrap();let root=demand.root();
        assert!(demand.call_has_empty_transport(root,call.target));
        let mut captures=0;let mut product_args=0;
        demand.visit_effective_value_uses(root,|_|Ok::<_,()>(()),|input|{match input.role {
            EffectiveUseRole::CapturedCallCallee(target) if target==call.target=>{assert_eq!(input.value,callee);captures+=1;},
            EffectiveUseRole::ProductArgument{call:target,..} if target==call.target=>product_args+=1,_=>{},
        }Ok(())}).unwrap();
        assert_eq!(captures,1);assert_eq!(product_args,0);assert!(demand.needs_execution(root,mark_operation));
        let storage=storage(program,&demand,&mut ledger);assert!(matches!(storage[callee.index()],ValueStorage::Required));
        drop(storage);demand.discard(Some(&mut ledger)).unwrap();assert_eq!(ledger.retained_bytes(),before);
        map.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn private_input_qualification_cannot_be_reused_for_script_execution() {
    checked("struct P{int x;}int step(P parameter){return parameter.x;}P source=P{1};print(step(source));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        for transport in [false,true] {
            let map=if transport {
                ImplementationMap::direct().with_function(layout(program,&uses,&mut ledger),&mut ledger,WorkDomain::Optional).unwrap()
            }else{
                let result=super::product_family::analyze(program,&uses,program.unit(body(program)).unwrap().parameters[0],super::product_family::FamilyRequest{
                    execution:JavaScriptExecution::Module,
                    attempt:AnalysisAttempt{plan:super::product_family::PRODUCT_FAMILY_PLAN,algorithm_version:super::product_family::PRODUCT_FAMILY_VERSION,work_quota:500_000},
                    scratch_bytes:800_000,output_bytes:800_000,
                },&mut ledger,WorkDomain::Optional).unwrap();
                let super::product_family::FamilyOutcome::Complete(family)=result.outcome else{panic!("{result:?}")};
                assert!(family.requires_module());
                ImplementationMap::direct().with_product(family,&mut ledger,WorkDomain::Optional).unwrap()
            };
            let before=ledger.retained_bytes();let mut script=contract();script.execution=JavaScriptExecution::Script;
            let error=match DemandPlan::build(program,Some(&uses),Some(&map),&script,DemandMode::Prune,Some((&mut ledger,WorkDomain::Optional))) {
                Ok(plan)=>{plan.discard(Some(&mut ledger)).unwrap();panic!("unsealed Script")},Err(error)=>error,
            };
            assert!(matches!(error,super::demand::DemandError::Unsupported(_)));
            assert_eq!(ledger.retained_bytes(),before);map.discard(&mut ledger).unwrap();
        }
        uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}
