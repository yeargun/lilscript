use super::*;
use crate::check::{FunctionType, ParameterPassing};
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use std::convert::Infallible;

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
fn leaf_and_single_child_payloads_need_no_heap_workspace() {
    let mut ledger = new_ledger(0, 100_000);
    let mut nested = Type::Enum("BorrowedName");
    for _ in 0..64 {
        nested = Type::Array(Box::new(nested));
    }
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let measured = measure_payload(Payload::Type(&Type::Int), &mut budget, |_| {
            Ok::<_, Infallible>(())
        })
        .unwrap();
        assert_eq!(
            measured,
            PayloadMeasure {
                owned_bytes: 0,
                nodes: 1,
                text_bytes: 0
            }
        );
        let measured = measure_payload(Payload::Type(&nested), &mut budget, |_| {
            Ok::<_, Infallible>(())
        })
        .unwrap();
        assert_eq!(measured.owned_bytes, 64 * size_of::<Type<'_>>() as u64);
        assert_eq!(measured.nodes, 65);
        assert_eq!(measured.text_bytes, "BorrowedName".len() as u64);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    }
    assert_eq!(ledger.peak_retained_bytes(), 0);
    assert!(ledger.work_used(WorkDomain::Optional) > 64);
}

#[test]
fn signature_defaults_names_and_actual_vector_capacities_share_one_walk() {
    let mut defaults = Vec::with_capacity(11);
    defaults.extend([
        DefaultValue::String("default-text"),
        DefaultValue::NewClass {
            name: "Created",
            args: vec![DefaultValue::String("argument")],
        },
    ]);
    let default_capacity = defaults.capacity();
    let mut parameters = Vec::with_capacity(7);
    parameters.push(FunctionParameter {
        ty: Type::Record(Box::new(Type::Class("Stored"))),
        passing: ParameterPassing::Value,
        default: Some(DefaultValue::Array(defaults)),
    });
    let parameter_capacity = parameters.capacity();
    let signature = FunctionSignature {
        params: parameters,
        return_type: Box::new(Type::String),
    };
    let mut ledger = new_ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut signatures = 0;
        let measured = measure_payload(Payload::Signature(&signature), &mut budget, |node| {
            if let Payload::Signature(signature) = node {
                signatures += 1;
                signature.validate_parameters()?;
            }
            Ok::<_, &'static str>(())
        })
        .unwrap();
        assert_eq!(signatures, 1);
        let nested_arguments = match signature.params[0].default.as_ref().unwrap() {
            DefaultValue::Array(values) => match &values[1] {
                DefaultValue::NewClass { args, .. } => args.capacity(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        let expected = size_of::<FunctionSignature<'_>>()
            + 2 * size_of::<usize>()
            + parameter_capacity * size_of::<FunctionParameter<'_>>()
            + 2 * size_of::<Type<'_>>()
            + (default_capacity + nested_arguments) * size_of::<DefaultValue<'_>>();
        assert_eq!(measured.owned_bytes, expected as u64);
        assert_eq!(
            measured.text_bytes,
            ("default-text".len() + "Created".len() + "argument".len() + "Stored".len()) as u64
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    assert!(ledger.peak_retained_bytes() > 0);
}

#[test]
fn admission_and_visitor_refusals_drop_branch_workspace_before_return() {
    let invalid = Type::Function(FunctionType::new(FunctionSignature {
        params: vec![FunctionParameter {
            ty: Type::Int,
            passing: ParameterPassing::MutableReference,
            default: Some(DefaultValue::Int(1)),
        }],
        return_type: Box::new(Type::Int),
    }));
    let root = Type::Union(vec![Type::Int, invalid, Type::String]);
    let mut enough = new_ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut enough, WorkDomain::Optional)));
        let result = measure_payload(Payload::Type(&root), &mut budget, |node| {
            if let Payload::Signature(signature) = node {
                signature.validate_parameters()?;
            }
            Ok::<_, &'static str>(())
        });
        assert!(matches!(
            result,
            Err(PayloadError::Visitor(
                "mutable-reference parameters cannot have defaults"
            ))
        ));
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    }
    assert!(enough.peak_retained_bytes() > 0);
    assert_eq!(enough.retained_bytes(), 0);
    for (memory, work) in [(0, 100_000), (1_000_000, 0)] {
        let mut limited = new_ledger(memory, work);
        {
            let mut budget = AllocationBudget::new(Some((&mut limited, WorkDomain::Optional)));
            let result = measure_payload(Payload::Type(&root), &mut budget, |_| {
                Ok::<_, Infallible>(())
            });
            assert!(matches!(result, Err(PayloadError::Allocation(_))));
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        }
        assert_eq!(limited.retained_bytes(), 0);
    }
}
