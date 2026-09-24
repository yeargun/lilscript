//! Outside-repository helper certificate/ownership tests; target execution is a
//! separate milestone gate. These use the actual preparation/facts/finish path.
use super::*;
use crate::compilation_policy::{BudgetPlan, ResourceLimits};
use crate::program::facts::{
    CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};

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
            baseline_work: 100_000_000,
            optional_work: 100_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 20_000_000,
        },
    )
    .unwrap()
}
fn request() -> FamilyRequest {
    FamilyRequest {
        execution: JavaScriptExecution::Module,
        attempt: AnalysisAttempt {
            plan: HELPER_FAMILY_PLAN,
            algorithm_version: HELPER_FAMILY_VERSION,
            work_quota: 600_000,
        },
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
    }
}
fn root(program: &Program<'_>, name: &str) -> CellId {
    program
        .cells
        .iter()
        .position(|cell| cell.name == name)
        .map(|n| CellId::from_index(n).unwrap())
        .unwrap()
}
fn finish(
    program: &Program<'_>,
    mut ready: PreparedHelper,
    ledger: &mut BudgetLedger,
) -> FamilyOutcome {
    let mut cache = RetainedFactsCache::new(
        CacheLimits {
            entries: 2,
            bytes: 200_000,
            result_bytes: 100_000,
        },
        ledger,
        WorkDomain::Baseline,
    )
    .unwrap();
    {
        let mut session = cache.session(ledger, WorkDomain::Baseline, 1).unwrap();
        let facts = session
            .query(
                program,
                ready.root().body,
                FactRequest {
                    attempt: AnalysisAttempt {
                        plan: LOCAL_FACTS_PLAN,
                        algorithm_version: LOCAL_FACTS_VERSION,
                        work_quota: 200_000,
                    },
                    result_bytes: 100_000,
                },
            )
            .unwrap();
        ready
            .check_body(program, facts.facts, facts.receipt)
            .unwrap();
    }
    let outcome = ready.finish(ledger).unwrap();
    cache.discard(ledger).unwrap();
    outcome
}
fn complete(
    program: &Program<'_>,
    uses: &UseIndex,
    name: &str,
    ledger: &mut BudgetLedger,
) -> HelperFamily {
    let prepared = prepare(
        program,
        uses,
        root(program, name),
        request(),
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    assert!(prepared.prerequisites.attempts >= 1);
    let PreparationOutcome::Ready(ready) = prepared.outcome else {
        panic!("ready reference helper: {:?}", prepared.outcome)
    };
    match finish(program, ready, ledger) {
        FamilyOutcome::Complete(family) => family,
        other => panic!("complete reference helper: {other:?}"),
    }
}

#[test]
fn mixed_reference_integer_loads_keep_unknown_domains_and_conversion_effects() {
    checked("struct P{int x;}extern int opaque();int helper(ref int slot){slot;slot+=1;return slot;}int plain=opaque();P state=P{opaque()};print(helper(ref plain));print(helper(ref state.x));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();let family=complete(program,&uses,"helper",&mut ledger);let data=program.unit(family.root().body).unwrap();let parameter=data.parameters[0];let mut loads=0;let mut writes=0;
        for(index,operation)in data.operations.iter().enumerate(){let id=OpId::from_index(index).unwrap();if let Some(access)=family.reference_access(id){assert_eq!(access.parameter(),parameter);let facts=family.operation_facts(id).unwrap();match operation.kind{
            OperationKind::Load(place)=>{loads+=1;assert_eq!(place,access.place());assert!(!facts.primitive_result,"whole-cell actual may remain an opaque raw value");assert!(facts.behavior.may_throw&&facts.behavior.may_reenter&&facts.behavior.may_diverge);assert_eq!(facts.behavior.reads,MemoryAccess::Unknown);},
            OperationKind::Store(_)=>{writes+=1;assert_eq!(facts.behavior.writes,MemoryAccess::Unknown);},
            OperationKind::CheckPlace(_)=>assert_eq!(facts.behavior.reads,MemoryAccess::Unknown),
            _=>panic!("witness on another operation"),
        }}}
        assert!(loads>=3&&writes>=1);assert!(family.frame_elision_allowed(JavaScriptExecution::Module));assert!(!family.frame_elision_allowed(JavaScriptExecution::Script));
        family.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),baseline);uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn whole_reference_replacement_uses_common_struct_construction_and_no_formal_bank() {
    checked("struct P{int x;int y;}int update(ref P whole,ref int leaf){whole=P{4,5};leaf+=2;return whole.x+whole.y;}P state=P{1,2};print(update(ref state,ref state.x));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let family=complete(program,&uses,"update",&mut ledger);let data=program.unit(family.root().body).unwrap();let mut construction=0;let mut whole_write=0;
        for(index,operation)in data.operations.iter().enumerate(){let id=OpId::from_index(index).unwrap();let facts=family.operation_facts(id).unwrap();match operation.kind{
            OperationKind::Allocate{kind:AllocationKind::Struct(_),..}=>{construction+=1;assert!(facts.behavior.may_exhaust_resources);assert!(!facts.behavior.creates_identity&&!facts.behavior.may_reenter);assert!(family.reference_access(id).is_none());},
            OperationKind::Store(place) if data.places[place.index()]==Place::Cell(data.parameters[0])=>{whole_write+=1;assert_eq!(family.reference_access(id).unwrap().parameter(),data.parameters[0]);assert_eq!(facts.behavior.writes,MemoryAccess::Unknown);},_=>{}
        }}
        assert_eq!(construction,1);assert_eq!(whole_write,1);family.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn external_reference_writer_changes_invalidate_the_retained_helper_certificate() {
    checked("struct P{int x;}int read(ref P value){return value.x;}void overwrite(ref P value){value=P{8};}P state=P{1};print(read(ref state));overwrite(ref state);",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let family=complete(program,&uses,"read",&mut ledger);let CellBinding::Function(writer)=program.cells[root(program,"overwrite").index()].binding else{panic!("writer")};assert!(family.dependencies().units().iter().any(|(unit,_)|*unit==writer));
        let mut changed=program.clone();let mut working=changed.units[writer.index()].clone().into_working();working.get_mut().operations.iter_mut().find(|operation|matches!(operation.kind,OperationKind::Constant(Constant::Integer(8)))).unwrap().kind=OperationKind::Constant(Constant::Integer(9));changed.units[writer.index()]=working.freeze();changed.verify().unwrap();let updated=uses.updated(&changed,&[writer],&mut ledger,WorkDomain::Optional).unwrap();
        assert_eq!(program.units[family.root().body.index()].revision(),changed.units[family.root().body.index()].revision());assert!(!family.dependencies().valid_for(&changed,&updated));
        updated.discard(&mut ledger).unwrap();family.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn reference_proof_unknown_preserves_leaf_helper_completion_and_escape_restrictions() {
    for(source,name,expected)in [
        ("struct P{int x;}extern P opaque();int read(ref P value){return value.x;}P state=P{1};P bad=opaque();print(read(ref state));print(read(ref bad));","read",UnknownReason::RequiredProductEvidence),
        ("struct P{int x;}int change(ref P value){value.x=1;throw 2;}P state=P{1};print(change(ref state));","change",UnknownReason::CompletionShape),
        ("struct P{int x;}int recur(ref P value){return recur(ref value);}P state=P{1};print(recur(ref state));","recur",UnknownReason::Recursion),
    ]{checked(source,|program|{let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();let prepared=prepare(program,&uses,root(program,name),request(),&mut ledger,WorkDomain::Optional).unwrap();assert!(matches!(prepared.outcome,PreparationOutcome::Unknown(reason) if reason==expected));assert_eq!(ledger.retained_bytes(),baseline);uses.discard(&mut ledger).unwrap();});}
}

#[test]
fn late_reference_prerequisite_work_refusal_restores_all_helper_storage() {
    checked("struct P{int x;}int read(ref P value){return value.x;}P state=P{1};print(read(ref state));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();let measured=prepare(program,&uses,root(program,"read"),request(),&mut ledger,WorkDomain::Optional).unwrap();let work=measured.receipt.logical_work+measured.prerequisites.logical_work;assert_eq!(measured.prerequisites.attempts,1);let PreparationOutcome::Ready(ready)=measured.outcome else{panic!("measured ready")};ready.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),baseline);
        let mut bounded=request();bounded.attempt.work_quota=work-1;let denied=prepare(program,&uses,root(program,"read"),bounded,&mut ledger,WorkDomain::Optional).unwrap();assert!(matches!(denied.outcome,PreparationOutcome::Truncated(ResourceLimit::Work)));assert_eq!(denied.prerequisites.attempts,1,"reference prerequisite completed before later refusal");assert_eq!(denied.prerequisites.logical_work,measured.prerequisites.logical_work);assert_eq!(ledger.retained_bytes(),baseline);
        uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn transferred_reference_dependency_output_is_released_on_merge_refusal_and_unwind() {
    checked("struct P{int x;}int read(ref P value){return value.x;}P state=P{1};print(read(ref state));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();
        for panic in [false,true]{let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
            let mut parent=Attempt{ledger:&mut ledger,domain:WorkDomain::Optional,request:request(),work:0,scratch:0,output:0,reservation:0,prerequisites:PrerequisiteWork::default()};
            let InputOutcome::Complete(inputs)=CallableInputs::for_cell_published(program,&uses,root(program,"read"),CallObservations::from_execution(JavaScriptExecution::Module),&mut parent).unwrap_or_else(|_|panic!("inputs")) else{panic!("private inputs")};
            let child_request=product_family::FamilyRequest{execution:JavaScriptExecution::Module,attempt:AnalysisAttempt{plan:product_family::PRODUCT_FAMILY_PLAN,algorithm_version:product_family::PRODUCT_FAMILY_VERSION,work_quota:200_000},scratch_bytes:500_000,output_bytes:500_000};let mut inherited=0;
            let analysis=product_family::run(child_request,product_family::PRODUCT_FAMILY_PLAN,product_family::PRODUCT_FAMILY_VERSION,parent.ledger,parent.domain,|child|product_family::certify_reference_parameters(program,&uses,&inputs,child,|_,_|Ok(())),|_,(_,bytes)|inherited=bytes).unwrap();
            let product_family::Outcome::Complete(dependencies)=analysis.outcome else{panic!("reference dependencies")};assert!(inherited>0&&!dependencies.cells().is_empty());let prior=parent.reservation;let scratch=parent.scratch;
            let result=parent.with_prerequisite_scratch(dependencies,inherited,|dependencies,scope|->ResultIn<()>{assert_eq!(scope.reservation,prior+inherited);assert_eq!(scope.scratch,scratch+inherited);assert!(!dependencies.creators().is_empty());if panic{panic!("after inherited charge transfer")};scope.request.output_bytes=scope.output;let _=scope.vector::<u64>(1,true)?;Ok(())});
            assert!(matches!(result,Err(Stop::Truncated(ResourceLimit::Output))));assert_eq!(parent.reservation,prior);assert_eq!(parent.scratch,scratch);inputs.discard(&mut parent).unwrap_or_else(|_|panic!("release inputs"));
        }));assert_eq!(outcome.is_err(),panic);assert_eq!(ledger.retained_bytes(),baseline);}
        uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}
