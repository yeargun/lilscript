use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::js::mentions::StatementMentions;

fn ledger(bytes: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: bytes,
        },
    )
    .unwrap()
}

fn values(count: u32) -> (Module, Vec<BindingId>) {
    let mut module = Module::default();
    let root = module.root;
    let mut bindings = Vec::new();
    for index in 0..count {
        let bound = binding(&mut module, root, index, &format!("value{index}"));
        let value = number(&mut module, index as f64);
        let array = expr(&mut module, Expr::Array(vec![value]));
        module.regions[root.index()]
            .statements
            .push(Statement::Let {
                binding: bound,
                value: Some(array),
            });
        bindings.push(bound);
    }
    (module, bindings)
}

#[test]
fn forwarding_reuses_statement_mentions_across_chained_deletions_and_releases_sets() {
    let (mut source, bindings) = values(40);
    for binding in bindings {
        let read = expr(&mut source, Expr::Binding(binding));
        capture(&mut source, read);
    }
    source.root_rows = vec![RootRow::new(0, Anchor::Anchored); 80];
    source.verify().unwrap();
    let expected = serde_json::to_string(&(0..40).map(|n| vec![n]).collect::<Vec<_>>()).unwrap();
    assert_eq!(execute(&source, "", PrintPolicy::default()), expected);
    let mut ledger = ledger(2_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut module = source.clone_in(&mut budget).unwrap();
        let retained = budget.retained_bytes(AllocationClass::Retained);
        // Test builds independently scan every index answer, including after
        // a preceding move changed positions and replaced a receiver's tree.
        assert_eq!(module.forward_single_uses(&mut budget).unwrap().0, 40);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), retained);
        module.verify().unwrap();
        assert_eq!(execute(&module, "", PrintPolicy::default()), expected);
        assert_eq!(module.forward_single_uses(&mut budget).unwrap().0, 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), retained);
        drop(module);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn forwarding_invalidates_mentions_when_a_moved_closure_adds_a_capture() {
    let mut module = Module::default();
    let root = module.root;
    let scope = module.regions[root.index()].scope;
    let callback = binding(&mut module, root, 0, "callback");
    let seed = binding(&mut module, root, 1, "seed");
    let body = module.region(scope);
    let read = expr(&mut module, Expr::Binding(seed));
    module.regions[body.index()].statements = vec![Statement::Return(Some(read))];
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
    let ten = number(&mut module, 10.0);
    let array = expr(&mut module, Expr::Array(vec![ten]));
    module.regions[root.index()].statements = vec![
        Statement::Let {
            binding: callback,
            value: Some(created),
        },
        Statement::Let {
            binding: seed,
            value: Some(array),
        },
    ];
    let zero = number(&mut module, 0.0);
    capture(&mut module, zero);
    let read = expr(&mut module, Expr::Binding(callback));
    let consume = host(&mut module, "consume");
    let used = call(&mut module, consume, vec![read], Invocation::Value);
    module.regions[root.index()]
        .statements
        .push(Statement::Evaluate(used));
    module.root_rows = vec![RootRow::new(0, Anchor::Anchored); 4];
    let setup = "function consume(f){capture(f())}";
    module.verify().unwrap();
    assert_eq!(execute(&module, setup, PrintPolicy::default()), "[0,[10]]");
    let mut budget = AllocationBudget::new(None);
    assert_eq!(module.forward_single_uses(&mut budget).unwrap().0, 1);
    module.verify().unwrap();
    assert_eq!(execute(&module, setup, PrintPolicy::default()), "[0,[10]]");
}

#[test]
fn statement_mention_cache_refusal_preserves_source_and_releases_partial_storage() {
    let (mut module, bindings) = values(80);
    let reads = bindings
        .iter()
        .map(|&binding| expr(&mut module, Expr::Binding(binding)))
        .collect();
    let array = expr(&mut module, Expr::Array(reads));
    capture(&mut module, array);
    module.root_rows = vec![RootRow::new(0, Anchor::Anchored); 81];
    module.verify().unwrap();
    let before = module.render(PrintPolicy::default()).unwrap();
    let mut refused = false;
    let mut completed = false;
    for bytes in [1, 2_000, 2_200, 2_500, 4_000, 20_000] {
        let mut ledger = ledger(bytes);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let mut cache = StatementMentions::default();
            match cache.contains(&module, module.root.index(), 80, bindings[0], &mut budget) {
                Ok(found) => {
                    assert!(found);
                    assert!(cache
                        .contains(&module, module.root.index(), 80, bindings[79], &mut budget)
                        .unwrap());
                    completed = true;
                }
                Err(AllocationError::Budget(_)) => refused = true,
                other => panic!("unexpected lookup: {other:?}"),
            }
            cache.discard(&mut budget).unwrap();
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
        }
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(module.render(PrintPolicy::default()).unwrap(), before);
    }
    assert!(
        refused && completed,
        "cover both partial refusal and a completed cache"
    );
}
