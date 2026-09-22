use super::publication::{
    CheckpointLimit, Compilation, OperationPatch, PublicationError, SemanticId, UnitPatch,
};
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind,
};

const WORK: u64 = 100_000_000;
const SMALL_WORK: u64 = 100_000;
const KEY_COUNT: usize = 1_000_000;

struct Fixture {
    unit: UnitId,
    allocation: OpId,
    integer: OpId,
    identity: AllocationId,
    key: StringId,
    operands: Vec<ValueId>,
}

fn checked(object: bool, inspect: impl FnOnce(Program<'_>, Fixture)) {
    let source = if object {
        "auto state=object{value:1};print(state);"
    } else {
        "Record<int> state=record{value:1};print(state.value??0);"
    };
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let fixture = program
        .units
        .iter()
        .find_map(|unit| {
            let data = unit.data();
            data.operations.iter().enumerate().find_map(|(index, op)| {
                let OperationKind::Allocate { identity, kind } = &op.kind else {
                    return None;
                };
                let keys = match kind {
                    AllocationKind::Object(keys) | AllocationKind::Record(keys) => keys,
                    _ => return None,
                };
                assert_eq!(keys.len(), 1);
                let operands = data.operands(op.operands).unwrap().to_vec();
                assert_eq!(operands.len(), 1);
                let integer = data
                    .operations
                    .iter()
                    .position(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(1))))
                    .unwrap();
                Some(Fixture {
                    unit: unit.id(),
                    allocation: OpId::from_index(index).unwrap(),
                    integer: OpId::from_index(integer).unwrap(),
                    identity: *identity,
                    key: keys[0],
                    operands,
                })
            })
        })
        .unwrap();
    inspect(program, fixture);
}

fn edit(
    compiler: &mut Compilation<'_>,
    base: SemanticId,
    fixture: &Fixture,
    operation: OpId,
    kind: &OperationKind,
    operands: &[ValueId],
    advance: bool,
) -> Result<SemanticId, PublicationError> {
    let revision = compiler.view(base)?.unit_revision(fixture.unit).unwrap();
    let patches = [UnitPatch {
        unit: fixture.unit,
        expected_revision: revision,
        operations: &[OperationPatch {
            operation,
            kind,
            operands,
        }],
        places: &[],
    }];
    if advance {
        compiler.advance_source(base, &patches, WorkDomain::Baseline)
    } else {
        compiler.edit_source(base, &patches, WorkDomain::Baseline)
    }
}

fn snapshot(compiler: &mut Compilation<'_>, base: SemanticId, fixture: &Fixture) -> [usize; 4] {
    compiler
        .with_semantic(base, |program, uses, _| {
            program.verify().unwrap();
            assert!(uses.valid_for(program));
            let data = program.unit(fixture.unit).unwrap();
            let op = &data.operations[fixture.allocation.index()];
            let OperationKind::Allocate { identity, kind } = &op.kind else {
                panic!("original allocation was replaced");
            };
            assert_eq!(*identity, fixture.identity);
            let keys = match kind {
                AllocationKind::Object(keys) | AllocationKind::Record(keys) => keys,
                _ => panic!("original key-bearing allocation was replaced"),
            };
            assert_eq!(keys.as_slice(), &[fixture.key]);
            assert_eq!(data.operands(op.operands).unwrap(), fixture.operands);
            [
                program as *const _ as usize,
                uses as *const _ as usize,
                data.operations.as_ptr() as usize,
                data.operands.as_ptr() as usize,
            ]
        })
        .unwrap()
}

