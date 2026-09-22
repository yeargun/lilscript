//! Proof/ownership tests; the target cohort owns actual artifact execution.
use super::function_layout::*;
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
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
            plan: FUNCTION_LAYOUT_PLAN,
            algorithm_version: FUNCTION_LAYOUT_VERSION,
            work_quota: 300_000,
        },
        scratch_bytes: 800_000,
        output_bytes: 800_000,
    }
}
fn body(program: &Program<'_>, name: &str) -> UnitId {
    program
        .cells
        .iter()
        .find_map(|cell| match cell.binding {
            CellBinding::Function(body) if cell.name == name => Some(body),
            _ => None,
        })
        .unwrap()
}
fn complete(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    ledger: &mut BudgetLedger,
) -> FunctionLayout {
    let result = analyze(program, uses, body, request(), ledger, WorkDomain::Optional).unwrap();
    assert_eq!(result.receipt.completion, AnalysisCompletion::Complete);
    match result.outcome {
        FamilyOutcome::Complete(layout) => layout,
        other => panic!("expected layout: {other:?}"),
    }
}
fn edit<'s>(
    program: &Program<'s>,
    unit: UnitId,
    modify: impl FnOnce(&mut UnitData),
) -> Program<'s> {
    let mut changed = program.clone();
    let mut working = changed.units[unit.index()].clone().into_working();
    modify(working.get_mut());
    changed.units[unit.index()] = working.freeze();
    changed.verify().unwrap();
    changed
}

#[test]
fn private_parameter_layouts_expand_raw_products_and_keep_reference_modes() {
    checked("struct A{int x;}struct B{string y;}extern int opaque();int read(A first,B second,ref int count){count+=1;return first.x+second.y.length;}int count=0;A one=A{opaque()};B two=B{\"ok\"};print(read(one,two,ref count));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let before=ledger.retained_bytes();let layout=complete(program,&uses,body(program,"read"),&mut ledger);
        assert_eq!(layout.parameters().iter().map(|p|p.position).collect::<Vec<_>>(),vec![0,1]);
        assert_ne!(layout.parameters()[0].schema,layout.parameters()[1].schema);
        assert_eq!(layout.transport(0),ProductTransport::Fields);assert_eq!(layout.transport(2),ProductTransport::Packed);
        assert_eq!(layout.inputs().calls().len(),1);assert!(layout.inputs().runtime_inputs_sealed());
        assert!(layout.dependencies().valid_for(program,&uses));
        assert_eq!(ledger.retained_bytes()-before,layout.retained_bytes());
        layout.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),before);uses.discard(&mut ledger).unwrap();
    });
}

#[test]
fn recursive_forwarding_joins_every_actual_without_manufacturing_reachability() {
    for (source,callee,expected) in [
        ("struct P{int x;}int recur(P value,int left){if(left==0){return value.x;}return recur(value,left-1);}int outer(P copy){return recur(copy,2);}P source=P{3};print(outer(source));","recur",true),
        // Complete private input evidence means this cycle has no entry. The
        // conditional presence theorem is vacuous, not an invented allocation.
        ("struct P{int x;}int cycle(P value){return cycle(value);}","cycle",true),
        ("struct P{int x;}extern P opaque();int cycle(P value){return cycle(value);}print(cycle(opaque()));","cycle",false),
        ("struct P{int x;}extern P opaque();int recur(P value,int left){if(left==0){return value.x;}return recur(value,left-1);}int outer(P copy){return recur(copy,2);}print(outer(opaque()));","recur",false),
    ] {
        checked(source,|program|{
            let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();
            let result=analyze(program,&uses,body(program,callee),request(),&mut ledger,WorkDomain::Optional).unwrap();
            match result.outcome {FamilyOutcome::Complete(layout) if expected=>{assert!(layout.dependencies().units().len()>=1);layout.discard(&mut ledger).unwrap();},FamilyOutcome::Unknown(UnknownReason::UnsupportedProducer) if !expected=>{},other=>panic!("{source}: {other:?}")}
            assert_eq!(ledger.retained_bytes(),baseline);uses.discard(&mut ledger).unwrap();
        });
    }
}

#[test]
fn expanded_parameters_reject_own_lexical_arguments_and_observable_callables() {
    for (source,reason) in [
        ("struct P{int x;}extern JsValue arguments;void read(P value){auto view=arguments;print(value.x);}read(P{1});",UnknownReason::ArgumentsObservation),
        ("struct P{int x;}extern JsValue arguments;void read(P value){auto view=()=>arguments;view();print(value.x);}read(P{1});",UnknownReason::ArgumentsObservation),
        ("struct P{int x;}export int read(P value){return value.x;}print(read(P{1}));",UnknownReason::CallableInterface),
        ("struct P{int x;}extern void retain(func(P)->int callback);int read(P value){return value.x;}retain(read);print(read(P{1}));",UnknownReason::CallableInterface),
    ] {
        checked(source,|program|{
            let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let baseline=ledger.retained_bytes();
            let result=analyze(program,&uses,body(program,"read"),request(),&mut ledger,WorkDomain::Optional).unwrap();
            assert!(matches!(result.outcome,FamilyOutcome::Unknown(found) if found==reason),"{source}: {:?}",result.outcome);
            assert_eq!(ledger.retained_bytes(),baseline);uses.discard(&mut ledger).unwrap();
        });
    }
}

