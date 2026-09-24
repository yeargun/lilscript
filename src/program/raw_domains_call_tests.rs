//! Callable input facts must work before choosing an inline representation.
use super::raw_domains::{Admission, DomainInputs, DomainProof, SourceRecipes, Subject};
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::mem::size_of;

const MEMORY: u64 = 10_000_000;
struct Meter<'a, 'b>(&'a mut AllocationBudget<'b>);
impl Admission for Meter<'_, '_> {
    type Error = AllocationError;
    fn work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.work(WorkKind::Analysis, amount as u64)
    }
    fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>, Self::Error> {
        self.work(capacity)?;
        self.0.vector(AllocationClass::Scratch, capacity)
    }
    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Self::Error> {
        self.work(1)?;
        self.0.push(AllocationClass::Scratch, values, value)
    }
    fn release<T>(&mut self, values: Vec<T>) -> Result<(), Self::Error> {
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(AllocationError::Capacity)?;
        drop(values);
        self.0.release(AllocationClass::Scratch, bytes as u64)
    }
    fn invalid(&self, _: &'static str) -> Self::Error {
        AllocationError::Capacity
    }
}
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
    let source = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&source).unwrap();
    let program = from_checked_source(&source, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program);
}
fn named_cell(program: &Program<'_>, name: &str) -> CellId {
    let matches: Vec<_> = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name)
        .collect();
    assert_eq!(matches.len(), 1, "unique fixture cell {name}");
    CellId::from_index(matches[0].0).unwrap()
}
fn creation(program: &Program<'_>, name: &str) -> (UnitId, OpId, UnitId) {
    let cell = named_cell(program, name);
    let owner = program.cells[cell.index()].owner;
    let unit = program.unit(owner).unwrap();
    let init = unit
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::Initialize(found) if found == cell))
        .unwrap();
    let value = unit.operands(init.operands).unwrap()[0];
    let operation = unit.values[value.index()].definition;
    let OperationKind::Closure(body) = unit.operations[operation.index()].kind else {
        panic!("callable fixture")
    };
    (owner, operation, body)
}
fn inputs(execution: JavaScriptExecution) -> DomainInputs<'static> {
    DomainInputs::empty().with_execution(execution)
}

const RECURSIVE: &str = "int descend(int value){int copy=value;value=copy;if(value>0){return descend(value-1);}return value;}";

fn check_recursive(source: &str, expected: bool) {
    checked(source, |program| {
        let (_, _, body) = creation(program, "descend");
        let parameter = program.unit(body).unwrap().parameters[0];
        let copy = named_cell(program, "copy");
        // Source scalar assignment needs no explicit copy operation. Insert a
        // checked, equivalent primitive CopyValue so both that recipe and the
        // load/store recurrence must carry the incoming domain correctly.
        let mut revised = program.clone();
        let mut working = revised.units[body.index()].clone().into_working();
        let unit = working.get_mut();
        let init = unit
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Initialize(cell) if cell == copy))
            .unwrap();
        let original = unit.operands(unit.operations[init].operands).unwrap()[0];
        let operation = OpId::from_index(unit.operations.len()).unwrap();
        let value = ValueId::from_index(unit.values.len()).unwrap();
        let range = OperandRange {
            start: unit.operands.len() as u32,
            len: 1,
        };
        unit.operands.push(original);
        let region = unit.operations[init].region;
        unit.operations.push(Operation {
            kind: OperationKind::CopyValue,
            operands: range,
            result: Some(value),
            region,
            origin: None,
            span: unit.operations[init].span,
        });
        unit.values.push(Value {
            ty: unit.values[original.index()].ty,
            definition: operation,
        });
        unit.operations[init].operands = OperandRange {
            start: unit.operands.len() as u32,
            len: 1,
        };
        unit.operands.push(value);
        let before = unit.regions[region.index()]
            .operations
            .iter()
            .position(|op| op.index() == init)
            .unwrap();
        unit.regions[region.index()]
            .operations
            .insert(before, operation);
        revised.units[body.index()] = working.freeze();
        revised.verify().unwrap();
        let program = &revised;
        assert!(program
            .unit(body)
            .unwrap()
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::CopyValue)));
        let mut ledger = ledger(1_000_000);
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        {
            let mut allocation = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut meter = Meter(&mut allocation);
            let roots = [Subject::Cell(parameter), Subject::Cell(copy)];
            let proof = DomainProof::build(
                program,
                &uses,
                &roots,
                inputs(JavaScriptExecution::Module),
                &SourceRecipes,
                &mut meter,
            )
            .unwrap();
            for root in roots {
                assert_eq!(proof.primitive(root, &mut meter).unwrap(), expected);
            }
            assert!(proof
                .dependencies()
                .creators()
                .iter()
                .any(|(found, stamp)| *found == body
                    && *stamp == uses.creators(body).unwrap().revision()));
            assert!(proof
                .dependencies()
                .valid_for(program, &uses, &mut meter)
                .unwrap());
            proof.discard(&mut meter).unwrap();
            assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
        }
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn recursive_private_inputs_and_copy_cycles_have_an_inductive_primitive_domain() {
    check_recursive(&format!("{RECURSIVE}print(descend(7));"), true);
}

#[test]
fn one_opaque_actual_poisons_recursive_parameter_and_copy_storage() {
    check_recursive(
        &format!("extern int opaque();{RECURSIVE}print(descend(7));print(descend(opaque()));"),
        false,
    );
}

