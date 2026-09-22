//! Direct target rules consume value occurrences, before naming or scoring.
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind};
use crate::output_budget::AllocationBudget;

fn project(module: &mut Module, elements: Vec<ExprId>, key: ExprId) -> ExprId {
    let object = expr(module, Expr::Array(elements));
    expr(
        module,
        Expr::Member {
            object,
            property: Property::Computed(key),
        },
    )
}

#[test]
fn literal_projection_preserves_raw_values_and_uses_no_new_target_storage() {
    for key in [0.0, -0.0, 1.0, 2.0, 3.0] {
        let mut module = Module::default();
        let elements = vec![
            number(&mut module, -0.0),
            number(&mut module, 4294967297.0),
            expr(&mut module, Expr::Literal(Literal::String("é\n".into()))),
            expr(&mut module, Expr::Literal(Literal::Null)),
        ];
        let key = number(&mut module, key);
        let value = project(&mut module, elements, key);
        capture(&mut module, value);
        let size = module.expressions.len();
        let folded = literal_array_projection(&module, value, &mut AllocationBudget::new(None))
            .unwrap()
            .unwrap();
        assert_eq!(module.expressions.len(), size);
        let before = execute(&module, "", PrintPolicy::default());
        let is_negative_zero = matches!(module.expressions[folded.index()],
            Expr::Literal(Literal::Number(value)) if value == 0.0 && value.is_sign_negative());
        if matches!(module.expressions[key.index()], Expr::Literal(Literal::Number(value)) if value == 0.0)
        {
            assert!(
                is_negative_zero,
                "projection must not normalize its raw value"
            );
        }
        rewrite::apply(
            &mut module,
            None,
            vec![(value.index(), rewrite::Replacement::Value(folded))],
        );
        assert_eq!(execute(&module, "", PrintPolicy::default()), before);
        verify::verify(&module).unwrap();
    }
}

#[test]
fn literal_projection_keeps_effects_and_absent_or_coercing_keys() {
    for key in [-1.0, 0.5, 2.0, 4294967295.0] {
        let mut module = Module::default();
        let a = number(&mut module, 11.0);
        let b = number(&mut module, 22.0);
        let key = number(&mut module, key);
        let value = project(&mut module, vec![a, b], key);
        assert!(
            literal_array_projection(&module, value, &mut AllocationBudget::new(None))
                .unwrap()
                .is_none()
        );
    }
    for effect in [false, true] {
        let mut module = Module::default();
        let first = number(&mut module, 11.0);
        let host = host(&mut module, "effect");
        let second = if effect {
            call(&mut module, host, vec![], Invocation::Value)
        } else {
            host
        };
        let key = number(&mut module, 0.0);
        let value = project(&mut module, vec![first, second], key);
        assert!(
            literal_array_projection(&module, value, &mut AllocationBudget::new(None))
                .unwrap()
                .is_none()
        );
    }
    let mut module = Module::default();
    let first = number(&mut module, 11.0);
    let key = host(&mut module, "key");
    let value = project(&mut module, vec![first], key);
    assert!(
        literal_array_projection(&module, value, &mut AllocationBudget::new(None))
            .unwrap()
            .is_none()
    );
}

#[test]
fn literal_projection_respects_the_existing_work_ledger_without_allocating() {
    let mut module = Module::default();
    let element = number(&mut module, 11.0);
    let key = number(&mut module, 0.0);
    let value = project(&mut module, vec![element; 64], key);
    for work in [0, 32, 65] {
        let mut ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: work,
                optional_work: 0,
                baseline_retained_bytes: 0,
                retained_bytes: 0,
            },
        )
        .unwrap();
        let result = literal_array_projection(
            &module,
            value,
            &mut AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline))),
        );
        assert_eq!(result.is_ok(), work == 65);
        assert_eq!(ledger.retained_bytes(), 0);
        if work == 65 {
            assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 65);
        }
    }
}
