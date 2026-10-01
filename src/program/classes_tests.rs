use super::*;

fn inspect(source: &str, check: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    check(&from_checked_source(&syntax, &semantics).unwrap());
}

#[test]
fn classes_follow_closed_producers_and_revision_owners() {
    inspect(
        "int pass(int value){return value;}int seed=2;print(pass(seed));",
        |program| {
            let closed = program.primitive_classes(Seal::Module);
            let open = program.primitive_classes(Seal::StructuralOnly);
            for (index, cell) in program.cells.iter().enumerate() {
                let id = CellId::from_index(index).unwrap();
                if cell.name == "seed" || cell.name == "value" {
                    assert!(closed.cell(id).primitive(), "{}", cell.name);
                    assert!(!open.cell(id).primitive(), "{}", cell.name);
                }
            }
            assert!(std::sync::Arc::ptr_eq(
                &closed,
                &program.primitive_classes(Seal::Module)
            ));
            let copy = program.clone();
            assert!(!std::sync::Arc::ptr_eq(
                &closed,
                &copy.primitive_classes(Seal::Module)
            ));
        },
    );
    inspect(
        "extern int host;int pass(int value){return value;}print(pass(host));",
        |program| {
            let classes = program.primitive_classes(Seal::Module);
            for (index, cell) in program.cells.iter().enumerate() {
                if cell.name == "host" || cell.name == "value" {
                    assert!(!classes.cell(CellId::from_index(index).unwrap()).primitive());
                }
            }
        },
    );
}

#[test]
fn q2_class_aggregate_and_default_owners_release_on_invalidation_and_refusal() {
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
    use crate::output_budget::{AllocationBudget, AllocationClass::Retained};
    let source =
        "int add(int n,int m=3){return n+m;}int[] items=[add(2),7];print(items[0]);print(add(4));";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let reference = from_checked_source(&syntax, &checked).unwrap();
    let classes = reference.primitive_classes(Seal::Module);
    let aggregates = reference.aggregates(Seal::Module);
    let defaults = super::defaults::plan(&reference, reference.effects(Seal::Module).graph());
    let mut outcomes = [false; 2];
    for (work, bytes) in [
        (64, 64 << 20),
        (100_000_000, 4096),
        (100_000_000, 8192),
        (100_000_000, 64 << 20),
    ] {
        let mut program = reference.clone();
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
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let result = (|| {
                let current = program.primitive_classes_in(Seal::Module, &mut budget)?;
                assert!(std::sync::Arc::ptr_eq(
                    &current,
                    &program.primitive_classes_in(Seal::Module, &mut budget)?
                ));
                for unit in program.units() {
                    for value in 0..unit.data().values.len() {
                        let value = ValueId::from_index(value).unwrap();
                        assert_eq!(
                            current.value(unit.id(), value),
                            classes.value(unit.id(), value)
                        );
                    }
                }
                let current = program.aggregates_in(Seal::Module, &mut budget)?;
                assert_eq!(current.complete, aggregates.complete);
                assert_eq!(current.sites.len(), aggregates.sites.len());
                for unit in program.units() {
                    for value in 0..unit.data().values.len() {
                        let value = ValueId::from_index(value).unwrap();
                        assert_eq!(
                            current.value(unit.id(), value),
                            aggregates.value(unit.id(), value)
                        );
                    }
                }
                drop(current);
                let before = budget.retained_bytes(Retained);
                let effects = program.effects_in(Seal::Module, &mut budget)?;
                budget.with_temporary(
                    |budget| super::defaults::plan_in(&program, effects.graph(), budget),
                    |plan, _| {
                        assert_eq!(plan.calls, defaults.calls);
                        assert_eq!(plan.lengths, defaults.lengths);
                        Ok::<_, crate::output_budget::AllocationError>(())
                    },
                )?;
                assert_eq!(
                    budget.retained_bytes(Retained),
                    before,
                    "default plan has no surviving storage"
                );
                drop(effects);
                Ok::<_, crate::output_budget::AllocationError>(())
            })();
            outcomes[usize::from(result.is_ok())] = true;
            if result.is_ok() && bytes == 64 << 20 {
                program.views.invalidate_in(&mut budget).unwrap();
                let next = program
                    .primitive_classes_in(Seal::Module, &mut budget)
                    .unwrap();
                assert!(next.deps().valid_for(&program));
                drop(next);
            }
            program.views.discard_in(&mut budget).unwrap();
            assert_eq!(budget.retained_bytes(Retained), 0);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
    assert_eq!(outcomes, [true, true]);
}

#[test]
fn q2_primitive_dependencies_cross_long_call_chains_without_global_sweeps() {
    let mut source = String::new();
    for index in 0..63 {
        source.push_str(&format!("int f{index}(){{return f{}();}}", index + 1));
    }
    source.push_str("int f63(){return 7;}print(f0());");
    inspect(&source, |program| {
        let classes = program.primitive_classes(Seal::Module);
        let graph = program.effects(Seal::Module);
        let mut known = 0;
        for unit in &program.units {
            for operation in &unit.data().operations {
                if let (OperationKind::Call(call), Some(value)) =
                    (&operation.kind, operation.result)
                {
                    if matches!(graph.graph().callee(unit.id(), *call), Callee::Unit(_)) {
                        assert!(
                            classes.value(unit.id(), value).primitive(),
                            "a closed constant-return chain stays primitive"
                        );
                        known += 1;
                    }
                }
            }
        }
        assert_eq!(known, 64);
    });
}

#[test]
fn q2_primitive_dependencies_join_every_writer_and_keep_open_roots_unknown() {
    inspect(
        "extern int host;int state=3;void update(){state=host;}update();print(state);",
        |program| {
            let classes = program.primitive_classes(Seal::Module);
            let state = program
                .cells
                .iter()
                .position(|cell| cell.name == "state")
                .unwrap();
            assert!(
                !classes.cell(CellId::from_index(state).unwrap()).primitive(),
                "one unknown writer blocks a primitive proof"
            );
        },
    );
    inspect(
        "int state=3;int read(){return state;}print(read());",
        |program| {
            let classes = program.primitive_classes(Seal::StructuralOnly);
            let state = program
                .cells
                .iter()
                .position(|cell| cell.name == "state")
                .unwrap();
            assert!(!classes.cell(CellId::from_index(state).unwrap()).primitive());
            for unit in &program.units {
                for operation in &unit.data().operations {
                    if let (OperationKind::Load(place), Some(value)) =
                        (&operation.kind, operation.result)
                    {
                        if matches!(unit.data().places[place.index()], Place::Cell(cell) if cell.index() == state)
                        {
                            assert!(!classes.value(unit.id(), value).primitive());
                        }
                    }
                }
            }
        },
    );
}
