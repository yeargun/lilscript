//! Target admission uses real checked programs/edits and the complete UseIndex.
//! These tests do not stand in for the separate native/JS lifetime cohort.
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

const FACTORY: &str = r#"
struct P {int current;int saved;}
func(int)->int make(int seed) {
    P state=P{seed,seed};
    P saved=state;
    return (int delta)=>{state.current+=delta;return state.current+saved.current;};
}
auto first=make(2);auto second=make(9);
print(first(3));print(second(4));print(first(1));
"#;

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program);
}

fn ledger(optional_work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work,
            baseline_retained_bytes: 0,
            retained_bytes: 32_000_000,
        },
    )
    .unwrap()
}

fn inspect_plan(
    program: &Program<'_>,
    inspect: impl FnOnce(Result<NativePlan<'_, '_>, NativeError>),
) {
    let mut ledger = ledger(10_000_000);
    let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
    let before = ledger.retained_bytes();
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        inspect(NativePlan::build(program, &uses, &mut budget));
    }
    assert_eq!(ledger.retained_bytes(), before);
    uses.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn captured_products_share_owned_cells_without_merging_independent_copies() {
    checked(FACTORY, |program| {
        inspect_plan(&program, |result| {
            let plan = result.unwrap();
            let captured = program
                .cells
                .iter()
                .enumerate()
                .filter_map(|(index, cell)| {
                    let id = CellId::from_index(index).unwrap();
                    plan.boxed_cell(id).then_some((id, cell.name.as_str()))
                })
                .collect::<Vec<_>>();
            assert_eq!(captured.len(), 2);
            assert!(captured.iter().any(|(_, name)| *name == "state"));
            assert!(captured.iter().any(|(_, name)| *name == "saved"));
            assert_ne!(captured[0].0, captured[1].0);
            let (body, unit) = program
                .units
                .iter()
                .find_map(|unit| {
                    (unit.data().kind == UnitKind::Closure).then_some((unit.id(), unit.data()))
                })
                .unwrap();
            assert!(plan.units[body.index()].has_environment);
            for (cell, _) in captured {
                let slot = plan.capture_slot(body, cell).unwrap();
                assert_eq!(unit.captures[slot], cell);
            }
            assert!(plan.needs_callable_runtime());
            assert!(plan.units.iter().any(|unit| unit
                .calls
                .iter()
                .any(|call| matches!(call, PreparedTarget::Callable { .. }))));
        });
    });
}

#[test]
fn nested_zero_capture_signatures_are_physical_values_and_static_route_stays_plain() {
    checked(
        "auto factory=()=>()=>()=>1;print(factory()()());",
        |program| {
            inspect_plan(&program, |result| {
                let plan = result.unwrap();
                assert!(plan.cells.iter().all(|cell| !cell.captured));
                assert!(plan.needs_callable_runtime());
                assert!(
                    plan.signatures
                        .iter()
                        .filter(|signature| signature.needed)
                        .count()
                        >= 3
                );
                for (index, signature) in plan.signatures.iter().enumerate() {
                    for ty in signature
                        .parameters
                        .iter()
                        .chain(std::iter::once(&signature.result))
                    {
                        if let NativeType::Callable(child) = ty {
                            assert!(*child < index);
                        }
                    }
                }
            });
        },
    );
    checked("int answer(){return 42;}print(answer());", |program| {
        inspect_plan(&program, |result| {
            let plan = result.unwrap();
            assert!(!plan.needs_callable_runtime());
            assert!(plan
                .units
                .iter()
                .all(|unit| !unit.has_environment && !unit.named_adapter_needed));
        });
    });
}

#[test]
fn a_checked_early_creation_edit_refuses_without_promoting_hoisted_storage_to_initialized() {
    checked(
        "func()->int make(){int state=1;return ()=>state;}print(make()());",
        |mut program| {
            let state = program
                .cells
                .iter()
                .position(|cell| cell.name == "state")
                .unwrap();
            let state = CellId::from_index(state).unwrap();
            let owner = program.cells[state.index()].owner;
            let mut working = program.units.remove(owner.index()).into_working();
            let data = working.get_mut();
            let initialize = data
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Initialize(cell) if cell == state))
                .unwrap();
            let creation = data
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Closure(_)))
                .unwrap();
            let creation = OpId::from_index(creation).unwrap();
            let region = data.operations[initialize].region;
            let operations = &mut data.regions[region.index()].operations;
            operations.retain(|&operation| operation != creation);
            let position = operations
                .iter()
                .position(|operation| operation.index() == initialize)
                .unwrap();
            operations.insert(position, creation);
            program.units.insert(owner.index(), working.freeze());
            program.verify().unwrap();
            inspect_plan(&program, |result| {
                assert!(matches!(
                    result,
                    Err(NativeError::Unsupported {
                        feature: "native cell access before initialization",
                        ..
                    })
                ))
            });
        },
    );
}

