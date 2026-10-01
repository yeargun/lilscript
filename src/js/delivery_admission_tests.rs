use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

fn ledger(work: u64, bytes: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: bytes,
        },
    )
    .unwrap()
}

fn fixture(writes: bool) -> (Module, EntryGraph) {
    let mut module = Module::default();
    let root = module.root.index();
    let binding = module.binding(Binding {
        source_symbol: None,
        scope: module.regions[root].scope,
        spelling: "counter".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let initial = module.expression(Expr::Literal(Literal::Number(1.0)), None);
    module.regions[root].statements.push(Statement::Let {
        binding,
        value: Some(initial),
    });
    module.root_rows.push(RootRow::new(0, Anchor::Anchored));
    for source in [1, 2] {
        let read = module.expression(Expr::Binding(binding), None);
        let value = if writes && source == 1 {
            module.expression(
                Expr::Assign {
                    target: read,
                    value: initial,
                },
                None,
            )
        } else {
            read
        };
        module.regions[root]
            .statements
            .push(Statement::Evaluate(value));
        module
            .root_rows
            .push(RootRow::new(source, Anchor::Anchored));
    }
    module.exports.push(Export {
        name: "counter".into(),
        binding,
    });
    module.entries = vec![
        EntryPublic {
            name: "first".into(),
            exports: vec![0],
        },
        EntryPublic {
            name: "second".into(),
            exports: vec![0],
        },
    ];
    let mut shared = EntrySet::single(0);
    shared.insert(1);
    let graph = EntryGraph {
        entries: vec![("first".into(), 1), ("second".into(), 2)],
        dynamic: vec![],
        imports: vec![vec![], vec![0], vec![0]],
        position: vec![0, 1, 2],
        orders: vec![vec![0, 1], vec![0, 2]],
        reach: vec![shared, EntrySet::single(0), EntrySet::single(1)],
        cycles: vec![None; 3],
        host_importers: vec![],
        paths: vec!["lib".into(), "first".into(), "second".into()],
    };
    (module, graph)
}

fn plan_bytes(plan: &DeliveryPlan) -> u64 {
    let mut bytes = vector_bytes(&plan.files).unwrap()
        + vector_bytes(&plan.entries).unwrap()
        + vector_bytes(&plan.setters).unwrap()
        + vector_bytes(&plan.entry_names).unwrap()
        + vector_bytes(&plan.naming).unwrap();
    for file in &plan.files {
        bytes += file.heap_bytes().unwrap();
    }
    for entry in &plan.entries {
        bytes += entry.name.capacity() as u64 + vector_bytes(&entry.closure).unwrap();
    }
    for name in &plan.entry_names {
        bytes += name.capacity() as u64;
    }
    for name in &plan.naming {
        bytes += (name.name.capacity()
            + name.template.capacity()
            + name.ext.capacity()
            + name.path.capacity()) as u64;
    }
    bytes
}

#[test]
fn q2_delivery_owns_exact_surviving_plan_storage_in_each_mode() {
    let (original, graph) = fixture(false);
    for mode in [
        DeliveryMode::Single,
        DeliveryMode::Split,
        DeliveryMode::PreserveModules,
    ] {
        let mut contract = DeliveryContract::single();
        contract.mode = mode;
        let mut inspection = original.clone();
        let expected = plan(
            &mut inspection,
            &graph,
            &contract,
            true,
            "js",
            &mut AllocationBudget::new(None),
        )
        .unwrap()
        .unwrap();
        let mut ledger = ledger(1_000_000, 1_000_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let mut module = original.clone_in(&mut budget).unwrap();
            let before = budget.retained_bytes(Retained);
            let planned = plan(&mut module, &graph, &contract, true, "js", &mut budget)
                .unwrap()
                .unwrap();
            assert_eq!(planned, expected, "{mode:?}");
            assert_eq!(module, original);
            assert_eq!(
                budget.retained_bytes(Retained),
                before + plan_bytes(&planned),
                "{mode:?}"
            );
            assert_eq!(budget.retained_bytes(Scratch), 0);
            drop(planned);
            drop(module);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn q2_delivery_setters_grow_the_existing_module_owner_and_refusals_release() {
    let (original, graph) = fixture(true);
    let mut contract = DeliveryContract::single();
    contract.mode = DeliveryMode::PreserveModules;
    let mut inspected = original.clone();
    let expected = plan(
        &mut inspected,
        &graph,
        &contract,
        true,
        "js",
        &mut AllocationBudget::new(None),
    )
    .unwrap()
    .unwrap();
    assert_eq!(expected.setters.len(), 1);
    let mut successes = 0;
    let mut refusals = 0;
    for (work, bytes) in [
        (1, 1_000_000),
        (100, 1_000_000),
        (500, 1_000_000),
        (1000, 1_000_000),
        (2000, 1_000_000),
        (1_000_000, 512),
        (1_000_000, 2048),
        (1_000_000, 4096),
        (1_000_000, 8192),
        (1_000_000, 1_000_000),
    ] {
        let mut ledger = ledger(work, bytes);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            match original.clone_in(&mut budget) {
                Err(AllocationError::Budget(_)) => {
                    refusals += 1;
                }
                Err(error) => panic!("unexpected copy error: {error:?}"),
                Ok(mut module) => {
                    match plan(&mut module, &graph, &contract, true, "js", &mut budget) {
                        Ok(Some(planned)) => {
                            assert_eq!(planned, expected);
                            assert_eq!(module, inspected);
                            assert_eq!(budget.retained_bytes(Scratch), 0);
                            successes += 1;
                            drop(planned);
                        }
                        Err(OutputError::Admission(AllocationError::Budget(_))) => {
                            refusals += 1;
                        }
                        other => panic!("unexpected planning result: {other:?}"),
                    }
                    drop(module);
                }
            }
        }
        assert_eq!(ledger.retained_bytes(), 0, "work={work} bytes={bytes}");
    }
    assert!(successes > 0 && refusals > 0);
}

#[test]
fn q2_delivery_failed_trial_preserves_the_parent_layout() {
    let (module, graph) = fixture(false);
    let mut ledger = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let facts = Facts::collect(&module, &|_| false, &mut budget).unwrap();
        let atoms = Atoms::build(&facts, &graph, false, &mut budget).unwrap();
        let labels = labels(&module, &facts, &atoms, &graph, &mut budget).unwrap();
        let mut layout = fallback(&facts, &atoms, &labels, &graph, &mut budget).unwrap();
        let positions = vec![vec![0, 1, NONE], vec![0, NONE, 1]];
        let context = Context {
            module: &module,
            facts: &facts,
            graph: &graph,
            hosted: &[],
            positions: &positions,
            namespaces: &[],
            preserve: false,
        };
        link(&context, &mut layout, &mut budget).unwrap();
        let reference = reference_orders(&context, &mut budget).unwrap();
        let before = layout.clone();
        let retained = budget.retained_bytes(Retained);
        // Moving shared effects into one entry loses them from the other entry.
        let shared = layout
            .files
            .iter()
            .position(|file| file.statements.contains(&0))
            .unwrap();
        assert!(!try_merge(&context, &mut layout, &reference, shared, 0, &mut budget).unwrap());
        assert_eq!(layout.files, before.files);
        assert_eq!(layout.file_of, before.file_of);
        assert_eq!(budget.retained_bytes(Retained), retained);
        assert_eq!(budget.retained_bytes(Scratch), 0);
        drop(reference);
        drop(layout);
        drop(labels);
        drop(atoms);
        drop(facts);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}
