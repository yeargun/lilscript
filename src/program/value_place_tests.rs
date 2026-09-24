//! Canonical evaluated places retain value snapshots and mutable storage roots.
use super::demand::{DemandMode, DemandPlan};
use super::uses::{CellUse, UseIndex, ValueUse};
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};

const SOURCE: &str = "struct Inner{int x;int y;}struct Outer{Inner inner;int z;}bool marker=true;Outer a=Outer{Inner{1,2},3};Outer b=a;b.inner.x=9;print(a.inner.x);print(b.inner.x);";

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program);
}

fn rewrite<'src>(
    program: &Program<'src>,
    unit: UnitId,
    change: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut result = program.clone();
    let mut working = result.units[unit.index()].clone().into_working();
    change(working.get_mut());
    result.units[unit.index()] = working.freeze();
    result
}

fn projected_store(program: &Program<'_>) -> (UnitId, OpId, PlaceId, PlaceId, CellId) {
    let unit = program.initialization[0];
    let data = program.unit(unit).unwrap();
    let (index, place) = data
        .operations
        .iter()
        .enumerate()
        .find_map(|(index, operation)| {
            if let OperationKind::Store(place) = operation.kind {
                matches!(data.places[place.index()], Place::Field { .. }).then_some((index, place))
            } else {
                None
            }
        })
        .unwrap();
    let mut root = place;
    while let Place::Field { base, .. } = data.places[root.index()] {
        root = base;
    }
    let Place::Cell(cell) = data.places[root.index()] else {
        panic!("fixture mutates a lexical product");
    };
    (unit, OpId::from_index(index).unwrap(), place, root, cell)
}

fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 1_000_000,
            optional_work: 1_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}

