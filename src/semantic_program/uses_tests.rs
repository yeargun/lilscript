use super::uses::*;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use std::sync::Arc;

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, source).unwrap();
    let facts = crate::analyze(&source).unwrap();
    let program = from_checked_source(&source, &facts).unwrap();
    program.verify().unwrap();
    inspect(&program);
}
fn limited(work: u64, memory: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}
fn ledger() -> BudgetLedger {
    limited(10_000_000, 10_000_000)
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
        panic!("function")
    };
    unit
}
fn edited<'src>(
    program: &Program<'src>,
    id: UnitId,
    edit: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut result = program.clone();
    let mut working = result.units[id.index()].clone().into_working();
    edit(working.get_mut());
    result.units[id.index()] = working.freeze();
    result.verify().unwrap();
    result
}
fn same_contents(left: &UseIndex, right: &UseIndex, program: &Program<'_>) {
    assert!(left.valid_for(program));
    assert!(right.valid_for(program));
    for frozen in &program.units {
        let a = left.unit(frozen.id()).unwrap();
        let b = right.unit(frozen.id()).unwrap();
        assert_eq!(a.revision(), b.revision());
        assert_eq!(a.cell_uses(), b.cell_uses());
        assert_eq!(a.closures(), b.closures());
        assert_eq!(
            left.creators(frozen.id()).unwrap().sites(),
            right.creators(frozen.id()).unwrap().sites()
        );
        for index in 0..frozen.data().calls.len() {
            let call = CallId::from_index(index).unwrap();
            assert_eq!(a.call_operation(call), b.call_operation(call));
        }
        assert!(a
            .call_operation(CallId::from_index(frozen.data().calls.len()).unwrap())
            .is_none());
        for index in 0..frozen.data().values.len() {
            let value = ValueId::from_index(index).unwrap();
            assert_eq!(a.value_uses(value), b.value_uses(value));
        }
        assert!(a
            .value_uses(ValueId::from_index(frozen.data().values.len()).unwrap())
            .is_none());
    }
    for index in 0..program.cells.len() {
        let id = CellId::from_index(index).unwrap();
        assert_eq!(
            left.cell(id).unwrap().sites(),
            right.cell(id).unwrap().sites()
        );
    }
}

