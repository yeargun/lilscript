use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::semantic::type_admission::TypeQueryAdmission;
use crate::semantic::type_relation::type_equal_with;
use crate::semantic::{normalize_union, NominalId, StructType};
use crate::stable_hash::StableHashMap as AHashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn signature(parameters: Vec<Type<'static>>, result: Type<'static>) -> FunctionType<'static> {
    FunctionType::new(FunctionSignature {
        params: parameters
            .into_iter()
            .enumerate()
            .map(|(index, ty)| FunctionParameter {
                ty,
                passing: crate::primitive::ParameterPassing::Value,
                default: (index == 1).then_some(DefaultValue::Array(vec![
                    DefaultValue::String("literal-default"),
                    DefaultValue::Int(3),
                ])),
            })
            .collect(),
        return_type: Box::new(result),
    })
}
fn ledger(memory: u64, work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 0,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}

#[test]
fn forward_substitution_matches_previous_checker_with_nested_types_and_union_collapse() {
    let parameter = Type::TypeParameter("T");
    let declaration = StructType {
        identity: NominalId::new(0, false),
        name: "Point",
    };
    let mut corpus = vec![
        Type::Int,
        Type::String,
        Type::Struct(declaration),
        parameter.clone(),
        Type::TypeParameter("Other"),
        Type::TypeParameter("$js"),
    ];
    for _ in 0..3 {
        let previous = corpus.clone();
        for item in previous {
            corpus.push(Type::Array(Box::new(item.clone())));
            corpus.push(Type::Nullable(Box::new(item.clone())));
            corpus.push(Type::Map(
                Box::new(parameter.clone()),
                Box::new(item.clone()),
            ));
            corpus.push(Type::Union(vec![
                item.clone(),
                Type::Int,
                parameter.clone(),
            ]));
            corpus.push(Type::Function(signature(
                vec![item.clone(), parameter.clone()],
                item,
            )));
        }
    }
    corpus.push(Type::StructInstance {
        declaration,
        args: vec![parameter.clone()],
    });
    corpus.push(Type::GenericFunction(GenericFunctionType {
        type_params: vec!["U"],
        signature: signature(
            vec![parameter.clone(), Type::TypeParameter("U")],
            parameter.clone(),
        ),
    }));
    for replacement in [
        Type::Int,
        Type::Struct(declaration),
        Type::Union(vec![Type::Int, Type::Float]),
    ] {
        let substitutions = AHashMap::from_iter([("T", replacement)]);
        for ty in &corpus {
            let expected = old_substitute_type(ty, &substitutions);
            let actual = substitute_type_with(
                ty,
                &mut |name, _: &mut Unmetered| Ok(substitutions.get(name)),
                &mut Unmetered,
            )
            .unwrap();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn admitted_callable_result_stays_live_through_exact_comparison_then_releases_scope() {
    let template = signature(
        vec![
            Type::TypeParameter("T"),
            Type::Array(Box::new(Type::TypeParameter("T"))),
        ],
        Type::Union(vec![Type::TypeParameter("T"), Type::Int]),
    );
    let argument = Type::Int;
    let mut budget = ledger(1_000_000, 1_000_000);
    {
        let mut scope = AllocationBudget::new(Some((&mut budget, WorkDomain::Optional)));
        let result = {
            let mut query = TypeQueryAdmission::new(&mut scope);
            let result = substitute_signature_with(
                &template,
                &mut |name, _: &mut TypeQueryAdmission<'_, '_>| {
                    Ok((name == "T").then_some(&argument))
                },
                &mut query,
            )
            .unwrap();
            let value = Type::Function(result);
            let expected = Type::Function(signature(
                vec![Type::Int, Type::Array(Box::new(Type::Int))],
                Type::Int,
            ));
            assert!(type_equal_with(&value, &expected, &mut query).unwrap());
            value
        };
        assert!(scope.retained_bytes(AllocationClass::Scratch) > 0);
        drop(result);
        assert!(scope.retained_bytes(AllocationClass::Scratch) > 0);
    }
    assert_eq!(budget.retained_bytes(), 0);
    assert!(budget.work_used(WorkDomain::Optional) > 0);
}

#[test]
fn partial_substitution_refusals_and_unwind_release_the_same_query_owner() {
    let template = signature(
        vec![
            Type::Array(Box::new(Type::TypeParameter("T"))),
            Type::TypeParameter("T"),
        ],
        Type::TypeParameter("T"),
    );
    let original = template.clone();
    let argument = Type::Record(Box::new(Type::String));
    for (memory, work) in [(0, 1_000_000), (1_000_000, 2)] {
        let mut budget = ledger(memory, work);
        {
            let mut scope = AllocationBudget::new(Some((&mut budget, WorkDomain::Optional)));
            let result = substitute_signature_with(
                &template,
                &mut |_, _: &mut TypeQueryAdmission<'_, '_>| Ok(Some(&argument)),
                &mut TypeQueryAdmission::new(&mut scope),
            );
            assert!(matches!(
                result,
                Err(AllocationError::Budget(BudgetError::MemoryExhausted(_)))
                    | Err(AllocationError::Budget(BudgetError::WorkExhausted(_)))
            ));
        }
        assert_eq!(budget.retained_bytes(), 0);
        assert_eq!(template, original);
    }
    let mut budget = ledger(1_000_000, 1_000_000);
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let mut scope = AllocationBudget::new(Some((&mut budget, WorkDomain::Optional)));
        let mut visits = 0;
        let _ = substitute_signature_with(
            &template,
            &mut |_, _: &mut TypeQueryAdmission<'_, '_>| {
                visits += 1;
                assert!(
                    visits < 2,
                    "query callback unwind after partial construction"
                );
                Ok(Some(&argument))
            },
            &mut TypeQueryAdmission::new(&mut scope),
        );
    }));
    assert!(failure.is_err());
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(template, original);
}

