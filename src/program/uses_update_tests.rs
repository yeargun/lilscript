use super::*;
use crate::compilation_policy::{BudgetPlan, ResourceLimits};

fn budget(work: u64, memory: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}
fn checked(text: &str, run: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, text).unwrap();
    let facts = crate::analyze(&source).unwrap();
    let program = from_checked_source(&source, &facts).unwrap();
    program.verify().unwrap();
    run(&program);
}
fn cell(program: &Program<'_>, name: &str) -> CellId {
    CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == name)
            .unwrap(),
    )
    .unwrap()
}
fn function(program: &Program<'_>, name: &str) -> UnitId {
    let CellBinding::Function(unit) = program.cells[cell(program, name).index()].binding else {
        panic!("function");
    };
    unit
}
fn edit<'a>(program: &Program<'a>, unit: UnitId, apply: impl FnOnce(&mut UnitData)) -> Program<'a> {
    let mut changed = program.clone();
    let mut data = changed.units[unit.index()].clone().into_working();
    apply(data.get_mut());
    changed.units[unit.index()] = data.freeze();
    changed.verify().unwrap();
    changed
}
fn literal<'a>(program: &Program<'a>, unit: UnitId) -> Program<'a> {
    edit(program, unit, |data| {
        let operation = data
            .operations
            .iter_mut()
            .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(_))))
            .unwrap();
        operation.kind = OperationKind::Constant(Constant::Integer(42));
    })
}
fn contents(left: &UseIndex, right: &UseIndex, program: &Program<'_>) {
    assert!(left.valid_for(program));
    assert!(right.valid_for(program));
    for frozen in &program.units {
        let a = left.unit(frozen.id()).unwrap();
        let b = right.unit(frozen.id()).unwrap();
        assert_eq!(a.revision(), b.revision());
        assert_eq!(a.cell_uses(), b.cell_uses());
        assert_eq!(a.closures(), b.closures());
        assert_eq!(
            left.creators(frozen.id()).unwrap().sites(),
            right.creators(frozen.id()).unwrap().sites()
        );
        for id in 0..frozen.data().values.len() {
            let id = ValueId::from_index(id).unwrap();
            assert_eq!(a.value_uses(id), b.value_uses(id));
        }
        for id in 0..frozen.data().calls.len() {
            let id = CallId::from_index(id).unwrap();
            assert_eq!(a.call_operation(id), b.call_operation(id));
        }
    }
    for id in 0..program.cells.len() {
        let id = CellId::from_index(id).unwrap();
        assert_eq!(
            left.cell(id).unwrap().sites(),
            right.cell(id).unwrap().sites()
        );
        assert_eq!(
            left.cell(id).unwrap().reference_exposed(),
            right.cell(id).unwrap().reference_exposed()
        );
    }
}
const SIMPLE: &str =
    "int first(){return 1;}int second(){return 2;}int ignored(){return 3;}first();";

