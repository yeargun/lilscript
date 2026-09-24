//! Semantic evidence tests; executable target/oracle coverage has its own owner.
use super::product_family::*;
use super::uses::UseIndex;
use super::*;
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
        execution: crate::compilation_contract::JavaScriptExecution::Module,
        attempt: AnalysisAttempt {
            plan: PRODUCT_FAMILY_PLAN,
            algorithm_version: PRODUCT_FAMILY_VERSION,
            work_quota: 200_000,
        },
        scratch_bytes: 500_000,
        output_bytes: 500_000,
    }
}
fn named(program: &Program<'_>, name: &str) -> CellId {
    let mut cells = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name && cell.binding == CellBinding::Local);
    let index = cells.next().unwrap().0;
    assert!(cells.next().is_none(), "unique {name}");
    CellId::from_index(index).unwrap()
}
fn complete(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    ledger: &mut BudgetLedger,
) -> ProductFamily {
    let result = analyze(program, uses, cell, request(), ledger, WorkDomain::Optional).unwrap();
    assert_eq!(result.receipt.completion, AnalysisCompletion::Complete);
    match result.outcome {
        FamilyOutcome::Complete(family) => family,
        other => panic!("expected product proof: {other:?}"),
    }
}
fn edited<'src>(
    program: &Program<'src>,
    unit: UnitId,
    edit: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut changed = program.clone();
    let mut working = changed.units[unit.index()].clone().into_working();
    edit(working.get_mut());
    changed.units[unit.index()] = working.freeze();
    changed.verify().unwrap();
    changed
}

