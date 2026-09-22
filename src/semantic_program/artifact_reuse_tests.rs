//! Exact-score reuse borrows live artifact bytes without merging their owners.
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::semantic_program::publication::{CheckpointLimit, Compilation};
use crate::structured_js::selection::Style;

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 128_000_000;

fn with_candidates(inspect: impl FnOnce(&mut Compilation<'_>, [CandidateId; 2], &ResolvedPolicy)) {
    let arena = bumpalo::Bump::new();
    let syntax =
        crate::parse_source(&arena, "export int value(int input){return input*3+1;}").unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = crate::semantic_program::from_checked_source(&syntax, &checked).unwrap();
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
    let policy = crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let candidates = std::array::from_fn(|_| {
        compilation
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap()
    });
    assert_ne!(candidates[0], candidates[1]);
    inspect(&mut compilation, candidates, &policy);
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

fn render(
    compilation: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
) -> ArtifactId {
    compilation
        .with_javascript_output_in(candidate, policy, WorkDomain::Baseline, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.retain_artifact(artifact)
        })
        .unwrap()
        .unwrap()
}

#[test]
fn retained_equal_bytes_reuse_requested_codecs_without_merging_records() {
    with_candidates(|compilation, candidates, policy| {
        let donor = render(compilation, candidates[0], policy);
        let target = render(compilation, candidates[1], policy);
        for codec in [CompressionCostModel::Gzip, CompressionCostModel::Brotli] {
            compilation
                .measure_artifact(donor, codec, WorkDomain::Baseline)
                .unwrap();
        }
        compilation
            .with_javascript_output_in(candidates[0], policy, WorkDomain::Baseline, |output| {
                output.with_retained_arena(|arena, budget| {
                    let donor_sizes = arena.with_artifact(donor, |view| view.sizes).unwrap();
                    let before = budget.with_ledger(|ledger| {
                        let ledger = ledger.unwrap().0;
                        (
                            ledger.work_by_kind(WorkKind::Codec),
                            ledger.retained_bytes(),
                        )
                    });
                    let sizes = arena
                        .reuse_scores(
                            target,
                            Objectives::One(CompressionCostModel::Brotli),
                            budget,
                        )
                        .unwrap();
                    assert_eq!(sizes.brotli11, donor_sizes.brotli11);
                    assert_eq!(sizes.gzip9, None, "an unrequested codec stays unmeasured");
                    let sizes = arena
                        .reuse_scores(target, Objectives::One(CompressionCostModel::Gzip), budget)
                        .unwrap();
                    assert_eq!(sizes, donor_sizes);
                    let after = budget.with_ledger(|ledger| {
                        let ledger = ledger.unwrap().0;
                        (
                            ledger.work_by_kind(WorkKind::Codec),
                            ledger.retained_bytes(),
                        )
                    });
                    assert_eq!(after, before, "reuse needs no codec work or retained bytes");
                    for (artifact, candidate) in [donor, target].into_iter().zip(candidates) {
                        assert_eq!(
                            arena
                                .with_artifact(artifact, |view| view.candidate)
                                .unwrap(),
                            candidate
                        );
                    }
                });
            })
            .unwrap();
        compilation.discard_artifact(donor).unwrap();
        assert!(compilation.with_artifact(target, |_| ()).is_ok());
        compilation.discard_artifact(target).unwrap();
    });
}

#[test]
fn refused_reuse_does_not_publish_a_partially_found_codec() {
    with_candidates(|compilation, candidates, policy| {
        let gzip = render(compilation, candidates[0], policy);
        let brotli = render(compilation, candidates[0], policy);
        let target = render(compilation, candidates[1], policy);
        compilation
            .measure_artifact(gzip, CompressionCostModel::Gzip, WorkDomain::Baseline)
            .unwrap();
        compilation
            .measure_artifact(brotli, CompressionCostModel::Brotli, WorkDomain::Baseline)
            .unwrap();
        compilation
            .with_javascript_output_in(candidates[0], policy, WorkDomain::Baseline, |output| {
                output.with_retained_arena(|arena, budget| {
                    let before = arena.with_artifact(target, |view| view.sizes).unwrap();
                    let raw = before.raw as u64;
                    budget.with_ledger(|ledger| {
                        let ledger = ledger.unwrap().0;
                        // Lookup, first donor visit and its exact byte comparison fit.
                        // Refuse the next donor after finding gzip but before publication.
                        ledger
                            .charge(WorkDomain::Optional, WorkKind::Analysis, WORK - raw - 2)
                            .unwrap();
                        let retained = ledger.retained_bytes();
                        let codecs = ledger.work_by_kind(WorkKind::Codec);
                        let mut optional =
                            AllocationBudget::new(Some((ledger, WorkDomain::Optional)));
                        assert!(matches!(
                            arena.reuse_scores(target, Objectives::All, &mut optional),
                            Err(CandidateError::Budget(BudgetError::WorkExhausted(
                                WorkDomain::Optional
                            )))
                        ));
                        assert_eq!(
                            arena.with_artifact(target, |view| view.sizes).unwrap(),
                            before
                        );
                        optional.with_ledger(|ledger| {
                            let ledger = ledger.unwrap().0;
                            assert_eq!(ledger.retained_bytes(), retained);
                            assert_eq!(ledger.work_by_kind(WorkKind::Codec), codecs);
                        });
                    });
                });
            })
            .unwrap();
    });
}

#[test]
fn discarded_donors_leave_no_score_history_or_retained_bytes() {
    with_candidates(|compilation, candidates, policy| {
        let donor = render(compilation, candidates[0], policy);
        let target = render(compilation, candidates[1], policy);
        compilation
            .measure_artifact(donor, CompressionCostModel::Brotli, WorkDomain::Baseline)
            .unwrap();
        compilation.discard_artifact(donor).unwrap();
        compilation
            .with_javascript_output_in(candidates[0], policy, WorkDomain::Baseline, |output| {
                output.with_retained_arena(|arena, budget| {
                    let before = budget.with_ledger(|ledger| {
                        let ledger = ledger.unwrap().0;
                        (
                            ledger.work_by_kind(WorkKind::Analysis),
                            ledger.retained_bytes(),
                        )
                    });
                    let sizes = arena.reuse_scores(target, Objectives::All, budget).unwrap();
                    assert_eq!(sizes.gzip9, None);
                    assert_eq!(sizes.brotli11, None);
                    let after = budget.with_ledger(|ledger| {
                        let ledger = ledger.unwrap().0;
                        (
                            ledger.work_by_kind(WorkKind::Analysis),
                            ledger.retained_bytes(),
                        )
                    });
                    assert_eq!(after.0 - before.0, 1 + arena.slots.len() as u64);
                    assert_eq!(after.1, before.1);
                });
            })
            .unwrap();
    });
}

#[test]
fn score_identity_preserves_dependency_bytes_and_separate_streams() {
    with_candidates(|compilation, candidates, policy| {
        compilation
            .with_javascript_output_in(candidates[0], policy, WorkDomain::Baseline, |output| {
                let artifact = output.render(&Plan::new(Style::Global)).unwrap();
                output
                    .with_artifact(artifact, |template| {
                        let view =
                    |primary: &'static str, dependency: Option<(&'static str, &'static str)>| {
                        let dependency =
                            dependency.map(|(specifier, javascript)| ArtifactResourceView {
                                specifier,
                                javascript,
                                output: template.output,
                                sizes: Sizes {
                                    raw: javascript.len(),
                                    gzip9: None,
                                    brotli11: None,
                                },
                                retained_capacity: javascript.len(),
                            });
                        ArtifactView {
                            javascript: primary,
                            dependency,
                            sizes: Sizes {
                                raw: primary.len()
                                    + dependency.map_or(0, |view| view.javascript.len()),
                                gzip9: None,
                                brotli11: None,
                            },
                            retained_capacity: primary.len(),
                            ..template
                        }
                    };
                        let package = view("ab", Some(("./data.mjs", "cd")));
                        output.with_allocation_budget(|budget| {
                            assert!(same_streams(package, package, budget).unwrap());
                            for different in [
                                view("ab", Some(("./data.mjs", "ce"))),
                                view("ab", Some(("./name.mjs", "cd"))),
                                view("ac", Some(("./data.mjs", "cd"))),
                                view("abc", Some(("./data.mjs", "d"))),
                                view("abcd", None),
                            ] {
                                assert_eq!(package.sizes.raw, different.sizes.raw);
                                assert!(!same_streams(package, different, budget).unwrap());
                            }
                            assert!(!same_streams(
                                view("ab", None),
                                view("ab", Some(("./data.mjs", ""))),
                                budget,
                            )
                            .unwrap());
                        });
                    })
                    .unwrap();
            })
            .unwrap();
    });
}