#[test]
fn captured_callable_payload_refuses_instead_of_leaking_an_unproved_ownership_graph() {
    checked(
        "func()->int make(){auto first=()=>1;return ()=>first();}print(make()());",
        |program| {
            inspect_plan(&program, |result| {
                assert!(matches!(
                    result,
                    Err(NativeError::Unsupported {
                        feature: "native captured dynamic callable payload",
                        ..
                    })
                ))
            });
        },
    );
}

#[test]
fn two_creators_of_one_body_keep_separate_creation_recipes() {
    checked(
        "func()->int make(bool choice){int state=1;return if(choice){()=>{state+=1;return state;}}else{()=>{state+=2;return state;}};}print(make(true)());print(make(false)());",
        |mut program| {
            let owner = program.cells.iter().find(|cell| cell.name == "state").unwrap().owner;
            let creations = program.unit(owner).unwrap().operations.iter().enumerate().filter_map(|(index, op)| match op.kind {
                OperationKind::Closure(body) => Some((index, body)),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(creations.len(), 2);
            let mut working = program.units.remove(owner.index()).into_working();
            working.get_mut().operations[creations[1].0].kind = OperationKind::Closure(creations[0].1);
            program.units.insert(owner.index(), working.freeze());
            program.verify().unwrap();
            inspect_plan(&program, |result| {
                let plan = result.unwrap();
                for (operation, _) in creations {
                    let result = program.unit(owner).unwrap().operations[operation].result.unwrap();
                    assert!(matches!(plan.units[owner.index()].values[result.index()], ValueStorage::Value(NativeType::Callable(_))));
                }
            });
        },
    );
}

#[test]
fn foreign_callback_binding_is_canonical_versioned_and_direct_only() {
    checked(
        "extern void keep(func()->int callback);func()->int make(){int state=1;return ()=>{state+=1;return state;};}keep(make());",
        |program| {
            let cell = CellId::from_index(program.cells.iter().position(|cell| cell.name == "keep").unwrap()).unwrap();
            let mut ledger = ledger(10_000_000);
            let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
            let before = ledger.retained_bytes();
            let rows = [NativeHostBinding { cell, link_name: "host_keep" }];
            let hosts = NativeHostBindings { callback_abi_version: 1, bindings: &rows };
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let plan = NativePlan::build_with_hosts(&program, &uses, &hosts, &mut budget).unwrap();
                assert!(matches!(plan.cell_storage(cell), ValueStorage::Host(0)));
                assert!(plan.units.iter().any(|unit| unit.calls.iter().any(|call| matches!(call, PreparedTarget::Host { binding: 0, .. }))));
            }
            assert_eq!(ledger.retained_bytes(), before);
            for (version, name, feature) in [
                (2, "host_keep", "native callback ABI version"),
                (1, "printf", "native callback provider symbol namespace"),
                (1, "host_", "native host link identifier"),
            ] {
                let rows = [NativeHostBinding { cell, link_name: name }];
                let hosts = NativeHostBindings { callback_abi_version: version, bindings: &rows };
                {
                    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    assert!(matches!(NativePlan::build_with_hosts(&program, &uses, &hosts, &mut budget), Err(NativeError::Unsupported { feature: actual, .. }) if actual == feature));
                }
                assert_eq!(ledger.retained_bytes(), before);
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn host_symbols_reject_only_aliases_and_wrappers_that_this_interface_emits() {
    for (source, collisions, legal) in [
        (
            "extern void first(func()->int callback);extern void second();first(()=>1);second();",
            &[
                "host_base_arg0",
                "host_base_arg0_call",
                "host_base_arg0_retain",
                "host_base_arg0_release",
                "host_base_result",
            ][..],
            &[
                "host_base_arg1",
                "host_base_arg00",
                "host_base_result_call",
                "host_base_ordinary_name",
            ][..],
        ),
        (
            "extern void first(int value);extern void second();first(1);second();",
            &["host_base_arg0", "host_base_result"][..],
            &[
                "host_base_arg0_call",
                "host_base_arg0_retain",
                "host_base_arg0_release",
                "host_base_arg1",
            ][..],
        ),
        (
            "extern func()->int first();extern void second();print(first()());second();",
            &[
                "host_base_result",
                "host_base_result_call",
                "host_base_result_retain",
                "host_base_result_release",
            ][..],
            &["host_base_arg0", "host_base_result_more"][..],
        ),
    ] {
        checked(source, |program| {
            let first = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "first")
                    .unwrap(),
            )
            .unwrap();
            let second = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "second")
                    .unwrap(),
            )
            .unwrap();
            let mut ledger = ledger(10_000_000);
            let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
            let before = ledger.retained_bytes();
            for (name, collision) in collisions
                .iter()
                .map(|&name| (name, true))
                .chain(legal.iter().map(|&name| (name, false)))
            {
                let mut rows = [
                    NativeHostBinding {
                        cell: first,
                        link_name: "host_base",
                    },
                    NativeHostBinding {
                        cell: second,
                        link_name: name,
                    },
                ];
                rows.sort_by_key(|row| row.cell);
                let hosts = NativeHostBindings {
                    callback_abi_version: 1,
                    bindings: &rows,
                };
                {
                    let mut budget =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    let result = NativePlan::build_with_hosts(&program, &uses, &hosts, &mut budget);
                    if collision {
                        assert!(
                            matches!(
                                result,
                                Err(NativeError::Unsupported {
                                    feature: "native host generated interface symbol collision",
                                    ..
                                })
                            ),
                            "{name}"
                        );
                    } else {
                        assert!(result.is_ok(), "legal provider {name}: {:?}", result.err());
                    }
                }
                assert_eq!(ledger.retained_bytes(), before, "{name}");
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
    // Provider order is CellId order, so check the reverse ownership direction
    // too: the longer spelling can be encountered before its base declaration.
    checked(
        "extern void first();extern void second(int value);first();second(1);",
        |program| {
            let mut rows = program
                .cells
                .iter()
                .enumerate()
                .filter_map(|(index, cell)| match cell.name.as_str() {
                    "first" => Some(NativeHostBinding {
                        cell: CellId::from_index(index).unwrap(),
                        link_name: "host_base_arg0",
                    }),
                    "second" => Some(NativeHostBinding {
                        cell: CellId::from_index(index).unwrap(),
                        link_name: "host_base",
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();
            rows.sort_by_key(|row| row.cell);
            let hosts = NativeHostBindings {
                callback_abi_version: 1,
                bindings: &rows,
            };
            let mut ledger = ledger(10_000_000);
            let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                assert!(matches!(
                    NativePlan::build_with_hosts(&program, &uses, &hosts, &mut budget),
                    Err(NativeError::Unsupported {
                        feature: "native host generated interface symbol collision",
                        ..
                    })
                ));
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn detached_formal_metadata_is_not_storage_and_a_new_read_requires_initialization() {
    checked(
        "extern void declaration(int metadata);int value=2;print(value);",
        |mut program| {
            let detached = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "metadata")
                    .unwrap(),
            )
            .unwrap();
            let value = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "value")
                    .unwrap(),
            )
            .unwrap();
            let foreign = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "declaration")
                    .unwrap(),
            )
            .unwrap();
            let rows = [NativeHostBinding {
                cell: foreign,
                link_name: "host_declaration",
            }];
            let hosts = NativeHostBindings {
                callback_abi_version: 1,
                bindings: &rows,
            };
            let mut ledger = ledger(10_000_000);
            let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
            assert!(uses.cell(detached).unwrap().sites().is_empty());
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let plan =
                    NativePlan::build_with_hosts(&program, &uses, &hosts, &mut budget).unwrap();
                assert!(!plan.boxed_cell(detached));
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);

            let owner = program.cells[value.index()].owner;
            let mut working = program.units.remove(owner.index()).into_working();
            let place = working
                .get_mut()
                .places
                .iter_mut()
                .find(|place| matches!(place, Place::Cell(cell) if *cell == value))
                .unwrap();
            *place = Place::Cell(detached);
            program.units.insert(owner.index(), working.freeze());
            program.verify().unwrap();
            let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
            assert!(uses
                .cell(detached)
                .unwrap()
                .sites()
                .iter()
                .any(|site| matches!(
                    site,
                    CellUseSite::Unit {
                        usage: CellUse::Read { .. },
                        ..
                    }
                )));
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                assert!(matches!(
                    NativePlan::build_with_hosts(&program, &uses, &hosts, &mut budget),
                    Err(NativeError::Unsupported {
                        feature: "native missing cell initialization",
                        ..
                    })
                ));
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn native_signature_refusal_and_unwind_release_the_actual_plan_buffers() {
    checked(FACTORY, |program| {
        let mut ledger = ledger(8);
        let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
        let before = ledger.retained_bytes();
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            assert!(matches!(
                NativePlan::build(&program, &uses, &mut budget),
                Err(NativeError::Allocation(_))
            ));
        }
        assert_eq!(ledger.retained_bytes(), before);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let plan = NativePlan::build(&program, &uses, &mut budget).unwrap();
            assert!(plan.needs_callable_runtime());
            panic!("native closure plan callback unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(ledger.retained_bytes(), before);
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