#[test]
fn empty_product_conditional_inputs_have_zero_width_without_losing_execution_evidence() {
    checked("struct Empty{}extern bool mark();void step(Empty value){}step(if(mark()){Empty{}}else{Empty{}});",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let layout=complete(program,&uses,body(program,"step"),&mut ledger);
        assert_eq!(layout.parameters().len(),1);assert!(program.structs[layout.parameters()[0].schema.index()].fields.is_empty());
        let call=layout.inputs().calls()[0];let data=program.unit(call.caller).unwrap();
        let arguments=data.arguments(data.calls[call.target.index()].arguments).unwrap();assert_eq!(arguments.len(),1);
        assert!(data.operations.iter().any(|op|matches!(op.kind,OperationKind::Select{..})));
        layout.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();
    });
}

#[test]
fn input_writer_revisions_invalidate_even_when_cell_use_locations_stay_equal() {
    checked("struct P{int x;}int untouched(){return 9;}int read(P value){return value.x;}P source=P{1};print(read(source));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let layout=complete(program,&uses,body(program,"read"),&mut ledger);
        let source=program.cells.iter().position(|c|c.name=="source").map(|n|CellId::from_index(n).unwrap()).unwrap();
        let owner=program.cells[source.index()].owner;
        let changed=edit(program,owner,|data|data.operations.iter_mut().find(|op|matches!(op.kind,OperationKind::Constant(Constant::Integer(1)))).unwrap().kind=OperationKind::Constant(Constant::Integer(2)));
        let updated=uses.updated(&changed,&[owner],&mut ledger,WorkDomain::Optional).unwrap();
        assert_eq!(uses.cell(source).unwrap().revision(),updated.cell(source).unwrap().revision());
        assert!(!layout.dependencies().valid_for(&changed,&updated));updated.discard(&mut ledger).unwrap();
        let untouched=body(program,"untouched");
        let changed=edit(program,untouched,|data|data.operations.iter_mut().find(|op|matches!(op.kind,OperationKind::Constant(Constant::Integer(9)))).unwrap().kind=OperationKind::Constant(Constant::Integer(10)));
        let updated=uses.updated(&changed,&[untouched],&mut ledger,WorkDomain::Optional).unwrap();
        assert!(layout.dependencies().valid_for(&changed,&updated));updated.discard(&mut ledger).unwrap();
        layout.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn layout_admission_script_and_discard_orders_preserve_original_charges() {
    checked("struct P{int x;int y;}int read(P value){return value.x;}P source=P{1,2};print(read(source));",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let before=ledger.retained_bytes();
        for (limit,reason) in [(0,ResourceLimit::Work),(1,ResourceLimit::Scratch),(2,ResourceLimit::Output)] {
            let mut request=request();match limit {0=>request.attempt.work_quota=0,1=>request.scratch_bytes=0,_=>request.output_bytes=0}
            let result=analyze(program,&uses,body(program,"read"),request,&mut ledger,WorkDomain::Optional).unwrap();
            assert!(matches!(result.outcome,FamilyOutcome::Truncated(found) if found==reason));assert_eq!(ledger.retained_bytes(),before);
        }
        let mut script=request();script.execution=JavaScriptExecution::Script;
        let result=analyze(program,&uses,body(program,"read"),script,&mut ledger,WorkDomain::Optional).unwrap();
        assert!(matches!(result.outcome,FamilyOutcome::Unknown(UnknownReason::ExecutionBoundary)));assert_eq!(ledger.retained_bytes(),before);
        let layout=complete(program,&uses,body(program,"read"),&mut ledger);uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(),layout.retained_bytes());layout.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn shared_callable_locator_uses_existing_owner_order_for_many_alias_calls() {
    struct LookupMeter {
        work: usize,
    }
    impl super::raw_domains::Admission for LookupMeter {
        type Error = &'static str;
        fn work(&mut self, amount: usize) -> Result<(), Self::Error> {
            self.work = self.work.checked_add(amount).ok_or("work overflow")?;
            Ok(())
        }
        fn vector<T>(&mut self, _: usize) -> Result<Vec<T>, Self::Error> {
            Err("locator allocated")
        }
        fn push<T>(&mut self, _: &mut Vec<T>, _: T) -> Result<(), Self::Error> {
            Err("locator allocated")
        }
        fn release<T>(&mut self, _: Vec<T>) -> Result<(), Self::Error> {
            Err("locator allocated")
        }
        fn invalid(&self, message: &'static str) -> Self::Error {
            message
        }
    }
    for count in [64, 512] {
        let mut source =
            String::from("struct P{int x;}auto read=(P value)=>value.x;P source=P{1};");
        for _ in 0..count {
            source.push_str("print(read(source));");
        }
        checked(&source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let cell = program
                .cells
                .iter()
                .position(|cell| cell.name == "read")
                .map(|n| CellId::from_index(n).unwrap())
                .unwrap();
            let owner = program.cells[cell.index()].owner;
            let data = program.unit(owner).unwrap();
            let initial = data
                .operations
                .iter()
                .find(|op| matches!(op.kind,OperationKind::Initialize(found) if found==cell))
                .unwrap();
            let value = data.operands(initial.operands).unwrap()[0];
            let OperationKind::Closure(body) =
                data.operations[data.values[value.index()].definition.index()].kind
            else {
                panic!("original callable producer")
            };
            let mut meter = LookupMeter { work: 0 };
            let mut found = 0;
            for n in 0..data.calls.len() {
                if let Some(actual) = super::callable_inputs::body_for_call(
                    program,
                    &uses,
                    owner,
                    CallId::from_index(n).unwrap(),
                    &mut meter,
                )
                .unwrap()
                {
                    assert_eq!(actual, body);
                    found += 1;
                }
            }
            assert_eq!(found, count);
            let visits = uses.unit(owner).unwrap().cell_uses().len();
            let logarithmic = (usize::BITS - visits.leading_zeros()) as usize;
            assert!(
                meter.work <= data.calls.len() * (logarithmic + 12),
                "{} visits for {} calls",
                meter.work,
                count
            );
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}
