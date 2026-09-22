//! Paid diagnostic descriptors use the same canonical encoder as search.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits,
    WorkDomain, WorkKind,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

const SOURCE: &str = "Record<int> state=record{x:1};print(state.x??0);";
const WORK: u64 = 1_000_000;
const MEMORY: u64 = 1_000_000;

fn policy() -> ResolvedPolicy {
    toml::from_str::<crate::config::ProjectConfig>(
        "[javascript]\nstrip_console=false\n[policy.tactics]\nscalar-replacement='on'",
    )
    .unwrap()
    .resolve_policy(CompilationRequest::JavaScript {
        preserve_root_exports: true,
    })
    .unwrap()
}

fn with_candidate(inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, CandidateId, CellId)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let cell = CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "state")
            .unwrap(),
    )
    .unwrap();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work: WORK,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 4 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let direct = compilation
        .direct_javascript(source, &policy(), WorkDomain::Baseline)
        .unwrap();
    inspect(&mut compilation, source, direct, cell);
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

fn descriptor(compilation: &mut Compilation<'_>, candidate: CandidateId) -> Vec<u32> {
    compilation
        .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| words.to_vec())
        .unwrap()
}

#[test]
fn completed_naming_variants_share_the_candidate_descriptor_after_source_disposal() {
    use crate::structured_js::selection::{Plan, Style};
    with_candidate(|compilation, source, candidate, _| {
        let policy = policy();
        let (expected, pointer) = compilation
            .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| {
                (words.to_vec(), words.as_ptr())
            })
            .unwrap();
        let [first, second] = [Style::Global, Style::Scoped].map(|style| {
            compilation
                .with_javascript_output(candidate, &policy, |output| {
                    let artifact = output.render(&Plan::new(style)).unwrap();
                    output
                        .with_artifact(artifact, |view| {
                            assert_eq!(view.implementation.recipe_words(), expected);
                            assert_eq!(view.implementation.recipe_words().as_ptr(), pointer);
                        })
                        .unwrap();
                    output.retain_artifact(artifact).unwrap()
                })
                .unwrap()
        });
        compilation.discard(candidate.semantic_id()).unwrap();
        compilation.discard(source).unwrap();
        compilation.discard_artifact(first).unwrap();
        let receipt = compilation
            .qualify_artifact(
                second,
                &policy,
                crate::config::CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        compilation
            .with_qualified_artifact(&receipt, |view, _| {
                assert_eq!(view.implementation.recipe_words(), expected);
                assert_eq!(view.implementation.recipe_words().as_ptr(), pointer);
            })
            .unwrap();
        assert!(!compilation
            .take_qualified_artifact(receipt)
            .unwrap()
            .is_empty());
    });
}

#[test]
fn descriptor_distinguishes_real_recipe_choices_without_retaining_another_candidate() {
    with_candidate(|compilation, _, direct, cell| {
        let scalar = match compilation
            .scalar_javascript(
                direct,
                cell,
                ScalarRequest {
                    max_work: 100_000,
                    scratch_bytes: 100_000,
                    output_bytes: 100_000,
                },
                &policy(),
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            ScalarOutcome::Published(candidate) => candidate,
            other => panic!("private record must qualify: {other:?}"),
        };
        let before = compilation.ledger().clone();
        let checkpoints = compilation.checkpoint_count();
        let direct_words = descriptor(compilation, direct);
        let scalar_words = descriptor(compilation, scalar);
        assert_ne!(direct_words, scalar_words);
        let cached_bytes = compilation.ledger().retained_bytes();
        assert!(cached_bytes > before.retained_bytes());
        assert_eq!(direct_words, descriptor(compilation, direct));
        assert_eq!(scalar_words, descriptor(compilation, scalar));
        assert_eq!(compilation.checkpoint_count(), checkpoints);
        assert_eq!(compilation.ledger().retained_bytes(), cached_bytes);
        assert!(
            compilation.ledger().work_used(WorkDomain::Optional)
                > before.work_used(WorkDomain::Optional)
        );
        assert_eq!(
            compilation.ledger().work_used(WorkDomain::Baseline),
            before.work_used(WorkDomain::Baseline)
        );
        assert_eq!(
            compilation.ledger().work_by_kind(WorkKind::Codec),
            before.work_by_kind(WorkKind::Codec)
        );
    });
}

