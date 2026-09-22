//! Prepared locations share one call owner and remain observable aliases.
use super::demand::{DemandMode, DemandPlan};
use super::facts::{self, EvaluationBehavior};
use super::publication::{CheckpointLimit, Compilation, PlacePatch, UnitPatch};
use super::raw_domains::{Admission, DomainInputs, DomainProof, SourceRecipes, Subject};
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain, WorkKind,
};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::primitive::ParameterPassing;

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program);
}
fn ledger(optional_work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 20_000_000,
            optional_work,
            baseline_retained_bytes: 0,
            retained_bytes: 20_000_000,
        },
    )
    .unwrap()
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
fn function(program: &Program<'_>, name: &str) -> UnitId {
    let CellBinding::Function(unit) = program.cells[cell(program, name).index()].binding else {
        panic!("named function")
    };
    unit
}
fn edit<'src>(
    program: &Program<'src>,
    unit: UnitId,
    mutate: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut edited = program.clone();
    let mut working = edited.units[unit.index()].clone().into_working();
    mutate(working.get_mut());
    edited.units[unit.index()] = working.freeze();
    edited
}

const ORDERED: &str = "int next(){return 3;}void use(int before,ref int left,int between,ref int right){left=before;right=between;}int x=1;int y=2;use(next(),ref x,next(),ref y);";

