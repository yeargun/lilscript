//! Constructor lookup and omitted operands share the ordinary prepared-call
//! protocol. Their builtin schema is authoritative; a fabricated callable type
//! cannot replace it, and checked edits cannot cross a live call envelope.
use super::publication::*;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::primitive::{intrinsic_call_contract, Intrinsic};
use crate::check::DefaultValue;

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program);
}

fn constructor_site(program: &Program<'_>) -> (UnitId, CallId, OpId, OpId) {
    for unit in program.units() {
        for (index, site) in unit.data().calls.iter().enumerate() {
            if matches!(
                site.target,
                CallTarget::Intrinsic {
                    operation: ResolvedIntrinsic::Constructor(Intrinsic::RegexNew),
                    ..
                }
            ) {
                let call = CallId::from_index(index).unwrap();
                let find = |wanted| {
                    OpId::from_index(unit.data().operations.iter().position(|operation| {
                        if wanted {
                            matches!(operation.kind, OperationKind::PrepareCall(found) if found == call)
                        } else {
                            matches!(operation.kind, OperationKind::Call(found) if found == call)
                        }
                    }).unwrap()).unwrap()
                };
                return (unit.id(), call, find(true), find(false));
            }
        }
    }
    panic!("fixture has a Regex constructor");
}

fn rewrite<'src>(
    program: &Program<'src>,
    unit: UnitId,
    edit: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut changed = program.clone();
    let mut working = changed.units[unit.index()].clone().into_working();
    edit(working.get_mut());
    changed.units[unit.index()] = working.freeze();
    changed
}

#[test]
fn regex_constructor_retains_shared_schema_and_actual_optional_arity() {
    let contract =
        intrinsic_call_contract(ResolvedIntrinsic::Constructor(Intrinsic::RegexNew)).unwrap();
    assert!(contract.receiver.is_none());
    assert_eq!(contract.parameters, [Type::String, Type::String]);
    assert_eq!(contract.defaults, [None, Some(DefaultValue::String(""))]);
    assert_eq!(contract.result, &Type::Regex);
    assert_eq!(contract.required_params(), 1);
    for (arity, accepted) in [
        (0, false),
        (1, true),
        (2, true),
        (3, false),
        (usize::MAX, false),
    ] {
        assert_eq!(contract.accepts_arity(arity), accepted);
    }
    checked("export Regex one(){return new Regex(\"x\");}export Regex two(){return new Regex(\"x\",\"g\");}", |program| {
        let mut supplied = Vec::new();
        for unit in program.units() {
            for (index, site) in unit.data().calls.iter().enumerate() {
                if !matches!(site.target, CallTarget::Intrinsic { operation: ResolvedIntrinsic::Constructor(Intrinsic::RegexNew), receiver: None }) { continue; }
                assert!(site.contract.signature.is_none(), "constructor schema does not masquerade as a source callee expression");
                assert_eq!(site.contract.defaults, DefaultConvention::PreserveOmission);
                let call = CallId::from_index(index).unwrap();
                let invocation = unit.data().operations.iter().find(|operation| matches!(operation.kind, OperationKind::Call(found) if found == call)).unwrap();
                assert_eq!(invocation.operands.len, 0);
                assert_eq!(site.arguments.len, site.contract.supplied);
                assert_eq!(&program.types[unit.data().values[invocation.result.unwrap().index()].ty.index()], contract.result);
                supplied.push(site.contract.supplied);
            }
        }
        assert_eq!(supplied, [1, 2]);
    });
}

#[test]
fn constructor_rejects_fabricated_signature_receiver_and_raw_result_type() {
    checked(
        "export Regex make(){string receiver=\"x\";return new Regex(receiver);}",
        |program| {
            let (unit, call, _, invocation) = constructor_site(program);
            let receiver = program
                .unit(unit)
                .unwrap()
                .operations
                .iter()
                .find(|operation| {
                    matches!(operation.kind, OperationKind::Constant(Constant::String(_)))
                })
                .unwrap()
                .result
                .unwrap();
            let string_type = program.unit(unit).unwrap().values[receiver.index()].ty;
            let shape =
                intrinsic_call_contract(ResolvedIntrinsic::Constructor(Intrinsic::RegexNew))
                    .unwrap()
                    .signature();
            let mut with_type = program.clone();
            let signature = TypeId::from_index(with_type.types.len()).unwrap();
            Arc::make_mut(&mut with_type.types).push(Type::Function(shape));
            for claimed in [signature, string_type] {
                let broken = rewrite(&with_type, unit, |data| {
                    data.calls[call.index()].contract.signature = Some(claimed)
                });
                assert!(broken
                    .verify()
                    .unwrap_err()
                    .contains("semantic operation type mismatch"));
            }
            let broken = rewrite(program, unit, |data| {
                let CallTarget::Intrinsic { receiver: slot, .. } =
                    &mut data.calls[call.index()].target
                else {
                    unreachable!()
                };
                *slot = Some(receiver);
            });
            // Only a receiver-free builtin constructor may omit a separately
            // checked callee signature. This malformed shape fails that
            // structural requirement before intrinsic type validation.
            assert!(broken
                .verify()
                .unwrap_err()
                .contains("call lost its checked signature"));
            let broken = rewrite(program, unit, |data| {
                let result = data.operations[invocation.index()].result.unwrap();
                data.values[result.index()].ty = string_type;
            });
            assert!(broken
                .verify()
                .unwrap_err()
                .contains("semantic operation type mismatch"));
        },
    );
}

