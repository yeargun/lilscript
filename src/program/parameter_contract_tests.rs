//! Passing mode and default ownership survive into the checked semantic core.
//! Passing modes cannot substitute a value argument for a prepared place, even
//! when a checked edit changes both the callee and call signature together.
use super::publication::{CheckpointLimit, Compilation, PublicationError};
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::primitive::{Intrinsic, ParameterPassing};
use crate::check::{DefaultValue, FunctionParameter, FunctionSignature, FunctionType};

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program);
}

fn change_signature<'src>(
    program: &mut Program<'src>,
    ty: TypeId,
    edit: impl FnOnce(&mut FunctionSignature<'src>),
) {
    let Type::Function(signature) = &program.types[ty.index()] else {
        panic!("fixture names an ordinary callable signature");
    };
    let mut signature = (**signature).clone();
    edit(&mut signature);
    Arc::make_mut(&mut program.types)[ty.index()] = Type::Function(FunctionType::new(signature));
}

fn named_type(program: &Program<'_>, name: &str) -> TypeId {
    program
        .cells
        .iter()
        .find(|cell| cell.name == name)
        .unwrap()
        .ty
}

#[test]
fn checked_value_parameters_retain_each_primitive_default_and_supplied_arity() {
    checked(
        "int sum(int left,int right){return left+right;}print(sum(1,2));print(\"abcd\".slice(1));",
        |program| {
            let Type::Function(signature) = &program.types[named_type(&program, "sum").index()]
            else {
                panic!("sum signature");
            };
            assert!(signature.params.iter().all(|parameter| {
                parameter.ty == Type::Int
                    && parameter.passing == ParameterPassing::Value
                    && parameter.default.is_none()
            }));
            let method = program
                .units()
                .iter()
                .find_map(|unit| {
                    unit.data().calls.iter().find(|site| {
                        matches!(
                            site.target,
                            CallTarget::Intrinsic {
                                operation: ResolvedIntrinsic::Method(Intrinsic::StringSlice),
                                ..
                            }
                        )
                    })
                })
                .unwrap();
            let Type::Function(signature) =
                &program.types[method.contract.signature.unwrap().index()]
            else {
                panic!("slice signature");
            };
            assert_eq!(method.contract.supplied, 1);
            assert_eq!(signature.params.len(), 2);
            assert_eq!(signature.params[0].passing, ParameterPassing::Value);
            assert!(signature.params[0].default.is_none());
            assert_eq!(signature.params[1].passing, ParameterPassing::Value);
            assert!(signature.params[1].default.is_some());
            assert_eq!(signature.required_params(), 1);
            assert!(signature.accepts_arity(1) && signature.accepts_arity(2));
            assert!(!signature.accepts_arity(0) && !signature.accepts_arity(3));
        },
    );
}

#[test]
fn a_shared_reference_signature_cannot_turn_value_arguments_into_prepared_places() {
    for source in [
        "int consume(int value){return value;}print(consume(1));",
        "extern int consume(int value);print(consume(1));",
    ] {
        checked(source, |mut program| {
            let ty = named_type(&program, "consume");
            // This updates the shared signature used by both declaration and
            // call. Even matching modes cannot invent a prepared place operand.
            change_signature(&mut program, ty, |signature| {
                signature.params[0].passing = ParameterPassing::MutableReference;
            });
            let error = program.verify().unwrap_err();
            assert!(
                error.contains("reference") || error.contains("passing mode"),
                "{error}"
            );
        });
    }
}

