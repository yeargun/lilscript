//! Outside-repository proof tests. Target owner separately executes artifacts.
use super::*;
use crate::compilation_policy::{BudgetPlan, ResourceLimits};

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
fn request() -> FamilyRequest {
    FamilyRequest {
        execution: JavaScriptExecution::Module,
        attempt: AnalysisAttempt {
            plan: PRODUCT_FAMILY_PLAN,
            algorithm_version: PRODUCT_FAMILY_VERSION,
            work_quota: 400_000,
        },
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
    }
}
fn cell(program: &Program<'_>, name: &str) -> CellId {
    let mut matches = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.name == name);
    let found = CellId::from_index(matches.next().unwrap().0).unwrap();
    assert!(matches.next().is_none(), "unique {name}");
    found
}
fn creation(program: &Program<'_>, name: &str) -> (UnitId, OpId, UnitId) {
    let id = cell(program, name);
    let owner = program.cells[id.index()].owner;
    let data = program.unit(owner).unwrap();
    let initialize = data
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::Initialize(c) if c == id))
        .unwrap();
    let value = data.operands(initialize.operands).unwrap()[0];
    let operation = data.values[value.index()].definition;
    let OperationKind::Closure(body) = data.operations[operation.index()].kind else {
        panic!("closure")
    };
    (owner, operation, body)
}
fn complete(
    program: &Program<'_>,
    uses: &UseIndex,
    root: CellId,
    ledger: &mut BudgetLedger,
) -> ProductFamily {
    let analysis = analyze(program, uses, root, request(), ledger, WorkDomain::Optional).unwrap();
    match analysis.outcome {
        Outcome::Complete(family) => family,
        other => panic!("complete: {other:?}"),
    }
}
fn inspect_references(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    mut inspect: impl FnMut(OpRef, ReferenceAccess),
) -> Result<Analysis<()>, FamilyError> {
    // Test-only scoped client of the common Attempt: no production holder or
    // public constructor is introduced. Actual payload drops before release.
    let mut detached = 0;
    let analysis = run(
        request,
        PRODUCT_FAMILY_PLAN,
        PRODUCT_FAMILY_VERSION,
        ledger,
        WorkDomain::Optional,
        |budget| {
            let InputOutcome::Complete(inputs) = CallableInputs::for_body_published(
                program,
                uses,
                body,
                CallObservations::from_execution(request.execution),
                budget,
            )?
            else {
                return Err(unknown(UnknownReason::CallableInterface));
            };
            let dependencies = certify_reference_parameters(
                program,
                uses,
                &inputs,
                budget,
                |operation, access| {
                    inspect(operation, access);
                    Ok(())
                },
            )?;
            drop(dependencies);
            inputs.discard(budget)?;
            Ok(())
        },
        |_, (_, bytes)| detached = bytes,
    );
    ledger.release(WorkDomain::Optional, detached).unwrap();
    analysis
}