#[test]
fn primitive_substitution_has_no_scratch_allocation_and_never_rewrites_argument_itself() {
    let mut budget = ledger(0, 1000);
    let argument = Type::TypeParameter("Outer");
    {
        let mut scope = AllocationBudget::new(Some((&mut budget, WorkDomain::Optional)));
        let result = substitute_type_with(
            &Type::TypeParameter("T"),
            &mut |_, _: &mut TypeQueryAdmission<'_, '_>| Ok(Some(&argument)),
            &mut TypeQueryAdmission::new(&mut scope),
        )
        .unwrap();
        assert_eq!(result, argument);
    }
    assert_eq!(budget.peak_retained_bytes(), 0);
    assert_eq!(budget.retained_bytes(), 0);
}

fn old_substitute_type<'src>(
    ty: &Type<'src>,
    substitutions: &AHashMap<&'src str, Type<'src>>,
) -> Type<'src> {
    match ty {
        Type::TypeParameter(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Type::Array(element) => Type::Array(Box::new(old_substitute_type(element, substitutions))),
        Type::Record(value) => Type::Record(Box::new(old_substitute_type(value, substitutions))),
        Type::Map(key, value) => Type::Map(
            Box::new(old_substitute_type(key, substitutions)),
            Box::new(old_substitute_type(value, substitutions)),
        ),
        Type::Set(element) => Type::Set(Box::new(old_substitute_type(element, substitutions))),
        Type::Task(value) => Type::Task(Box::new(old_substitute_type(value, substitutions))),
        Type::Generator(value) => {
            Type::Generator(Box::new(old_substitute_type(value, substitutions)))
        }
        Type::Nullable(inner) => {
            Type::Nullable(Box::new(old_substitute_type(inner, substitutions)))
        }
        Type::Union(members) => normalize_union(
            members
                .iter()
                .map(|member| old_substitute_type(member, substitutions))
                .collect(),
        ),
        Type::StructInstance { declaration, args } => Type::StructInstance {
            declaration: *declaration,
            args: args
                .iter()
                .map(|argument| old_substitute_type(argument, substitutions))
                .collect(),
        },
        Type::ClassInstance { name, args } => Type::ClassInstance {
            name,
            args: args
                .iter()
                .map(|argument| old_substitute_type(argument, substitutions))
                .collect(),
        },
        Type::Function(signature) => Type::Function(FunctionType::new(FunctionSignature {
            params: signature
                .params
                .iter()
                .map(|parameter| FunctionParameter {
                    ty: old_substitute_type(&parameter.ty, substitutions),
                    passing: parameter.passing,
                    default: parameter.default.clone(),
                })
                .collect(),
            return_type: Box::new(old_substitute_type(&signature.return_type, substitutions)),
        })),
        Type::GenericFunction(function) => Type::GenericFunction(GenericFunctionType {
            type_params: function.type_params.clone(),
            signature: FunctionType::new(FunctionSignature {
                params: function
                    .signature
                    .params
                    .iter()
                    .map(|parameter| FunctionParameter {
                        ty: old_substitute_type(&parameter.ty, substitutions),
                        passing: parameter.passing,
                        default: parameter.default.clone(),
                    })
                    .collect(),
                return_type: Box::new(old_substitute_type(
                    &function.signature.return_type,
                    substitutions,
                )),
            }),
        }),
        _ => ty.clone(),
    }
}