#[test]
fn sole_argument_ranges_preserve_nested_calls_reference_positions_and_parameter_ordinals() {
    checked(ORDERED, |program| {
        let body = program.unit(function(&program, "use")).unwrap();
        for (position, &parameter) in body.parameters.iter().enumerate() {
            assert_eq!(
                program.cells[parameter.index()].binding,
                CellBinding::Parameter(position as u32)
            );
            assert_eq!(
                program.parameter(parameter).unwrap().passing,
                if position % 2 == 0 {
                    ParameterPassing::Value
                } else {
                    ParameterPassing::MutableReference
                }
            );
        }
        let owner = program.initialization[0];
        let unit = program.unit(owner).unwrap();
        let (outer_index, outer) = unit
            .calls
            .iter()
            .enumerate()
            .find(|(_, site)| site.arguments.len == 4)
            .unwrap();
        let outer_id = CallId::from_index(outer_index).unwrap();
        let arguments = unit.arguments(outer.arguments).unwrap();
        assert!(matches!(
            arguments,
            [
                CallArgument::Value(_),
                CallArgument::Reference(_),
                CallArgument::Value(_),
                CallArgument::Reference(_)
            ]
        ));
        assert_eq!(
            unit.call_arguments.len(),
            4,
            "nested zero-argument calls own empty ranges"
        );
        let mut observed = Vec::new();
        for &operation in &unit.regions[unit.entry.index()].operations {
            match unit.operations[operation.index()].kind {
                OperationKind::Call(call) => {
                    assert_eq!(unit.operations[operation.index()].operands.len, 0);
                    observed.push(if call == outer_id { "invoke" } else { "value" });
                }
                OperationKind::PrepareReference { call, position } => {
                    assert_eq!(call, outer_id);
                    observed.push(if position == 1 {
                        "left"
                    } else {
                        assert_eq!(position, 3);
                        "right"
                    });
                }
                _ => {}
            }
        }
        assert_eq!(observed, ["value", "left", "value", "right", "invoke"]);
        let mut budget = ledger(20_000_000);
        let uses = UseIndex::build(&program, &mut budget, WorkDomain::Baseline).unwrap();
        let call = uses.unit(owner).unwrap().call_operation(outer_id).unwrap();
        for (position, argument) in arguments.iter().enumerate() {
            if let CallArgument::Value(value) = argument {
                assert!(uses
                    .unit(owner)
                    .unwrap()
                    .value_uses(*value)
                    .unwrap()
                    .contains(&ValueUse::CallArgument {
                        operation: call,
                        call: outer_id,
                        position: position as u32
                    }));
            }
        }
        for name in ["x", "y"] {
            let users = uses.cell(cell(&program, name)).unwrap();
            assert!(users.reference_exposed());
            assert!(users.sites().iter().any(|site| matches!(site, CellUseSite::Unit {usage:CellUse::Reference{call,..},..} if *call==outer_id)));
        }
        uses.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn verifier_rejects_unowned_aliased_or_unprepared_arguments_and_false_parameter_ordinals() {
    checked(ORDERED, |program| {
        let owner = program.initialization[0];
        let unit = program.unit(owner).unwrap();
        let outer = unit
            .calls
            .iter()
            .position(|site| site.arguments.len == 4)
            .unwrap();
        let preparation = unit
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::PrepareReference { position: 1, .. }))
            .unwrap();
        for variant in 0..7 {
            let revised = edit(&program, owner, |data| match variant {
                0 => data.call_arguments.push(data.call_arguments[0]),
                1 => data.calls[outer].arguments.start = u32::MAX,
                2 => {
                    let other = (outer + 1) % data.calls.len();
                    data.calls[other].arguments = data.calls[outer].arguments
                }
                3 => {
                    data.operations[preparation].kind = OperationKind::CheckPlace(
                        match data.call_arguments[data.calls[outer].arguments.start as usize + 1] {
                            CallArgument::Reference(place) => place,
                            _ => unreachable!(),
                        },
                    )
                }
                4 => {
                    data.operations[preparation].kind = OperationKind::PrepareReference {
                        call: CallId::from_index(outer).unwrap(),
                        position: 3,
                    }
                }
                5 => {
                    let CallArgument::Value(value) =
                        data.call_arguments[data.calls[outer].arguments.start as usize]
                    else {
                        unreachable!()
                    };
                    let call = data
                        .operations
                        .iter()
                        .position(
                            |op| matches!(op.kind,OperationKind::Call(id) if id.index()==outer),
                        )
                        .unwrap();
                    data.operations[call].operands = OperandRange {
                        start: data.operands.len() as u32,
                        len: 1,
                    };
                    data.operands.push(value);
                }
                _ => {
                    let schedule = &mut data.regions[data.entry.index()].operations;
                    let before=schedule.iter().position(|id|matches!(data.operations[id.index()].kind,OperationKind::PrepareCall(call) if call.index()==outer)).unwrap();
                    let reference = schedule
                        .iter()
                        .position(|id| id.index() == preparation)
                        .unwrap();
                    schedule.swap(before, reference);
                }
            });
            assert!(
                revised.verify().is_err(),
                "malformed reference edit {variant} was accepted"
            );
        }
        let mut revised = program.clone();
        let parameter = program.unit(function(&program, "use")).unwrap().parameters[1];
        Arc::make_mut(&mut revised.cells)[parameter.index()].binding = CellBinding::Parameter(0);
        assert!(revised.verify().unwrap_err().contains("ordinal"));
    });
}

#[test]
fn reference_formal_access_and_preparation_remain_effectful_when_results_are_discarded() {
    checked("void set(ref int target){target=7;int discarded=target+1;}int state=1;set(ref state);print(state);",|program| {
        let mut budget=ledger(20_000_000);
        let uses=UseIndex::build(&program,&mut budget,WorkDomain::Baseline).unwrap();
        let mut config=crate::config::ProjectConfig::default();config.javascript.strip_console=false;
        let policy=config.resolve_policy(CompilationRequest::JavaScript{preserve_root_exports:true}).unwrap();
        let demand=DemandPlan::build(&program,Some(&uses),None,policy.javascript_contract().unwrap(),DemandMode::Prune,Some((&mut budget,WorkDomain::Optional))).unwrap();
        let body=function(&program,"set");
        let creation=OpId::from_index(program.unit(program.initialization[0]).unwrap().operations.iter().position(|op|matches!(op.kind,OperationKind::Closure(found) if found==body)).unwrap()).unwrap();
        let context=demand.child(demand.root(),creation).unwrap();
        let data=program.unit(body).unwrap();
        let domains=vec![false;data.values.len()];
        let mut accesses=0;
        for (index,operation) in data.operations.iter().enumerate() {
            if matches!(operation.kind,OperationKind::Load(_)|OperationKind::Store(_)) {
                accesses+=1;
                assert_eq!(facts::operation_evaluation_behavior(&program,data,operation,&domains),EvaluationBehavior::UNKNOWN);
                assert!(demand.needs_execution(context,OpId::from_index(index).unwrap()));
            }
        }
        assert_eq!(accesses,2);
        let root=demand.root();
        for (index,operation) in program.unit(program.initialization[0]).unwrap().operations.iter().enumerate() {
            if matches!(operation.kind,OperationKind::PrepareReference{..}) {
                assert!(demand.needs_execution(root,OpId::from_index(index).unwrap()));
            }
        }
        demand.discard(Some(&mut budget)).unwrap();
        uses.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(),0);
    });
}

#[test]
fn plain_reference_store_checks_current_parents_before_rhs_without_loading_old_leaf() {
    checked("extern int rhs();void plain(ref int target){target=rhs();}void compound(ref int target){target+=rhs();}int state=1;plain(ref state);compound(ref state);", |program| {
        for name in ["plain", "compound"] {
            let data = program.unit(function(&program,name)).unwrap();
            let check = data.operations.iter().position(|operation|matches!(operation.kind,OperationKind::CheckPlace(_)));
            let load = data.operations.iter().position(|operation|matches!(operation.kind,OperationKind::Load(place) if matches!(data.places[place.index()],Place::Cell(cell) if program.is_reference_parameter(cell))));
            let rhs = data.operations.iter().position(|operation|matches!(operation.kind,OperationKind::Call(_))).unwrap();
            let store = data.operations.iter().position(|operation|matches!(operation.kind,OperationKind::Store(_))).unwrap();
            if name == "plain" {
                assert!(check.unwrap() < rhs && rhs < store);
                assert!(load.is_none(),"checking a dynamic field alias must not coerce/read its old int leaf");
            } else {
                assert!(check.is_none(),"compound assignment already observes its old leaf");
                assert!(load.unwrap() < rhs && rhs < store);
            }
        }
    });
}

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
        let bytes = (values.capacity() * std::mem::size_of::<T>()) as u64;
        drop(values);
        self.0.release(AllocationClass::Scratch, bytes)
    }
    fn invalid(&self, _: &'static str) -> Self::Error {
        AllocationError::Capacity
    }
}