#[test]
fn overlapping_reference_writers_close_presence_without_joining_banks() {
    checked("struct Inner{int x;int y;}struct P{Inner inner;int z;}void leaf(ref int target,int delta){target+=delta;}void update(ref P whole,ref Inner part,ref int same){whole=P{Inner{40,50},60};part.y=70;leaf(ref same,2);}P state=P{Inner{1,2},3};P saved=state;P other=P{Inner{4,5},6};update(ref state,ref state.inner,ref state.inner.x);update(ref other,ref other.inner,ref other.inner.x);print(saved.inner.x);", |program| {
        let mut ledger = ledger(); let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let baseline = ledger.retained_bytes();
        let family = complete(program, &uses, cell(program,"state"), &mut ledger);
        let mut expected = vec![cell(program,"state"),cell(program,"saved")]; expected.sort_unstable();
        assert_eq!(family.cells().iter().map(|row|row.cell).collect::<Vec<_>>(), expected);
        assert!(!family.cells().iter().any(|row| program.is_reference_parameter(row.cell)));
        assert!(family.requires_module());
        assert!(family.dependencies().cells().iter().any(|(id,_)| *id == cell(program,"other")));
        let (_,_,body) = creation(program,"update");
        assert!(family.operations().iter().all(|entry| entry.operation.unit != body), "reference operations are not per-bank recipes");
        let mut visits=0;
        let result=inspect_references(program,&uses,body,request(),&mut ledger,|operation,access| {
            visits+=1;assert_eq!(operation.unit,body);assert!(program.is_reference_parameter(access.parameter()));
            let kind=&program.unit(body).unwrap().operations[operation.operation.index()].kind;
            assert!(matches!(kind,OperationKind::Load(place)|OperationKind::Store(place)|OperationKind::CheckPlace(place) if *place==access.place()));
        }).unwrap();
        assert!(matches!(result.outcome,Outcome::Complete(())));assert!(visits>=3);
        family.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),baseline);uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn opaque_reference_writers_and_actuals_poison_the_whole_presence_closure() {
    for source in [
        "struct P{int x;}extern P opaque();void change(ref P target){target=opaque();}P state=P{1};change(ref state);print(state.x);",
        "struct P{int x;}extern P opaque();void change(ref P target){target.x=2;}P state=P{1};P bad=opaque();change(ref state);change(ref bad);print(state.x);",
    ] { checked(source, |program| {
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();
        let result=analyze(program,&uses,cell(program,"state"),request(),&mut ledger,WorkDomain::Optional).unwrap();
        assert!(matches!(result.outcome,Outcome::Unknown(UnknownReason::UnsupportedProducer)));
        let (_,_,body)=creation(program,"change");let mut called=false;
        let result=inspect_references(program,&uses,body,request(),&mut ledger,|_,_|called=true).unwrap();
        assert!(matches!(result.outcome,Outcome::Unknown(UnknownReason::UnsupportedProducer)));assert!(!called);
        assert_eq!(ledger.retained_bytes(),baseline);uses.discard(&mut ledger).unwrap();
    }); }
}

#[test]
fn recursive_reference_presence_is_conditional_and_one_opaque_entry_rejects_it() {
    for (source,good) in [
        ("struct P{int x;}void cycle(ref P value){cycle(ref value);}",true),
        ("struct P{int x;}void cycle(ref P value){cycle(ref value);}P state=P{1};cycle(ref state);",true),
        ("struct P{int x;}extern P opaque();void cycle(ref P value){cycle(ref value);}P state=opaque();cycle(ref state);",false),
    ] {checked(source,|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();
        let (_,_,body)=creation(program,"cycle");let result=inspect_references(program,&uses,body,request(),&mut ledger,|_,_|{}).unwrap();
        assert_eq!(matches!(result.outcome,Outcome::Complete(())),good);
        assert_eq!(ledger.retained_bytes(),baseline);uses.discard(&mut ledger).unwrap();
    });}
}