#[test]
fn uses_include_implicit_place_call_yield_and_capture_occurrences() {
    checked(
        r#"
        extern (func(int,int)->int)[] object();extern int key();
        extern int argument(int value);
        export int exported=1;
        int[] values=[10];values[0]+=argument(2);
        print(object()[key()](argument(2),argument(3)));
        print("abc".charCodeAt(argument(0)));
        func()->int make(int seed){int state=seed;return ()=>state;}
        int result=if(true){1}else{2};
        try{throw 1;}catch(auto error){print(error);}
    "#,
        |program| {
            let mut budget = ledger();
            let index = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
            let (
                mut places,
                mut keys,
                mut callees,
                mut receivers,
                mut yields,
                mut captures,
                mut parameters,
                mut catches,
            ) = (0, 0, 0, 0, 0, 0, 0, 0);
            for frozen in &program.units {
                let unit = frozen.data();
                let uses = index.unit(frozen.id()).unwrap();
                // Explicit assertions against independently selected semantic sites
                // catch omission of roles that a full rebuild would also omit.
                for (op_index, operation) in unit.operations.iter().enumerate() {
                    let op = OpId::from_index(op_index).unwrap();
                    let mut check_place = |place: PlaceId| {
                        if let Place::Index { receiver, key } = unit.places[place.index()] {
                            assert!(uses.value_uses(receiver).unwrap().contains(
                                &ValueUse::PlaceReceiver {
                                    operation: op,
                                    place
                                }
                            ));
                            assert!(uses.value_uses(key).unwrap().contains(&ValueUse::PlaceKey {
                                operation: op,
                                place
                            }));
                            places += 1;
                            keys += 1;
                        }
                    };
                    match operation.kind {
                        OperationKind::Closure(body) => {
                            assert!(index.creators(body).unwrap().sites().contains(
                                &ClosureCreation {
                                    unit: frozen.id(),
                                    operation: op,
                                }
                            ));
                        }
                        OperationKind::Load(place) | OperationKind::Store(place) => {
                            check_place(place)
                        }
                        OperationKind::PrepareCall(call) => match unit.calls[call.index()].target {
                            CallTarget::Reference { place } => check_place(place),
                            CallTarget::Value { callee, .. } => {
                                assert!(uses
                                    .value_uses(callee)
                                    .unwrap()
                                    .contains(&ValueUse::CallCallee { prepare: op, call }));
                                callees += 1;
                            }
                            CallTarget::Intrinsic {
                                receiver: Some(receiver),
                                ..
                            } => {
                                assert!(uses
                                    .value_uses(receiver)
                                    .unwrap()
                                    .contains(&ValueUse::CallReceiver { prepare: op, call }));
                                receivers += 1;
                            }
                            _ => {}
                        },
                        OperationKind::Try {
                            catch: Some((Some(cell), region)),
                            ..
                        } => {
                            assert!(uses.cell_uses().contains(&CellReference {
                                cell,
                                usage: CellUse::CatchBinding {
                                    operation: op,
                                    region
                                }
                            }));
                            catches += 1;
                        }
                        _ => {}
                    }
                }
                for (region, data) in unit.regions.iter().enumerate() {
                    if let Some(value) = data.result {
                        assert!(uses
                            .value_uses(value)
                            .unwrap()
                            .contains(&ValueUse::RegionResult(
                                RegionId::from_index(region).unwrap()
                            )));
                        yields += 1;
                    }
                }
                for (position, &cell) in unit.parameters.iter().enumerate() {
                    assert!(uses.cell_uses().contains(&CellReference {
                        cell,
                        usage: CellUse::Parameter(position as u32)
                    }));
                    parameters += 1;
                }
                for &cell in &unit.captures {
                    assert!(uses.cell_uses().contains(&CellReference {
                        cell,
                        usage: CellUse::Capture
                    }));
                    assert!(index
                        .cell(cell)
                        .unwrap()
                        .sites()
                        .contains(&CellUseSite::Unit {
                            unit: frozen.id(),
                            usage: CellUse::Capture
                        }));
                    captures += 1;
                }
            }
            assert!(
                places >= 3
                    && keys >= 3
                    && callees > 0
                    && receivers > 0
                    && yields >= 2
                    && captures > 0
                    && parameters > 0
                    && catches == 1
            );
            let exported = cell(program, "exported");
            assert!(index
                .cell(exported)
                .unwrap()
                .sites()
                .contains(&CellUseSite::Export { index: 0 }));
            assert_eq!(budget.retained_bytes(), index.retained_bytes());
            index.discard(&mut budget).unwrap();
            assert_eq!(budget.retained_bytes(), 0);
        },
    );
}

const CREATOR_SOURCE: &str = "int first(){return 1;}int second(){return 2;}int untouched(){return 3;}int maker(){auto local=()=>4;return 0;}";

fn add_creation(unit: &mut UnitData, body: UnitId) -> OpId {
    let template = unit
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::Closure(_)))
        .unwrap()
        .clone();
    let operation = OpId::from_index(unit.operations.len()).unwrap();
    let value = ValueId::from_index(unit.values.len()).unwrap();
    unit.values.push(Value {
        ty: unit.values[template.result.unwrap().index()].ty,
        definition: operation,
    });
    let region = template.region;
    unit.operations.push(Operation {
        kind: OperationKind::Closure(body),
        result: Some(value),
        ..template
    });
    // The new semantic result has no consumer. A complete creator index must
    // nevertheless include it, independently of later demand decisions.
    unit.regions[region.index()].operations.insert(0, operation);
    operation
}

