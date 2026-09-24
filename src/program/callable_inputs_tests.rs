use super::callable_inputs::*;
use super::raw_domains::Admission;
use super::record_family::OpRef;
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::mem::size_of;

struct Meter<'a, 'b>(&'a mut AllocationBudget<'b>);
impl Admission for Meter<'_, '_> {
    type Error = AllocationError;
    fn work(&mut self, n: usize) -> Result<(), Self::Error> {
        self.0.work(WorkKind::Analysis, n as u64)
    }
    fn vector<T>(&mut self, n: usize) -> Result<Vec<T>, Self::Error> {
        self.work(n)?;
        self.0.vector(AllocationClass::Scratch, n)
    }
    fn push<T>(&mut self, target: &mut Vec<T>, value: T) -> Result<(), Self::Error> {
        self.work(1)?;
        self.0.push(AllocationClass::Scratch, target, value)
    }
    fn release<T>(&mut self, value: Vec<T>) -> Result<(), Self::Error> {
        let bytes = value
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(AllocationError::Capacity)?;
        drop(value);
        self.0.release(AllocationClass::Scratch, bytes as u64)
    }
    fn invalid(&self, _: &'static str) -> Self::Error {
        AllocationError::Capacity
    }
}
const MEMORY: u64 = 10_000_000;
fn ledger(work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap()
}
fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    inspect(&program);
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
fn creation(program: &Program<'_>, cell: CellId) -> (OpRef, UnitId) {
    let unit = program.cells[cell.index()].owner;
    let data = program.unit(unit).unwrap();
    let initialize = data
        .operations
        .iter()
        .find(|op| matches!(op.kind,OperationKind::Initialize(found) if found==cell))
        .unwrap();
    let value = data.operands(initialize.operands).unwrap()[0];
    let operation = data.values[value.index()].definition;
    let OperationKind::Closure(body) = data.operations[operation.index()].kind else {
        panic!("fixture callable initialization")
    };
    (OpRef { unit, operation }, body)
}
fn complete(outcome: InputOutcome) -> CallableInputs {
    let InputOutcome::Complete(proof) = outcome else {
        panic!("complete explicit inputs expected: {outcome:?}")
    };
    proof
}
fn module() -> CallObservations {
    CallObservations::from_execution(JavaScriptExecution::Module)
}

