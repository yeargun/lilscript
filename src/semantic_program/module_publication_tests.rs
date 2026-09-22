//! Global module/header verification is admitted even when a source edit
//! changes one tiny anonymous unit and all payload tables remain small.
use super::facts::{CacheLimits, ExactValue, ValueKnowledge};
use super::publication::*;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind};
use std::mem::size_of;

const CLOSURES: usize = 8192;
const MEMORY: u64 = 256_000_000;

fn many_units<R>(inspect: impl FnOnce(Program<'_>, UnitId, UnitId, OpId, ValueId) -> R) -> R {
    let source = "()=>7;".repeat(CLOSURES);
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, &source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    assert_eq!(program.units().len(), CLOSURES + 1);
    assert!(program.cells().is_empty());
    assert!(program.types.len() <= 4);
    assert!(program.strings.len() <= 2);
    assert_eq!(program.modules().len(), 1);
    let mut closures = program
        .units()
        .iter()
        .filter(|unit| unit.data().kind == UnitKind::Closure);
    let changed = closures.next().unwrap().id();
    let unrelated = closures.next_back().unwrap().id();
    let data = program.unit(changed).unwrap();
    let (index, operation) = data
        .operations
        .iter()
        .enumerate()
        .find(|(_, operation)| {
            matches!(
                operation.kind,
                OperationKind::Constant(Constant::Integer(7))
            )
        })
        .unwrap();
    let operation_id = OpId::from_index(index).unwrap();
    let value = operation.result.unwrap();
    inspect(program, changed, unrelated, operation_id, value)
}

fn compilation<'src>(optional_bytes: u64) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                // Restrict only optional work; frontend adoption/cache ownership
                // still has the complete fixture's baseline memory allowance.
                baseline_retained_bytes: MEMORY - optional_bytes,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap()
}

