use super::*;
use crate::config::{parse_project_config, ProjectConfig};

fn resolve(source: &str, request: CompilationRequest) -> ResolvedPolicy {
    parse_project_config(source)
        .unwrap()
        .config
        .resolve_policy(request)
        .unwrap()
}

fn js(source: &str) -> ResolvedPolicy {
    resolve(
        source,
        CompilationRequest::JavaScript {
            preserve_root_exports: true,
        },
    )
}

#[test]
fn explicit_risk_policy_is_effort_independent_and_legacy_is_preserved() {
    for codec in ["raw", "gzip", "brotli"] {
        for effort in [0, 8, 13, 14, 15, 16] {
            for permission in ["auto", "on", "off"] {
                let source = format!("objective.codecs='{codec}'\neffort.level={effort}\n[policy]\nversion=3\n[policy.tactics]\nstartup-reconstruction='{permission}'\nstring-array-packing='{permission}'");
                let modern = js(&source);
                for (tactic, risk) in [
                    (TacticId::StartupReconstruction, RuntimeRisk::Startup),
                    (TacticId::StringArrayPacking, RuntimeRisk::Startup),
                    (TacticId::StringArrayPacking, RuntimeRisk::Recurring),
                ] {
                    assert_eq!(
                        modern
                            .check_tactic_permissions(&[TacticUse { tactic, risk }])
                            .is_ok(),
                        permission == "on",
                        "{source}/{tactic:?}/{risk:?}"
                    );
                }
                assert!(!modern
                    .diagnostics()
                    .iter()
                    .any(|message| message.starts_with("compatibility:")));
                let legacy = js(&source.replace("version=3", "version=2"));
                assert_eq!(
                    legacy
                        .check_tactic_permissions(&[TacticUse {
                            tactic: TacticId::StartupReconstruction,
                            risk: RuntimeRisk::Startup
                        }])
                        .is_ok(),
                    permission == "on" || (permission == "auto" && effort == 16)
                );
                assert_ne!(legacy.fingerprint(), modern.fingerprint());
                // Neither a version nor more effort grants a producer a risk
                // class absent from its own registry declaration.
                for policy in [legacy, modern] {
                    assert!(policy
                        .check_tactic_permissions(&[TacticUse {
                            tactic: TacticId::StartupReconstruction,
                            risk: RuntimeRisk::Recurring
                        }])
                        .is_err());
                }
            }
        }
    }
}

#[test]
fn origins_explain_equivalent_permissions_without_splitting_cache_identity() {
    let old = js("optimization.inlining=false");
    let current = js("policy.tactics.inlining='off'");
    assert_eq!(old.fingerprint(), current.fingerprint());
    let row = |policy: &ResolvedPolicy, tactic| {
        policy.resolution()["tactics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == tactic)
            .unwrap()
            .clone()
    };
    assert_eq!(row(&old, "inlining")["origin"], "legacy-alias");
    assert_eq!(row(&current, "inlining")["origin"], "policy");
    assert_eq!(row(&current, "inlining")["status"], "disabled");
    let blocked = js("policy.tactics.identifier-mangling='off'\npolicy.tactics.naming-search='on'");
    assert_eq!(
        row(&blocked, "naming-search")["status"],
        "prerequisite-disabled"
    );
    let unavailable = js("policy.tactics.helper-sharing='on'");
    assert_eq!(row(&unavailable, "helper-sharing")["status"], "unavailable");
    let modern = js("policy.version=3\neffort.level=16");
    assert_eq!(
        row(&modern, "startup-reconstruction")["status"],
        "automatic-off"
    );
}

#[test]
fn producer_dependencies_do_not_disable_independent_implementations() {
    use TacticId as T;
    use TacticProducer as P;
    let policy =
        js("policy.tactics.target-compaction='off'\npolicy.tactics.dead-code-elimination='off'");
    assert!(!T::Inlining.spec().producer_enabled(P::SharedRules, &policy));
    assert!(!T::Inlining
        .spec()
        .producer_enabled(P::JavaScriptFormation, &policy));
    assert!(T::Inlining
        .spec()
        .producer_enabled(P::StructuralSearch, &policy));
    assert!(T::ConstantFolding
        .spec()
        .producer_enabled(P::SharedRules, &policy));
    let no_fold = js("policy.tactics.constant-folding='off'");
    assert!(!T::StringPooling
        .spec()
        .producer_enabled(P::StructuralSearch, &no_fold));
    assert!(T::StringPooling
        .spec()
        .producer_enabled(P::OutputFamilies, &no_fold));
    let native = ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap();
    for tactic in T::ALL {
        for producer in tactic.spec().producers {
            assert_eq!(
                tactic.spec().producer_enabled(*producer, &native),
                *producer == P::SharedRules
            );
        }
    }
}

#[test]
fn registry_risk_classes_are_an_admission_boundary_even_for_explicit_on() {
    for tactic in TacticId::ALL {
        let policy = js(&format!(
            "policy.version=3\npolicy.tactics.{}='on'",
            tactic.spec().name
        ));
        for risk in [
            RuntimeRisk::Neutral,
            RuntimeRisk::Startup,
            RuntimeRisk::Recurring,
        ] {
            assert_eq!(
                policy
                    .check_tactic_permissions(&[TacticUse { tactic, risk }])
                    .is_ok(),
                !tactic.spec().producers.is_empty() && tactic.spec().risks.contains(&risk),
                "{tactic:?}/{risk:?}"
            );
        }
    }
}
