use super::*;
use crate::compilation_policy::{BudgetPlan, ResourceLimits};
use std::process::Command;

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 100_000_000;
const SOURCE: &str = "int answer(){return 11;}int other(){return 7;}print(answer());";

fn checked(source: &str, inspect: impl FnOnce(Program<'_>, UnitId, OpId, UnitId)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let function = |name| {
        let cell = program.cells.iter().find(|cell| cell.name == name).unwrap();
        let CellBinding::Function(unit) = cell.binding else {
            panic!("function")
        };
        unit
    };
    let answer = function("answer");
    let other = function("other");
    let operation = OpId::from_index(
        program
            .unit(answer)
            .unwrap()
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(_))))
            .unwrap(),
    )
    .unwrap();
    inspect(program, answer, operation, other);
}
fn owner<'src>(limit: usize, memory: u64, deadline: bool) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits {
                wall_time_ms: deadline.then_some(100_000),
                ..ResourceLimits::default()
            },
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: limit },
    )
    .unwrap()
}
fn patch(
    compiler: &mut Compilation<'_>,
    base: SemanticId,
    unit: UnitId,
    operation: OpId,
    value: i32,
    advance: bool,
    domain: WorkDomain,
) -> Result<SemanticId, PublicationError> {
    let revision = compiler.view(base)?.unit_revision(unit).unwrap();
    let kind = OperationKind::Constant(Constant::Integer(value));
    let patches = [UnitPatch {
        unit,
        expected_revision: revision,
        operations: &[OperationPatch {
            operation,
            kind: &kind,
            operands: &[],
        }],
        places: &[],
    }];
    if advance {
        compiler.advance_source(base, &patches, domain)
    } else {
        compiler.edit_source(base, &patches, domain)
    }
}
fn observe(compiler: &mut Compilation<'_>, source: SemanticId) -> String {
    let javascript = compiler
        .with_semantic(source, |program, uses, _| {
            assert!(uses.valid_for(program));
            program
                .to_javascript()
                .unwrap()
                .render(crate::js::PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap()
        })
        .unwrap();
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &javascript])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}
fn addresses(compiler: &mut Compilation<'_>, id: SemanticId, unit: UnitId) -> [usize; 4] {
    compiler
        .with_semantic(id, |program, uses, _| {
            let data = program.unit(unit).unwrap();
            [
                program as *const _ as usize,
                uses as *const _ as usize,
                data.operations.as_ptr() as usize,
                data.operands.as_ptr() as usize,
            ]
        })
        .unwrap()
}
fn matches_rebuild(compiler: &mut Compilation<'_>, id: SemanticId) {
    compiler
        .with_semantic(id, |program, uses, ledger| {
            assert!(uses.valid_for(program));
            let rebuilt = UseIndex::build(program, ledger, WorkDomain::Baseline).unwrap();
            for unit in &program.units {
                let old = uses.unit(unit.id()).unwrap();
                let new = rebuilt.unit(unit.id()).unwrap();
                assert_eq!(old.cell_uses(), new.cell_uses());
                assert_eq!(old.closures(), new.closures());
                for value in 0..unit.data().values.len() {
                    let value = ValueId::from_index(value).unwrap();
                    assert_eq!(old.value_uses(value), new.value_uses(value));
                }
                for call in 0..unit.data().calls.len() {
                    let call = CallId::from_index(call).unwrap();
                    assert_eq!(old.call_operation(call), new.call_operation(call));
                }
                assert_eq!(
                    uses.creators(unit.id()).unwrap().sites(),
                    rebuilt.creators(unit.id()).unwrap().sites()
                );
            }
            for cell in 0..program.cells.len() {
                let cell = CellId::from_index(cell).unwrap();
                assert_eq!(
                    uses.cell(cell).unwrap().sites(),
                    rebuilt.cell(cell).unwrap().sites()
                );
                assert_eq!(
                    uses.cell(cell).unwrap().reference_exposed(),
                    rebuilt.cell(cell).unwrap().reference_exposed()
                );
            }
            rebuilt.discard(ledger).unwrap();
        })
        .unwrap();
}

