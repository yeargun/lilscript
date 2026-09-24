//! Ordered address checks do not create or coerce a projected leaf value.
use super::demand::{DemandMode, DemandPlan, EffectiveUseSite};
use super::facts::{operation_evaluation_behavior, primitive_result_domain};
use super::uses::{CellUse, UseIndex};
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};

const SOURCE: &str =
    "struct Inner{int x;}struct Outer{Inner inner;}Outer state=Outer{Inner{1}};state.inner.x=9;";

fn checked(inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program);
}
fn rewrite<'src>(
    program: &Program<'src>,
    unit: UnitId,
    edit: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut result = program.clone();
    let mut working = result.units[unit.index()].clone().into_working();
    edit(working.get_mut());
    result.units[unit.index()] = working.freeze();
    result
}
fn preflight(program: &Program<'_>) -> (UnitId, OpId, PlaceId, PlaceId, CellId) {
    let unit = program.initialization[0];
    let data = program.unit(unit).unwrap();
    let (index, mut root) = data
        .operations
        .iter()
        .enumerate()
        .find_map(|(index, operation)| {
            if let OperationKind::CheckPlace(place) = operation.kind {
                Some((index, place))
            } else {
                None
            }
        })
        .expect("source lowering emits an explicit preflight");
    let place = root;
    while let Place::Field { base, .. } = data.places[root.index()] {
        root = base;
    }
    let Place::Cell(cell) = data.places[root.index()] else {
        panic!("writable lexical root")
    };
    (unit, OpId::from_index(index).unwrap(), place, root, cell)
}
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}

#[test]
fn plain_projected_assignment_checks_the_address_before_rhs_without_loading_the_leaf() {
    checked(|program| {
        let (unit, check, place, _, _) = preflight(program);
        let data = program.unit(unit).unwrap();
        assert_eq!(
            data.operations
                .iter()
                .filter(|op| matches!(op.kind, OperationKind::CheckPlace(_)))
                .count(),
            1
        );
        let preflight = &data.operations[check.index()];
        assert!(preflight.result.is_none());
        assert!(data.operands(preflight.operands).unwrap().is_empty());
        let (store_index, store) = data
            .operations
            .iter()
            .enumerate()
            .find(|(_, op)| matches!(op.kind, OperationKind::Store(p) if p==place))
            .unwrap();
        let rhs = data.operands(store.operands).unwrap()[0];
        let schedule = &data.regions[preflight.region.index()].operations;
        let position = |operation| schedule.iter().position(|id| *id == operation).unwrap();
        assert!(position(check) < position(data.values[rhs.index()].definition));
        assert!(
            position(data.values[rhs.index()].definition)
                < position(OpId::from_index(store_index).unwrap())
        );
        assert!(
            !data
                .operations
                .iter()
                .any(|op| matches!(op.kind, OperationKind::Load(p) if p==place)),
            "preflight must not materialize a typed int leaf and invoke its normalization"
        );
    });
}

#[test]
fn checked_preflight_cannot_have_an_operand_or_value_result() {
    checked(|program| {
        let (unit, check, _, _, cell) = preflight(program);
        let data = program.unit(unit).unwrap();
        let initialized = data
            .operations
            .iter()
            .find(|op| matches!(op.kind, OperationKind::Initialize(c) if c==cell))
            .unwrap();
        let available = data.operands(initialized.operands).unwrap()[0];
        let with_operand = rewrite(program, unit, |data| {
            let start = data.operands.len() as u32;
            data.operands.push(available);
            data.operations[check.index()].operands = OperandRange { start, len: 1 };
        });
        assert!(with_operand
            .verify()
            .unwrap_err()
            .contains("operation signature mismatch"));
        let with_result = rewrite(program, unit, |data| {
            let result = ValueId::from_index(data.values.len()).unwrap();
            data.values.push(Value {
                ty: data.values[available.index()].ty,
                definition: check,
            });
            data.operations[check.index()].result = Some(result);
        });
        assert!(with_result
            .verify()
            .unwrap_err()
            .contains("operation signature mismatch"));
    });
}

#[test]
fn preflight_rejects_nonprojected_dangling_and_read_only_roots() {
    checked(|program| {
        let (unit, check, _, root, cell) = preflight(program);
        let direct = rewrite(program, unit, |data| {
            data.operations[check.index()].kind = OperationKind::CheckPlace(root);
        });
        assert!(direct
            .verify()
            .unwrap_err()
            .contains("writable stored field"));
        let dangling = rewrite(program, unit, |data| {
            data.operations[check.index()].kind =
                OperationKind::CheckPlace(PlaceId::from_index(data.places.len()).unwrap());
        });
        assert!(dangling
            .verify()
            .unwrap_err()
            .contains("dangling evaluated place"));
        let read_only = rewrite(program, unit, |data| {
            let initializer = data
                .operations
                .iter()
                .find(|op| matches!(op.kind, OperationKind::Initialize(c) if c==cell))
                .unwrap();
            let snapshot = data.operands(initializer.operands).unwrap()[0];
            data.places[root.index()] = Place::Value(snapshot);
        });
        assert!(read_only
            .verify()
            .unwrap_err()
            .contains("writable stored field"));
    });
}