#[test]
fn nested_place_update_indexes_the_current_root_read_and_write_once() {
    checked(SOURCE, |program| {
        let (unit, operation, place, _, cell) = projected_store(program);
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Optional).unwrap();
        let actual = uses
            .unit(unit)
            .unwrap()
            .cell_uses()
            .iter()
            .filter(|site| site.cell == cell)
            .map(|site| site.usage)
            .collect::<Vec<_>>();
        assert_eq!(
            actual
                .iter()
                .filter(|&&usage| usage == CellUse::Read { operation, place })
                .count(),
            1
        );
        assert_eq!(
            actual
                .iter()
                .filter(|&&usage| usage == CellUse::Write { operation, place })
                .count(),
            1
        );
        let mut config = crate::config::ProjectConfig::default();
        config.javascript.strip_console = false;
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let contract = policy.javascript_contract().unwrap();
        let plan = DemandPlan::build(
            program,
            Some(&uses),
            None,
            contract,
            DemandMode::Prune,
            Some((&mut ledger, WorkDomain::Optional)),
        )
        .unwrap();
        assert!(
            plan.needs_execution(plan.root(), operation),
            "projected write stays conservative without a product-domain proof"
        );
        assert!(
            plan.needs_cell(plan.root(), cell),
            "required projected update must retain its mutable root"
        );
        plan.discard(Some(&mut ledger)).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn temporary_value_projection_loads_are_valid_and_index_the_snapshot() {
    checked(
        "struct Point{int x;}Point make(){return Point{7};}print(make().x);",
        |program| {
            let unit = program.initialization[0];
            let data = program.unit(unit).unwrap();
            let (place, value) = data
                .places
                .iter()
                .enumerate()
                .find_map(|(index, place)| {
                    if let Place::Field { base, .. } = place {
                        if let Place::Value(value) = data.places[base.index()] {
                            return Some((PlaceId::from_index(index).unwrap(), value));
                        }
                    }
                    None
                })
                .expect("temporary field read retains a read-only value root");
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            assert!(uses.unit(unit).unwrap().value_uses(value).unwrap().iter()
            .any(|usage| matches!(usage, ValueUse::PlaceReceiver { place: found, .. } if *found == place)));
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn even_unused_projection_bases_must_precede_their_places() {
    checked(SOURCE, |program| {
        let (unit, _, selected, _, _) = projected_store(program);
        let Place::Field { field, .. } = program.unit(unit).unwrap().places[selected.index()]
        else {
            unreachable!()
        };
        for offset in [0, 1] {
            let changed = rewrite(program, unit, |data| {
                let base = PlaceId::from_index(data.places.len() + offset).unwrap();
                data.places.push(Place::Field { base, field });
            });
            let error = changed.verify().unwrap_err();
            assert!(error.contains("base must precede"), "{error}");
        }
    });
}

#[test]
fn checked_edit_cannot_write_through_a_value_root() {
    checked(SOURCE, |program| {
        let (unit, _, _, root, cell) = projected_store(program);
        let data = program.unit(unit).unwrap();
        let initializer = data.operations.iter().find(|operation| matches!(operation.kind, OperationKind::Initialize(found) if found == cell)).unwrap();
        let value = data.operands(initializer.operands).unwrap()[0];
        let changed = rewrite(program, unit, |data| {
            data.places[root.index()] = Place::Value(value)
        });
        let error = changed.verify().unwrap_err();
        assert!(
            error.contains("read-only evaluated place")
                || error.contains("place check requires a writable stored field"),
            "{error}"
        );
    });
}

#[test]
fn checked_projection_owner_load_store_and_constructor_types_are_enforced() {
    checked(SOURCE, |program| {
        let (unit, store, place, _, _) = projected_store(program);
        let wrong_field = program
            .fields
            .iter()
            .find(|field| field.name == "z")
            .unwrap()
            .identity;
        let changed = rewrite(program, unit, |data| {
            let Place::Field { base, .. } = data.places[place.index()] else {
                unreachable!()
            };
            data.places[place.index()] = Place::Field {
                base,
                field: wrong_field,
            };
        });
        assert!(changed
            .verify()
            .unwrap_err()
            .contains("incompatible nominal owner"));
        let boolean = program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Constant(Constant::Boolean(_))
                )
            })
            .unwrap()
            .result
            .unwrap();
        let changed = rewrite(program, unit, |data| {
            let operand = data.operations[store.index()].operands.start as usize;
            data.operands[operand] = boolean;
        });
        assert!(changed.verify().unwrap_err().contains("type mismatch"));
        let changed = rewrite(program, unit, |data| {
            let allocation = data
                .operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Allocate {
                            kind: AllocationKind::Struct(_),
                            ..
                        }
                    )
                })
                .unwrap();
            data.operands[allocation.operands.start as usize] = boolean;
        });
        assert!(changed.verify().unwrap_err().contains("type mismatch"));
        let changed = rewrite(program, unit, |data| {
            let load = data.operations.iter().find(|operation| matches!(operation.kind,
                OperationKind::Load(found) if matches!(data.places[found.index()], Place::Field { .. }))).unwrap();
            let result = load.result.unwrap();
            data.values[result.index()].ty = data.values[boolean.index()].ty;
        });
        assert!(changed.verify().unwrap_err().contains("type mismatch"));
    });
}

#[test]
fn unavailable_value_place_receiver_is_rejected_at_its_use() {
    checked(
        "struct Point{int x;}Point make(){return Point{7};}print(make().x);Point later=Point{8};",
        |program| {
            let unit = program.initialization[0];
            let data = program.unit(unit).unwrap();
            let later = program
                .cells
                .iter()
                .position(|cell| cell.name == "later")
                .unwrap();
            let initializer = data
                .operations
                .iter()
                .find(|operation| {
                    matches!(operation.kind,
            OperationKind::Initialize(cell) if cell.index() == later)
                })
                .unwrap();
            let future = data.operands(initializer.operands).unwrap()[0];
            let base = data
                .places
                .iter()
                .enumerate()
                .find_map(|(index, place)| {
                    matches!(place, Place::Value(_)).then_some(PlaceId::from_index(index).unwrap())
                })
                .unwrap();
            let changed = rewrite(program, unit, |data| {
                data.places[base.index()] = Place::Value(future)
            });
            let error = changed.verify().unwrap_err();
            assert!(error.contains("place reads an unavailable"), "{error}");
        },
    );
}