const BATCH_SOURCE: &str = "int answer(){return 11+2;}int other(){return 7+3;}int untouched(){return 5;}print(answer());print(other());print(untouched());";

fn batch_sites(program: &Program<'_>, units: [UnitId; 2]) -> [(UnitId, OpId); 2] {
    assert!(units[0] < units[1]);
    units.map(|unit| {
        let operation = program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .position(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
            .unwrap();
        (unit, OpId::from_index(operation).unwrap())
    })
}

fn patch_batch(
    compiler: &mut Compilation<'_>,
    base: SemanticId,
    sites: [(UnitId, OpId); 2],
    invalid_second: bool,
    advance: bool,
    domain: WorkDomain,
) -> Result<SemanticId, PublicationError> {
    let kinds = [
        OperationKind::Constant(Constant::Integer(101)),
        OperationKind::Constant(if invalid_second {
            Constant::Boolean(true)
        } else {
            Constant::Integer(202)
        }),
    ];
    let revisions =
        sites.map(|(unit, _)| compiler.view(base).unwrap().unit_revision(unit).unwrap());
    let operations = std::array::from_fn::<_, 2, _>(|index| {
        [OperationPatch {
            operation: sites[index].1,
            kind: &kinds[index],
            operands: &[],
        }]
    });
    let patches = std::array::from_fn::<_, 2, _>(|index| UnitPatch {
        unit: sites[index].0,
        expected_revision: revisions[index],
        operations: &operations[index],
        places: &[],
    });
    if advance {
        compiler.advance_source(base, &patches, domain)
    } else {
        compiler.edit_source(base, &patches, domain)
    }
}

fn retained_batch_output(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
) -> (CandidateId, ArtifactId, String) {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    let policy = config
        .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let candidate = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let artifact = compiler
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&crate::js::selection::Plan::new(
                crate::js::selection::Style::Global,
            ))?;
            output.retain_artifact(artifact)
        })
        .unwrap()
        .unwrap();
    let text = compiler
        .with_artifact(artifact, |view| view.javascript.to_owned())
        .unwrap();
    (candidate, artifact, text)
}

fn assert_batch_output(compiler: &Compilation<'_>, artifact: ArtifactId, original: &str) {
    compiler
        .with_artifact(artifact, |view| {
            assert_eq!(view.javascript, original);
            let result =
                crate::program::native_tests::execute(Command::new("node").args([
                    "--input-type=module",
                    "-e",
                    view.javascript,
                ]));
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            assert_eq!(String::from_utf8(result.stdout).unwrap(), "13\n10\n5\n");
        })
        .unwrap();
}

