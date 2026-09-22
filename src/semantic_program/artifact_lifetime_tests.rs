//! Output byte ownership is explicit at the compilation boundary. These tests
//! exercise one staging arena, retained incumbents, and terminal handoff.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
    WorkKind,
};
use crate::structured_js::selection::{Objective, Plan, Style};
use std::panic::{catch_unwind, AssertUnwindSafe};

const SOURCE: &str = "export int compute(int input){return input*3+1;}";
const WORK: u64 = 100_000_000;
const MEMORY: u64 = 256_000_000;

fn policy() -> ResolvedPolicy {
    crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn with_candidate<R>(
    optional_work: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, CandidateId, &ResolvedPolicy) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy();
    let candidate = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    // These tests isolate output lifetimes. The lazily retained candidate
    // descriptor is a separate owner and must exist before byte baselines.
    compilation
        .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| {
            assert!(!words.is_empty());
        })
        .unwrap();
    let answer = inspect(&mut compilation, candidate, &policy);
    compilation.discard(candidate.semantic_id()).unwrap();
    compilation.discard(source).unwrap();
    assert_eq!(compilation.checkpoint_count(), 0);
    assert_eq!(compilation.finish().retained_bytes(), 0);
    answer
}
fn code(plan: Style) -> Plan {
    Plan::new(plan)
}

#[test]
fn scoped_artifacts_release_on_success_returned_error_and_unwind() {
    with_candidate(WORK, |compilation, candidate, policy| {
        let before = compilation.ledger().retained_bytes();
        let escaped = compilation
            .with_javascript_output(candidate, policy, |output| {
                let artifact = output.render(&code(Style::Global)).unwrap();
                output
                    .with_artifact(artifact, |view| {
                        assert_eq!(view.candidate, candidate);
                        assert!(view.javascript.contains("compute"));
                        assert_eq!(view.sizes.raw, view.javascript.len());
                        assert!(view.retained_capacity >= view.javascript.len());
                    })
                    .unwrap();
                artifact
            })
            .unwrap();
        assert_eq!(compilation.ledger().retained_bytes(), before);
        compilation
            .with_javascript_output(candidate, policy, |output| {
                assert!(output.with_artifact(escaped, |_| ()).is_err());
                assert!(output.take_artifact(escaped).is_err());
                assert!(output.retain_artifact(escaped).is_err());
            })
            .unwrap();
        let error: Result<(), &'static str> = compilation
            .with_javascript_output(candidate, policy, |output| {
                output.render(&code(Style::Scoped)).unwrap();
                Err("user callback declined the candidate")
            })
            .unwrap();
        assert!(error.is_err());
        assert_eq!(compilation.ledger().retained_bytes(), before);
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            compilation
                .with_javascript_output(candidate, policy, |output| {
                    output.render(&code(Style::Source)).unwrap();
                    panic!("after admitted artifact creation");
                })
                .unwrap();
        }));
        assert!(unwind.is_err());
        assert_eq!(compilation.ledger().retained_bytes(), before);
        // A failed callback leaves the original candidate usable.
        let text = compilation
            .with_javascript_output(candidate, policy, |output| {
                let artifact = output.render(&code(Style::Global))?;
                output.take_artifact(artifact)
            })
            .unwrap()
            .unwrap();
        assert!(text.contains("compute"));
        assert_eq!(compilation.ledger().retained_bytes(), before);
    });
}

#[test]
fn explicit_retention_survives_callback_unwind_and_releases_at_terminal_handoff() {
    with_candidate(WORK, |compilation, candidate, policy| {
        let before = compilation.ledger().retained_bytes();
        let mut retained = None;
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            compilation
                .with_javascript_output(candidate, policy, |output| {
                    let artifact = output.render(&code(Style::Global)).unwrap();
                    retained = Some(output.retain_artifact(artifact).unwrap());
                    assert!(output.with_artifact(artifact, |_| ()).is_err());
                    // A second unretained sibling still belongs to the callback.
                    output.render(&code(Style::Source)).unwrap();
                    panic!("after explicit incumbent retention");
                })
                .unwrap();
        }));
        assert!(unwind.is_err());
        let artifact = retained.unwrap();
        let capacity = compilation
            .with_artifact(artifact, |view| {
                assert_eq!(view.candidate, candidate);
                assert!(view.javascript.contains("compute"));
                view.retained_capacity as u64
            })
            .unwrap();
        assert!(compilation.ledger().retained_bytes() >= before + capacity);
        let with_payload = compilation.ledger().retained_bytes();
        let text = compilation.take_artifact(artifact).unwrap();
        assert_eq!(
            compilation.ledger().retained_bytes(),
            with_payload - capacity
        );
        assert!(compilation.with_artifact(artifact, |_| ()).is_err());
        assert!(compilation.take_artifact(artifact).is_err());
        assert!(compilation.discard_artifact(artifact).is_err());
        let script = format!(
            "const m=await import('data:text/javascript,'+encodeURIComponent({}));console.log(m.compute(7),m.compute(2147483647));",
            serde_json::to_string(&text).unwrap()
        );
        let process = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            process.status.success(),
            "{}",
            String::from_utf8_lossy(&process.stderr)
        );
        assert_eq!(
            String::from_utf8(process.stdout).unwrap(),
            "22 2147483646\n"
        );
    });
}

