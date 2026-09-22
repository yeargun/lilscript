//! Cost evidence is an admission input, not a placeholder runtime estimate.
use crate::compilation_policy::{
    AdmissionError, CandidateCost, CandidateCostEvidence, CompilationRequest, ResolvedPolicy,
    RuntimeRisk, TacticId, TacticUse,
};
use std::cmp::Ordering;

fn policy(source: &str) -> ResolvedPolicy {
    toml::from_str::<crate::config::ProjectConfig>(source)
        .unwrap()
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn performance(bytes: u64, score: Option<u64>) -> CandidateCostEvidence {
    CandidateCostEvidence {
        performance_score: score,
        ..CandidateCostEvidence::size_only(bytes)
    }
}

#[test]
fn unconstrained_size_ranking_uses_exact_bytes_and_leaves_canonical_ties_to_the_owner() {
    let policy = policy("");
    let baseline = CandidateCostEvidence::size_only(1_000_000);
    let smaller = CandidateCostEvidence::size_only(999_998);
    let larger = CandidateCostEvidence::size_only(999_999);
    for cost in [smaller, larger, baseline] {
        assert_eq!(policy.admit_evidence(&[], cost, baseline), Ok(()));
        assert_eq!(cost.performance_score, None);
        assert_eq!(cost.startup_work, None);
        assert_eq!(cost.recurring_work, None);
        assert_eq!(cost.runtime_memory_bytes, None);
    }
    // A one-byte improvement must survive normalization against a large root.
    assert_eq!(
        policy.compare_evidence(smaller, larger, baseline),
        Ok(Some(Ordering::Less))
    );
    assert_eq!(
        policy.compare_evidence(smaller, smaller, baseline),
        Ok(Some(Ordering::Equal))
    );
    // Even known slower performance cannot outweigh a smaller exact artifact
    // under unconstrained SizeFirst. No runtime estimate is invented here.
    assert_eq!(
        policy.compare_evidence(
            performance(99, Some(u64::MAX)),
            performance(100, Some(0)),
            performance(100, Some(1)),
        ),
        Ok(Some(Ordering::Less))
    );
}

#[test]
fn each_absolute_runtime_bound_distinguishes_unknown_zero_and_over_limit() {
    for (key, reason, field) in [
        ("max_startup_work", "startup work", 0),
        ("max_recurring_work", "recurring work", 1),
        ("max_runtime_memory_bytes", "runtime memory", 2),
    ] {
        let policy = policy(&format!("[policy.constraints]\n{key}=0\n"));
        let unknown = CandidateCostEvidence::size_only(1);
        assert_eq!(
            policy.admit_evidence(&[], unknown, unknown),
            Err(AdmissionError::MissingCostEvidence(reason)),
            "{key}: unknown must not satisfy an explicit zero limit"
        );
        for (value, expected) in [(0, Ok(())), (1, Err(AdmissionError::Constraint(reason)))] {
            let mut cost = unknown;
            match field {
                0 => cost.startup_work = Some(value),
                1 => cost.recurring_work = Some(value),
                2 => cost.runtime_memory_bytes = Some(value),
                _ => unreachable!(),
            }
            // Absolute bounds need this candidate's estimate. Keeping the
            // baseline unknown does not turn the bound into a relative test.
            assert_eq!(policy.admit_evidence(&[], cost, unknown), expected, "{key}");
        }
    }
}

#[test]
fn a_performance_bound_requires_both_estimates_and_preserves_the_zero_baseline() {
    let zero_limit = policy("[policy.constraints]\nmax_performance_regression_percent=0\n");
    let unknown = performance(100, None);
    let zero = performance(100, Some(0));
    assert_eq!(
        zero_limit.admit_evidence(&[], unknown, zero),
        Err(AdmissionError::MissingCostEvidence("performance estimate"))
    );
    assert_eq!(
        zero_limit.admit_evidence(&[], zero, unknown),
        Err(AdmissionError::MissingCostEvidence(
            "baseline performance estimate"
        ))
    );
    assert_eq!(zero_limit.admit_evidence(&[], zero, zero), Ok(()));
    assert_eq!(
        zero_limit.admit_evidence(&[], performance(1, Some(1)), zero),
        Err(AdmissionError::Constraint("performance estimate"))
    );
    let ten_percent = policy("[policy.constraints]\nmax_performance_regression_percent=10\n");
    let baseline = performance(100, Some(100));
    assert_eq!(
        ten_percent.admit_evidence(&[], performance(90, Some(110)), baseline),
        Ok(())
    );
    assert_eq!(
        ten_percent.admit_evidence(&[], performance(1, Some(111)), baseline),
        Err(AdmissionError::Constraint("performance estimate"))
    );
}

#[test]
fn every_performance_priority_rejects_missing_required_admission_and_rank_inputs() {
    for priority in [
        "balanced",
        "performance-first",
        "realistic-performance-first",
    ] {
        let policy = policy(&format!("[javascript]\npriority='{priority}'\n"));
        let unknown = performance(1, None);
        let known = performance(100, Some(100));
        assert_eq!(
            policy.admit_evidence(&[], unknown, known),
            Err(AdmissionError::MissingCostEvidence("performance estimate")),
            "{priority}"
        );
        assert_eq!(
            policy.admit_evidence(&[], known, unknown),
            Err(AdmissionError::MissingCostEvidence(
                "baseline performance estimate"
            )),
            "{priority}"
        );
        // Unconstrained startup/recurring/memory fields can remain unknown.
        assert_eq!(policy.admit_evidence(&[], known, known), Ok(()));
        for (left, right, baseline, reason) in [
            (unknown, known, known, "performance estimate"),
            (known, unknown, known, "performance estimate"),
            (known, known, unknown, "baseline performance estimate"),
        ] {
            assert_eq!(
                policy.compare_evidence(left, right, baseline),
                Err(AdmissionError::MissingCostEvidence(reason)),
                "{priority}"
            );
        }
        assert_eq!(
            policy.compare_evidence(performance(99, Some(100)), known, known),
            Ok(Some(Ordering::Less)),
            "{priority}"
        );
    }
}

#[test]
fn known_legacy_costs_keep_their_existing_admission_and_priority_order() {
    let baseline = CandidateCost {
        transfer_bytes: 1_000,
        performance_score: 100,
        startup_work: 5,
        recurring_work: 3,
        runtime_memory_bytes: 8,
    };
    let short_slow = CandidateCost {
        transfer_bytes: 800,
        performance_score: 110,
        ..baseline
    };
    let long_fast = CandidateCost {
        transfer_bytes: 900,
        performance_score: 95,
        ..baseline
    };
    for (priority, expected) in [
        ("size-first", Ordering::Less),
        ("balanced", Ordering::Less),
        ("performance-first", Ordering::Greater),
        ("realistic-performance-first", Ordering::Greater),
    ] {
        let policy = policy(&format!("[javascript]\npriority='{priority}'\n[javascript.performance]\nmax_regression_percent=5\n"));
        for cost in [short_slow, long_fast] {
            // A ranking penalty is not an unconfigured hard constraint.
            assert_eq!(policy.admit(&[], cost, baseline), Ok(()));
            assert_eq!(
                policy.admit_evidence(&[], cost.into(), baseline.into()),
                Ok(())
            );
        }
        assert_eq!(
            policy.compare(short_slow, long_fast, baseline),
            Some(expected)
        );
        assert_eq!(
            policy.compare_evidence(short_slow.into(), long_fast.into(), baseline.into()),
            Ok(Some(expected))
        );
    }
    let bounded = policy("[policy.constraints]\nmax_runtime_memory_bytes=7\n");
    assert_eq!(
        bounded.admit(&[], short_slow, baseline),
        Err(AdmissionError::Constraint("runtime memory"))
    );
    assert_eq!(
        bounded.admit_evidence(&[], short_slow.into(), baseline.into()),
        Err(AdmissionError::Constraint("runtime memory"))
    );
}

#[test]
fn known_runtime_increases_still_require_their_own_declared_risk() {
    let policy = policy("[policy.tactics]\nstring-array-packing='on'\n");
    let baseline = CandidateCostEvidence {
        startup_work: Some(3),
        recurring_work: Some(5),
        ..CandidateCostEvidence::size_only(100)
    };
    for (risk, cost) in [
        (
            RuntimeRisk::Startup,
            CandidateCostEvidence {
                startup_work: Some(4),
                ..baseline
            },
        ),
        (
            RuntimeRisk::Recurring,
            CandidateCostEvidence {
                recurring_work: Some(6),
                ..baseline
            },
        ),
    ] {
        for uses in [
            &[][..],
            &[TacticUse {
                tactic: TacticId::StringArrayPacking,
                risk: RuntimeRisk::Neutral,
            }][..],
        ] {
            assert_eq!(
                policy.admit_evidence(uses, cost, baseline),
                Err(AdmissionError::UndeclaredRuntimeRisk(risk))
            );
        }
        assert_eq!(
            policy.admit_evidence(
                &[TacticUse {
                    tactic: TacticId::StringArrayPacking,
                    risk
                }],
                cost,
                baseline,
            ),
            Ok(())
        );
    }
}

#[test]
fn unknown_costs_never_bypass_explicit_tactic_permission() {
    let policy = policy("[policy.tactics]\nscalar-replacement='off'\n");
    let unknown = CandidateCostEvidence::size_only(1);
    assert_eq!(
        policy.admit_evidence(
            &[TacticUse {
                tactic: TacticId::ScalarReplacement,
                risk: RuntimeRisk::Neutral
            }],
            unknown,
            unknown,
        ),
        Err(AdmissionError::ForbiddenTactic(TacticId::ScalarReplacement))
    );
}

#[test]
fn mixed_known_and_unknown_size_ties_form_a_transitive_total_preorder() {
    let policy = policy("");
    let values = [
        performance(99, None),
        performance(100, Some(0)),
        performance(100, Some(80)),
        performance(100, Some(100)),
        performance(100, Some(u64::MAX)),
        performance(100, None),
        performance(101, Some(0)),
    ];
    for baseline_performance in [None, Some(0), Some(100)] {
        let baseline = performance(100, baseline_performance);
        let compare = |left, right| {
            policy
                .compare_evidence(left, right, baseline)
                .unwrap()
                .unwrap()
        };
        for &left in &values {
            assert_eq!(policy.admit_evidence(&[], left, baseline), Ok(()));
            assert_eq!(compare(left, left), Ordering::Equal);
            for &right in &values {
                let order = compare(left, right);
                assert_eq!(order, compare(right, left).reverse());
                for &third in &values {
                    if order != Ordering::Greater && compare(right, third) != Ordering::Greater {
                        assert_ne!(
                            compare(left, third),
                            Ordering::Greater,
                            "nontransitive: {left:?}, {right:?}, {third:?}, baseline={baseline:?}"
                        );
                    }
                    if order == Ordering::Equal {
                        assert_eq!(compare(left, third), compare(right, third), "inconsistent equivalence: {left:?}, {right:?}, {third:?}, baseline={baseline:?}");
                    }
                }
            }
        }
        let known = performance(100, Some(u64::MAX));
        let unknown = performance(100, None);
        assert_eq!(
            compare(known, unknown),
            if baseline_performance.is_some() {
                Ordering::Less
            } else {
                Ordering::Equal
            },
            "an unavailable normalization baseline must not invent a performance ordering"
        );
    }
}