fn cache(compilation: &mut Compilation<'_>) {
    compilation
        .enable_local_facts(
            CacheLimits {
                entries: 4,
                bytes: 100_000,
                result_bytes: 20_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
}

fn observe(
    compilation: &mut Compilation<'_>,
    source: SemanticId,
    unit: UnitId,
    value: ValueId,
    expected: i32,
    hit: bool,
) {
    compilation
        .with_local_facts(WorkDomain::Baseline, 1, |group| {
            let found = group
                .query(
                    source,
                    unit,
                    LocalFactsRequest {
                        work_quota: 10_000,
                        result_bytes: 20_000,
                    },
                )
                .unwrap();
            assert_eq!(found.cache_hit(), hit);
            assert_eq!(
                found.facts().value(value),
                ValueKnowledge::Exact(ExactValue::Integer(expected))
            );
        })
        .unwrap();
}

fn edit(
    compilation: &mut Compilation<'_>,
    source: SemanticId,
    unit: UnitId,
    operation: OpId,
    constant: Constant,
) -> Result<SemanticId, PublicationError> {
    let revision = compilation.view(source)?.unit_revision(unit).unwrap();
    let kind = OperationKind::Constant(constant);
    compilation.edit_source(
        source,
        &[UnitPatch {
            unit,
            expected_revision: revision,
            operations: &[OperationPatch {
                operation,
                kind: &kind,
                operands: &[],
            }],
            places: &[],
        }],
        WorkDomain::Optional,
    )
}

#[test]
fn sparse_edit_rejects_invalid_types_without_global_verifier_workspace() {
    many_units(|program, changed, unrelated, operation, value| {
        let count = program.units().len();
        // The pending snapshot copies these two all-unit handle arrays. 32KiB
        // permits fixed metadata and the complete changed-body verifier. The
        // historical global verifier arrays exceeded this allowance. Semantic
        // rejection must now be reachable while retained-fork handles stay paid.
        let pending_handles = count * (size_of::<FrozenUnit>() + size_of::<(WorkDomain, u64)>());
        let optional_bytes = pending_handles as u64 + 32 * 1024;
        let mut compilation = compilation(optional_bytes);
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        cache(&mut compilation);
        observe(&mut compilation, source, unrelated, value, 7, false);
        let view = compilation.view(source).unwrap();
        let before_revision = view.unit_revision(changed);
        let before_index = view.unit_uses(changed).unwrap() as *const _ as usize;
        let tables = view.tables_revision();
        let retained = compilation.ledger().retained_bytes();
        let work = compilation.ledger().work_used(WorkDomain::Optional);
        let result = edit(
            &mut compilation,
            source,
            changed,
            operation,
            Constant::Boolean(true),
        );
        assert!(
            matches!(result, Err(PublicationError::InvalidReplacement)),
            "fixed edits must reach type rejection within changed-body workspace: {result:?}"
        );
        assert!(compilation.ledger().work_used(WorkDomain::Optional) > work);
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        assert_eq!(
            compilation.ledger().retained_bytes_in(WorkDomain::Optional),
            0
        );
        assert_eq!(compilation.checkpoint_count(), 1);
        let view = compilation.view(source).unwrap();
        assert_eq!(view.unit_revision(changed), before_revision);
        assert_eq!(
            view.unit_uses(changed).unwrap() as *const _ as usize,
            before_index
        );
        assert_eq!(view.tables_revision(), tables);
        observe(&mut compilation, source, unrelated, value, 7, true);
        assert_eq!(compilation.finish().retained_bytes(), 0);
    });
}

#[test]
fn global_module_metadata_does_not_reanalyze_unrelated_bodies_or_leak_shared_tables() {
    for discard_base_first in [false, true] {
        many_units(|program, changed, unrelated, operation, value| {
            let count = program.units().len();
            let changed_operations = program.unit(changed).unwrap().operations.len();
            let mut compilation = compilation(MEMORY / 2);
            let source = compilation
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            cache(&mut compilation);
            observe(&mut compilation, source, changed, value, 7, false);
            observe(&mut compilation, source, unrelated, value, 7, false);
            let before = compilation.ledger().retained_bytes();
            assert!(
                matches!(
                    edit(
                        &mut compilation,
                        source,
                        changed,
                        operation,
                        Constant::Boolean(true)
                    ),
                    Err(PublicationError::InvalidReplacement)
                ),
                "the low-budget test targets a real verifier rejection"
            );
            assert_eq!(compilation.ledger().retained_bytes(), before);
            let old = compilation.view(source).unwrap();
            let tables = old.tables_revision();
            let old_unrelated = old.unit_revision(unrelated);
            let old_index = old.unit_uses(unrelated).unwrap() as *const _ as usize;
            let edited = edit(
                &mut compilation,
                source,
                changed,
                operation,
                Constant::Integer(19),
            )
            .unwrap();
            let view = compilation.view(edited).unwrap();
            let receipt = view.receipt();
            assert_eq!(receipt.verified_units, 1);
            assert_eq!(receipt.verified_operations, changed_operations);
            assert_eq!(receipt.index.rebuilt_units, 1);
            assert!(
                receipt.logical_work >= count as u64,
                "retaining a fork still pays for destination headers"
            );
            assert_eq!(view.tables_revision(), tables);
            assert_eq!(view.unit_revision(unrelated), old_unrelated);
            assert_eq!(
                view.unit_uses(unrelated).unwrap() as *const _ as usize,
                old_index
            );
            observe(&mut compilation, edited, unrelated, value, 7, true);
            observe(&mut compilation, edited, changed, value, 19, false);
            observe(&mut compilation, source, changed, value, 7, true);
            let (first, last) = if discard_base_first {
                (source, edited)
            } else {
                (edited, source)
            };
            compilation.discard(first).unwrap();
            assert_eq!(compilation.view(last).unwrap().unit_count(), count);
            compilation.discard(last).unwrap();
            compilation.discard_local_facts().unwrap();
            assert_eq!(compilation.checkpoint_count(), 0);
            // AllocationBudget uses Render for buffer creation/movement, also
            // in semantic verification. Only the two tiny changed bodies pay
            // this tariff, not the thousands of unrelated unit bodies.
            let allocation_work = compilation.ledger().work_by_kind(WorkKind::Render);
            assert!(allocation_work > 0 && allocation_work < count as u64);
            assert_eq!(compilation.ledger().work_by_kind(WorkKind::Codec), 0);
            assert_eq!(compilation.finish().retained_bytes(), 0);
        });
    }
}
