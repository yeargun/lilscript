//! Method signatures are language metadata, not a consequence of whichever
//! JavaScript spelling a target currently supports. Checked edits must retain
//! the receiver, array result, omission convention and exact primitive type.
use super::*;
use crate::primitive::{intrinsic_call_contract, Intrinsic};
use crate::check::{DefaultValue, FunctionSignature, FunctionType};

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program);
}

fn method_site(program: &Program<'_>, wanted: Intrinsic) -> (UnitId, CallId) {
    program
        .units()
        .iter()
        .find_map(|unit| {
            unit.data()
                .calls
                .iter()
                .enumerate()
                .find_map(|(index, site)| {
                    matches!(site.target, CallTarget::Intrinsic {
                        operation: ResolvedIntrinsic::Method(found), ..
                    } if found == wanted)
                    .then_some((unit.id(), CallId::from_index(index).unwrap()))
                })
        })
        .expect("fixture has the requested checked intrinsic")
}

fn rewrite<'src>(
    program: &Program<'src>,
    unit: UnitId,
    change: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut changed = program.clone();
    let mut working = changed.units[unit.index()].clone().into_working();
    change(working.get_mut());
    changed.units[unit.index()] = working.freeze();
    changed
}

fn reject_signature_change<'src>(
    program: &Program<'src>,
    intrinsic: Intrinsic,
    change: impl FnOnce(&mut FunctionSignature<'src>),
) {
    let (unit, call) = method_site(program, intrinsic);
    let ty = program.unit(unit).unwrap().calls[call.index()]
        .contract
        .signature
        .unwrap();
    let Type::Function(signature) = &program.types[ty.index()] else {
        panic!("checked method signature")
    };
    let mut signature = (**signature).clone();
    change(&mut signature);
    let mut broken = program.clone();
    Arc::make_mut(&mut broken.types)[ty.index()] = Type::Function(FunctionType::new(signature));
    let error = broken.verify().unwrap_err();
    assert!(
        error.contains("call signature disagrees with its primitive operation"),
        "the intrinsic owner must reject the forged signature: {error}",
    );
}

#[test]
fn slice_and_split_retain_checked_signatures_and_source_arity() {
    checked(
        r#"
        export string suffix(string value,int start){return value.slice(start);}
        export string crop(string value,int start,int end){return value.slice(start,end);}
        export string[] lines(string value,string separator){return value.split(separator);}
        "#,
        |program| {
            let mut observed = Vec::new();
            for unit in program.units() {
                for (index, site) in unit.data().calls.iter().enumerate() {
                    let CallTarget::Intrinsic {
                        operation,
                        receiver: Some(receiver),
                    } = site.target
                    else {
                        continue;
                    };
                    let contract = intrinsic_call_contract(operation).unwrap();
                    assert_eq!(site.contract.defaults, DefaultConvention::PreserveOmission);
                    assert!(
                        contract.matches(&program.types[site.contract.signature.unwrap().index()])
                    );
                    assert_eq!(
                        &program.types[unit.data().values[receiver.index()].ty.index()],
                        &Type::String,
                    );
                    let call = CallId::from_index(index).unwrap();
                    let operation = unit
                        .data()
                        .operations
                        .iter()
                        .find(|operation| matches!(operation.kind, OperationKind::Call(found) if found == call))
                        .unwrap();
                    assert_eq!(operation.operands.len, 0);
                    assert_eq!(site.arguments.len, site.contract.supplied);
                    let result = operation.result.unwrap();
                    assert_eq!(
                        &program.types[unit.data().values[result.index()].ty.index()],
                        contract.result,
                    );
                    observed.push(site.contract.supplied);
                }
            }
            assert_eq!(observed, [1, 2, 1]);
        },
    );
}

#[test]
fn slice_rejects_compatible_but_nonprimitive_parameters_defaults_and_result() {
    checked(
        "export string crop(string value,int start,int end){return value.slice(start,end);}",
        |program| {
            // Int actuals assign to Float, so ordinary callable compatibility
            // cannot establish this intrinsic's exact parameter contract.
            reject_signature_change(program, Intrinsic::StringSlice, |signature| {
                signature.params[0].ty = Type::Float;
            });
            // Both arguments are supplied. The primitive's omission meaning
            // still belongs to its signature, not this particular call's arity.
            reject_signature_change(program, Intrinsic::StringSlice, |signature| {
                signature.params[1].default = Some(DefaultValue::Int(0));
            });
            reject_signature_change(program, Intrinsic::StringSlice, |signature| {
                signature.return_type = Box::new(Type::Bool);
            });
        },
    );
}

#[test]
fn split_rejects_forged_array_element_scalar_result_and_optional_separator() {
    checked(
        "export string[] lines(string value,string separator){return value.split(separator);}",
        |program| {
            reject_signature_change(program, Intrinsic::StringSplit, |signature| {
                signature.return_type = Box::new(Type::Array(Box::new(Type::Int)));
            });
            reject_signature_change(program, Intrinsic::StringSplit, |signature| {
                signature.return_type = Box::new(Type::String);
            });
            reject_signature_change(program, Intrinsic::StringSplit, |signature| {
                signature.params[0].default = Some(DefaultValue::String(""));
            });
        },
    );
}

#[test]
fn string_methods_reject_nonstring_receivers_even_with_matching_call_arguments() {
    for (intrinsic, source) in [
        (
            Intrinsic::StringSlice,
            "export string crop(string value){int wrong=7;return value.slice(0,1);}",
        ),
        (
            Intrinsic::StringSplit,
            "export string[] lines(string value){int wrong=7;return value.split(\"\\n\");}",
        ),
    ] {
        checked(source, |program| {
            let (unit, call) = method_site(program, intrinsic);
            let wrong = program
                .unit(unit)
                .unwrap()
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
            let broken = rewrite(program, unit, |data| {
                let CallTarget::Intrinsic { receiver, .. } = &mut data.calls[call.index()].target
                else {
                    unreachable!()
                };
                *receiver = Some(wrong);
            });
            let error = broken.verify().unwrap_err();
            assert!(
                error.contains("call signature disagrees with its primitive operation"),
                "wrong receiver for {intrinsic:?}: {error}",
            );
        });
    }
}

#[test]
fn string_methods_reject_changed_supplied_arity_and_materialized_omission() {
    for (intrinsic, source) in [
        (
            Intrinsic::StringSlice,
            "export string suffix(string value){return value.slice(1);}",
        ),
        (
            Intrinsic::StringSplit,
            "export string[] lines(string value){return value.split(\"\\n\");}",
        ),
    ] {
        checked(source, |program| {
            let (unit, call) = method_site(program, intrinsic);
            let omitted = rewrite(program, unit, |data| {
                data.calls[call.index()].contract.supplied = 0;
            });
            assert!(omitted
                .verify()
                .unwrap_err()
                .contains("preserved omission has synthesized arguments"));
            let materialized = rewrite(program, unit, |data| {
                data.calls[call.index()].contract.defaults = DefaultConvention::MaterializeAtCaller;
            });
            assert!(materialized
                .verify()
                .unwrap_err()
                .contains("call argument convention disagrees with its target"));
        });
    }
}
