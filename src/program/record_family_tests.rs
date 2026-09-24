use super::record_family::*;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&source).unwrap();
    let program = from_checked_source(&source, &semantics).unwrap();
    inspect(&program);
}
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}
fn request() -> FamilyRequest {
    FamilyRequest {
        attempt: AnalysisAttempt {
            plan: RECORD_FAMILY_PLAN,
            algorithm_version: RECORD_FAMILY_VERSION,
            work_quota: 100_000,
        },
        scratch_bytes: 100_000,
        output_bytes: 100_000,
    }
}
fn state(program: &Program<'_>) -> CellId {
    CellId::from_index(
        program
            .cells
            .iter()
            .enumerate()
            .find_map(|(index, cell)| {
                let id = CellId::from_index(index).unwrap();
                (cell.name == "state"
                    && program.unit(cell.owner).unwrap().operations.iter().any(
                        |operation| matches!(operation.kind, OperationKind::Initialize(cell) if cell == id),
                    ))
                .then_some(index)
            })
            .unwrap(),
    )
    .unwrap()
}
fn complete(program: &Program<'_>, uses: &UseIndex, ledger: &mut BudgetLedger) -> RecordFamily {
    let analysis = analyze(
        program,
        uses,
        state(program),
        request(),
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    assert_eq!(analysis.receipt.completion, AnalysisCompletion::Complete);
    match analysis.outcome {
        FamilyOutcome::Complete(family) => family,
        other => panic!("expected complete family: {other:?}"),
    }
}

#[test]
fn captured_record_families_cover_sharing_reentry_finally_and_fresh_loop_activation() {
    // These are the same source shapes as the independently executed frozen
    // baseline cases. This test checks semantic evidence, not emitted JS.
    for source in [
        r#"extern void keep(func()->int read);extern int saved(int which);
        func()->int make(int seed){Record<int> state=record{count:seed};keep(()=>state.count??0);return ()=>{state.count=(state.count??0)+1;return state.count??0;};}
        auto a=make(2);auto b=make(9);print(a());print(saved(0));print(saved(1));print(b());print(a());"#,
        r#"extern void visit(func()->int read,func()->void change);extern int saved();
        func()->int make(){Record<int> state=record{x:0};return ()=>{try{state.x=1;visit(()=>state.x??0,()=>{state.x=7;});return state.x??0;}finally{state.x=(state.x??0)+10;}};}
        auto step=make();print(step());print(saved());try{print(step());}catch(auto error){print(99);}print(saved());"#,
        r#"auto a=()=>0;auto b=()=>0;int i=0;while(i<2){Record<int> state=record{x:i};if(i==0){a=()=>{state.x=(state.x??0)+1;return state.x??0;};}else{b=()=>{state.x=(state.x??0)+1;return state.x??0;};}i+=1;}print(a());print(b());print(a());"#,
    ] {
        checked(source, |program| {
            let before: Vec<_> = program
                .units
                .iter()
                .map(|unit| (unit.revision(), unit.data().operations.len()))
                .collect();
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let baseline = ledger.retained_bytes();
            let family = complete(program, &uses, &mut ledger);
            assert!(family.dependencies().valid_for(program, &uses));
            assert!(!family.captures().is_empty());
            assert!(family
                .projections()
                .iter()
                .any(|projection| projection.kind == ReadOrWrite::Write));
            assert!(family
                .projections()
                .iter()
                .any(|projection| projection.kind == ReadOrWrite::Read));
            let cell = &program.cells[state(program).index()];
            assert_eq!(
                family.activation(),
                Activation {
                    unit: cell.owner,
                    region: cell.region
                }
            );
            let owner = program.unit(cell.owner).unwrap();
            assert!(
                matches!(owner.operations[family.allocation().operation.index()].kind,
                OperationKind::Allocate { kind: AllocationKind::Record(_), identity } if identity == family.root().allocation)
            );
            assert!(
                matches!(owner.operations[family.initialize().operation.index()].kind,
                OperationKind::Initialize(id) if id == state(program))
            );
            for &handle in family.handle_loads() {
                assert!(matches!(
                    program.unit(handle.unit).unwrap().operations[handle.operation.index()].kind,
                    OperationKind::Load(_)
                ));
            }
            assert_eq!(
                before,
                program
                    .units
                    .iter()
                    .map(|unit| (unit.revision(), unit.data().operations.len()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(ledger.retained_bytes(), baseline + family.retained_bytes());
            family.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), baseline);
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn unsupported_record_uses_and_early_reentry_have_explicit_unknown_results() {
    for (source, expected) in [
        (
            r#"extern string key();func()->int make(){Record<int> state=record{x:1};return ()=>state[key()]??0;}print(make()());"#,
            UnknownReason::DynamicKey,
        ),
        (
            r#"extern void mutate(Record<int> state);func()->int make(){Record<int> state=record{x:1};mutate(state);return ()=>state.x??0;}print(make()());"#,
            UnknownReason::WholeValueUse,
        ),
        (
            r#"func()->int make(){Record<int> state=record{x:1};Record<int> other=state;return ()=>{other.x=9;return state.x??0;};}print(make()());"#,
            UnknownReason::WholeValueUse,
        ),
        (
            r#"func()->int make(){Record<int> state=record{x:1};return ()=>{state=record{x:9};return state.x??0;};}print(make()());"#,
            UnknownReason::Reassigned,
        ),
        (
            r#"extern int invoke(func()->int read);func()->int make(){Record<int> state=record{x:invoke(()=>state.x??0)};return ()=>state.x??0;}try{print(make()());}catch(auto error){print(99);}"#,
            UnknownReason::EarlyCaptureOrRead,
        ),
        (
            r#"export Record<int> state=record{x:1};print(state.x??0);"#,
            UnknownReason::WholeValueUse,
        ),
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let baseline = ledger.retained_bytes();
            let result = analyze(
                program,
                &uses,
                state(program),
                request(),
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(
                matches!(result.outcome, FamilyOutcome::Unknown(reason) if reason == expected),
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
fn missing_special_keys_and_throwing_unused_initializers_keep_semantic_producers() {
    for source in [
        r#"func()->int make(){Record<int> state=record{__proto__:4,"quoted-key":6};return ()=>{print(state.missing==null);state.missing=7;return (state["__proto__"]??0)+(state["quoted-key"]??0);};}print(make()());"#,
        r#"extern int value(int n);func()->int make(){Record<int> state=record{used:value(1),unused:value(2)};return ()=>state.used??0;}try{print(make()());}catch(auto error){print(99);}"#,
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let family = complete(program, &uses, &mut ledger);
            let slot = |name: &str| {
                family.slots().iter().find(|slot| {
                    program.strings[slot.key.index()]
                        .code_units()
                        .eq(name.encode_utf16())
                })
            };
            if source.contains("missing") {
                assert_eq!(slot("missing").unwrap().initial_value, None);
                assert!(slot("__proto__").unwrap().initial_value.is_some());
                assert!(slot("quoted-key").unwrap().initial_value.is_some());
                let missing = family
                    .slots()
                    .iter()
                    .position(|field| field.key == slot("missing").unwrap().key)
                    .unwrap();
                assert!(family
                    .projections()
                    .iter()
                    .any(|use_| use_.slot as usize == missing && use_.kind == ReadOrWrite::Write));
            } else {
                // The first recipe keeps the union of initialized/observed
                // slots. Later storage pruning may omit this unused slot, but
                // it must never omit the original throwing initializer call.
                let unused = slot("unused").unwrap();
                let owner = program.unit(family.root().unit).unwrap();
                let value = unused.initial_value.unwrap();
                assert!(matches!(
                    owner.operations[owner.values[value.index()].definition.index()].kind,
                    OperationKind::Call(_)
                ));
                assert_eq!(
                    owner
                        .operations
                        .iter()
                        .filter(|op| matches!(op.kind, OperationKind::Call(_)))
                        .count(),
                    2
                );
            }
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
        });
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
fn unchanged_use_set_does_not_hide_changed_consumer_and_unrelated_facts_survive() {
    checked(
        r#"func()->int make(){Record<int> state=record{x:1,y:2};return ()=>state.x??0;}int unrelated(){return 4;}print(make()());"#,
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let family = complete(program, &uses, &mut ledger);
            let unrelated = program
                .cells
                .iter()
                .find_map(|cell| match cell.binding {
                    CellBinding::Function(unit) if cell.name == "unrelated" => Some(unit),
                    _ => None,
                })
                .unwrap();
            assert!(!family
                .dependencies()
                .units()
                .iter()
                .any(|(id, _)| *id == unrelated));
            let changed = edited(program, unrelated, |unit| {
                let op = unit
                    .operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(4))))
                    .unwrap();
                op.kind = OperationKind::Constant(Constant::Integer(5));
            });
            let updated = uses
                .updated(&changed, &[unrelated], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(family.dependencies().valid_for(&changed, &updated));
            updated.discard(&mut ledger).unwrap();

            let consumer = family.captures()[0];
            let y = StringId::from_index(
                program
                    .strings
                    .iter()
                    .position(|value| value.code_units().eq("y".encode_utf16()))
                    .unwrap(),
            )
            .unwrap();
            let changed = edited(program, consumer, |unit| {
                let place = unit
                    .places
                    .iter_mut()
                    .find(|place| matches!(place, Place::Member { .. }))
                    .unwrap();
                let Place::Member { key, .. } = place else {
                    unreachable!()
                };
                *key = y;
            });
            let updated = uses
                .updated(&changed, &[consumer], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert_eq!(
                uses.cell(state(program)).unwrap().revision(),
                updated.cell(state(program)).unwrap().revision()
            );
            assert!(!family.dependencies().valid_for(&changed, &updated));
            assert!(!family.dependencies().valid_for(&changed, &uses));
            updated.discard(&mut ledger).unwrap();
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
        },
    );
}

#[test]
fn previously_uninvolved_capturing_unit_invalidates_absence_proof_and_rechecks_tdz() {
    checked(
        "int remote(){return 0;}Record<int> state=record{x:1};print(state.x??0);",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let family = complete(program, &uses, &mut ledger);
            assert!(family.captures().is_empty());
            let state = state(program);
            let ty = program.cells[state.index()].ty;
            let remote = program
                .cells
                .iter()
                .find_map(|cell| match cell.binding {
                    CellBinding::Function(unit) if cell.name == "remote" => Some(unit),
                    _ => None,
                })
                .unwrap();
            let changed = edited(program, remote, |unit| {
                unit.captures.push(state);
                let place = PlaceId::from_index(unit.places.len()).unwrap();
                unit.places.push(Place::Cell(state));
                let operation = OpId::from_index(unit.operations.len()).unwrap();
                let value = ValueId::from_index(unit.values.len()).unwrap();
                unit.values.push(Value {
                    ty,
                    definition: operation,
                });
                unit.operations.push(Operation {
                    kind: OperationKind::Load(place),
                    operands: OperandRange {
                        start: unit.operands.len() as u32,
                        len: 0,
                    },
                    result: Some(value),
                    region: unit.entry,
                    origin: None,
                    span: Span::default(),
                });
                unit.regions[unit.entry.index()]
                    .operations
                    .insert(0, operation);
            });
            assert_eq!(
                program.units[family.root().unit.index()].revision(),
                changed.units[family.root().unit.index()].revision()
            );
            let updated = uses
                .updated(&changed, &[remote], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert_ne!(
                uses.cell(state).unwrap().revision(),
                updated.cell(state).unwrap().revision()
            );
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
            // The declaration's Closure operation precedes state initialization.
            // A stronger invocation-time proof might accept it; this family does not.
            assert!(matches!(
                result.outcome,
                FamilyOutcome::Unknown(UnknownReason::EarlyCaptureOrRead)
            ));
            updated.discard(&mut ledger).unwrap();
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn work_and_storage_truncation_are_distinct_and_owned_result_charges_release() {
    checked(
        "func()->int make(){Record<int> state=record{x:1};return ()=>state.x??0;}print(make()());",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let baseline = ledger.retained_bytes();
            for (which, expected) in [
                (0, ResourceLimit::Work),
                (1, ResourceLimit::Output),
                (2, ResourceLimit::Scratch),
            ] {
                let mut req = request();
                match which {
                    0 => req.attempt.work_quota = 1,
                    1 => req.output_bytes = 1,
                    _ => req.scratch_bytes = 0,
                }
                let first = analyze(
                    program,
                    &uses,
                    state(program),
                    req,
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
                assert!(
                    matches!(first.outcome, FamilyOutcome::Truncated(limit) if limit == expected)
                );
                assert_eq!(first.receipt.completion, AnalysisCompletion::Truncated);
                let second = analyze(
                    program,
                    &uses,
                    state(program),
                    req,
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
                assert_eq!(first.receipt, second.receipt);
                assert_eq!(ledger.retained_bytes(), baseline);
            }
            let family = complete(program, &uses, &mut ledger);
            assert!(family.retained_bytes() <= request().output_bytes);
            assert_eq!(ledger.retained_bytes(), baseline + family.retained_bytes());
            family.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), baseline);

            let mut too_large = request();
            too_large.output_bytes = 10_000_000;
            assert!(matches!(
                analyze(
                    program,
                    &uses,
                    state(program),
                    too_large,
                    &mut ledger,
                    WorkDomain::Optional
                ),
                Err(FamilyError::Budget(_))
            ));
            assert_eq!(ledger.retained_bytes(), baseline);
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}
