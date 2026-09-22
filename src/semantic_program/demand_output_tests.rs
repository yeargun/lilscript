//! Complete output uses the compilation owner's selected work domain.
//! Tests explicitly hand off final bytes and require temporary target cleanup.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits,
    WorkDomain, WorkKind,
};
use crate::structured_js::selection::{Plan, Style};

const SOURCE: &str = "export int compute(int input){int scaled=input*3;return scaled+1;}";
const WORK: u64 = 10_000_000;
const MEMORY: u64 = 10_000_000;

fn policy(configuration: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(configuration).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn plan() -> BudgetPlan {
    BudgetPlan {
        baseline_work: WORK,
        optional_work: WORK,
        baseline_retained_bytes: 0,
        retained_bytes: MEMORY,
    }
}

fn with_candidate<R>(
    source: &str,
    policy: &ResolvedPolicy,
    plan: BudgetPlan,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, CandidateId) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let ledger = BudgetLedger::new(ResourceLimits::default(), plan).unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let direct = compilation
        .direct_javascript(source, policy, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compilation, source, direct);
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

fn emit(
    compilation: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    domain: WorkDomain,
) -> String {
    compilation
        .with_javascript_output_in(candidate, policy, domain, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .unwrap()
        .unwrap()
}

fn assert_complete_output_work(before: &BudgetLedger, after: &BudgetLedger, domain: WorkDomain) {
    let analysis = after.work_by_kind(WorkKind::Analysis) - before.work_by_kind(WorkKind::Analysis);
    let render = after.work_by_kind(WorkKind::Render) - before.work_by_kind(WorkKind::Render);
    assert!(
        analysis > 0,
        "demand and target verification must be charged"
    );
    assert!(
        render > 0,
        "target construction, naming and text must be charged"
    );
    assert_eq!(
        after.work_by_kind(WorkKind::Codec),
        before.work_by_kind(WorkKind::Codec),
        "rendering raw JavaScript must not invoke an unrequested codec"
    );
    let total: u64 = [
        WorkKind::Analysis,
        WorkKind::Edit,
        WorkKind::Render,
        WorkKind::Codec,
    ]
    .into_iter()
    .map(|kind| after.work_by_kind(kind) - before.work_by_kind(kind))
    .sum();
    assert_eq!(after.work_used(domain) - before.work_used(domain), total);
    let other = match domain {
        WorkDomain::Baseline => WorkDomain::Optional,
        WorkDomain::Optional => WorkDomain::Baseline,
    };
    assert_eq!(after.work_used(other), before.work_used(other));
}

#[test]
fn explicit_optional_output_charges_analysis_and_render_and_releases_all_temporary_storage() {
    let resolved = policy("[javascript]\nstrip_console=false\n");
    with_candidate(
        SOURCE,
        &resolved,
        plan(),
        |compilation, source, candidate| {
            compilation
                .with_implementation_description(candidate, WorkDomain::Optional, |_| ())
                .unwrap();
            let before = compilation.ledger().clone();
            let revisions = compilation
                .with_semantic(source, |program, _, _| {
                    program
                        .units
                        .iter()
                        .map(FrozenUnit::revision)
                        .collect::<Vec<_>>()
                })
                .unwrap();
            let output = emit(compilation, candidate, &resolved, WorkDomain::Optional);
            assert!(output.contains("compute"));
            let after = compilation.ledger();
            assert_complete_output_work(&before, after, WorkDomain::Optional);
            assert_eq!(after.retained_bytes(), before.retained_bytes());
            assert!(
                after.peak_retained_bytes() >= before.retained_bytes() + output.capacity() as u64
            );
            compilation
                .with_semantic(source, |program, _, _| {
                    assert_eq!(
                        program
                            .units
                            .iter()
                            .map(FrozenUnit::revision)
                            .collect::<Vec<_>>(),
                        revisions
                    );
                })
                .unwrap();
        },
    );
}

#[test]
fn compatibility_output_spends_baseline_and_preserves_optional_budget() {
    let resolved = policy("[javascript]\nstrip_console=false\n");
    with_candidate(SOURCE, &resolved, plan(), |compilation, _, candidate| {
        compilation
            .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
            .unwrap();
        let before = compilation.ledger().clone();
        let output = compilation
            .with_javascript_output(candidate, &resolved, |output| {
                let artifact = output.render(&Plan::new(Style::Global))?;
                output.take_artifact(artifact)
            })
            .unwrap()
            .unwrap();
        let after = compilation.ledger();
        assert_complete_output_work(&before, after, WorkDomain::Baseline);
        assert_eq!(after.retained_bytes(), before.retained_bytes());
        assert_eq!(
            output,
            emit(compilation, candidate, &resolved, WorkDomain::Optional)
        );
    });
}

#[test]
fn interrupted_optional_preparation_never_calls_output_and_leaves_baseline_usable() {
    let resolved = policy("[javascript]\nstrip_console=false\n");
    with_candidate(
        SOURCE,
        &resolved,
        plan(),
        |compilation, source, candidate| {
            let expected = emit(compilation, candidate, &resolved, WorkDomain::Optional);
            let before_probe = compilation.ledger().work_used(WorkDomain::Optional);
            compilation
                .with_javascript_output_in(candidate, &resolved, WorkDomain::Optional, |_| ())
                .unwrap();
            let preparation_work =
                compilation.ledger().work_used(WorkDomain::Optional) - before_probe;
            assert!(preparation_work > 1);
            // Retain just less than preparation's measured complete work, so
            // rejection must still happen before the output callback. Rendering
            // is measured separately and cannot inflate this admission quota.
            compilation
                .with_semantic(source, |_, _, ledger| {
                    ledger
                        .charge(
                            WorkDomain::Optional,
                            WorkKind::Render,
                            WORK - ledger.work_used(WorkDomain::Optional) - (preparation_work - 1),
                        )
                        .unwrap();
                })
                .unwrap();
            let before = compilation.ledger().clone();
            let mut callback_called = false;
            let result = compilation.with_javascript_output_in(
                candidate,
                &resolved,
                WorkDomain::Optional,
                |_| callback_called = true,
            );
            assert!(matches!(
                result,
                Err(CandidateError::Budget(BudgetError::WorkExhausted(
                    WorkDomain::Optional
                )))
            ));
            assert!(!callback_called);
            assert_eq!(
                compilation.ledger().retained_bytes(),
                before.retained_bytes()
            );
            assert_eq!(
                compilation.ledger().work_used(WorkDomain::Baseline),
                before.work_used(WorkDomain::Baseline)
            );
            let actual = compilation
                .with_javascript_output(candidate, &resolved, |output| {
                    let artifact = output.render(&Plan::new(Style::Global))?;
                    output.take_artifact(artifact)
                })
                .unwrap()
                .unwrap();
            assert_eq!(actual, expected);
        },
    );
}

#[test]
fn optional_output_cannot_spend_reserved_baseline_memory() {
    let resolved = policy("[javascript]\nstrip_console=false\n");
    let mut reserved = plan();
    reserved.baseline_retained_bytes = MEMORY;
    with_candidate(SOURCE, &resolved, reserved, |compilation, _, candidate| {
        let before = compilation.ledger().clone();
        let mut callback_called = false;
        let result = compilation.with_javascript_output_in(
            candidate,
            &resolved,
            WorkDomain::Optional,
            |_| callback_called = true,
        );
        assert!(matches!(
            result,
            Err(CandidateError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        ));
        assert!(!callback_called);
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
        assert_eq!(
            compilation.ledger().work_used(WorkDomain::Baseline),
            before.work_used(WorkDomain::Baseline)
        );
        compilation
            .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
            .unwrap();
        let prepared = compilation.ledger().retained_bytes();
        assert!(emit(compilation, candidate, &resolved, WorkDomain::Baseline).contains("compute"));
        assert_eq!(compilation.ledger().retained_bytes(), prepared);
    });
}

#[test]
fn unsupported_formation_releases_completed_demand_without_calling_output() {
    // Any formation the new backend still refuses will do: an exported array
    // of value structs the function mutates has no D2 adapter. (Array methods,
    // `codePointLength`, exported defaults, exported class instances and the
    // exported closure over module `arguments` used here before are
    // supported now.)
    let resolved = policy("[javascript]\nstrip_console=false\n");
    let source = "struct P{int x;}export void add(P[] items){items.push(P{1});}";
    with_candidate(source, &resolved, plan(), |compilation, _, candidate| {
        let before = compilation.ledger().clone();
        let mut callback_called = false;
        let result = compilation.with_javascript_output_in(
            candidate,
            &resolved,
            WorkDomain::Optional,
            |_| callback_called = true,
        );
        assert!(matches!(result, Err(CandidateError::Unsupported(_))));
        assert!(!callback_called);
        assert!(
            compilation.ledger().work_by_kind(WorkKind::Analysis)
                > before.work_by_kind(WorkKind::Analysis)
        );
        assert_eq!(
            compilation.ledger().retained_bytes(),
            before.retained_bytes()
        );
        assert_eq!(
            compilation.ledger().work_used(WorkDomain::Baseline),
            before.work_used(WorkDomain::Baseline)
        );
    });
}

#[test]
fn output_respects_dead_code_permission_on_the_same_semantic_candidate() {
    let enabled =
        policy("[javascript]\nstrip_console=false\n[policy.tactics]\ndead-code-elimination='on'\n");
    let forbidden = policy(
        "[javascript]\nstrip_console=false\n[policy.tactics]\ndead-code-elimination='off'\n",
    );
    // The discarded expression has actual primitive producers. A public
    // parameter alone cannot justify removing an observable JS coercion.
    let source = "export int compute(int input){int discardedProduct=17*7919;return input+1;}";
    with_candidate(source, &enabled, plan(), |compilation, _, candidate| {
        compilation
            .with_implementation_description(candidate, WorkDomain::Optional, |_| ())
            .unwrap();
        let retained = compilation.ledger().retained_bytes();
        let mut artifacts = Vec::new();
        for (policy, keep_unused) in [(&enabled, false), (&forbidden, true)] {
            let output = compilation
                .with_javascript_output_in(candidate, policy, WorkDomain::Optional, |output| {
                    let artifact = output.render(&Plan::new(Style::Source))?;
                    output.take_artifact(artifact)
                })
                .unwrap()
                .unwrap();
            assert_eq!(output.contains("discardedProduct"), keep_unused, "{output}");
            assert_eq!(output.contains("7919"), keep_unused, "{output}");
            assert_eq!(compilation.ledger().retained_bytes(), retained);
            artifacts.push(output);
        }
        // The independent expected result covers public function arity/name and
        // signed integer behavior while the structural checks test permission.
        let script = format!(
            "const result=[];for(const source of {}){{const library=await import('data:text/javascript,'+encodeURIComponent(source));result.push([library.compute.name,library.compute.length,[-4,0,7,2147483647].map(library.compute)]);}}process.stdout.write(JSON.stringify(result));",
            serde_json::to_string(&artifacts).unwrap(),
        );
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for the output policy contract test");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let observed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            observed,
            serde_json::json!([
                ["compute", 1, [-3, 1, 8, -2147483648_i64]],
                ["compute", 1, [-3, 1, 8, -2147483648_i64]],
            ])
        );
    });
}

#[test]
fn public_scalar_annotations_do_not_remove_conversion_hooks_or_exceptions() {
    let enabled =
        policy("[javascript]\nstrip_console=false\n[policy.tactics]\ndead-code-elimination='on'\n");
    let forbidden = policy(
        "[javascript]\nstrip_console=false\n[policy.tactics]\ndead-code-elimination='off'\n",
    );
    let source = "export int compute(int input){int discardedProduct=input*7919;return input+1;}";
    with_candidate(source, &enabled, plan(), |compilation, _, candidate| {
        for policy in [&enabled, &forbidden] {
            let javascript = compilation
                .with_javascript_output_in(candidate, policy, WorkDomain::Optional, |output| {
                    let artifact = output.render(&Plan::new(Style::Scoped))?;
                    output.take_artifact(artifact)
                })
                .unwrap()
                .unwrap();
            let script = format!(
                r#"
                const library=await import('data:text/javascript,'+encodeURIComponent({}));
                const seen=[];
                const value={{[Symbol.toPrimitive](hint){{seen.push(hint);return 2;}}}};
                seen.push(library.compute(value));
                const sentinel={{}};
                try{{library.compute({{[Symbol.toPrimitive](hint){{seen.push(hint);throw sentinel;}}}});}}
                catch(error){{seen.push(error===sentinel);}}
                process.stdout.write(JSON.stringify(seen));
            "#,
                serde_json::to_string(&javascript).unwrap()
            );
            let result = std::process::Command::new("node")
                .args(["--input-type=module", "-e", &script])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                r#"["number","default",3,"number",true]"#,
                "{javascript}"
            );
        }
    });
}