#[test]
fn constructor_rejects_arity_synthesis_argument_types_and_changed_convention() {
    checked(
        "export Regex make(){int wrong=7;return new Regex(\"x\",\"g\");}",
        |program| {
            let (unit, call, _, _) = constructor_site(program);
            let data = program.unit(unit).unwrap();
            let input = data.arguments(data.calls[call.index()].arguments).unwrap()[0];
            let wrong = data
                .operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Constant(Constant::Integer(7))
                    )
                })
                .unwrap()
                .result
                .unwrap();
            for arity in [0, 3] {
                let broken = rewrite(program, unit, |data| {
                    let range = data.calls[call.index()].arguments;
                    let start = range.start as usize;
                    let end = start + range.len as usize;
                    data.call_arguments
                        .splice(start..end, std::iter::repeat_n(input, arity));
                    for site in &mut data.calls {
                        if site.arguments.start as usize >= end {
                            site.arguments.start =
                                (site.arguments.start as usize + arity - range.len as usize) as u32;
                        }
                    }
                    data.calls[call.index()].arguments.len = arity as u32;
                    data.calls[call.index()].contract.supplied = arity as u32;
                });
                assert!(
                    broken
                        .verify()
                        .unwrap_err()
                        .contains("semantic operation type mismatch"),
                    "invalid source arity {arity}"
                );
            }
            for position in [0, 1] {
                let broken = rewrite(program, unit, |data| {
                    let start = data.calls[call.index()].arguments.start as usize;
                    data.call_arguments[start + position] = CallArgument::Value(wrong);
                });
                assert!(broken
                    .verify()
                    .unwrap_err()
                    .contains("semantic operation type mismatch"));
            }
            let broken = rewrite(program, unit, |data| {
                data.calls[call.index()].contract.supplied = 1
            });
            assert!(broken
                .verify()
                .unwrap_err()
                .contains("preserved omission has synthesized arguments"));
            let broken = rewrite(program, unit, |data| {
                data.calls[call.index()].contract.defaults = DefaultConvention::MaterializeAtCaller
            });
            assert!(broken
                .verify()
                .unwrap_err()
                .contains("call argument convention disagrees with its target"));
        },
    );
}

#[test]
fn builtin_constructors_cannot_borrow_another_constructors_checked_shape() {
    checked("export Regex make(){return new Regex(\"x\");}", |program| {
        let (unit, call, _, _) = constructor_site(program);
        for operation in [
            Intrinsic::MapNew,
            Intrinsic::ArrayBufferNew,
            Intrinsic::SymbolNew,
        ] {
            // No static contract: the checked shape (operands, result) rules.
            assert!(intrinsic_call_contract(ResolvedIntrinsic::Constructor(operation)).is_none());
            let broken = rewrite(program, unit, |data| {
                data.calls[call.index()].target = CallTarget::Intrinsic {
                    operation: ResolvedIntrinsic::Constructor(operation),
                    receiver: None,
                };
            });
            assert!(broken.verify().is_err(), "{operation:?} borrowed Regex's shape");
        }
    });
    for source in [
        "Map<string,int> value=new Map<string,int>();",
        "ArrayBuffer value=new ArrayBuffer(8);",
        "Symbol value=new Symbol(\"x\");",
        "Uint8Array value=new Uint8Array(4);",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        from_checked_source(&syntax, &semantics)
            .unwrap_or_else(|error| panic!("{source}: {error:?}"))
            .verify()
            .unwrap();
    }
}

#[test]
fn checked_edits_cannot_consume_constructor_preparation_out_of_order() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "extern string pattern();extern string flags();export Regex make(){return new Regex(pattern(),flags());}").unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    let (unit, constructor, prepare, invocation) = constructor_site(&program);
    let data = program.unit(unit).unwrap();
    let sequence = &data.regions[data.operations[prepare.index()].region.index()].operations;
    let position = |operation| {
        sequence
            .iter()
            .position(|&found| found == operation)
            .unwrap()
    };
    assert_eq!(data.operations[invocation.index()].operands.len, 0);
    let inputs = data
        .arguments(data.calls[constructor.index()].arguments)
        .unwrap();
    let calls: Vec<_> = inputs
        .iter()
        .map(|argument| {
            let CallArgument::Value(value) = *argument else {
                panic!("value argument")
            };
            data.values[value.index()].definition
        })
        .collect();
    let preparations: Vec<_> = calls.iter().map(|operation| {
        let OperationKind::Call(call) = data.operations[operation.index()].kind else { panic!("effectful argument") };
        OpId::from_index(data.operations.iter().position(|operation| matches!(operation.kind, OperationKind::PrepareCall(found) if found == call)).unwrap()).unwrap()
    }).collect();
    let order = [
        prepare,
        preparations[0],
        calls[0],
        preparations[1],
        calls[1],
        invocation,
    ]
    .map(position);
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    let revision = program.units[unit.index()].revision();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    for (operation, kind) in [
        (prepare, OperationKind::Call(constructor)),
        (preparations[0], OperationKind::PrepareCall(constructor)),
        (calls[0], OperationKind::Call(constructor)),
    ] {
        let retained = compilation.ledger().retained_bytes();
        let error = compilation
            .edit_source(
                source,
                &[UnitPatch {
                    unit,
                    expected_revision: revision,
                    operations: &[OperationPatch {
                        operation,
                        kind: &kind,
                        operands: &[],
                    }],
                    places: &[],
                }],
                WorkDomain::Optional,
            )
            .unwrap_err();
        assert_eq!(error, PublicationError::InvalidReplacement);
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        compilation
            .with_semantic(source, |program, uses, _| {
                program.verify().unwrap();
                assert!(uses.valid_for(program));
            })
            .unwrap();
    }
    compilation.discard(source).unwrap();
    assert_eq!(compilation.finish().retained_bytes(), 0);
}