#[test]
fn retained_and_scoped_handles_cannot_cross_owner_boundaries_or_revive() {
    with_candidate(WORK, |left, candidate, policy| {
        let artifact = left
            .with_javascript_output(candidate, policy, |output| {
                let scoped = output.render(&code(Style::Global))?;
                output.retain_artifact(scoped)
            })
            .unwrap()
            .unwrap();
        with_candidate(WORK, |right, right_candidate, right_policy| {
            let before = right.ledger().retained_bytes();
            assert!(right.with_artifact(artifact, |_| ()).is_err());
            assert!(right.take_artifact(artifact).is_err());
            assert!(right.discard_artifact(artifact).is_err());
            assert!(right
                .measure_artifact(artifact, Objective::Raw, WorkDomain::Optional)
                .is_err());
            assert_eq!(right.ledger().retained_bytes(), before);
            let foreign_scoped = left
                .with_javascript_output(candidate, policy, |output| {
                    output.render(&code(Style::Global)).unwrap()
                })
                .unwrap();
            right
                .with_javascript_output(right_candidate, right_policy, |output| {
                    assert!(output.measure(foreign_scoped, Objective::Raw).is_err());
                    assert!(output.with_artifact(foreign_scoped, |_| ()).is_err());
                })
                .unwrap();
            assert_eq!(right.ledger().retained_bytes(), before);
        });
        let capacity = left
            .with_artifact(artifact, |view| view.retained_capacity as u64)
            .unwrap();
        let before = left.ledger().retained_bytes();
        left.discard_artifact(artifact).unwrap();
        assert_eq!(left.ledger().retained_bytes(), before - capacity);
        let replacement = left
            .with_javascript_output(candidate, policy, |output| {
                let scoped = output.render(&code(Style::Scoped))?;
                output.retain_artifact(scoped)
            })
            .unwrap()
            .unwrap();
        assert_ne!(artifact, replacement);
        assert!(left.with_artifact(artifact, |_| ()).is_err());
        assert!(left.with_artifact(replacement, |_| ()).is_ok());
        // finish owns the final live artifact; no explicit discard is needed.
    });
}

#[test]
fn complete_artifact_scores_are_cached_across_scoped_to_retained_transfer() {
    with_candidate(WORK, |compilation, candidate, policy| {
        let artifact = compilation
            .with_javascript_output_in(candidate, policy, WorkDomain::Optional, |output| {
                let artifact = output.render(&code(Style::Global))?;
                let raw = output.measure(artifact, Objective::Raw)?;
                let gzip = output.measure(artifact, Objective::Gzip)?;
                let brotli = output.measure(artifact, Objective::Brotli)?;
                output.with_artifact(artifact, |view| {
                    assert_eq!(view.javascript.len(), raw);
                    assert_eq!(view.sizes.gzip9, Some(gzip));
                    assert_eq!(view.sizes.brotli11, Some(brotli));
                })?;
                output.retain_artifact(artifact)
            })
            .unwrap()
            .unwrap();
        let before = compilation.ledger().clone();
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let expected = compilation
                .with_artifact(artifact, |view| view.sizes.get(codec).unwrap())
                .unwrap();
            assert_eq!(
                compilation
                    .measure_artifact(artifact, codec, WorkDomain::Optional)
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            compilation.ledger().work_by_kind(WorkKind::Codec),
            before.work_by_kind(WorkKind::Codec) + 3,
            "cached objectives charge one lookup each without another encoder call"
        );
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
    });
}

#[test]
fn optional_denial_preserves_incumbent_and_baseline_output_allowance() {
    with_candidate(0, |compilation, candidate, policy| {
        let artifact = compilation
            .with_javascript_output(candidate, policy, |output| {
                let scoped = output.render(&code(Style::Global))?;
                output.retain_artifact(scoped)
            })
            .unwrap()
            .unwrap();
        let before = compilation.ledger().clone();
        let mut entered = false;
        assert!(compilation
            .with_javascript_output_in(candidate, policy, WorkDomain::Optional, |_| entered = true)
            .is_err());
        assert!(!entered);
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
        assert_eq!(
            compilation.ledger().work_used(WorkDomain::Baseline),
            before.work_used(WorkDomain::Baseline)
        );
        let next = compilation
            .with_javascript_output(candidate, policy, |output| {
                let scoped = output.render(&code(Style::Global))?;
                output.take_artifact(scoped)
            })
            .unwrap()
            .unwrap();
        compilation
            .with_artifact(artifact, |view| assert_eq!(view.javascript, next))
            .unwrap();
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
    });
}