#[test]
fn direct_parameter_knowledge_requires_an_explicit_module_execution_seal() {
    checked(
        "int compute(int value){return value+1;}print(compute(7));",
        |program| {
            let (_, _, body) = creation(program, "compute");
            let root = Subject::Cell(program.unit(body).unwrap().parameters[0]);
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            for (input, expected) in [
                (DomainInputs::empty(), false),
                (inputs(JavaScriptExecution::Script), false),
                (inputs(JavaScriptExecution::Module), true),
            ] {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                let proof =
                    DomainProof::build(program, &uses, &[root], input, &SourceRecipes, &mut meter)
                        .unwrap();
                assert_eq!(proof.primitive(root, &mut meter).unwrap(), expected);
                proof.discard(&mut meter).unwrap();
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn an_opaque_second_creator_invalidates_and_poisons_previously_private_body_inputs() {
    checked("extern int opaque();auto compute=(int value)=>value+1;auto maker=()=>{auto other=(int value)=>value+1;return other(opaque());};int unrelated(){return 1;}print(compute(7));print(maker());", |program| {
        let (_, _, body) = creation(program, "compute");
        let (creator, operation, _) = creation(program, "other");
        let root = Subject::Cell(program.unit(body).unwrap().parameters[0]);
        let mut changed = program.clone();
        let mut working = changed.units[creator.index()].clone().into_working();
        working.get_mut().operations[operation.index()].kind = OperationKind::Closure(body);
        changed.units[creator.index()] = working.freeze();
        changed.verify().unwrap();
        assert_eq!(program.units[body.index()].revision(), changed.units[body.index()].revision());
        let mut ledger = ledger(5_000_000);
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let updated = uses.updated(&changed, &[creator], &mut ledger, WorkDomain::Baseline).unwrap();
        {
            let mut allocation = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut meter = Meter(&mut allocation);
            let old = DomainProof::build(program, &uses, &[root], inputs(JavaScriptExecution::Module), &SourceRecipes, &mut meter).unwrap();
            assert!(old.primitive(root, &mut meter).unwrap());
            assert!(!old.dependencies().units().iter().any(|(unit, _)| *unit == creator), "unrelated old producer must not be scanned");
            assert!(!old.dependencies().valid_for(&changed, &updated, &mut meter).unwrap(), "new producer must invalidate through its creator-set stamp");
            let joined = DomainProof::build(&changed, &updated, &[root], inputs(JavaScriptExecution::Module), &SourceRecipes, &mut meter).unwrap();
            assert!(!joined.primitive(root, &mut meter).unwrap());
            assert!(joined.dependencies().units().iter().any(|(unit, _)| *unit == creator));
            assert!(joined.dependencies().valid_for(&changed, &updated, &mut meter).unwrap());
            joined.discard(&mut meter).unwrap();
            old.discard(&mut meter).unwrap();
            assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
        }
        updated.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn incoming_call_evidence_releases_all_storage_on_late_work_and_memory_refusal() {
    let source = format!(
        "int compute(int value){{int copy=value;value=copy;return value;}}{}",
        "print(compute(7));".repeat(128)
    );
    checked(&source, |program| {
        let (_, _, body) = creation(program, "compute");
        let root = Subject::Cell(program.unit(body).unwrap().parameters[0]);
        let mut measuring = ledger(5_000_000);
        let uses = UseIndex::build(program, &mut measuring, WorkDomain::Baseline).unwrap();
        let (work, live) = {
            let mut allocation =
                AllocationBudget::new(Some((&mut measuring, WorkDomain::Optional)));
            let mut meter = Meter(&mut allocation);
            let proof = DomainProof::build(
                program,
                &uses,
                &[root],
                inputs(JavaScriptExecution::Module),
                &SourceRecipes,
                &mut meter,
            )
            .unwrap();
            // A retained complete creator dependency establishes that this path
            // actually built incoming-call evidence, rather than stopping at
            // the parameter's unknown boundary before its owned arrays exist.
            assert!(proof
                .dependencies()
                .creators()
                .iter()
                .any(|(unit, _)| *unit == body));
            let work = meter
                .0
                .with_ledger(|ledger| ledger.unwrap().0.work_used(WorkDomain::Optional));
            let live = meter.0.retained_bytes(AllocationClass::Scratch);
            assert!(live > 0);
            assert!(proof.primitive(root, &mut meter).unwrap());
            proof.discard(&mut meter).unwrap();
            assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            (work, live)
        };
        uses.discard(&mut measuring).unwrap();
        assert_eq!(measuring.retained_bytes(), 0);
        for memory_denial in [false, true] {
            let mut ledger = ledger(if memory_denial { 5_000_000 } else { work - 1 });
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let baseline = ledger.retained_bytes();
            let blocked = if memory_denial {
                let blocked = MEMORY - baseline - (live - 1);
                ledger.retain(WorkDomain::Optional, blocked).unwrap();
                blocked
            } else {
                0
            };
            {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                assert!(matches!(
                    DomainProof::build(
                        program,
                        &uses,
                        &[root],
                        inputs(JavaScriptExecution::Module),
                        &SourceRecipes,
                        &mut meter
                    ),
                    Err(AllocationError::Budget(_))
                ));
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            assert_eq!(ledger.retained_bytes(), baseline + blocked);
            ledger.release(WorkDomain::Optional, blocked).unwrap();
            assert!(uses.valid_for(program));
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}