#[test]
fn complete_inputs_cover_recursive_and_effectful_bodies_without_an_inline_choice() {
    checked(
        "extern void observe(int value);int compute(int value){observe(value);if(value<=0){return 7;}return compute(value-1);}print(compute(3));",
        |program| {
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let target = cell(program, "compute");
            let (producer, body) = creation(program, target);
            {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                let proof = complete(
                    CallableInputs::for_body(program, &uses, body, module(), &mut meter).unwrap(),
                );
                assert_eq!(proof.scope(), InputScope::Body(body));
                assert!(proof.runtime_inputs_sealed());
                assert_eq!(proof.producers().len(), 1);
                assert_eq!(proof.calls().len(), 2);
                assert_eq!(proof.producers()[0].creation, producer);
                assert_eq!(proof.producers()[0].cell, target);
                assert!(proof.calls().iter().any(|call| call.caller == body));
                for call in proof.calls() {
                    let caller = program.unit(call.caller).unwrap();
                    assert_eq!(
                        uses.unit(call.caller).unwrap().call_operation(call.target),
                        Some(call.call)
                    );
                    assert!(
                        matches!(caller.operations[call.prepare.index()].kind,OperationKind::PrepareCall(id) if id==call.target)
                    );
                    assert_eq!(caller.calls[call.target.index()].contract.supplied, 1);
                }
                assert!(
                    proof
                        .dependencies()
                        .valid_for(program, &uses, &mut meter)
                        .unwrap()
                );
                proof.discard(&mut meter).unwrap();
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn execution_seal_is_explicit_and_opaque_callable_uses_never_become_closed_inputs() {
    checked(
        "int compute(int value){return value+1;}print(compute(7));",
        |program| {
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let target = cell(program, "compute");
            let (producer, body) = creation(program, target);
            {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                let script = CallObservations::from_execution(JavaScriptExecution::Script);
                assert!(matches!(
                    CallableInputs::for_body(program, &uses, body, script, &mut meter).unwrap(),
                    InputOutcome::Unknown(UnknownReason::ExecutionBoundary)
                ));
                let conditional = complete(
                    CallableInputs::for_cell(program, &uses, target, script, &mut meter).unwrap(),
                );
                assert_eq!(conditional.scope(), InputScope::Producer(producer));
                assert!(!conditional.runtime_inputs_sealed());
                assert!(conditional.dependencies().creators().is_empty());
                conditional.discard(&mut meter).unwrap();
                let sealed = complete(
                    CallableInputs::for_producer(program, &uses, producer, module(), &mut meter)
                        .unwrap(),
                );
                assert!(sealed.runtime_inputs_sealed());
                sealed.discard(&mut meter).unwrap();
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
    for source in [
        "int compute(int value){return value+1;}auto alias=compute;print(alias(7));",
        "export int compute(int value){return value+1;}print(compute(7));",
        "extern void retain(func(int)->int callback);int compute(int value){return value+1;}retain(compute);print(compute(7));",
        "func(int)->int compute=(int value)=>value;compute=(int value)=>value+1;print(compute(7));",
        "int compute(int value){return value+1;}",
    ] {
        checked(source, |program| {
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                assert!(
                    matches!(
                        CallableInputs::for_cell(
                            program,
                            &uses,
                            cell(program, "compute"),
                            module(),
                            &mut meter
                        )
                        .unwrap(),
                        InputOutcome::Unknown(_)
                    ),
                    "{source}"
                );
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn a_new_independent_creator_invalidates_body_evidence_but_not_the_original_producer() {
    checked(
        "extern int opaque();auto compute=(int value)=>value+1;auto maker=()=>{auto other=(int value)=>value+1;return other(opaque());};int unrelated(){return 1;}print(compute(7));print(maker());",
        |program| {
            let mut ledger = ledger(5_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let (producer, body) = creation(program, cell(program, "compute"));
            let (donor, _) = creation(program, cell(program, "other"));
            let mut revised = program.clone();
            let mut working = revised.units[donor.unit.index()].clone().into_working();
            working.get_mut().operations[donor.operation.index()].kind =
                OperationKind::Closure(body);
            revised.units[donor.unit.index()] = working.freeze();
            revised.verify().unwrap();
            let updated = uses
                .updated(&revised, &[donor.unit], &mut ledger, WorkDomain::Baseline)
                .unwrap();
            assert_ne!(
                uses.creators(body).unwrap().revision(),
                updated.creators(body).unwrap().revision()
            );
            let (_, unrelated) = creation(program, cell(program, "unrelated"));
            let mut independent = program.clone();
            let mut working = independent.units[unrelated.index()].clone().into_working();
            let constant = working
                .get_mut()
                .operations
                .iter_mut()
                .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(1))))
                .unwrap();
            constant.kind = OperationKind::Constant(Constant::Integer(2));
            independent.units[unrelated.index()] = working.freeze();
            independent.verify().unwrap();
            let independent_uses = uses
                .updated(
                    &independent,
                    &[unrelated],
                    &mut ledger,
                    WorkDomain::Baseline,
                )
                .unwrap();
            {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                let old_body = complete(
                    CallableInputs::for_body(program, &uses, body, module(), &mut meter).unwrap(),
                );
                let original = complete(
                    CallableInputs::for_producer(program, &uses, producer, module(), &mut meter)
                        .unwrap(),
                );
                assert_eq!(old_body.calls().len(), 1);
                assert_eq!(old_body.dependencies().creators().len(), 1);
                assert!(original.dependencies().creators().is_empty());
                assert!(
                    !old_body
                        .dependencies()
                        .valid_for(&revised, &updated, &mut meter)
                        .unwrap()
                );
                assert!(
                    original
                        .dependencies()
                        .valid_for(&revised, &updated, &mut meter)
                        .unwrap()
                );
                assert!(
                    old_body
                        .dependencies()
                        .valid_for(&independent, &independent_uses, &mut meter)
                        .unwrap()
                );
                let joined = complete(
                    CallableInputs::for_body(&revised, &updated, body, module(), &mut meter)
                        .unwrap(),
                );
                assert_eq!(joined.producers().len(), 2);
                assert_eq!(joined.calls().len(), 2);
                assert!(joined.calls().iter().any(|call| call.caller == donor.unit));
                // Input evidence retains the opaque call argument instead of
                // asserting that the joined body receives primitive values.
                let opaque_call = joined
                    .calls()
                    .iter()
                    .find(|call| call.caller == donor.unit)
                    .unwrap();
                let caller = revised.unit(opaque_call.caller).unwrap();
                let CallArgument::Value(value) = caller
                    .arguments(caller.calls[opaque_call.target.index()].arguments)
                    .unwrap()[0] else { panic!("opaque value argument") };
                assert!(matches!(
                    caller.operations[caller.values[value.index()].definition.index()].kind,
                    OperationKind::Call(_)
                ));
                joined.discard(&mut meter).unwrap();
                original.discard(&mut meter).unwrap();
                old_body.discard(&mut meter).unwrap();
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            independent_uses.discard(&mut ledger).unwrap();
            updated.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn work_and_memory_refusal_release_partial_provider_storage() {
    checked(
        "int compute(int value){return value+1;}print(compute(7));print(compute(8));",
        |program| {
            let (_, body) = creation(program, cell(program, "compute"));
            for work in [0, 1, 10, 30, 100] {
                let mut ledger = ledger(work);
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let baseline = ledger.retained_bytes();
                {
                    let mut allocation =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    let mut meter = Meter(&mut allocation);
                    match CallableInputs::for_body(program, &uses, body, module(), &mut meter) {
                        Ok(InputOutcome::Complete(proof)) => proof.discard(&mut meter).unwrap(),
                        Ok(other) => {
                            panic!("valid callable did not lose semantic eligibility: {other:?}")
                        }
                        Err(_) => {}
                    }
                    assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
                }
                assert_eq!(ledger.retained_bytes(), baseline);
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
            for headroom in [0, 32, 128, 512] {
                let mut ledger = ledger(1_000_000);
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let baseline = ledger.retained_bytes();
                let filler = MEMORY - baseline - headroom;
                ledger.retain(WorkDomain::Optional, filler).unwrap();
                {
                    let mut allocation =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    let mut meter = Meter(&mut allocation);
                    match CallableInputs::for_body(program, &uses, body, module(), &mut meter) {
                        Ok(InputOutcome::Complete(proof)) => proof.discard(&mut meter).unwrap(),
                        Ok(other) => panic!("memory refusal is not semantic Unknown: {other:?}"),
                        Err(_) => {}
                    }
                    assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
                }
                assert_eq!(ledger.retained_bytes(), baseline + filler);
                ledger.release(WorkDomain::Optional, filler).unwrap();
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        },
    );
}
