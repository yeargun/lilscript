//! Exact-score reuse borrows live artifact bytes without merging their owners.
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::js::selection::Style;
use crate::program::publication::{CheckpointLimit, Compilation};

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 128_000_000;

fn with_candidates(inspect: impl FnOnce(&mut Compilation<'_>, [CandidateId; 2], &ResolvedPolicy)) {
    let arena = bumpalo::Bump::new();
    let syntax =
        crate::parse_source(&arena, "export int value(int input){return input*3+1;}").unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = crate::program::from_checked_source(&syntax, &checked).unwrap();
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
fn discarded_bytes_keep_only_scores_with_identical_cold_work_and_fresh_admission() {
    for physical_reuse in [true, false] {
        let run = || {
            with_candidates(|compilation, candidates, policy| {
                let donor = render(compilation, candidates[0], policy);
                // First use allocates the bounded memo; subsequent cold/warm
                // encodes have the same deterministic tariff and lookup work.
                let size = compilation
                    .measure_artifact(donor, CompressionCostModel::Brotli, WorkDomain::Baseline)
                    .unwrap();
                compilation.discard_artifact(donor).unwrap();
                let target = render(compilation, candidates[1], policy);
                let before = compilation.ledger().work_by_kind(WorkKind::Codec);
                assert_eq!(
                    compilation
                        .measure_artifact(
                            target,
                            CompressionCostModel::Brotli,
                            WorkDomain::Baseline
                        )
                        .unwrap(),
                    size
                );
                let after = compilation.ledger().work_by_kind(WorkKind::Codec);
                assert!(
                    after - before > 1,
                    "replay pays encoder work, not just a lookup"
                );
                let denied: crate::config::ProjectConfig =
                    toml::from_str("[policy.tactics]\nidentifier-mangling='off'").unwrap();
                let denied = denied
                    .resolve_policy(CompilationRequest::JavaScript {
                        preserve_root_exports: true,
                    })
                    .unwrap();
                assert!(compilation
                    .qualify_artifact(
                        target,
                        &denied,
                        CompressionCostModel::Brotli,
                        ArtifactRuntimeEvidence::default(),
                        None,
                        WorkDomain::Baseline
                    )
                    .is_err());
                compilation.discard_artifact(target).unwrap();
            })
        };
        if physical_reuse {
            run();
        } else {
            compression_cache::without_reuse(run);
        }
    }
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
fn live_donor_reuse_does_not_consume_the_byte_measurement_memo() {
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