#[test]
fn prepared_install_reuses_shell_and_untouched_chunks_with_original_domain_cleanup() {
    checked(SIMPLE, |program| {
        let first = function(program, "first");
        let second = function(program, "second");
        let changed = literal(program, first);
        for baseline_first in [false, true] {
            let mut ledger = budget(10_000_000, 10_000_000);
            let baseline = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut active = baseline
                .updated(program, &[], &mut ledger, WorkDomain::Optional)
                .unwrap();
            let shell = (
                active.units.as_ptr(),
                active.cells.as_ptr(),
                active.creators.as_ptr(),
                active.exports.as_ptr(),
            );
            let untouched = active.unit(second).unwrap() as *const UnitUses;
            let old_first = baseline.unit(first).unwrap() as *const UnitUses;
            let prepared = active
                .prepare_replacements(&changed, &[first], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(prepared.valid_for(&active));
            assert_eq!(prepared.validation_work(), 1);
            assert_eq!(prepared.receipt().rebuilt_units, 1);
            assert_eq!(prepared.cell_changes().len(), 0);
            let work = ledger.work_used(WorkDomain::Optional);
            ledger
                .charge(WorkDomain::Optional, WorkKind::Analysis, 10_000_000 - work)
                .unwrap();
            active.install_replacements(prepared, &mut ledger);
            assert_eq!(
                ledger.work_used(WorkDomain::Optional),
                10_000_000,
                "install seeks no fresh work"
            );
            assert_eq!(
                shell,
                (
                    active.units.as_ptr(),
                    active.cells.as_ptr(),
                    active.creators.as_ptr(),
                    active.exports.as_ptr()
                )
            );
            assert_eq!(untouched, active.unit(second).unwrap() as *const UnitUses);
            assert_eq!(old_first, baseline.unit(first).unwrap() as *const UnitUses);
            assert!(baseline.valid_for(program));
            assert!(active.valid_for(&changed));
            if baseline_first {
                baseline.discard(&mut ledger).unwrap();
                active.discard(&mut ledger).unwrap();
            } else {
                active.discard(&mut ledger).unwrap();
                baseline.discard(&mut ledger).unwrap();
            }
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}

#[test]
fn prepared_owner_guard_rejects_wrong_and_stale_indexes_even_for_empty_changes() {
    checked(SIMPLE, |program| {
        let mut ledger = budget(10_000_000, 10_000_000);
        let mut original = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let other = UseIndex::build(program, &mut ledger, WorkDomain::Optional).unwrap();
        let first = original
            .prepare_replacements(program, &[], &mut ledger, WorkDomain::Optional)
            .unwrap();
        let stale = original
            .prepare_replacements(program, &[], &mut ledger, WorkDomain::Optional)
            .unwrap();
        assert!(!first.valid_for(&other));
        assert!(first.valid_for(&original));
        assert_eq!(first.receipt().rebuilt_units, 0);
        assert_eq!(first.cell_changes().len(), 0);
        let before = original
            .creators(function(program, "first"))
            .unwrap()
            .revision();
        original.install_replacements(first, &mut ledger);
        assert!(!stale.valid_for(&original));
        assert_eq!(
            original
                .creators(function(program, "first"))
                .unwrap()
                .revision(),
            before
        );
        stale.discard(&mut ledger).unwrap();
        other.discard(&mut ledger).unwrap();
        original.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn prepared_negative_reference_and_creator_sets_match_full_rebuild() {
    checked(
        "void bump(ref int x){x+=1;}int a=1;int b=2;bump(ref a);func()->int first(){return ()=>1;}func()->int second(){return ()=>2;}",
        |program| {
            let a = cell(program, "a");
            let b = cell(program, "b");
            let init = program.initialization[0];
            let changed = edit(program, init, |data| {
                let place = data
                    .places
                    .iter_mut()
                    .find(|place| **place == Place::Cell(a))
                    .unwrap();
                *place = Place::Cell(b);
            });
            let first = function(program, "first");
            let second = function(program, "second");
            let child = |unit: UnitId| {
                program
                    .unit(unit)
                    .unwrap()
                    .operations
                    .iter()
                    .find_map(|op| {
                        if let OperationKind::Closure(body) = op.kind {
                            Some(body)
                        } else {
                            None
                        }
                    })
                    .unwrap()
            };
            let old_child = child(first);
            let new_child = child(second);
            let changed = edit(&changed, first, |data| {
                let op = data
                    .operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Closure(_)))
                    .unwrap();
                op.kind = OperationKind::Closure(new_child);
            });
            let mut ledger = budget(10_000_000, 10_000_000);
            let mut active = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            assert!(active.cell(a).unwrap().reference_exposed());
            assert!(!active.cell(b).unwrap().reference_exposed());
            let old_stamp = active.creators(old_child).unwrap().revision();
            let prepared = active
                .prepare_replacements(&changed, &[init, first], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(
                prepared
                    .cell_changes()
                    .any(|(id, old, new)| id == a && old != new)
            );
            active.install_replacements(prepared, &mut ledger);
            assert!(!active.cell(a).unwrap().reference_exposed());
            assert!(active.cell(b).unwrap().reference_exposed());
            assert!(active.creators(old_child).unwrap().sites().is_empty());
            assert_ne!(active.creators(old_child).unwrap().revision(), old_stamp);
            assert_eq!(active.creators(new_child).unwrap().sites().len(), 2);
            let rebuilt = UseIndex::build(&changed, &mut ledger, WorkDomain::Optional).unwrap();
            contents(&active, &rebuilt, &changed);
            rebuilt.discard(&mut ledger).unwrap();
            active.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn prepared_denials_and_unwind_preserve_original_index_and_release_partial_chunks() {
    checked(SIMPLE, |program| {
        let first = function(program, "first");
        let changed = literal(program, first);
        let mut measuring = budget(10_000_000, 10_000_000);
        let index = UseIndex::build(program, &mut measuring, WorkDomain::Baseline).unwrap();
        let baseline_peak = measuring.peak_retained_bytes();
        let prepared = index
            .prepare_replacements(&changed, &[first], &mut measuring, WorkDomain::Optional)
            .unwrap();
        let peak = measuring.peak_retained_bytes();
        let work = measuring.work_used(WorkDomain::Optional);
        assert!(peak > baseline_peak);
        prepared.discard(&mut measuring).unwrap();
        index.discard(&mut measuring).unwrap();
        for (work_limit, memory_limit) in [
            (0, 10_000_000),
            (work / 2, 10_000_000),
            (work - 1, 10_000_000),
            (10_000_000, peak - 1),
        ] {
            let mut ledger = budget(work_limit, memory_limit);
            let index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let retained = ledger.retained_bytes();
            let pointer = index.units.as_ptr();
            assert!(matches!(
                index.prepare_replacements(&changed, &[first], &mut ledger, WorkDomain::Optional),
                Err(UseError::Budget(_))
            ));
            assert!(index.valid_for(program));
            assert_eq!(index.units.as_ptr(), pointer);
            assert_eq!(ledger.retained_bytes(), retained);
            index.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
        let mut ledger = budget(10_000_000, 10_000_000);
        let index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let retained = ledger.retained_bytes();
        let prepared = index
            .prepare_replacements(&changed, &[first], &mut ledger, WorkDomain::Optional)
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = Pending {
                update: Some(prepared),
                destination: None,
                ledger: &mut ledger,
            };
            panic!("abandon admitted replacement chunks");
        }));
        assert!(result.is_err());
        assert_eq!(ledger.retained_bytes(), retained);
        assert!(index.valid_for(program));
        index.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn prepared_fork_failure_consumes_only_its_temporary_ownership() {
    checked(SIMPLE, |program| {
        let first = function(program, "first");
        let second = function(program, "second");
        let changed = literal(program, first);
        let wrong = literal(program, second);
        let mut ledger = budget(10_000_000, 10_000_000);
        let index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let retained = ledger.retained_bytes();
        let prepared = index
            .prepare_replacements(&changed, &[first], &mut ledger, WorkDomain::Optional)
            .unwrap();
        assert!(matches!(
            index.fork_replacements(&wrong, prepared, &mut ledger, WorkDomain::Optional),
            Err(UseError::InvalidChangeSet)
        ));
        assert_eq!(ledger.retained_bytes(), retained);
        assert!(index.valid_for(program));
        index.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

fn unit_changes(
    previous: &Program<'_>,
    current: &Program<'_>,
    ids: &[UnitId],
) -> Vec<publication::UnitChange> {
    ids.iter()
        .map(|&unit| publication::UnitChange {
            unit,
            previous: previous.units[unit.index()].revision(),
            current: current.units[unit.index()].revision(),
        })
        .collect()
}

#[test]
fn sparse_cell_merge_keeps_equal_occurrences_from_another_changed_unit() {
    checked(
        "int a=1;int b=2;int first(){return a+b+1;}int second(){return a+2;}first();second();",
        |program| {
            let a = cell(program, "a");
            let b = cell(program, "b");
            let first = function(program, "first");
            let second = function(program, "second");
            let changed = edit(program, first, |data| {
                *data
                    .places
                    .iter_mut()
                    .find(|place| **place == Place::Cell(a))
                    .unwrap() = Place::Cell(b);
            });
            let changed = literal(&changed, second);
            let mut ledger = budget(10_000_000, 10_000_000);
            let mut index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let old_a = index.cell(a).unwrap().revision();
            let prepared = index
                .prepare_replacements(
                    &changed,
                    &[second, first],
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            assert!(
                prepared
                    .units
                    .iter()
                    .find(|row| row.unit == first)
                    .unwrap()
                    .cells_changed
            );
            assert!(
                !prepared
                    .units
                    .iter()
                    .find(|row| row.unit == second)
                    .unwrap()
                    .cells_changed
            );
            index.install_replacements(prepared, &mut ledger);
            assert!(index
                .cell(a)
                .unwrap()
                .sites()
                .iter()
                .any(|site| matches!(site,
            CellUseSite::Unit { unit, usage: CellUse::Read { .. } } if *unit == second)));
            assert!(!index
                .cell(a)
                .unwrap()
                .sites()
                .iter()
                .any(|site| matches!(site,
            CellUseSite::Unit { unit, usage: CellUse::Read { .. } } if *unit == first)));
            assert_ne!(index.cell(a).unwrap().revision(), old_a);
            let rebuilt = UseIndex::build(&changed, &mut ledger, WorkDomain::Optional).unwrap();
            contents(&index, &rebuilt, &changed);
            rebuilt.discard(&mut ledger).unwrap();
            index.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn sparse_creator_merge_keeps_equal_occurrences_from_another_changed_unit() {
    checked(
        "func()->int first(){return ()=>1;}func()->int second(){return ()=>2;}func()->int third(){return ()=>3;}",
        |program| {
            let first = function(program, "first");
            let second = function(program, "second");
            let third = function(program, "third");
            let child = |unit: UnitId| {
                program
                    .unit(unit)
                    .unwrap()
                    .operations
                    .iter()
                    .find_map(|operation| {
                        if let OperationKind::Closure(body) = operation.kind {
                            Some(body)
                        } else {
                            None
                        }
                    })
                    .unwrap()
            };
            let common = child(first);
            let destination = child(third);
            let baseline = edit(program, second, |data| {
                data.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Closure(_)))
                    .unwrap()
                    .kind = OperationKind::Closure(common);
            });
            let changed = edit(&baseline, first, |data| {
                data.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Closure(_)))
                    .unwrap()
                    .kind = OperationKind::Closure(destination);
            });
            // A genuine new revision with unchanged occurrences, as a kind-only
            // edit can also produce. It shares the affected body's creator set.
            let changed = edit(&changed, second, |_| {});
            let mut ledger = budget(10_000_000, 10_000_000);
            let mut index = UseIndex::build(&baseline, &mut ledger, WorkDomain::Baseline).unwrap();
            assert_eq!(index.creators(common).unwrap().sites().len(), 2);
            let prepared = index
                .prepare_replacements(
                    &changed,
                    &[first, second],
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            assert!(
                prepared
                    .units
                    .iter()
                    .find(|row| row.unit == first)
                    .unwrap()
                    .creators_changed
            );
            assert!(
                !prepared
                    .units
                    .iter()
                    .find(|row| row.unit == second)
                    .unwrap()
                    .creators_changed
            );
            index.install_replacements(prepared, &mut ledger);
            assert_eq!(index.creators(common).unwrap().sites().len(), 1);
            assert_eq!(index.creators(common).unwrap().sites()[0].unit, second);
            assert_eq!(index.creators(destination).unwrap().sites().len(), 2);
            let rebuilt = UseIndex::build(&changed, &mut ledger, WorkDomain::Optional).unwrap();
            contents(&index, &rebuilt, &changed);
            rebuilt.discard(&mut ledger).unwrap();
            index.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn general_preparation_sorts_declared_ids_and_keeps_undeclared_change_guards() {
    checked(SIMPLE, |program| {
        let first = function(program, "first");
        let second = function(program, "second");
        let ignored = function(program, "ignored");
        let changed = literal(&literal(program, first), second);
        let mut ledger = budget(10_000_000, 10_000_000);
        let index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let retained = ledger.retained_bytes();
        let prepared = index
            .prepare_replacements(
                &changed,
                &[ignored, second, first],
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
        assert_eq!(
            prepared
                .units
                .iter()
                .map(|row| row.unit)
                .collect::<Vec<_>>(),
            [first, second]
        );
        assert!(prepared
            .units
            .iter()
            .all(|row| !row.cells_changed && !row.creators_changed));
        assert_eq!(prepared.receipt().rebuilt_cell_sets, 0);
        assert_eq!(prepared.receipt().rebuilt_creator_sets, 0);
        prepared.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), retained);
        for declared in [
            &[first, first][..],
            &[
                first,
                second,
                UnitId::from_index(program.units.len()).unwrap(),
            ][..],
        ] {
            assert!(matches!(
                index.prepare_replacements(&changed, declared, &mut ledger, WorkDomain::Optional),
                Err(UseError::InvalidChangeSet)
            ));
            assert_eq!(ledger.retained_bytes(), retained);
        }
        assert!(
            matches!(index.prepare_replacements(&changed, &[first], &mut ledger, WorkDomain::Optional), Err(UseError::UndeclaredUnitChange(unit)) if unit == second)
        );
        assert_eq!(ledger.retained_bytes(), retained);
        index.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn fixed_footprint_validation_checks_order_and_both_revisions_without_table_scans() {
    let mut measured = None;
    for unrelated in [0, 128] {
        let mut source = SIMPLE.to_owned();
        for index in 0..unrelated {
            use std::fmt::Write;
            write!(source, "int unrelated{index}={index};int read{index}(){{return unrelated{index};}}read{index}();").unwrap();
        }
        checked(&source, |program| {
            let first = function(program, "first");
            let second = function(program, "second");
            let changed = literal(&literal(program, first), second);
            let changes = unit_changes(program, &changed, &[first, second]);
            let mut ledger = budget(10_000_000, 10_000_000);
            let index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let retained = ledger.retained_bytes();
            let before = ledger.work_used(WorkDomain::Optional);
            index
                .validate_fixed_changes(&changed, &changes, &mut ledger, WorkDomain::Optional)
                .unwrap();
            let work = ledger.work_used(WorkDomain::Optional) - before;
            assert_eq!(ledger.retained_bytes(), retained);
            if let Some(previous) = measured {
                assert_eq!(work, previous);
            } else {
                measured = Some(work);
            }
            let mut bad = changes.clone();
            bad.reverse();
            assert!(matches!(
                index.validate_fixed_changes(&changed, &bad, &mut ledger, WorkDomain::Optional),
                Err(UseError::InvalidChangeSet)
            ));
            let mut bad = changes.clone();
            bad[1] = bad[0];
            assert!(matches!(
                index.validate_fixed_changes(&changed, &bad, &mut ledger, WorkDomain::Optional),
                Err(UseError::InvalidChangeSet)
            ));
            for change_previous in [false, true] {
                let mut bad = changes.clone();
                if change_previous {
                    bad[0].previous = RevisionId::fresh();
                } else {
                    bad[0].current = RevisionId::fresh();
                }
                assert!(matches!(
                    index.validate_fixed_changes(&changed, &bad, &mut ledger, WorkDomain::Optional),
                    Err(UseError::InvalidChangeSet)
                ));
            }
            let mut bad = changes.clone();
            bad[0].current = bad[0].previous;
            assert!(matches!(
                index.validate_fixed_changes(&changed, &bad, &mut ledger, WorkDomain::Optional),
                Err(UseError::InvalidChangeSet)
            ));
            let mut changed_tables = changed.clone();
            changed_tables.tables_revision = RevisionId::fresh();
            assert!(matches!(
                index.validate_fixed_changes(
                    &changed_tables,
                    &changes,
                    &mut ledger,
                    WorkDomain::Optional
                ),
                Err(UseError::UnstampedTables)
            ));
            assert_eq!(ledger.retained_bytes(), retained);
            index.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn fixed_fork_row_check_rejects_another_valid_edit_against_the_same_original_index() {
    checked(SIMPLE, |program| {
        let first = function(program, "first");
        let changed = literal(program, first);
        let other = literal(program, first); // Equal bytes, distinct new unit revision.
        let changes = unit_changes(program, &changed, &[first]);
        let other_changes = unit_changes(program, &other, &[first]);
        let mut ledger = budget(10_000_000, 10_000_000);
        let index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let retained = ledger.retained_bytes();
        index
            .validate_fixed_changes(&changed, &changes, &mut ledger, WorkDomain::Optional)
            .unwrap();
        index
            .validate_fixed_changes(&other, &other_changes, &mut ledger, WorkDomain::Optional)
            .unwrap();
        let prepared = index
            .prepare_replacements(&changed, &[first], &mut ledger, WorkDomain::Optional)
            .unwrap();
        assert!(prepared.valid_for(&index));
        index
            .validate_fixed_prepared(&changes, &prepared, &mut ledger, WorkDomain::Optional)
            .unwrap();
        // Exercise the same owning fork and row guard as the capability entry;
        // no test constructs or exports a FixedUnitEdits capability.
        let result = index.fork_checked(
            &other,
            prepared,
            &mut ledger,
            WorkDomain::Optional,
            |prepared, ledger| {
                index.validate_fixed_prepared(
                    &other_changes,
                    prepared,
                    ledger,
                    WorkDomain::Optional,
                )
            },
        );
        assert!(matches!(result, Err(UseError::InvalidChangeSet)));
        assert_eq!(ledger.retained_bytes(), retained);
        assert!(index.valid_for(program));
        index.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn sparse_reverse_merge_refusals_release_all_new_chunks() {
    checked(
        "void bump(ref int x){x+=1;}int a=1;int b=2;bump(ref a);func()->int first(){return ()=>1;}func()->int second(){return ()=>2;}",
        |program| {
            let a = cell(program, "a");
            let b = cell(program, "b");
            let init = program.initialization[0];
            let first = function(program, "first");
            let second = function(program, "second");
            let destination = program
                .unit(second)
                .unwrap()
                .operations
                .iter()
                .find_map(|op| {
                    if let OperationKind::Closure(body) = op.kind {
                        Some(body)
                    } else {
                        None
                    }
                })
                .unwrap();
            let changed = edit(program, init, |data| {
                *data
                    .places
                    .iter_mut()
                    .find(|place| **place == Place::Cell(a))
                    .unwrap() = Place::Cell(b);
            });
            let changed = edit(&changed, first, |data| {
                data.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Closure(_)))
                    .unwrap()
                    .kind = OperationKind::Closure(destination);
            });
            let mut measuring = budget(10_000_000, 10_000_000);
            let index = UseIndex::build(program, &mut measuring, WorkDomain::Baseline).unwrap();
            let baseline_peak = measuring.peak_retained_bytes();
            let prepared = index
                .prepare_replacements(
                    &changed,
                    &[first, init],
                    &mut measuring,
                    WorkDomain::Optional,
                )
                .unwrap();
            assert!(prepared.receipt().rebuilt_cell_sets > 0);
            assert!(prepared.receipt().rebuilt_creator_sets > 0);
            let work = measuring.work_used(WorkDomain::Optional);
            let peak = measuring.peak_retained_bytes();
            assert!(peak > baseline_peak);
            prepared.discard(&mut measuring).unwrap();
            index.discard(&mut measuring).unwrap();
            for (work_limit, memory_limit) in [
                (0, 10_000_000),
                (work / 3, 10_000_000),
                (work * 2 / 3, 10_000_000),
                (work - 1, 10_000_000),
                (10_000_000, peak - 1),
            ] {
                let mut ledger = budget(work_limit, memory_limit);
                let index = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let retained = ledger.retained_bytes();
                assert!(matches!(
                    index.prepare_replacements(
                        &changed,
                        &[first, init],
                        &mut ledger,
                        WorkDomain::Optional
                    ),
                    Err(UseError::Budget(_))
                ));
                assert_eq!(ledger.retained_bytes(), retained);
                assert!(index.valid_for(program));
                assert!(index.cell(a).unwrap().reference_exposed());
                assert!(!index.cell(b).unwrap().reference_exposed());
                index.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        },
    );
}