#[test]
fn a_new_creator_invalidates_unchanged_body_knowledge_and_preserves_other_sets() {
    checked(CREATOR_SOURCE, |program| {
        let first = function(program, "first");
        let maker = function(program, "maker");
        let untouched = function(program, "untouched");
        let mut creation = None;
        let changed = edited(program, maker, |unit| {
            creation = Some(add_creation(unit, first))
        });
        assert_eq!(
            program.units[first.index()].revision(),
            changed.units[first.index()].revision()
        );
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let baseline = budget.retained_bytes();
        let next = base
            .updated(&changed, &[maker], &mut budget, WorkDomain::Optional)
            .unwrap();
        assert!(base.valid_for(program));
        assert!(!base.valid_for(&changed));
        assert!(next.valid_for(&changed));
        assert_ne!(
            base.creators(first).unwrap().revision(),
            next.creators(first).unwrap().revision()
        );
        assert_eq!(base.creators(first).unwrap().sites().len(), 1);
        assert_eq!(next.creators(first).unwrap().sites().len(), 2);
        assert!(next
            .creators(first)
            .unwrap()
            .sites()
            .contains(&ClosureCreation {
                unit: maker,
                operation: creation.unwrap()
            }));
        assert_eq!(
            base.creators(untouched).unwrap().revision(),
            next.creators(untouched).unwrap().revision()
        );
        assert!(std::ptr::eq(
            base.creators(untouched).unwrap().sites(),
            next.creators(untouched).unwrap().sites()
        ));
        assert_eq!(next.receipt().rebuilt_creator_sets, 1);
        assert_eq!(
            budget.retained_bytes(),
            baseline + next.receipt().allocated_bytes
        );
        let rebuilt = UseIndex::build(&changed, &mut budget, WorkDomain::Optional).unwrap();
        same_contents(&next, &rebuilt, &changed);
        rebuilt.discard(&mut budget).unwrap();
        base.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), next.retained_bytes());
        next.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn redirected_and_removed_creators_refresh_only_old_and_new_body_sets() {
    checked(CREATOR_SOURCE, |program| {
        let maker = function(program, "maker");
        let second = function(program, "second");
        let first = function(program, "first");
        let (operation, previous) = program
            .unit(maker)
            .unwrap()
            .operations
            .iter()
            .enumerate()
            .find_map(|(index, op)| match op.kind {
                OperationKind::Closure(body) => Some((OpId::from_index(index).unwrap(), body)),
                _ => None,
            })
            .unwrap();
        let changed = edited(program, maker, |unit| {
            unit.operations[operation.index()].kind = OperationKind::Closure(second)
        });
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        assert_eq!(
            base.creators(previous).unwrap().sites(),
            &[ClosureCreation {
                unit: maker,
                operation
            }]
        );
        let next = base
            .updated(&changed, &[maker], &mut budget, WorkDomain::Optional)
            .unwrap();
        assert!(next.creators(previous).unwrap().sites().is_empty());
        for body in [previous, second] {
            assert_ne!(
                base.creators(body).unwrap().revision(),
                next.creators(body).unwrap().revision()
            );
        }
        assert_eq!(next.creators(second).unwrap().sites().len(), 2);
        assert!(next
            .creators(second)
            .unwrap()
            .sites()
            .contains(&ClosureCreation {
                unit: maker,
                operation
            }));
        assert_eq!(
            base.creators(first).unwrap().revision(),
            next.creators(first).unwrap().revision()
        );
        assert_eq!(next.receipt().rebuilt_creator_sets, 2);
        let rebuilt = UseIndex::build(&changed, &mut budget, WorkDomain::Optional).unwrap();
        same_contents(&next, &rebuilt, &changed);
        rebuilt.discard(&mut budget).unwrap();
        next.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), base.retained_bytes());
        base.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn unchanged_creator_locations_keep_their_stamp_but_require_current_unit_revisions() {
    checked(CREATOR_SOURCE, |program| {
        let maker = function(program, "maker");
        let body = program
            .unit(maker)
            .unwrap()
            .operations
            .iter()
            .find_map(|op| match op.kind {
                OperationKind::Closure(body) => Some(body),
                _ => None,
            })
            .unwrap();
        let changed = edited(program, maker, |unit| {
            unit.operations
                .iter_mut()
                .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(0))))
                .unwrap()
                .kind = OperationKind::Constant(Constant::Integer(9));
        });
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let retained = budget.retained_bytes();
        assert!(
            matches!(base.updated(&changed, &[], &mut budget, WorkDomain::Optional), Err(UseError::UndeclaredUnitChange(unit)) if unit == maker)
        );
        assert_eq!(budget.retained_bytes(), retained);
        let next = base
            .updated(&changed, &[maker], &mut budget, WorkDomain::Optional)
            .unwrap();
        assert!(!base.valid_for(&changed));
        assert!(next.valid_for(&changed));
        assert_ne!(
            base.unit(maker).unwrap().revision(),
            next.unit(maker).unwrap().revision()
        );
        assert_eq!(
            base.creators(body).unwrap().revision(),
            next.creators(body).unwrap().revision()
        );
        assert!(std::ptr::eq(
            base.creators(body).unwrap().sites(),
            next.creators(body).unwrap().sites()
        ));
        assert_eq!(next.receipt().rebuilt_creator_sets, 0);
        next.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), base.retained_bytes());
        base.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn creator_update_denial_releases_new_chunks_in_their_original_domain() {
    checked(CREATOR_SOURCE, |program| {
        let first = function(program, "first");
        let maker = function(program, "maker");
        let changed = edited(program, maker, |unit| {
            add_creation(unit, first);
        });
        let mut measuring = ledger();
        let base = UseIndex::build(program, &mut measuring, WorkDomain::Baseline).unwrap();
        let baseline_peak = measuring.peak_retained_bytes();
        let next = base
            .updated(&changed, &[maker], &mut measuring, WorkDomain::Optional)
            .unwrap();
        let work = next.receipt().logical_work;
        let peak = measuring.peak_retained_bytes();
        assert!(peak > baseline_peak);
        next.discard(&mut measuring).unwrap();
        base.discard(&mut measuring).unwrap();
        for (quota, memory) in [(work - 1, 10_000_000), (10_000_000, peak - 1)] {
            let mut budget = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 10_000_000,
                    optional_work: quota,
                    baseline_retained_bytes: 0,
                    retained_bytes: memory,
                },
            )
            .unwrap();
            let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
            let before = budget.retained_bytes();
            assert!(matches!(
                base.updated(&changed, &[maker], &mut budget, WorkDomain::Optional),
                Err(UseError::Budget(_))
            ));
            assert_eq!(budget.retained_bytes(), before);
            assert!(base.valid_for(program));
            assert_eq!(base.creators(first).unwrap().sites().len(), 1);
            base.discard(&mut budget).unwrap();
            assert_eq!(budget.retained_bytes(), 0);
        }
    });
}