#[test]
fn descriptor_callback_error_and_unwind_preserve_the_shared_original_domain() {
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        with_candidate(|compilation, _, candidate, _| {
            let before = compilation.ledger().clone();
            let result = compilation
                .with_recipe_descriptor(candidate, domain, |words| {
                    assert!(!words.is_empty());
                    Err::<(), _>("caller error")
                })
                .unwrap();
            assert_eq!(result, Err("caller error"));
            let cached = compilation.ledger().clone();
            assert!(cached.retained_bytes() > before.retained_bytes());
            let interrupted = catch_unwind(AssertUnwindSafe(|| {
                compilation
                    .with_recipe_descriptor(candidate, domain, |words| {
                        assert!(!words.is_empty());
                        panic!("descriptor caller interrupted");
                    })
                    .unwrap();
            }));
            assert!(interrupted.is_err());
            for checked in [WorkDomain::Baseline, WorkDomain::Optional] {
                assert_eq!(
                    compilation.ledger().retained_bytes_in(checked),
                    cached.retained_bytes_in(checked)
                );
                if checked == domain {
                    assert!(compilation.ledger().work_used(checked) > before.work_used(checked));
                } else {
                    assert_eq!(
                        compilation.ledger().work_used(checked),
                        before.work_used(checked)
                    );
                }
            }
            assert!(!descriptor(compilation, candidate).is_empty());
        });
    }
}

#[test]
fn descriptor_work_and_memory_denial_never_call_the_observer_or_leak_backing() {
    for memory in [false, true] {
        with_candidate(|compilation, source, candidate, _| {
            let before_probe = compilation.ledger().work_used(WorkDomain::Optional);
            let expected = descriptor(compilation, candidate);
            let required = compilation.ledger().work_used(WorkDomain::Optional) - before_probe;
            assert!(required > 2);
            // A different unpublished descriptor must still pass all admission;
            // querying the warmed candidate would need no new backing.
            let candidate = compilation
                .direct_javascript(source, &policy(), WorkDomain::Baseline)
                .unwrap();
            let padding = compilation
                .with_semantic(source, |_, _, ledger| {
                    if memory {
                        let padding = MEMORY - ledger.retained_bytes();
                        ledger.retain(WorkDomain::Optional, padding).unwrap();
                        padding
                    } else {
                        let consume =
                            WORK - ledger.work_used(WorkDomain::Optional) - (required - 1);
                        ledger
                            .charge(WorkDomain::Optional, WorkKind::Analysis, consume)
                            .unwrap();
                        0
                    }
                })
                .unwrap();
            let before = compilation.ledger().clone();
            let mut called = false;
            let result =
                compilation
                    .with_recipe_descriptor(candidate, WorkDomain::Optional, |_| called = true);
            assert!(!called);
            let wanted = if memory {
                BudgetError::MemoryExhausted(WorkDomain::Optional)
            } else {
                BudgetError::WorkExhausted(WorkDomain::Optional)
            };
            assert!(matches!(result, Err(CandidateError::Budget(error)) if error == wanted));
            assert_eq!(
                compilation.ledger().retained_bytes(),
                before.retained_bytes()
            );
            assert_eq!(
                compilation.ledger().work_used(WorkDomain::Baseline),
                before.work_used(WorkDomain::Baseline)
            );
            if padding != 0 {
                compilation
                    .with_semantic(source, |_, _, ledger| {
                        ledger.release(WorkDomain::Optional, padding).unwrap()
                    })
                    .unwrap();
            }
            let actual = compilation
                .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| words.to_vec())
                .unwrap();
            assert_eq!(actual, expected);
        });
    }
}

#[test]
fn descriptor_rejects_foreign_and_recycled_candidate_handles_before_admission() {
    with_candidate(|compilation, source, candidate, _| {
        with_candidate(|_, _, foreign, _| {
            let before = compilation.ledger().clone();
            assert!(matches!(
                compilation.with_recipe_descriptor(foreign, WorkDomain::Optional, |_| panic!(
                    "foreign observer called"
                )),
                Err(CandidateError::Publication(
                    PublicationError::UnknownCheckpoint
                ))
            ));
            assert_eq!(
                compilation.ledger().work_used(WorkDomain::Optional),
                before.work_used(WorkDomain::Optional)
            );
            assert_eq!(
                compilation.ledger().retained_bytes(),
                before.retained_bytes()
            );
        });
        let expected = descriptor(compilation, candidate);
        compilation.discard(candidate.semantic_id()).unwrap();
        let replacement = compilation
            .direct_javascript(source, &policy(), WorkDomain::Baseline)
            .unwrap();
        let before = compilation.ledger().clone();
        assert!(matches!(
            compilation.with_recipe_descriptor(candidate, WorkDomain::Optional, |_| panic!(
                "stale observer called"
            )),
            Err(CandidateError::Publication(
                PublicationError::UnknownCheckpoint
            ))
        ));
        assert_eq!(
            compilation.ledger().work_used(WorkDomain::Optional),
            before.work_used(WorkDomain::Optional)
        );
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
        assert_eq!(descriptor(compilation, replacement), expected);
    });
}