#[test]
fn reference_defaults_and_required_after_default_records_are_invalid() {
    checked(
        "int consume(int value){return value;}print(consume(1));",
        |mut program| {
            let ty = named_type(&program, "consume");
            change_signature(&mut program, ty, |signature| {
                signature.params[0].passing = ParameterPassing::MutableReference;
                signature.params[0].default = Some(DefaultValue::Int(1));
            });
            let Type::Function(signature) = &program.types[ty.index()] else {
                unreachable!()
            };
            let reason = signature.validate_parameters().unwrap_err();
            assert!(reason.contains("default"), "{reason}");
            assert_eq!(program.verify().unwrap_err(), reason);
        },
    );
    checked(
        "int consume(int first,int second){return first+second;}print(consume(1,2));",
        |mut program| {
            let ty = named_type(&program, "consume");
            change_signature(&mut program, ty, |signature| {
                signature.params[0].default = Some(DefaultValue::Int(1));
            });
            let Type::Function(signature) = &program.types[ty.index()] else {
                unreachable!()
            };
            let reason = signature.validate_parameters().unwrap_err();
            assert!(reason.contains("default"), "{reason}");
            assert_eq!(program.verify().unwrap_err(), reason);
        },
    );
}

#[test]
fn intrinsic_passing_mode_mismatch_is_rejected_even_when_types_and_arity_match() {
    checked("print(\"abc\".charCodeAt(0));", |mut program| {
        let ty = program
            .units()
            .iter()
            .find_map(|unit| {
                unit.data().calls.iter().find_map(|site| {
                    matches!(
                        site.target,
                        CallTarget::Intrinsic {
                            operation: ResolvedIntrinsic::Method(Intrinsic::StringCharCodeAt),
                            ..
                        }
                    )
                    .then(|| site.contract.signature.unwrap())
                })
            })
            .unwrap();
        change_signature(&mut program, ty, |signature| {
            signature.params[0].passing = ParameterPassing::MutableReference;
        });
        assert!(program
            .verify()
            .unwrap_err()
            .contains("primitive operation"));
    });
}

#[test]
fn unused_deep_parameter_records_are_validated_and_failed_adoption_releases_storage() {
    for shape in 0..5 {
        checked("print(1);", |mut program| {
            let malformed = Type::Function(FunctionType::new(FunctionSignature {
                params: vec![
                    FunctionParameter::defaulted(Type::Int, DefaultValue::Int(1)),
                    FunctionParameter::value(Type::Int),
                ],
                return_type: Box::new(Type::Int),
            }));
            let nested = match shape {
                0 => malformed,
                1 => Type::Record(Box::new(malformed)),
                2 => Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::Array(Box::new(malformed)))],
                    return_type: Box::new(Type::Void),
                })),
                3 => Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(malformed),
                })),
                _ => {
                    let mut nested = malformed;
                    for _ in 0..256 {
                        nested = Type::Array(Box::new(nested));
                    }
                    nested
                }
            };
            Arc::make_mut(&mut program.types).push(nested);
            let reason = program.verify().unwrap_err();
            assert_eq!(
                reason,
                "required parameters cannot follow defaulted parameters"
            );
            let ledger = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 1_000_000,
                    optional_work: 0,
                    baseline_retained_bytes: 0,
                    retained_bytes: 8_000_000,
                },
            )
            .unwrap();
            let mut compilation =
                Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
            let retained = compilation.ledger().retained_bytes();
            let work = compilation.ledger().work_used(WorkDomain::Baseline);
            let error = compilation
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap_err();
            assert!(
                matches!(error, PublicationError::InvalidParameterContract(found) if found == reason)
            );
            assert_eq!(compilation.checkpoint_count(), 0);
            assert_eq!(compilation.ledger().retained_bytes(), retained);
            assert!(compilation.ledger().work_used(WorkDomain::Baseline) > work);
            assert_eq!(compilation.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn well_formed_unused_reference_types_do_not_claim_executable_place_arguments() {
    checked("print(1);", |mut program| {
        let reference = Type::Function(FunctionType::new(FunctionSignature {
            params: vec![FunctionParameter {
                ty: Type::Int,
                passing: ParameterPassing::MutableReference,
                default: None,
            }],
            return_type: Box::new(Type::Int),
        }));
        Arc::make_mut(&mut program.types).push(Type::Record(Box::new(reference)));
        // This is well-formed metadata with no materialized body or invocation.
        // Invocation still requires mode-matched prepared place arguments.
        program.verify().unwrap();
    });
}
