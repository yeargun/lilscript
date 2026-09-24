use super::*;
use crate::ast::BinaryOp;
use crate::check::binary_types::{checked_binary_type_with, BinaryTypeError};
use crate::check::type_relation::{is_type_assignable_with, type_equal_with};
use crate::check::{DefaultValue, FunctionParameter, FunctionSignature, FunctionType};
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use std::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn new_ledger(memory: u64, work: u64) -> BudgetLedger {
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
fn primitive_queries_charge_work_without_allocating_payload_workspace() {
    let mut ledger = new_ledger(0, 100_000);
    {
        let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut query = TypeQueryAdmission::new(&mut scope);
        assert_eq!(
            is_type_assignable_with(&Type::Float, &Type::Int, &mut query),
            Ok(true)
        );
        assert_eq!(
            is_type_assignable_with(&Type::Int, &Type::String, &mut query),
            Ok(false)
        );
        assert_eq!(query.clone_type(&Type::Bool), Ok(Type::Bool));
    }
    assert!(ledger.work_used(WorkDomain::Optional) > 0);
    assert_eq!(ledger.peak_retained_bytes(), 0);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn repeated_named_and_default_equality_pays_each_queried_payload() {
    let name = "n".repeat(2048);
    let default = DefaultValue::Array(vec![DefaultValue::String(&name); 5]);
    let mut ledger = new_ledger(1_000_000, 1_000_000);
    let first;
    {
        let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut query = TypeQueryAdmission::new(&mut scope);
        query
            .admit(RelationEvent::DefaultEquality {
                left: &default,
                right: &default,
            })
            .unwrap();
    }
    first = ledger.work_used(WorkDomain::Optional);
    assert!(first >= 2 * (5 * name.len()) as u64);
    {
        let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut query = TypeQueryAdmission::new(&mut scope);
        query
            .admit(RelationEvent::DefaultEquality {
                left: &default,
                right: &default,
            })
            .unwrap();
    }
    assert_eq!(ledger.work_used(WorkDomain::Optional), 2 * first);
    let mut limited = new_ledger(1_000_000, first - 1);
    {
        let mut scope = AllocationBudget::new(Some((&mut limited, WorkDomain::Optional)));
        let result = TypeQueryAdmission::new(&mut scope).admit(RelationEvent::DefaultEquality {
            left: &default,
            right: &default,
        });
        assert!(matches!(
            result,
            Err(AllocationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Optional
            )))
        ));
    }
    assert_eq!(limited.retained_bytes(), 0);
}

#[test]
fn clone_allowance_lives_through_comparison_and_refusal_preserves_inputs() {
    let source = Type::Array(Box::new(Type::Int));
    let bytes = size_of::<Type<'_>>() as u64;
    let mut limited = new_ledger(bytes - 1, 100_000);
    {
        let mut scope = AllocationBudget::new(Some((&mut limited, WorkDomain::Optional)));
        let result = TypeQueryAdmission::new(&mut scope).clone_type(&source);
        assert!(matches!(
            result,
            Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        ));
    }
    assert_eq!(limited.retained_bytes(), 0);
    assert_eq!(source, Type::Array(Box::new(Type::Int)));
    let mut enough = new_ledger(bytes, 100_000);
    {
        let mut scope = AllocationBudget::new(Some((&mut enough, WorkDomain::Optional)));
        let cloned = {
            let mut query = TypeQueryAdmission::new(&mut scope);
            let cloned = query.clone_type(&source).unwrap();
            assert!(type_equal_with(&cloned, &source, &mut query).unwrap());
            cloned
        };
        assert_eq!(scope.retained_bytes(AllocationClass::Scratch), bytes);
        drop(cloned);
        assert_eq!(
            scope.retained_bytes(AllocationClass::Scratch),
            bytes,
            "query owns allowance until its scope ends"
        );
    }
    assert_eq!(enough.retained_bytes(), 0);
}

#[test]
fn binary_expected_result_survives_comparison_and_failed_construction_cleans_up() {
    let left = Type::Nullable(Box::new(Type::Array(Box::new(Type::Int))));
    let right = Type::Array(Box::new(Type::Float));
    let expected = Type::Array(Box::new(Type::Float));
    for memory in [0, 10_000] {
        let mut ledger = new_ledger(memory, 1_000_000);
        {
            let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut query = TypeQueryAdmission::new(&mut scope);
            let result = checked_binary_type_with(BinaryOp::Nullish, &left, &right, &mut query);
            if memory == 0 {
                assert!(matches!(
                    result,
                    Err(BinaryTypeError::Admission(AllocationError::Budget(
                        BudgetError::MemoryExhausted(WorkDomain::Optional)
                    )))
                ));
            } else {
                let result = result.unwrap();
                assert!(type_equal_with(&result, &expected, &mut query).unwrap());
                drop(query);
                assert!(
                    scope.retained_bytes(AllocationClass::Scratch) >= size_of::<Type<'_>>() as u64
                );
                drop(result);
            }
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn query_vector_denial_and_unwind_preserve_charge_ownership() {
    let bytes = size_of::<Type<'_>>() as u64;
    let mut ledger = new_ledger(12 * bytes - 1, 100_000);
    {
        let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut values = Vec::new();
        {
            let mut query = TypeQueryAdmission::new(&mut scope);
            for _ in 0..4 {
                query.push_type(&mut values, Type::Int).unwrap();
            }
            assert_eq!(values.capacity(), 4);
            assert!(matches!(
                query.push_type(&mut values, Type::String),
                Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                    WorkDomain::Optional
                )))
            ));
        }
        assert_eq!(values, vec![Type::Int; 4]);
        assert_eq!(scope.retained_bytes(AllocationClass::Scratch), 4 * bytes);
        drop(values);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    let source = Type::Array(Box::new(Type::Int));
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut query = TypeQueryAdmission::new(&mut scope);
        let _cloned = query.clone_type(&source).unwrap();
        panic!("injected after admitted type clone");
    }));
    assert!(panic.is_err());
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn unmetered_verification_uses_plain_results_and_shared_function_clones() {
    let source = Type::Function(FunctionType::new(FunctionSignature {
        params: vec![FunctionParameter::defaulted(
            Type::Int,
            DefaultValue::Array(vec![DefaultValue::Int(1); 8]),
        )],
        return_type: Box::new(Type::Int),
    }));
    let mut scope = AllocationBudget::new(None);
    let cloned = {
        let mut query = TypeQueryAdmission::new(&mut scope);
        assert!(!query.metered);
        let cloned = query.clone_type(&source).unwrap();
        assert!(type_equal_with(&cloned, &source, &mut query).unwrap());
        cloned
    };
    let (Type::Function(original), Type::Function(cloned_signature)) = (&source, &cloned) else {
        unreachable!()
    };
    assert!(std::ptr::eq(&**original, &**cloned_signature));
    drop(cloned);
    assert_eq!(scope.retained_bytes(AllocationClass::Scratch), 0);
}