#[test]
fn product_copy_component_covers_distinct_cells_and_original_snapshot_occurrences() {
    checked(
        "struct P{int x;int y;}P state=P{1,2};P saved=state;state=P{3,4};P third=(saved=state);state.x=5;saved.y=6;print(third.x);print(state.x);print(saved.x);",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let baseline = ledger.retained_bytes();
            let family = complete(program, &uses, named(program, "saved"), &mut ledger);
            let cells: Vec<_> = family.cells().iter().map(|entry| entry.cell).collect();
            let mut expected = vec![
                named(program, "state"),
                named(program, "saved"),
                named(program, "third"),
            ];
            expected.sort_unstable();
            assert_eq!(cells, expected);
            assert_eq!(family.root(), expected[0]);
            assert_eq!(family.fields().len(), 2);
            let mut snapshots = 0;
            let mut source_store_has_result = false;
            for entry in family.operations() {
                let data = program.unit(entry.operation.unit).unwrap();
                let op = &data.operations[entry.operation.operation.index()];
                match entry.kind {
                    ProductOperationKind::Snapshot { cell, value } => {
                        snapshots += 1;
                        assert_eq!(op.result, Some(value));
                        assert!(
                            matches!(op.kind,OperationKind::Load(place) if data.places[place.index()]==Place::Cell(cell))
                        );
                    }
                    ProductOperationKind::Assign { input, .. } => {
                        assert_eq!(data.operands(op.operands), Some(&[input][..]));
                        source_store_has_result |= op.result.is_some();
                    }
                    ProductOperationKind::Access(access) => {
                        assert_eq!(family.access(entry.operation), Some(&access));
                        assert!((access.slot() as usize) < family.fields().len());
                    }
                    _ => {}
                }
            }
            assert!(snapshots >= 2);
            assert!(
                !source_store_has_result,
                "source assignment expressions reuse their RHS value"
            );
            let owner = program.cells[named(program, "state").index()].owner;
            let data = program.unit(owner).unwrap();
            let saved = named(program, "saved");
            let third = named(program, "third");
            let (store_index,store)=data.operations.iter().enumerate().find(|(_,op)|matches!(op.kind,OperationKind::Store(place) if data.places[place.index()]==Place::Cell(saved))).unwrap();
            let input = data.operands(store.operands).unwrap()[0];
            let third_init = data
                .operations
                .iter()
                .find(|op| matches!(op.kind,OperationKind::Initialize(cell) if cell==third))
                .unwrap();
            let source_result = data.operands(third_init.operands).unwrap()[0];
            let origin = |mut value: ValueId| {
                while matches!(
                    data.operations[data.values[value.index()].definition.index()].kind,
                    OperationKind::CopyValue
                ) {
                    let op = &data.operations[data.values[value.index()].definition.index()];
                    value = data.operands(op.operands).unwrap()[0];
                }
                value
            };
            assert_eq!(
                origin(input),
                origin(source_result),
                "source transfer observes the same immutable RHS, not a subsequent load of saved"
            );
            // The authoritative core makes every Store effect-only. A value
            // result belongs to the existing RHS/CopyValue, even after edits.
            let mut invalid = program.clone();
            let mut working = invalid.units[owner.index()].clone().into_working();
            let unit = working.get_mut();
            let explicit = ValueId::from_index(unit.values.len()).unwrap();
            unit.values.push(Value {
                ty: unit.values[input.index()].ty,
                definition: OpId::from_index(store_index).unwrap(),
            });
            unit.operations[store_index].result = Some(explicit);
            invalid.units[owner.index()] = working.freeze();
            assert!(
                invalid
                    .verify()
                    .unwrap_err()
                    .contains("operation signature mismatch")
            );
            assert!(family.dependencies().valid_for(program, &uses));
            assert_eq!(ledger.retained_bytes(), baseline + family.retained_bytes());
            family.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), baseline);
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn nested_packed_leaves_are_proved_without_flattening_their_independent_storage() {
    checked(
        "struct Inner{int x;int y;}struct Outer{Inner inner;int[] items;}Inner leaf=Inner{1,2};Outer state=Outer{leaf,[3]};Outer saved=state;leaf.x=4;state.inner.y=5;state.items[0]=6;print(saved.inner.y);print(saved.items[0]);",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let family = complete(program, &uses, named(program, "state"), &mut ledger);
            assert_eq!(family.cells().len(), 2);
            assert_eq!(family.fields().len(), 2);
            assert!(
                !family
                    .cells()
                    .iter()
                    .any(|entry| entry.cell == named(program, "leaf"))
            );
            assert!(
                family
                    .dependencies()
                    .cells()
                    .iter()
                    .any(|(cell, _)| *cell == named(program, "leaf")),
                "packed producer storage remains a dependency"
            );
            assert!(family.operations().iter().any(|entry|matches!(entry.kind,ProductOperationKind::Access(access) if access.slot()==0 && matches!(program.unit(entry.operation.unit).unwrap().operations[entry.operation.operation.index()].kind,OperationKind::Store(_)))));
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn captured_product_access_qualifies_all_creator_paths_and_preserves_raw_conversions() {
    for source in [
        "struct P{int x;int y;}export func(int)->int make(int seed){P state=P{seed,0};P saved=state;return (int delta)=>{state.x+=delta;saved.y+=1;return state.x+saved.x+saved.y;};}",
        "struct P{int x;}extern int opaque();extern void keep(func()->void change);P state=P{opaque()};P saved=state;keep(()=>{state=P{7};});auto read=()=>{state.x;return saved.x;};print(read());",
        "struct P{int x;}auto a=()=>0;auto b=()=>0;int i=0;while(i<2){P state=P{i};if(i==0){a=()=>{state.x+=1;return state.x;};}else{b=()=>state.x;}i+=1;}print(a());print(b());",
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let family = complete(program, &uses, named(program, "state"), &mut ledger);
            assert!(!family.dependencies().creators().is_empty());
            for entry in family.operations() {
                if let ProductOperationKind::Access(access) = entry.kind {
                    let data = program.unit(entry.operation.unit).unwrap();
                    if matches!(
                        data.operations[entry.operation.operation.index()].kind,
                        OperationKind::Load(_)
                    ) {
                        assert_eq!(family.access(entry.operation), Some(&access));
                        assert!(
                            data.operations[entry.operation.operation.index()]
                                .result
                                .is_some(),
                            "actual typed read still exists"
                        );
                    }
                }
            }
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn open_product_boundaries_and_unproved_presence_are_semantic_unknowns() {
    for (source, expected) in [
        (
            "struct P{int x;}extern void take(P p);P state=P{1};take(state);",
            UnknownReason::WholeValueUse,
        ),
        (
            "struct P{int x;}JsValue make(){P state=P{1};return state;}auto result=make();",
            UnknownReason::WholeValueUse,
        ),
        (
            "struct P{int x;}extern int run(func()->int read);P state=P{run(()=>state.x)};",
            UnknownReason::EarlyCaptureOrRead,
        ),
        (
            "struct P{int x;}P state=P{1};auto read=()=>{P copy=state;return copy.x;};print(read());",
            UnknownReason::CrossActivationCopy,
        ),
        (
            "struct Inner{int x;}struct P{Inner inner;}extern Inner opaque();P state=P{opaque()};print(state.inner.x);",
            UnknownReason::UnsupportedProducer,
        ),
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let baseline = ledger.retained_bytes();
            let result = analyze(
                program,
                &uses,
                named(program, "state"),
                request(),
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(
                matches!(result.outcome,FamilyOutcome::Unknown(reason) if reason==expected),
                "{source}: {:?}",
                result.outcome
            );
            assert_eq!(result.receipt.completion, AnalysisCompletion::Complete);
            assert_eq!(ledger.retained_bytes(), baseline);
            uses.discard(&mut ledger).unwrap();
        });
    }
}

#[test]
fn product_dependency_stamps_cover_new_copy_consumers_and_unchanged_writer_locations() {
    checked(
        "struct P{int x;}int unrelated(){return 4;}P state=P{1};P other=P{9};print(state.x);",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let state = named(program, "state");
            let other = named(program, "other");
            let family = complete(program, &uses, state, &mut ledger);
            let owner = program.cells[state.index()].owner;
            let unrelated = program
                .cells
                .iter()
                .find_map(|cell| match cell.binding {
                    CellBinding::Function(unit) if cell.name == "unrelated" => Some(unit),
                    _ => None,
                })
                .unwrap();
            let changed = edited(program, unrelated, |unit| {
                unit.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(4))))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Integer(5))
            });
            let updated = uses
                .updated(&changed, &[unrelated], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(family.dependencies().valid_for(&changed, &updated));
            updated.discard(&mut ledger).unwrap();
            let changed = edited(program, owner, |unit| {
                unit.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(1))))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Integer(2))
            });
            let updated = uses
                .updated(&changed, &[owner], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert_eq!(
                uses.cell(state).unwrap().revision(),
                updated.cell(state).unwrap().revision()
            );
            assert!(!family.dependencies().valid_for(&changed, &updated));
            updated.discard(&mut ledger).unwrap();
            let changed = edited(program, owner, |unit| {
                let input = unit
                    .operations
                    .iter()
                    .find_map(|op| {
                        matches!(op.kind,OperationKind::Initialize(cell) if cell==state)
                            .then(|| unit.operands(op.operands).unwrap()[0])
                    })
                    .unwrap();
                let range = unit
                    .operations
                    .iter()
                    .find(|op| matches!(op.kind,OperationKind::Initialize(cell) if cell==other))
                    .unwrap()
                    .operands;
                unit.operands[range.start as usize] = input;
            });
            let updated = uses
                .updated(&changed, &[owner], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(!family.dependencies().valid_for(&changed, &updated));
            let expanded = complete(&changed, &updated, state, &mut ledger);
            assert_eq!(expanded.cells().len(), 2);
            expanded.discard(&mut ledger).unwrap();
            updated.discard(&mut ledger).unwrap();
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn adding_reference_exposure_invalidates_closed_component_evidence() {
    checked(
        "struct P{int x;}void change(ref P value){value.x=8;}P state=P{1};P other=P{2};change(ref other);print(state.x);",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let state = named(program, "state");
            let other = named(program, "other");
            let family = complete(program, &uses, state, &mut ledger);
            let owner = program.cells[state.index()].owner;
            let changed = edited(program, owner, |unit| {
                let reference = unit
                    .call_arguments
                    .iter()
                    .find_map(|argument| match argument {
                        CallArgument::Reference(place) => Some(*place),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(unit.places[reference.index()], Place::Cell(other));
                unit.places[reference.index()] = Place::Cell(state);
            });
            let updated = uses
                .updated(&changed, &[owner], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(updated.cell(state).unwrap().reference_exposed());
            assert!(!family.dependencies().valid_for(&changed, &updated));
            let result = analyze(
                &changed,
                &updated,
                state,
                request(),
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            let FamilyOutcome::Complete(reproved) = result.outcome else {
                panic!("new complete reference closure: {:?}", result.outcome);
            };
            assert!(reproved.dependencies().valid_for(&changed, &updated));
            assert!(reproved.cells().iter().all(|entry| !changed.is_reference_parameter(entry.cell)));
            reproved.discard(&mut ledger).unwrap();
            updated.discard(&mut ledger).unwrap();
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn product_attempt_denial_and_both_evidence_discard_orders_restore_original_domain() {
    checked(
        "struct P{int x;int y;}P state=P{1,2};P saved=state;state.x=3;print(saved.y);",
        |program| {
            for first_index in [false, true] {
                let mut ledger = ledger();
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let baseline = ledger.retained_bytes();
                for limit in [
                    ResourceLimit::Work,
                    ResourceLimit::Scratch,
                    ResourceLimit::Output,
                ] {
                    let mut bounded = request();
                    match limit {
                        ResourceLimit::Work => bounded.attempt.work_quota = 0,
                        ResourceLimit::Scratch => bounded.scratch_bytes = 0,
                        ResourceLimit::Output => bounded.output_bytes = 0,
                    }
                    let result = analyze(
                        program,
                        &uses,
                        named(program, "state"),
                        bounded,
                        &mut ledger,
                        WorkDomain::Optional,
                    )
                    .unwrap();
                    assert!(
                        matches!(result.outcome,FamilyOutcome::Truncated(reason) if reason==limit)
                    );
                    assert_eq!(result.receipt.completion, AnalysisCompletion::Truncated);
                    assert_eq!(ledger.retained_bytes(), baseline);
                }
                let family = complete(program, &uses, named(program, "state"), &mut ledger);
                let retained = family.retained_bytes();
                assert!(retained > 0);
                if first_index {
                    uses.discard(&mut ledger).unwrap();
                    assert_eq!(ledger.retained_bytes(), retained);
                    family.discard(&mut ledger).unwrap();
                } else {
                    family.discard(&mut ledger).unwrap();
                    assert_eq!(ledger.retained_bytes(), baseline);
                    uses.discard(&mut ledger).unwrap();
                }
                assert_eq!(ledger.retained_bytes(), 0);
            }
        },
    );
}

#[test]
fn product_parameter_origins_and_call_return_boundaries_do_not_merge_banks() {
    checked("struct P{int x;}P copy(P argument){P local=argument;local.x=8;return local;}P source=P{1};P result=copy(source);print(result.x);", |program| {
        let mut ledger=ledger();
        let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let source=named(program,"source");
        let caller=complete(program,&uses,source,&mut ledger);
        assert_eq!(caller.cells().iter().map(|c|c.cell).collect::<Vec<_>>(),vec![source]);
        assert!(caller.requires_module());
        let body=program.cells.iter().find_map(|c| match c.binding {CellBinding::Function(body) if c.name=="copy"=>Some(body),_=>None}).unwrap();
        let formal=program.unit(body).unwrap().parameters[0];
        let callee=complete(program,&uses,formal,&mut ledger);
        assert_eq!(callee.cells().len(),2);
        assert!(callee.cells().iter().any(|c|c.cell==formal && c.origin==ProductOrigin::Parameter));
        assert!(callee.cells().iter().all(|c|program.cells[c.cell.index()].owner==body));
        assert!(callee.requires_module());
        assert!(!callee.cells().iter().any(|c|c.cell==source));
        assert!(callee.dependencies().units().iter().any(|(u,_)|*u==program.cells[source.index()].owner));
        // Return transport does not assert actual presence for an opaque call result.
        let incoming=analyze(program,&uses,named(program,"result"),request(),&mut ledger,WorkDomain::Optional).unwrap();
        assert!(matches!(incoming.outcome,FamilyOutcome::Unknown(UnknownReason::UnsupportedProducer)));
        callee.discard(&mut ledger).unwrap();caller.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(),0);
    });
}