#[test]
fn unrelated_unit_edit_reuses_local_storage_and_cell_absence_stamps() {
    checked("Record<int> state=record{x:1};int read(){return state.x??0;}int unrelated(){return 0;}print(read());", |program| {
        let other = function(program, "unrelated");
        let changed = edited(program, other, |unit| {
            unit.operations.iter_mut().find(|operation| matches!(operation.kind, OperationKind::Constant(Constant::Integer(0)))).unwrap().kind = OperationKind::Constant(Constant::Integer(9));
        });
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let next = base.updated(&changed, &[other], &mut budget, WorkDomain::Optional).unwrap();
        assert_eq!(next.receipt().rebuilt_units, 1);
        assert_eq!(next.receipt().rebuilt_cell_sets, 0);
        let reader = function(program, "read");
        assert!(std::ptr::eq(base.unit(reader).unwrap(), next.unit(reader).unwrap()));
        assert!(!std::ptr::eq(base.unit(other).unwrap(), next.unit(other).unwrap()));
        assert_eq!(base.cell(cell(program, "state")).unwrap().revision(), next.cell(cell(program, "state")).unwrap().revision());
        let rebuilt = UseIndex::build(&changed, &mut budget, WorkDomain::Optional).unwrap();
        same_contents(&next, &rebuilt, &changed);
        rebuilt.discard(&mut budget).unwrap();
        base.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), next.retained_bytes());
        next.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

const ESCAPE_SOURCE: &str = "extern void consume(Record<int> value);Record<int> state=record{x:1};int read(){return state.x??0;}int other(){consume(record{x:0});return 0;}print(read());";

fn add_escape(program: &Program<'_>, unit: &mut UnitData) {
    let state = cell(program, "state");
    unit.captures.push(state);
    let place = PlaceId::from_index(unit.places.len()).unwrap();
    unit.places.push(Place::Cell(state));
    let operation = unit
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Allocate {
                    kind: AllocationKind::Record(_),
                    ..
                }
            )
        })
        .unwrap();
    operation.kind = OperationKind::Load(place);
    operation.operands.len = 0;
}