#[test]
fn new_reference_creator_invalidates_old_evidence_and_joins_opaque_actuals() {
    checked("struct P{int x;}extern P opaque();auto change=(ref P value)=>{value.x=7;return value.x;};P state=P{1};print(change(ref state));auto maker=()=>{P bad=opaque();auto other=(ref P value)=>value.x;return other(ref bad);};print(maker());",|program|{
        let (_,_,body)=creation(program,"change");let (owner,operation,_)=creation(program,"other");
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let family=complete(program,&uses,cell(program,"state"),&mut ledger);
        assert!(family.dependencies().creators().iter().any(|(id,_)|*id==body));
        let mut changed=program.clone();let mut working=changed.units[owner.index()].clone().into_working();working.get_mut().operations[operation.index()].kind=OperationKind::Closure(body);changed.units[owner.index()]=working.freeze();changed.verify().unwrap();
        assert_eq!(program.units[body.index()].revision(),changed.units[body.index()].revision());
        let updated=uses.updated(&changed,&[owner],&mut ledger,WorkDomain::Optional).unwrap();assert!(!family.dependencies().valid_for(&changed,&updated));
        let result=analyze(&changed,&updated,cell(&changed,"state"),request(),&mut ledger,WorkDomain::Optional).unwrap();assert!(matches!(result.outcome,Outcome::Unknown(UnknownReason::UnsupportedProducer)));
        updated.discard(&mut ledger).unwrap();family.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn reference_query_refusal_and_callback_unwind_release_the_existing_attempt() {
    checked(
        "struct P{int x;}void change(ref P value){value.x+=1;}P state=P{1};change(ref state);",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let baseline = ledger.retained_bytes();
            let (_, _, body) = creation(program, "change");
            let measured =
                inspect_references(program, &uses, body, request(), &mut ledger, |_, _| {})
                    .unwrap();
            assert!(matches!(measured.outcome, Outcome::Complete(())));
            assert!(measured.receipt.logical_work > 100);
            for memory in [false, true] {
                let mut bounded = request();
                if memory {
                    bounded.scratch_bytes = 256;
                } else {
                    bounded.attempt.work_quota = measured.receipt.logical_work / 2;
                }
                let mut called = false;
                let result =
                    inspect_references(program, &uses, body, bounded, &mut ledger, |_, _| {
                        called = true
                    })
                    .unwrap();
                assert!(matches!(result.outcome, Outcome::Truncated(_)));
                assert!(result.receipt.logical_work > 0);
                assert!(!called);
                assert_eq!(ledger.retained_bytes(), baseline);
            }
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = inspect_references(program, &uses, body, request(), &mut ledger, |_, _| {
                    panic!("reference inspection")
                });
            }));
            assert!(panic.is_err());
            assert_eq!(ledger.retained_bytes(), baseline);
            let mut script = request();
            script.execution = JavaScriptExecution::Script;
            let result = analyze(
                program,
                &uses,
                cell(program, "state"),
                script,
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(matches!(
                result.outcome,
                Outcome::Unknown(UnknownReason::ExecutionBoundary)
            ));
            assert_eq!(ledger.retained_bytes(), baseline);
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn reference_preparation_requires_exact_local_initialization_dominance() {
    checked("struct P{int x;}void change(ref P value){value.x+=1;}int run(){P local=P{1};change(ref local);return local.x;}print(run());print(run());",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let root=cell(program,"local");let family=complete(program,&uses,root,&mut ledger);
        let owner=program.cells[root.index()].owner;let data=program.unit(owner).unwrap();
        let initialize=data.operations.iter().position(|op|matches!(op.kind,OperationKind::Initialize(found) if found==root)).map(|n|OpId::from_index(n).unwrap()).unwrap();
        let call=data.operations.iter().position(|op|matches!(op.kind,OperationKind::Call(_))).map(|n|OpId::from_index(n).unwrap()).unwrap();
        let mut changed=program.clone();let mut working=changed.units[owner.index()].clone().into_working();let data=working.get_mut();let operations=&mut data.regions[data.entry.index()].operations;
        let initial=operations.iter().position(|op|*op==initialize).unwrap();operations.remove(initial);let after=operations.iter().position(|op|*op==call).unwrap()+1;operations.insert(after,initialize);
        changed.units[owner.index()]=working.freeze();changed.verify().unwrap();
        let updated=uses.updated(&changed,&[owner],&mut ledger,WorkDomain::Optional).unwrap();
        assert_eq!(uses.cell(root).unwrap().revision(),updated.cell(root).unwrap().revision(),"only the ordered body changed");assert!(!family.dependencies().valid_for(&changed,&updated));
        let result=analyze(&changed,&updated,root,request(),&mut ledger,WorkDomain::Optional).unwrap();assert!(matches!(result.outcome,Outcome::Unknown(UnknownReason::EarlyCaptureOrRead)));
        let (_,_,body)=creation(&changed,"change");let mut called=false;let result=inspect_references(&changed,&updated,body,request(),&mut ledger,|_,_|called=true).unwrap();assert!(matches!(result.outcome,Outcome::Unknown(UnknownReason::EarlyCaptureOrRead)));assert!(!called);
        updated.discard(&mut ledger).unwrap();family.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn supplied_producer_inputs_cannot_hide_a_second_opaque_creator() {
    checked("struct P{int x;}extern P opaque();auto change=(ref P value)=>{value.x=7;return value.x;};P state=P{1};print(change(ref state));auto maker=()=>{P bad=opaque();auto other=(ref P value)=>value.x;return other(ref bad);};print(maker());",|program|{
        let (_,_,body)=creation(program,"change");let (owner,operation,_)=creation(program,"other");
        let mut changed=program.clone();let mut working=changed.units[owner.index()].clone().into_working();working.get_mut().operations[operation.index()].kind=OperationKind::Closure(body);changed.units[owner.index()]=working.freeze();changed.verify().unwrap();
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let updated=uses.updated(&changed,&[owner],&mut ledger,WorkDomain::Optional).unwrap();let baseline=ledger.retained_bytes();let mut called=false;
        let analysis=run(request(),PRODUCT_FAMILY_PLAN,PRODUCT_FAMILY_VERSION,&mut ledger,WorkDomain::Optional,|budget|{
            let InputOutcome::Complete(inputs)=CallableInputs::for_cell_published(program,&uses,cell(program,"change"),CallObservations::from_execution(JavaScriptExecution::Module),budget)? else{panic!("private producer")};
            assert!(matches!(inputs.scope(),InputScope::Producer(_)));
            assert!(inputs.dependencies().valid_for_published(&changed,&updated,budget)?);
            let dependencies=certify_reference_parameters(&changed,&updated,&inputs,budget,|_,_|{called=true;Ok(())})?;
            drop(dependencies);inputs.discard(budget)?;Ok(())
        },|_,_|panic!("opaque second creator must reject before retention")).unwrap();
        assert!(matches!(analysis.outcome,Outcome::Unknown(UnknownReason::UnsupportedProducer)));assert!(!called);assert_eq!(ledger.retained_bytes(),baseline);
        updated.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn a_narrowed_opaque_leaf_cannot_supply_nominal_reference_writer_presence() {
    checked("struct P{int x;}struct Holder{P good;JsValue erased;}extern JsValue bad();Holder holder=Holder{P{1},bad()};auto change=(ref P dst)=>{dst=holder.good;return 0;};P state=P{2};print(change(ref state));",|program|{
        let (_,_,body)=creation(program,"change");let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let family=complete(program,&uses,cell(program,"state"),&mut ledger);
        let good=program.fields.iter().find(|field|field.name=="good").unwrap().identity;let erased=program.fields.iter().find(|field|field.name=="erased").unwrap().identity;
        let mut changed=program.clone();let mut working=changed.units[body.index()].clone().into_working();let place=working.get_mut().places.iter_mut().find(|place|matches!(place,Place::Field{field,..} if *field==good)).unwrap();let Place::Field{field,..}=place else{unreachable!()};*field=erased;changed.units[body.index()]=working.freeze();changed.verify().unwrap();
        let updated=uses.updated(&changed,&[body],&mut ledger,WorkDomain::Optional).unwrap();assert!(!family.dependencies().valid_for(&changed,&updated));
        let result=analyze(&changed,&updated,cell(&changed,"state"),request(),&mut ledger,WorkDomain::Optional).unwrap();assert!(matches!(result.outcome,Outcome::Unknown(UnknownReason::UnsupportedProducer)));
        let mut called=false;let result=inspect_references(&changed,&updated,body,request(),&mut ledger,|_,_|called=true).unwrap();assert!(matches!(result.outcome,Outcome::Unknown(UnknownReason::UnsupportedProducer)));assert!(!called);
        updated.discard(&mut ledger).unwrap();family.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}