#[test]
fn multi_unit_batch_fork_and_advance_preserve_old_outputs_and_unaffected_facts() {
    for (advance, retain_candidate) in [(false, true), (true, false), (true, true)] {
        checked(BATCH_SOURCE, |program, first, _, second| {
            let sites = batch_sites(&program, [first, second]);
            let CellBinding::Function(untouched) = program
                .cells
                .iter()
                .find(|cell| cell.name == "untouched")
                .unwrap()
                .binding
            else {
                panic!("function")
            };
            let mut compiler = owner(6, MEMORY, false);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let (candidate, artifact, original) = retained_batch_output(&mut compiler, source);
            if !retain_candidate {
                compiler.discard(candidate.semantic_id()).unwrap();
            }
            compiler
                .enable_local_facts(
                    CacheLimits {
                        entries: 4,
                        bytes: 100_000,
                        result_bytes: 20_000,
                    },
                    WorkDomain::Baseline,
                )
                .unwrap();
            let request = LocalFactsRequest {
                work_quota: 50_000,
                result_bytes: 20_000,
            };
            compiler
                .with_local_facts(WorkDomain::Baseline, 1, |session| {
                    assert!(!session
                        .query(source, untouched, request)
                        .unwrap()
                        .cache_hit());
                })
                .unwrap();
            let revisions = [first, second]
                .map(|unit| compiler.view(source).unwrap().unit_revision(unit).unwrap());
            let before = [first, second].map(|unit| addresses(&mut compiler, source, unit));
            let untouched_revision = compiler.view(source).unwrap().unit_revision(untouched);
            let next = patch_batch(
                &mut compiler,
                source,
                sites,
                false,
                advance,
                WorkDomain::Baseline,
            )
            .unwrap();
            let receipt = compiler.view(next).unwrap().receipt();
            assert_eq!(receipt.verified_units, 2);
            assert_eq!(receipt.index.rebuilt_units, 2);
            assert_eq!(
                (receipt.copied_units, receipt.reused_units),
                if advance && !retain_candidate {
                    (0, 2)
                } else {
                    (2, 0)
                }
            );
            assert_eq!(compiler.view(next).unwrap().changes().len(), 2);
            for (index, unit) in [first, second].into_iter().enumerate() {
                assert_ne!(
                    compiler.view(next).unwrap().unit_revision(unit),
                    Some(revisions[index])
                );
                if advance && !retain_candidate {
                    // Operand vectors are rebuilt, but the unique unit and
                    // operation backing remain the existing allocations.
                    assert_eq!(
                        addresses(&mut compiler, next, unit)[..3],
                        before[index][..3]
                    );
                }
                if retain_candidate {
                    assert_eq!(
                        compiler
                            .view(candidate.semantic_id())
                            .unwrap()
                            .unit_revision(unit),
                        Some(revisions[index])
                    );
                    assert_eq!(
                        addresses(&mut compiler, candidate.semantic_id(), unit),
                        before[index]
                    );
                }
            }
            if advance {
                assert!(compiler.view(source).is_err());
            } else {
                matches_rebuild(&mut compiler, source);
                assert_eq!(observe(&mut compiler, source), "13\n10\n5\n");
            }
            assert_eq!(
                compiler.view(next).unwrap().unit_revision(untouched),
                untouched_revision
            );
            compiler
                .with_local_facts(WorkDomain::Baseline, 1, |session| {
                    assert!(session.query(next, untouched, request).unwrap().cache_hit());
                })
                .unwrap();
            matches_rebuild(&mut compiler, next);
            assert_eq!(observe(&mut compiler, next), "101\n202\n5\n");
            compiler.discard(next).unwrap();
            if !advance {
                compiler.discard(source).unwrap();
            }
            if retain_candidate {
                compiler.discard(candidate.semantic_id()).unwrap();
            }
            assert_batch_output(&compiler, artifact, &original);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn multi_unit_batch_invalid_second_replacement_restores_both_mutated_units() {
    for advance in [false, true] {
        checked(BATCH_SOURCE, |program, first, _, second| {
            let sites = batch_sites(&program, [first, second]);
            let mut compiler = owner(4, MEMORY, false);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let (candidate, artifact, original) = retained_batch_output(&mut compiler, source);
            compiler.discard(candidate.semantic_id()).unwrap();
            let revisions =
                [first, second].map(|unit| compiler.view(source).unwrap().unit_revision(unit));
            let before = [first, second].map(|unit| addresses(&mut compiler, source, unit));
            let retained = compiler.ledger.retained_bytes();
            // Boolean is a well-formed replacement payload, but cannot produce
            // the second operation's checked int result. Both swaps precede verification.
            let result = patch_batch(
                &mut compiler,
                source,
                sites,
                true,
                advance,
                WorkDomain::Baseline,
            );
            assert!(
                matches!(result, Err(PublicationError::InvalidReplacement)),
                "{result:?}"
            );
            assert_eq!(compiler.checkpoint_count(), 1);
            assert_eq!(compiler.ledger.retained_bytes(), retained);
            for (index, unit) in [first, second].into_iter().enumerate() {
                assert_eq!(
                    compiler.view(source).unwrap().unit_revision(unit),
                    revisions[index]
                );
                assert_eq!(addresses(&mut compiler, source, unit), before[index]);
            }
            matches_rebuild(&mut compiler, source);
            assert_eq!(observe(&mut compiler, source), "13\n10\n5\n");
            assert_batch_output(&compiler, artifact, &original);
            let retry = patch_batch(
                &mut compiler,
                source,
                sites,
                false,
                advance,
                WorkDomain::Baseline,
            )
            .unwrap();
            matches_rebuild(&mut compiler, retry);
            assert_eq!(observe(&mut compiler, retry), "101\n202\n5\n");
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn multi_unit_batch_resource_refusal_and_unwind_preserve_all_original_owners() {
    let (mut successful_work, mut additional_retained) = (0, 0);
    checked(BATCH_SOURCE, |program, first, _, second| {
        let sites = batch_sites(&program, [first, second]);
        let mut compiler = owner(6, MEMORY, false);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let _retained = retained_batch_output(&mut compiler, source);
        let before = compiler.ledger.retained_bytes();
        let next = patch_batch(
            &mut compiler,
            source,
            sites,
            false,
            false,
            WorkDomain::Optional,
        )
        .unwrap();
        successful_work = compiler.view(next).unwrap().receipt().logical_work;
        additional_retained = compiler.ledger.retained_bytes() - before;
        assert!(successful_work > 1 && additional_retained > 1);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    for mode in 0..5 {
        checked(BATCH_SOURCE, |program, first, _, second| {
            let sites = batch_sites(&program, [first, second]);
            let mut compiler = owner(6, MEMORY, mode == 2);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let (candidate, artifact, original) = retained_batch_output(&mut compiler, source);
            let revisions =
                [first, second].map(|unit| compiler.view(source).unwrap().unit_revision(unit));
            let before = [first, second].map(|unit| addresses(&mut compiler, source, unit));
            let retained = compiler.ledger.retained_bytes();
            let padding = if mode == 1 {
                let padding = MEMORY - retained - (additional_retained - 1);
                compiler
                    .ledger
                    .retain(WorkDomain::Optional, padding)
                    .unwrap();
                padding
            } else {
                0
            };
            match mode {
                0 => compiler
                    .ledger
                    .charge(
                        WorkDomain::Optional,
                        WorkKind::Edit,
                        WORK - (successful_work - 1),
                    )
                    .unwrap(),
                2 => DEADLINE_AT.with(|phase| phase.set(Some(EditPhase::Prepared))),
                3 => PANIC_AT.with(|phase| phase.set(Some(EditPhase::Applied))),
                4 => PANIC_AT.with(|phase| phase.set(Some(EditPhase::Prepared))),
                _ => {}
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                patch_batch(
                    &mut compiler,
                    source,
                    sites,
                    false,
                    false,
                    WorkDomain::Optional,
                )
            }));
            if mode >= 3 {
                assert!(result.is_err(), "unwind mode {mode}");
            } else {
                let result = result.unwrap();
                assert!(
                    matches!(
                        result,
                        Err(PublicationError::Budget(_))
                            | Err(PublicationError::Uses(UseError::Budget(_)))
                    ),
                    "refusal mode {mode}: {result:?}"
                );
            }
            if padding != 0 {
                compiler
                    .ledger
                    .release(WorkDomain::Optional, padding)
                    .unwrap();
            }
            assert_eq!(compiler.ledger.retained_bytes(), retained);
            assert_eq!(compiler.checkpoint_count(), 2);
            for (index, unit) in [first, second].into_iter().enumerate() {
                assert_eq!(
                    compiler.view(source).unwrap().unit_revision(unit),
                    revisions[index]
                );
                assert_eq!(addresses(&mut compiler, source, unit), before[index]);
                assert_eq!(
                    compiler
                        .view(candidate.semantic_id())
                        .unwrap()
                        .unit_revision(unit),
                    revisions[index]
                );
                assert_eq!(
                    addresses(&mut compiler, candidate.semantic_id(), unit),
                    before[index]
                );
            }
            // Reset only the deterministic test clock to permit the independent
            // full-index oracle; rollback already completed under the expired deadline.
            if mode == 2 {
                compiler
                    .ledger
                    .set_deadline_elapsed_for_test(std::time::Duration::ZERO);
            }
            matches_rebuild(&mut compiler, source);
            matches_rebuild(&mut compiler, candidate.semantic_id());
            assert_eq!(observe(&mut compiler, source), "13\n10\n5\n");
            assert_batch_output(&compiler, artifact, &original);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn advance_reuses_one_slot_and_unique_arenas_without_changing_unrelated_facts() {
    checked(SOURCE, |program, unit, operation, other| {
        let mut compiler = owner(1, MEMORY, false);
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        compiler
            .enable_local_facts(
                CacheLimits {
                    entries: 4,
                    bytes: 100_000,
                    result_bytes: 20_000,
                },
                WorkDomain::Baseline,
            )
            .unwrap();
        let request = LocalFactsRequest {
            work_quota: 50_000,
            result_bytes: 20_000,
        };
        compiler
            .with_local_facts(WorkDomain::Baseline, 1, |session| {
                assert!(!session.query(base, other, request).unwrap().cache_hit());
            })
            .unwrap();
        let before = addresses(&mut compiler, base, unit);
        let unchanged = compiler.view(base).unwrap().unit_revision(other).unwrap();
        assert!(matches!(
            patch(
                &mut compiler,
                base,
                unit,
                operation,
                12,
                false,
                WorkDomain::Baseline
            ),
            Err(PublicationError::StoreFull)
        ));
        let next = patch(
            &mut compiler,
            base,
            unit,
            operation,
            12,
            true,
            WorkDomain::Baseline,
        )
        .unwrap();
        assert!(matches!(
            compiler.view(base),
            Err(PublicationError::UnknownCheckpoint)
        ));
        assert_eq!(compiler.checkpoint_count(), 1);
        assert_eq!(addresses(&mut compiler, next, unit), before);
        assert_eq!(
            compiler.view(next).unwrap().unit_revision(other),
            Some(unchanged)
        );
        let receipt = compiler.view(next).unwrap().receipt();
        assert_eq!(
            (
                receipt.copied_units,
                receipt.reused_units,
                receipt.copied_payload_bytes
            ),
            (0, 1, 0)
        );
        compiler
            .with_local_facts(WorkDomain::Baseline, 1, |session| {
                assert!(session.query(next, other, request).unwrap().cache_hit());
            })
            .unwrap();
        matches_rebuild(&mut compiler, next);
        assert_eq!(observe(&mut compiler, next), "12\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn same_edits_preserve_a_retained_baseline_and_copy_only_shared_successors() {
    for advance in [false, true] {
        checked(SOURCE, |program, unit, operation, _| {
            let mut compiler = owner(4, MEMORY, false);
            let baseline = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let mut current = patch(
                &mut compiler,
                baseline,
                unit,
                operation,
                12,
                false,
                WorkDomain::Baseline,
            )
            .unwrap();
            let mut copied = 0;
            for value in 13..21 {
                let next = patch(
                    &mut compiler,
                    current,
                    unit,
                    operation,
                    value,
                    advance,
                    WorkDomain::Baseline,
                )
                .unwrap();
                let receipt = compiler.view(next).unwrap().receipt();
                copied += receipt.copied_units;
                assert_eq!(receipt.reused_units, usize::from(advance));
                if !advance {
                    compiler.discard(current).unwrap();
                }
                current = next;
            }
            assert_eq!(copied, if advance { 0 } else { 8 });
            assert_eq!(observe(&mut compiler, baseline), "11\n");
            assert_eq!(observe(&mut compiler, current), "20\n");
            matches_rebuild(&mut compiler, current);
            compiler.discard(baseline).unwrap();
            compiler.discard(current).unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn retained_candidate_forces_copy_and_original_charge_domains_survive_both_drop_orders() {
    for oldest_first in [false, true] {
        checked(SOURCE, |program, unit, operation, _| {
            let mut compiler = owner(4, MEMORY, false);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let mut config = crate::config::ProjectConfig::default();
            config.javascript.strip_console = false;
            let policy = config
                .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                })
                .unwrap();
            let candidate = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            assert!(matches!(
                patch(
                    &mut compiler,
                    candidate.semantic_id(),
                    unit,
                    operation,
                    13,
                    true,
                    WorkDomain::Baseline
                ),
                Err(PublicationError::InvalidPatch)
            ));
            let next = patch(
                &mut compiler,
                source,
                unit,
                operation,
                12,
                true,
                WorkDomain::Optional,
            )
            .unwrap();
            assert_eq!(compiler.view(next).unwrap().receipt().copied_units, 1);
            assert_eq!(observe(&mut compiler, candidate.semantic_id()), "11\n");
            assert_eq!(observe(&mut compiler, next), "12\n");
            let after = patch(
                &mut compiler,
                next,
                unit,
                operation,
                13,
                true,
                WorkDomain::Optional,
            )
            .unwrap();
            assert_eq!(compiler.view(after).unwrap().receipt().reused_units, 1);
            let order = if oldest_first {
                [candidate.semantic_id(), after]
            } else {
                [after, candidate.semantic_id()]
            };
            for source in order {
                compiler.discard(source).unwrap();
            }
            let ledger = compiler.finish();
            assert_eq!(ledger.retained_bytes_in(WorkDomain::Baseline), 0);
            assert_eq!(ledger.retained_bytes_in(WorkDomain::Optional), 0);
        });
    }
}

#[test]
fn invalid_edits_and_unwind_restore_the_original_checkpoint_after_mutation() {
    for phase in [EditPhase::Applied, EditPhase::Verified, EditPhase::Prepared] {
        for shared in [false, true] {
            checked(SOURCE, |program, unit, operation, _| {
                let mut compiler = owner(3, MEMORY, false);
                let original = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let source = if shared {
                    patch(
                        &mut compiler,
                        original,
                        unit,
                        operation,
                        11,
                        false,
                        WorkDomain::Baseline,
                    )
                    .unwrap()
                } else {
                    original
                };
                if shared {
                    let policy = crate::config::ProjectConfig::default()
                        .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
                            preserve_root_exports: true,
                        })
                        .unwrap();
                    compiler
                        .direct_javascript(source, &policy, WorkDomain::Baseline)
                        .unwrap();
                }
                let before = compiler.ledger.retained_bytes();
                let revision = compiler.view(source).unwrap().unit_revision(unit);
                let address = addresses(&mut compiler, source, unit);
                PANIC_AT.with(|p| p.set(Some(phase)));
                let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = patch(
                        &mut compiler,
                        source,
                        unit,
                        operation,
                        99,
                        true,
                        WorkDomain::Baseline,
                    );
                }));
                assert!(failed.is_err());
                assert_eq!(compiler.ledger.retained_bytes(), before);
                assert_eq!(compiler.view(source).unwrap().unit_revision(unit), revision);
                assert_eq!(addresses(&mut compiler, source, unit), address);
                assert_eq!(observe(&mut compiler, source), "11\n");
                matches_rebuild(&mut compiler, source);
                assert_eq!(compiler.finish().retained_bytes(), 0);
            });
        }
    }
    checked(SOURCE, |program, unit, operation, _| {
        let mut compiler = owner(1, MEMORY, false);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let before = compiler.ledger.retained_bytes();
        let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
        let invalid = OperationKind::Return;
        let result = compiler.advance_source(
            source,
            &[UnitPatch {
                unit,
                expected_revision: revision,
                operations: &[OperationPatch {
                    operation,
                    kind: &invalid,
                    operands: &[],
                }],
                places: &[],
            }],
            WorkDomain::Baseline,
        );
        assert!(matches!(result, Err(PublicationError::InvalidReplacement)));
        assert_eq!(compiler.ledger.retained_bytes(), before);
        assert_eq!(
            compiler.view(source).unwrap().unit_revision(unit),
            Some(revision)
        );
        assert_eq!(observe(&mut compiler, source), "11\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn work_memory_and_deadline_refusals_preserve_valid_source_and_release_partial_owners() {
    let mut successful_work = 0;
    let mut successful_peak = 0;
    checked(SOURCE, |program, unit, operation, _| {
        let mut compiler = owner(1, MEMORY, false);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let next = patch(
            &mut compiler,
            source,
            unit,
            operation,
            12,
            true,
            WorkDomain::Baseline,
        )
        .unwrap();
        successful_work = compiler.view(next).unwrap().receipt().logical_work;
        successful_peak = compiler.ledger.peak_retained_bytes();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    for mode in 0..3 {
        checked(SOURCE, |program, unit, operation, _| {
            let mut compiler = owner(
                1,
                if mode == 1 {
                    successful_peak - 1
                } else {
                    MEMORY
                },
                mode == 2,
            );
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let before = compiler.ledger.retained_bytes();
            let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
            if mode == 0 {
                let spend =
                    WORK - compiler.ledger.work_used(WorkDomain::Baseline) - (successful_work - 1);
                compiler
                    .ledger
                    .charge(WorkDomain::Baseline, WorkKind::Edit, spend)
                    .unwrap();
            } else if mode == 2 {
                DEADLINE_AT.with(|phase| phase.set(Some(EditPhase::Prepared)));
            }
            let result = patch(
                &mut compiler,
                source,
                unit,
                operation,
                12,
                true,
                WorkDomain::Baseline,
            );
            assert!(matches!(
                result,
                Err(PublicationError::Budget(_)) | Err(PublicationError::Uses(UseError::Budget(_)))
            ));
            assert_eq!(compiler.ledger.retained_bytes(), before);
            assert_eq!(
                compiler.view(source).unwrap().unit_revision(unit),
                Some(revision)
            );
            assert_eq!(observe(&mut compiler, source), "11\n");
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn operand_rebuild_keeps_unedited_shared_ranges_and_rolls_back_every_original_range() {
    checked(
        "int answer(){int a=1;return a+a;}int other(){return 7;}print(answer());",
        |mut program, unit, _, _| {
            let data = program.units[unit.index()].unique_edit().unwrap().0;
            let binary = data
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::IntBinary(_)))
                .unwrap();
            let binary = OpId::from_index(binary).unwrap();
            let old_range = data.operations[binary.index()].operands;
            let value = data.operands[old_range.start as usize];
            // A valid return operand shares the binary input slice, with a gap in
            // the backing arena. Editing the binary must not rewrite this return.
            let returned = data
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Return))
                .unwrap();
            data.operations[returned].operands = OperandRange {
                start: old_range.start,
                len: 1,
            };
            data.operands.push(value);
            program.verify().unwrap();
            let mut compiler = owner(1, MEMORY, false);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
            let before_ranges: Vec<_> = compiler
                .view(source)
                .unwrap()
                .unit(unit)
                .unwrap()
                .operations
                .iter()
                .map(|op| op.operands)
                .collect();
            let before_values = compiler
                .view(source)
                .unwrap()
                .unit(unit)
                .unwrap()
                .operands
                .clone();
            let input = compiler
                .view(source)
                .unwrap()
                .unit(unit)
                .unwrap()
                .operations
                .iter()
                .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(1))))
                .unwrap()
                .result
                .unwrap();
            let kind = OperationKind::IntBinary(crate::primitive::IntBinary::Add);
            let patches = [UnitPatch {
                unit,
                expected_revision: revision,
                operations: &[OperationPatch {
                    operation: binary,
                    kind: &kind,
                    operands: &[input, input],
                }],
                places: &[],
            }];
            PANIC_AT.with(|p| p.set(Some(EditPhase::Applied)));
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = compiler.advance_source(source, &patches, WorkDomain::Baseline);
            }))
            .is_err());
            let restored = compiler.view(source).unwrap();
            assert_eq!(restored.unit(unit).unwrap().operands, before_values);
            assert_eq!(
                restored
                    .unit(unit)
                    .unwrap()
                    .operations
                    .iter()
                    .map(|op| op.operands)
                    .collect::<Vec<_>>(),
                before_ranges
            );
            let next = compiler
                .advance_source(source, &patches, WorkDomain::Baseline)
                .unwrap();
            assert_eq!(observe(&mut compiler, next), "1\n");
            matches_rebuild(&mut compiler, next);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        },
    );
}

#[test]
fn advancing_reference_place_updates_both_negative_exposure_facts_and_execution() {
    let source = "int a=1;int b=2;void mutate(ref int value){value=9;}int answer(){int ignored=11;int held=b;mutate(ref a);return a;}int other(){return 7;}print(answer());print(a);print(b);";
    checked(source, |program, unit, _, _| {
        let cell = |name| {
            CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == name)
                    .unwrap(),
            )
            .unwrap()
        };
        let a = cell("a");
        let b = cell("b");
        let data = program.unit(unit).unwrap();
        let place = data
            .call_arguments
            .iter()
            .find_map(|arg| match arg {
                CallArgument::Reference(place) if data.places[place.index()] == Place::Cell(a) => {
                    Some(*place)
                }
                _ => None,
            })
            .unwrap();
        let mut compiler = owner(2, MEMORY, false);
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let old = [a, b].map(|cell| {
            let view = compiler.view(base).unwrap();
            let users = view.cell_users(cell).unwrap();
            (users.revision(), users.reference_exposed())
        });
        assert_eq!([old[0].1, old[1].1], [true, false]);
        let render = |compiler: &mut Compilation<'_>, source| {
            let mut config = crate::config::ProjectConfig::default();
            config.javascript.strip_console = false;
            let policy = config
                .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                })
                .unwrap();
            let candidate = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let javascript = compiler
                .with_javascript_output(candidate, &policy, |output| {
                    let artifact = output.render(&crate::js::selection::Plan::new(
                        crate::js::selection::Style::Global,
                    ))?;
                    output.take_artifact(artifact)
                })
                .unwrap()
                .unwrap();
            compiler.discard(candidate.semantic_id()).unwrap();
            let result = Command::new("node")
                .args(["--input-type=module", "-e", &javascript])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{javascript}",
                String::from_utf8_lossy(&result.stderr)
            );
            String::from_utf8(result.stdout).unwrap()
        };
        assert_eq!(render(&mut compiler, base), "9\n9\n2\n");
        let revision = compiler.view(base).unwrap().unit_revision(unit).unwrap();
        let replacement = Place::Cell(b);
        let next = compiler
            .advance_source(
                base,
                &[UnitPatch {
                    unit,
                    expected_revision: revision,
                    operations: &[],
                    places: &[PlacePatch {
                        place,
                        replacement: &replacement,
                    }],
                }],
                WorkDomain::Baseline,
            )
            .unwrap();
        let view = compiler.view(next).unwrap();
        assert_eq!(view.receipt().reused_units, 1);
        for (index, cell) in [a, b].into_iter().enumerate() {
            let users = view.cell_users(cell).unwrap();
            assert_eq!(users.reference_exposed(), index == 1);
            assert_ne!(users.revision(), old[index].0);
            let change = view
                .cell_changes()
                .iter()
                .find(|change| change.cell == cell)
                .unwrap();
            assert_eq!(
                (change.previous, change.current),
                (old[index].0, users.revision())
            );
        }
        matches_rebuild(&mut compiler, next);
        assert_eq!(render(&mut compiler, next), "1\n1\n9\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