#[test]
fn preflight_keeps_root_execution_and_dependencies_when_no_store_or_leaf_value_survives() {
    checked(|program| {
        let (unit, check, place, _, cell) = preflight(program);
        // Remove the final Store completely, retaining a valid earlier
        // CheckPlace and an unused RHS constant. The check must root itself.
        let program = rewrite(program, unit, |data| {
            let index = data.operations.len() - 1;
            let operation = OpId::from_index(index).unwrap();
            assert!(matches!(data.operations[index].kind, OperationKind::Store(p) if p==place));
            let region = data.operations[index].region;
            assert_eq!(
                data.regions[region.index()].operations.pop(),
                Some(operation)
            );
            data.operations.pop();
        });
        program.verify().unwrap();
        let data = program.unit(unit).unwrap();
        let rhs = data
            .operations
            .iter()
            .find_map(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Constant(Constant::Integer(9))
                )
                .then(|| operation.result.unwrap())
            })
            .unwrap();
        let effects = operation_evaluation_behavior(
            &program,
            None,
            unit,
            data,
            &data.operations[check.index()],
            &[],
        );
        assert!(effects.may_throw && effects.requires_evaluation());
        assert!(!primitive_result_domain(
            &program,
            data,
            &data.operations[check.index()],
            &[]
        ));
        let mut ledger = ledger();
        let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
        let references: Vec<_> = uses
            .unit(unit)
            .unwrap()
            .cell_uses()
            .iter()
            .filter(|site| {
                site.cell == cell
                    && matches!(site.usage, CellUse::Read { operation, .. } if operation==check)
            })
            .collect();
        assert_eq!(references.len(), 1);
        assert_eq!(
            references[0].usage,
            CellUse::Read {
                operation: check,
                place
            }
        );
        assert!(!uses
            .unit(unit)
            .unwrap()
            .cell_uses()
            .iter()
            .any(|site| matches!(site.usage, CellUse::Write { .. })));
        let policy = crate::config::ProjectConfig::default()
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let plan = DemandPlan::build(
            &program,
            Some(&uses),
            None,
            policy.javascript_contract().unwrap(),
            DemandMode::Prune,
            Some((&mut ledger, WorkDomain::Optional)),
        )
        .unwrap();
        let context = plan.root();
        assert!(plan.needs_operation(context, check));
        assert!(plan.needs_execution(context, check));
        assert!(!plan.needs_production(context, check));
        assert!(plan.needs_cell(context, cell));
        assert!(!plan.needs_value(context, rhs));
        let mut source_values = Vec::new();
        let mut steps = 0;
        plan.visit_effective_site_uses(
            context,
            EffectiveUseSite::Operation(check),
            |work| {
                steps += work;
                Ok::<_, ()>(())
            },
            |usage| {
                source_values.push(usage);
                Ok(())
            },
        )
        .unwrap();
        assert!(
            source_values.is_empty(),
            "address check must not create a leaf ValueId dependency"
        );
        assert!(
            steps >= 3,
            "the common cursor pays for the nested place path"
        );
        // With the Store removed, only CheckPlace has an indivisible field
        // path. Required SSA storage cannot conceal its target nesting cost.
        for limit in [crate::js::MAX_NESTING, 8] {
            let mut storage: Vec<_> = data
                .values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    if plan.needs_value(context, ValueId::from_index(index).unwrap())
                        && !matches!(
                            data.operations[value.definition.index()].kind,
                            OperationKind::Constant(_)
                        )
                    {
                        super::value_placement::ValueStorage::Required
                    } else {
                        super::value_placement::ValueStorage::Absent
                    }
                })
                .collect();
            let mut budget = crate::output_budget::AllocationBudget::new(Some((
                &mut ledger,
                WorkDomain::Optional,
            )));
            let placement = super::value_placement::plan(
                data,
                &plan,
                context,
                &mut storage,
                super::value_placement::PlacementDepth {
                    enclosing: 0,
                    limit,
                },
                &mut budget,
            );
            if limit == 8 {
                assert!(matches!(
                    placement,
                    Err(crate::output_budget::AllocationError::Capacity)
                ));
            } else {
                placement.unwrap();
            }
            assert_eq!(
                budget.retained_bytes(crate::output_budget::AllocationClass::Scratch),
                0
            );
        }
        plan.discard(Some(&mut ledger)).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
