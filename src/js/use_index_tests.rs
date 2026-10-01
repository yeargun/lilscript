use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

fn cycle(count: usize) -> (Module, Vec<BindingId>) {
    let mut module = Module::default();
    let names = (0..count)
        .map(|n| binding(&mut module, RegionId::new(0), n as u32, &format!("f{n}")))
        .collect::<Vec<_>>();
    for n in 0..count {
        let body = module.region(ScopeId::new(0));
        let callee = expr(&mut module, Expr::Binding(names[(n + 1) % count]));
        let value = call(&mut module, callee, vec![], Invocation::Value);
        module.regions[body.index()]
            .statements
            .push(Statement::Return(Some(value)));
        let function = FunctionId::new(module.functions.len());
        module.functions.push(Function {
            parameters: vec![],
            rest: false,
            body,
            arrow: false,
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: Suspension::None,
        });
        module.regions[0].statements.push(Statement::Function {
            binding: names[n],
            function,
        });
    }
    (module, names)
}

#[test]
fn q2_use_index_closes_dead_chains_and_recursive_components_once() {
    for count in [1, 2, 64, 256] {
        let (mut module, _) = cycle(count);
        assert_eq!(
            module
                .prune_declarations(&mut AllocationBudget::new(None))
                .unwrap(),
            count
        );
        assert!(module.regions[0].statements.is_empty());
        assert_eq!(
            module
                .prune_declarations(&mut AllocationBudget::new(None))
                .unwrap(),
            0
        );
        // Break the cycle: a chain longer than the former four-pass ceiling.
        let (mut module, _) = cycle(count);
        let value = number(&mut module, 7.0);
        let last = module.functions.last().unwrap().body.index();
        module.regions[last].statements = vec![Statement::Return(Some(value))];
        assert_eq!(
            module
                .prune_declarations(&mut AllocationBudget::new(None))
                .unwrap(),
            count
        );
    }
}

#[test]
fn q2_use_index_preserves_exported_and_pinned_recursive_closures() {
    for pinned in [false, true] {
        let (mut module, names) = cycle(64);
        if pinned {
            module.bindings[names[19].index()].pinned = true;
        } else {
            module.exports.push(Export {
                binding: names[19],
                name: "invoke".into(),
            });
        }
        assert_eq!(
            module
                .prune_declarations(&mut AllocationBudget::new(None))
                .unwrap(),
            0
        );
        assert_eq!(module.regions[0].statements.len(), 64);
    }
}

#[test]
fn q2_use_index_preserves_effects_captures_and_parallel_delivery_rows() {
    let mut module = Module::default();
    let dead = binding(&mut module, RegionId::new(0), 0, "dead");
    let live = binding(&mut module, RegionId::new(0), 1, "live");
    let effect = binding(&mut module, RegionId::new(0), 2, "effect");
    let n = number(&mut module, 41.0);
    module.regions[0].statements.push(Statement::Let {
        binding: live,
        value: Some(n),
    });
    let n = number(&mut module, 99.0);
    module.regions[0].statements.push(Statement::Let {
        binding: dead,
        value: Some(n),
    });
    let body = module.region(ScopeId::new(0));
    let read = expr(&mut module, Expr::Binding(live));
    module.regions[body.index()]
        .statements
        .push(Statement::Return(Some(read)));
    module.functions.push(Function {
        parameters: vec![],
        rest: false,
        body,
        arrow: false,
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: Suspension::None,
    });
    let f = expr(&mut module, Expr::Function(FunctionId::new(0)));
    let invoke = host(&mut module, "invoke");
    let value = call(&mut module, invoke, vec![f], Invocation::Value);
    module.regions[0].statements.push(Statement::Let {
        binding: effect,
        value: Some(value),
    });
    let n = number(&mut module, 42.0);
    capture(&mut module, n);
    module.root_rows = (0..4).map(|n| RootRow::new(n, Anchor::Anchored)).collect();
    let expected = execute(
        &module,
        "function invoke(f){capture(f());return 1}",
        PrintPolicy::default(),
    );
    assert_eq!(expected, "[41,42]");
    module.open_journal();
    let before = module.clone();
    assert_eq!(
        module
            .prune_declarations(&mut AllocationBudget::new(None))
            .unwrap(),
        1
    );
    let journal = module.take_journal();
    module.check_journal(&before, &journal).unwrap();
    assert_eq!(
        module
            .root_rows
            .iter()
            .map(|row| row.module)
            .collect::<Vec<_>>(),
        [0, 2, 3]
    );
    assert_eq!(
        execute(
            &module,
            "function invoke(f){capture(f());return 1}",
            PrintPolicy::default()
        ),
        expected
    );
}

#[test]
fn q2_use_index_admission_is_atomic_and_releases_scratch() {
    let (original, _) = cycle(32);
    for (work, bytes, succeeds) in [
        (1, 1_000_000, false),
        (1_000_000, 128, false),
        (1_000_000, 1_000_000, true),
    ] {
        let mut ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: work,
                optional_work: 0,
                baseline_retained_bytes: 0,
                retained_bytes: bytes,
            },
        )
        .unwrap();
        let mut module = original.clone();
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            budget.retain(AllocationClass::Retained, 64).unwrap();
            let result = module.prune_declarations(&mut budget);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert_eq!(module.regions, original.regions);
            }
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), 64);
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn q2_delivery_edits_invalidate_selected_logical_assignment_proofs() {
    let mut module = Module::default();
    let flag = binding(&mut module, RegionId::new(0), 0, "flag");
    let initial = host(&mut module, "input");
    module.regions[0].statements.push(Statement::Let {
        binding: flag,
        value: Some(initial),
    });
    let condition = expr(&mut module, Expr::Binding(flag));
    let target = expr(&mut module, Expr::Binding(flag));
    let value = expr(&mut module, Expr::Literal(Literal::Bool(false)));
    let assign = expr(&mut module, Expr::Assign { target, value });
    let yes = module.region(ScopeId::new(0));
    module.regions[yes.index()]
        .statements
        .push(Statement::Evaluate(assign));
    module.regions[0].statements.push(Statement::If {
        condition,
        yes,
        no: None,
    });
    let read = expr(&mut module, Expr::Binding(flag));
    capture(&mut module, read);
    let hits = host(&mut module, "hits");
    capture(&mut module, hits);
    let mut budget = AllocationBudget::new(None);
    module.identify_spelling_sites(0, &mut budget).unwrap();
    let choices = ChoiceMap::SEEDS.with_all([(
        ChoiceKey {
            family: ChoiceFamily::LogicalAssignment,
            site: module.region_site(yes).unwrap(),
        },
        AltId(1),
    )]);
    module
        .form_spelling_choices(
            OutputFamilies::NONE,
            TargetRules::SEMANTIC,
            &choices,
            true,
            2022,
            &mut budget,
        )
        .unwrap();
    assert!(module
        .render(PrintPolicy::default())
        .unwrap()
        .contains("&&="));
    module.set_expression(target, Expr::Host("setter".into()));
    module.set_expression(
        assign,
        Expr::Call {
            callee: target,
            arguments: vec![value],
            invocation: Invocation::Value,
        },
    );
    module
        .refresh_print_choices(
            OutputFamilies::NONE,
            TargetRules::SEMANTIC,
            &choices,
            2022,
            &mut budget,
        )
        .unwrap();
    let setup = "const input=true;let hits=0;function setter(v){hits++;return v}";
    assert_eq!(execute(&module, setup, PrintPolicy::default()), "[true,1]");
}