#[test]
fn unseen_consumer_invalidates_state_use_set_without_changing_producer() {
    checked(ESCAPE_SOURCE, |program| {
        let other = function(program, "other");
        let changed = edited(program, other, |unit| add_escape(program, unit));
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let base_bytes = budget.retained_bytes();
        let next = base
            .updated(&changed, &[other], &mut budget, WorkDomain::Optional)
            .unwrap();
        assert_eq!(
            budget.retained_bytes(),
            base_bytes + next.receipt().allocated_bytes
        );
        let state = cell(program, "state");
        assert_ne!(
            base.cell(state).unwrap().revision(),
            next.cell(state).unwrap().revision()
        );
        assert!(!base
            .cell(state)
            .unwrap()
            .sites()
            .iter()
            .any(|site| matches!(site, CellUseSite::Unit { unit, .. } if *unit == other)));
        assert!(next
            .cell(state)
            .unwrap()
            .sites()
            .contains(&CellUseSite::Unit {
                unit: other,
                usage: CellUse::Capture
            }));
        assert!(next.cell(state).unwrap().sites().iter().any(|site| matches!(site, CellUseSite::Unit { unit, usage: CellUse::Read { .. } } if *unit == other)));
        let owner = program.cells[state.index()].owner;
        assert!(std::ptr::eq(
            base.unit(owner).unwrap(),
            next.unit(owner).unwrap()
        ));
        let escaped = changed
            .unit(other)
            .unwrap()
            .operations
            .iter()
            .find_map(|operation| {
                if let OperationKind::Load(place) = operation.kind {
                    (changed.unit(other).unwrap().places[place.index()] == Place::Cell(state))
                        .then_some(operation.result.unwrap())
                } else {
                    None
                }
            })
            .unwrap();
        assert!(next.unit(other).unwrap().value_uses(escaped).unwrap().iter().any(|usage| matches!(usage, ValueUse::CallArgument { operation, .. } if matches!(changed.unit(other).unwrap().operations[operation.index()].kind, OperationKind::Call(_)))));
        let rebuilt = UseIndex::build(&changed, &mut budget, WorkDomain::Optional).unwrap();
        same_contents(&next, &rebuilt, &changed);
        rebuilt.discard(&mut budget).unwrap();
        next.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), base.retained_bytes());
        base.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn appended_unit_and_new_public_reference_refresh_negative_use_knowledge() {
    checked(ESCAPE_SOURCE, |program| {
        let mut changed = program.clone();
        let new = UnitId::from_index(changed.units.len()).unwrap();
        let mut data = program.unit(function(program, "other")).unwrap().clone();
        add_escape(program, &mut data);
        changed.units.push(WorkingUnit::new(new, data).freeze());
        changed.verify().unwrap();
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let appended = base
            .updated(&changed, &[new], &mut budget, WorkDomain::Optional)
            .unwrap();
        let state = cell(program, "state");
        assert_ne!(
            base.cell(state).unwrap().revision(),
            appended.cell(state).unwrap().revision()
        );
        Arc::make_mut(&mut changed.exports).push(Export {
            name: "state".into(),
            target: InterfaceTarget::Value(state),
        });
        Arc::make_mut(&mut changed.modules)[changed.entry.index()]
            .exports
            .end += 1;
        changed.tables_revision = RevisionId::fresh();
        changed.verify().unwrap();
        let exported = appended
            .updated(&changed, &[], &mut budget, WorkDomain::Optional)
            .unwrap();
        assert_ne!(
            appended.cell(state).unwrap().revision(),
            exported.cell(state).unwrap().revision()
        );
        assert!(exported
            .cell(state)
            .unwrap()
            .sites()
            .contains(&CellUseSite::Export { index: 0 }));
        assert_eq!(exported.receipt().rebuilt_units, 0);
        let rebuilt = UseIndex::build(&changed, &mut budget, WorkDomain::Optional).unwrap();
        same_contents(&exported, &rebuilt, &changed);
        rebuilt.discard(&mut budget).unwrap();
        base.discard(&mut budget).unwrap();
        appended.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), exported.retained_bytes());
        exported.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn incomplete_change_lists_and_unstamped_exports_are_rejected_without_leaks() {
    checked(ESCAPE_SOURCE, |program| {
        let other = function(program, "other");
        let changed = edited(program, other, |unit| add_escape(program, unit));
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let retained = budget.retained_bytes();
        assert_eq!(
            base.updated(&changed, &[], &mut budget, WorkDomain::Optional)
                .unwrap_err(),
            UseError::UndeclaredUnitChange(other)
        );
        assert_eq!(
            base.updated(&changed, &[other, other], &mut budget, WorkDomain::Optional)
                .unwrap_err(),
            UseError::InvalidChangeSet
        );
        let mut stale_tables = program.clone();
        Arc::make_mut(&mut stale_tables.exports).push(Export {
            name: "state".into(),
            target: InterfaceTarget::Value(cell(program, "state")),
        });
        Arc::make_mut(&mut stale_tables.modules)[stale_tables.entry.index()]
            .exports
            .end += 1;
        assert_eq!(
            base.updated(&stale_tables, &[], &mut budget, WorkDomain::Optional)
                .unwrap_err(),
            UseError::UnstampedTables
        );
        assert_eq!(budget.retained_bytes(), retained);
        assert!(base.valid_for(program));
        base.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn construction_failures_charge_work_and_release_all_private_storage() {
    checked(ESCAPE_SOURCE, |program| {
        let mut full_budget = ledger();
        let complete = UseIndex::build(program, &mut full_budget, WorkDomain::Baseline).unwrap();
        let work = complete.receipt().logical_work;
        let memory = full_budget.peak_retained_bytes();
        complete.discard(&mut full_budget).unwrap();
        for quota in [0, 1, work / 4, work / 2, work - 1] {
            let mut budget = limited(quota, 10_000_000);
            assert!(matches!(
                UseIndex::build(program, &mut budget, WorkDomain::Baseline),
                Err(UseError::Budget(_))
            ));
            assert_eq!(budget.retained_bytes(), 0, "work quota {quota}");
            assert!(budget.work_used(WorkDomain::Baseline) <= quota);
        }
        for quota in [0, 1, memory / 4, memory / 2, memory - 1] {
            let mut budget = limited(10_000_000, quota);
            assert!(matches!(
                UseIndex::build(program, &mut budget, WorkDomain::Baseline),
                Err(UseError::Budget(_))
            ));
            assert_eq!(budget.retained_bytes(), 0, "memory quota {quota}");
            assert!(budget.peak_retained_bytes() <= quota);
        }
    });
}

#[test]
fn failed_incremental_construction_preserves_shared_chunks_and_base() {
    checked(ESCAPE_SOURCE, |program| {
        let other = function(program, "other");
        let changed = edited(program, other, |unit| add_escape(program, unit));
        let mut measuring = ledger();
        let base = UseIndex::build(program, &mut measuring, WorkDomain::Baseline).unwrap();
        let baseline_peak = measuring.peak_retained_bytes();
        let next = base
            .updated(&changed, &[other], &mut measuring, WorkDomain::Optional)
            .unwrap();
        let work = next.receipt().logical_work;
        let update_peak = measuring.peak_retained_bytes();
        assert!(update_peak > baseline_peak);
        next.discard(&mut measuring).unwrap();
        base.discard(&mut measuring).unwrap();
        for optional_work in [0, 1, work / 4, work / 2, work - 1] {
            let mut budget = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 10_000_000,
                    optional_work,
                    baseline_retained_bytes: 0,
                    retained_bytes: 10_000_000,
                },
            )
            .unwrap();
            let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
            let retained = budget.retained_bytes();
            assert!(matches!(
                base.updated(&changed, &[other], &mut budget, WorkDomain::Optional),
                Err(UseError::Budget(_))
            ));
            assert_eq!(
                budget.retained_bytes(),
                retained,
                "optional work {optional_work}"
            );
            assert!(base.valid_for(program));
            base.discard(&mut budget).unwrap();
            assert_eq!(budget.retained_bytes(), 0);
        }
        // Every quota admits the complete baseline. The last quota fails at
        // the update's peak after building private unit/cell chunks, rather
        // than merely rejecting its initial snapshot reservation.
        for quota in [
            baseline_peak,
            (baseline_peak + update_peak) / 2,
            update_peak - 1,
        ] {
            let mut budget = limited(10_000_000, quota);
            let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
            let retained = budget.retained_bytes();
            assert!(matches!(
                base.updated(&changed, &[other], &mut budget, WorkDomain::Optional),
                Err(UseError::Budget(
                    crate::compilation_policy::BudgetError::MemoryExhausted(_)
                ))
            ));
            assert_eq!(
                budget.retained_bytes(),
                retained,
                "incremental memory quota {quota}"
            );
            assert!(budget.peak_retained_bytes() <= quota);
            assert!(base.valid_for(program));
            base.discard(&mut budget).unwrap();
            assert_eq!(budget.retained_bytes(), 0);
        }
    });
}

const NESTED_CALL_SOURCE: &str = "extern int tick(int value);int run(){return tick(tick(1))+tick(2);}int other(){return tick(3);}";

#[test]
fn call_operations_follow_invocation_identity_across_nested_calls_and_incremental_renumbering() {
    checked(NESTED_CALL_SOURCE, |program| {
        let run = function(program, "run");
        let other = function(program, "other");
        let first = CallId::from_index(0).unwrap();
        let second = CallId::from_index(1).unwrap();
        let changed = edited(program, run, |unit| {
            assert!(unit.calls.len() >= 3);
            // Rename two call identities consistently. Operation/region/value
            // identities and all evaluation schedules remain unchanged.
            unit.calls.swap(first.index(), second.index());
            for operation in &mut unit.operations {
                match &mut operation.kind {
                    OperationKind::PrepareCall(call)
                    | OperationKind::Call(call)
                    | OperationKind::PrepareReference { call, .. } => {
                        if *call == first {
                            *call = second;
                        } else if *call == second {
                            *call = first;
                        }
                    }
                    _ => {}
                }
            }
        });
        for oldest_first in [false, true] {
            let mut budget = ledger();
            let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
            // Independently enumerate actual operations. In particular outer
            // call0 is invoked after inner call1, so neither arena position nor
            // preparation order can stand in for the reverse relationship.
            for frozen in &program.units {
                let uses = base.unit(frozen.id()).unwrap();
                for (index, operation) in frozen.data().operations.iter().enumerate() {
                    if let OperationKind::Call(call) = operation.kind {
                        assert_eq!(uses.call_operation(call), OpId::from_index(index));
                    }
                }
                assert_eq!(
                    uses.call_operation(CallId::from_index(frozen.data().calls.len()).unwrap()),
                    None
                );
            }
            let first_op = base.unit(run).unwrap().call_operation(first).unwrap();
            let second_op = base.unit(run).unwrap().call_operation(second).unwrap();
            assert!(first_op.index() > second_op.index());
            let before = budget.retained_bytes();
            let updated = base
                .updated(&changed, &[run], &mut budget, WorkDomain::Optional)
                .unwrap();
            assert_eq!(
                budget.retained_bytes(),
                before + updated.receipt().allocated_bytes
            );
            assert_eq!(updated.receipt().rebuilt_units, 1);
            assert!(std::ptr::eq(
                base.unit(other).unwrap(),
                updated.unit(other).unwrap()
            ));
            assert_eq!(
                updated.unit(run).unwrap().call_operation(first),
                Some(second_op)
            );
            assert_eq!(
                updated.unit(run).unwrap().call_operation(second),
                Some(first_op)
            );
            assert_eq!(
                base.unit(run).unwrap().call_operation(first),
                Some(first_op)
            );
            let rebuilt = UseIndex::build(&changed, &mut budget, WorkDomain::Optional).unwrap();
            same_contents(&updated, &rebuilt, &changed);
            rebuilt.discard(&mut budget).unwrap();
            if oldest_first {
                base.discard(&mut budget).unwrap();
                assert_eq!(budget.retained_bytes(), updated.retained_bytes());
                updated.discard(&mut budget).unwrap();
            } else {
                updated.discard(&mut budget).unwrap();
                assert_eq!(budget.retained_bytes(), base.retained_bytes());
                base.discard(&mut budget).unwrap();
            }
            assert_eq!(budget.retained_bytes(), 0);
        }
    });
}

#[test]
fn invalid_call_mapping_is_rejected_without_leaking_partial_unit_index() {
    checked(NESTED_CALL_SOURCE, |program| {
        let run = function(program, "run");
        let mut budget = ledger();
        let base = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let retained = budget.retained_bytes();
        for (mutation, reason) in [
            (0, "duplicate call operation"),
            (1, "missing call operation"),
            (2, "call operation target"),
        ] {
            // Exercise the index's own malformed-input boundary; this input
            // deliberately does not pass the semantic verifier.
            let mut changed = program.clone();
            let mut working = changed.units[run.index()].clone().into_working();
            let unit = working.get_mut();
            let first = unit
                .operations
                .iter()
                .find_map(|operation| match operation.kind {
                    OperationKind::Call(call) => Some(call),
                    _ => None,
                })
                .unwrap();
            let dangling = CallId::from_index(unit.calls.len()).unwrap();
            let operation = unit
                .operations
                .iter_mut()
                .filter(|operation| matches!(operation.kind, OperationKind::Call(_)))
                .nth(1)
                .unwrap();
            operation.kind = match mutation {
                0 => OperationKind::Call(first),
                1 => OperationKind::Constant(Constant::Integer(0)),
                _ => OperationKind::Call(dangling),
            };
            changed.units[run.index()] = working.freeze();
            assert_eq!(
                base.updated(&changed, &[run], &mut budget, WorkDomain::Optional)
                    .unwrap_err(),
                UseError::InvalidProgram(reason)
            );
            assert_eq!(budget.retained_bytes(), retained, "{reason}");
            assert!(base.valid_for(program));
        }
        base.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}