const RETARGET: &str =
    "void set(ref int parameter){parameter=7;}int x=1;int y=2;set(ref y);print(x);print(y);";

#[test]
fn new_reference_exposure_invalidates_unexposed_raw_domain_knowledge() {
    checked(RETARGET, |program| {
        let owner = program.initialization[0];
        let x = cell(&program, "x");
        let y = cell(&program, "y");
        let parameter = cell(&program, "parameter");
        let data = program.unit(owner).unwrap();
        let reference = data
            .call_arguments
            .iter()
            .find_map(|argument| {
                if let CallArgument::Reference(place) = argument {
                    Some(*place)
                } else {
                    None
                }
            })
            .unwrap();
        let mut budget = ledger(20_000_000);
        let uses = UseIndex::build(&program, &mut budget, WorkDomain::Baseline).unwrap();
        let revised = edit(&program, owner, |data| {
            data.places[reference.index()] = Place::Cell(x)
        });
        revised.verify().unwrap();
        let updated = uses
            .updated(&revised, &[owner], &mut budget, WorkDomain::Optional)
            .unwrap();
        assert!(!uses.cell(x).unwrap().reference_exposed());
        assert!(updated.cell(x).unwrap().reference_exposed());
        assert!(uses.cell(y).unwrap().reference_exposed());
        assert!(!updated.cell(y).unwrap().reference_exposed());
        let inputs = DomainInputs::empty().with_execution(JavaScriptExecution::Module);
        {
            let mut allocation = AllocationBudget::new(Some((&mut budget, WorkDomain::Optional)));
            let mut meter = Meter(&mut allocation);
            let roots = [Subject::Cell(x), Subject::Cell(parameter)];
            let old =
                DomainProof::build(&program, &uses, &roots, inputs, &SourceRecipes, &mut meter)
                    .unwrap();
            assert!(old.primitive(Subject::Cell(x), &mut meter).unwrap());
            assert!(
                !old.primitive(Subject::Cell(parameter), &mut meter).unwrap(),
                "formal type grants no alias/domain proof"
            );
            assert!(!old
                .dependencies()
                .valid_for(&revised, &updated, &mut meter)
                .unwrap());
            let next = DomainProof::build(
                &revised,
                &updated,
                &roots,
                inputs,
                &SourceRecipes,
                &mut meter,
            )
            .unwrap();
            assert!(!next.primitive(Subject::Cell(x), &mut meter).unwrap());
            next.discard(&mut meter).unwrap();
            old.discard(&mut meter).unwrap();
            assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
        }
        updated.discard(&mut budget).unwrap();
        uses.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn source_place_publication_retargets_aliases_atomically_and_failed_admission_keeps_owner() {
    for optional_work in [0, 20_000_000] {
        checked(RETARGET, |program| {
            let owner = program.initialization[0];
            let x = cell(&program, "x");
            let y = cell(&program, "y");
            let reference = program
                .unit(owner)
                .unwrap()
                .call_arguments
                .iter()
                .find_map(|argument| {
                    if let CallArgument::Reference(place) = argument {
                        Some(*place)
                    } else {
                        None
                    }
                })
                .unwrap();
            let mut compiler =
                Compilation::new(ledger(optional_work), CheckpointLimit { max_live: 3 }).unwrap();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let view = compiler.view(source).unwrap();
            let revision = view.unit_revision(owner).unwrap();
            let old_x = view.cell_users(x).unwrap().revision();
            let old_y = view.cell_users(y).unwrap().revision();
            let retained = compiler.ledger().retained_bytes();
            let result = compiler.edit_source(
                source,
                &[UnitPatch {
                    unit: owner,
                    expected_revision: revision,
                    operations: &[],
                    places: &[PlacePatch {
                        place: reference,
                        replacement: &Place::Cell(x),
                    }],
                }],
                WorkDomain::Optional,
            );
            if optional_work == 0 {
                assert!(result.is_err());
                assert_eq!(compiler.ledger().retained_bytes(), retained);
                let view = compiler.view(source).unwrap();
                assert_eq!(view.cell_users(x).unwrap().revision(), old_x);
                assert_eq!(view.cell_users(y).unwrap().revision(), old_y);
            } else {
                let next = result.unwrap();
                let view = compiler.view(next).unwrap();
                assert_ne!(view.cell_users(x).unwrap().revision(), old_x);
                assert_ne!(view.cell_users(y).unwrap().revision(), old_y);
                assert!(view.cell_users(x).unwrap().reference_exposed());
                assert!(!view.cell_users(y).unwrap().reference_exposed());
                assert!(!compiler
                    .view(source)
                    .unwrap()
                    .cell_users(x)
                    .unwrap()
                    .reference_exposed());
                compiler.discard(next).unwrap();
            }
            compiler.discard(source).unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