fn rejects_large_keys(object: bool) {
    for advance in [false, true] {
        for small_work in [false, true] {
            checked(object, |program, fixture| {
                let mut compiler = Compilation::new(
                    BudgetLedger::new(
                        ResourceLimits::default(),
                        BudgetPlan {
                            baseline_work: WORK,
                            optional_work: 0,
                            baseline_retained_bytes: 0,
                            retained_bytes: 100_000_000,
                        },
                    )
                    .unwrap(),
                    CheckpointLimit { max_live: 2 },
                )
                .unwrap();
                let base = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let revision = compiler.view(base).unwrap().unit_revision(fixture.unit);
                let before = snapshot(&mut compiler, base, &fixture);
                let retained = compiler.ledger().retained_bytes();
                let peak = compiler.ledger().peak_retained_bytes();
                if small_work {
                    compiler
                        .with_semantic(base, |_, _, ledger| {
                            // Leave an ordinary edit's budget after frontend adoption,
                            // without changing the original allocation's charge domain.
                            let spend = WORK - ledger.work_used(WorkDomain::Baseline) - SMALL_WORK;
                            ledger.charge(WorkDomain::Baseline, WorkKind::Edit, spend)
                        })
                        .unwrap()
                        .unwrap();
                }
                // The supplied patch is caller-owned. Every key is valid, so ample
                // work reaches the actual key/operand arity mismatch after scanning.
                let keys = vec![fixture.key; KEY_COUNT];
                let malformed = OperationKind::Allocate {
                    identity: fixture.identity,
                    kind: if object {
                        AllocationKind::Object(keys)
                    } else {
                        AllocationKind::Record(keys)
                    },
                };
                let result = edit(
                    &mut compiler,
                    base,
                    &fixture,
                    fixture.allocation,
                    &malformed,
                    &fixture.operands,
                    advance,
                );
                assert_eq!(
                    result,
                    Err(if small_work {
                        PublicationError::Budget(BudgetError::WorkExhausted(WorkDomain::Baseline))
                    } else {
                        PublicationError::InvalidReplacement
                    }),
                    "object={object}, advance={advance}, small_work={small_work}"
                );
                assert_eq!(compiler.checkpoint_count(), 1);
                assert_eq!(compiler.ledger().retained_bytes(), retained);
                assert_eq!(
                    compiler.view(base).unwrap().unit_revision(fixture.unit),
                    revision
                );
                assert_eq!(snapshot(&mut compiler, base, &fixture), before);
                if small_work {
                    // A refusal must precede retaining the copied key payload, even
                    // though all temporary transaction owners have since rolled back.
                    assert!(
                        compiler.ledger().peak_retained_bytes() - peak
                            < (KEY_COUNT * std::mem::size_of::<StringId>()) as u64
                    );
                }
                // The same checkpoint and remaining budget can publish a real edit.
                // This also rules out a test that only exhausts transaction setup.
                let next = edit(
                    &mut compiler,
                    base,
                    &fixture,
                    fixture.integer,
                    &OperationKind::Constant(Constant::Integer(2)),
                    &[],
                    advance,
                )
                .unwrap();
                assert!(matches!(
                    compiler
                        .view(next)
                        .unwrap()
                        .unit(fixture.unit)
                        .unwrap()
                        .operations[fixture.integer.index()]
                    .kind,
                    OperationKind::Constant(Constant::Integer(2))
                ));
                snapshot(&mut compiler, next, &fixture);
                assert_ne!(
                    compiler.view(next).unwrap().unit_revision(fixture.unit),
                    revision
                );
                if advance {
                    assert!(matches!(
                        compiler.view(base),
                        Err(PublicationError::UnknownCheckpoint)
                    ));
                } else {
                    assert_eq!(snapshot(&mut compiler, base, &fixture), before);
                    compiler.discard(base).unwrap();
                }
                compiler.discard(next).unwrap();
                assert_eq!(compiler.finish().retained_bytes(), 0);
            });
        }
    }
}

#[test]
fn malformed_object_keys_pay_before_copy_and_preserve_checkpoint_for_retry() {
    rejects_large_keys(true);
}

#[test]
fn malformed_record_keys_pay_before_copy_and_preserve_checkpoint_for_retry() {
    rejects_large_keys(false);
}
