use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

fn sites() -> (Module, String) {
    let mut module = Module::default();
    let root = module.root;
    let scope = module.regions[root.index()].scope;
    let mut expected = Vec::new();
    for index in 0..30 {
        let bound = binding(&mut module, root, index, &format!("function{index}"));
        let body = module.region(scope);
        let value = number(&mut module, index as f64);
        let output = host(&mut module, "capture");
        let effect = call(&mut module, output, vec![value], Invocation::Value);
        module.regions[body.index()].statements =
            vec![Statement::Evaluate(effect), Statement::Return(Some(value))];
        let function = FunctionId::new(module.functions.len());
        module.functions.push(Function {
            parameters: vec![],
            body,
            arrow: true,
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: Suspension::None,
        });
        let created = expr(&mut module, Expr::Function(function));
        module.regions[root.index()]
            .statements
            .push(Statement::Let {
                binding: bound,
                value: Some(created),
            });
        let callee = expr(&mut module, Expr::Binding(bound));
        let called = call(&mut module, callee, vec![], Invocation::Reference);
        expected.push(index);
        match index % 5 {
            0 => module.regions[root.index()]
                .statements
                .push(Statement::Evaluate(called)),
            1 => {
                let discarded = expr(
                    &mut module,
                    Expr::Unary {
                        op: Unary::Void,
                        value: called,
                    },
                );
                module.regions[root.index()]
                    .statements
                    .push(Statement::Evaluate(discarded));
            }
            2 | 3 => {
                let result = binding(&mut module, root, index + 30, &format!("result{index}"));
                let initializer = if index % 5 == 2 {
                    called
                } else {
                    number(&mut module, 0.0)
                };
                module.regions[root.index()]
                    .statements
                    .push(Statement::Let {
                        binding: result,
                        value: Some(initializer),
                    });
                if index % 5 == 3 {
                    let target = expr(&mut module, Expr::Binding(result));
                    let assigned = expr(
                        &mut module,
                        Expr::Assign {
                            target,
                            value: called,
                        },
                    );
                    let discarded = expr(
                        &mut module,
                        Expr::Unary {
                            op: Unary::Void,
                            value: assigned,
                        },
                    );
                    module.regions[root.index()]
                        .statements
                        .push(Statement::Evaluate(discarded));
                }
                let read = expr(&mut module, Expr::Binding(result));
                capture(&mut module, read);
                expected.push(index);
            }
            _ => {
                let outer = module.region(scope);
                module.regions[outer.index()].statements = vec![Statement::Return(Some(called))];
                let wrapper = FunctionId::new(module.functions.len());
                module.functions.push(Function {
                    parameters: vec![],
                    body: outer,
                    arrow: true,
                    name: FunctionName::Unobserved,
                    strict: false,
                    length: None,
                    suspension: Suspension::None,
                });
                let wrapper = expr(&mut module, Expr::Function(wrapper));
                let result = call(&mut module, wrapper, vec![], Invocation::Value);
                capture(&mut module, result);
                expected.push(index);
            }
        }
    }
    module.root_rows =
        vec![RootRow::new(0, Anchor::Anchored); module.regions[root.index()].statements.len()];
    module.verify().unwrap();
    (module, serde_json::to_string(&expected).unwrap())
}

fn ledger(bytes: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: bytes,
        },
    )
    .unwrap()
}

#[test]
fn indexed_call_sites_preserve_effects_and_release_each_rounds_scratch() {
    let (source, expected) = sites();
    assert_eq!(execute(&source, "", PrintPolicy::default()), expected);
    let mut ledger = ledger(1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut module = source.clone_in(&mut budget).unwrap();
        // Every selected/rejected site also compares against the independent
        // scan oracle inside the candidate lookup in test builds.
        assert_eq!(
            module.inline_single_calls(true, true, &mut budget).unwrap(),
            30
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        module.verify().unwrap();
        assert_eq!(execute(&module, "", PrintPolicy::default()), expected);
        assert_eq!(
            module.inline_single_calls(true, true, &mut budget).unwrap(),
            0
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(module);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn a_refused_call_index_leaves_the_tree_unchanged() {
    let (mut module, _) = sites();
    let before = module.render(PrintPolicy::default()).unwrap();
    let mut ledger = ledger(1);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        assert!(matches!(
            module.inline_single_calls(true, true, &mut budget),
            Err(AllocationError::Budget(_))
        ));
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    assert_eq!(module.render(PrintPolicy::default()).unwrap(), before);
    module.verify().unwrap();
}
