use super::*;
use crate::config::{parse_project_config, ProjectConfig};

fn resolve(source: &str, request: CompilationRequest) -> ResolvedPolicy {
    parse_project_config(source)
        .unwrap()
        .config
        .resolve_policy(request)
        .unwrap()
}

#[test]
fn s3_call_families_honor_independent_vetoes_and_parameterized_runtime_permission() {
    use crate::program::publication::OutputTactics;
    for codec in ["raw", "gzip", "brotli"] {
        for permission in ["auto", "on", "off"] {
            let policy = js(&format!("objective.codecs='{codec}'\neffort.level=13\n[policy.tactics]\nhelper-sharing='{permission}'"));
            let mut output = OutputTactics::from_policy(&policy);
            output.families.helper_sharing = true;
            assert_eq!(output.check_policy(&policy).is_ok(), permission != "off");
            output.families.parameterized_helpers = true;
            assert_eq!(output.check_policy(&policy).is_ok(), permission == "on");
        }
        for tactic in ["inlining", "call-specialization"] {
            let policy = js(&format!("objective.codecs='{codec}'\n[policy.tactics]\n{tactic}='off'"));
            let mut output = OutputTactics::from_policy(&policy);
            if tactic == "inlining" { output.families.expression_inlining = true; }
            else { output.families.call_specialization = true; }
            assert!(output.check_policy(&policy).is_err());
        }
    }
}

#[test]
fn q1_explicit_sites_keep_permissions_risk_and_alternative_validation() {
    use crate::program::publication::OutputTactics;
    use crate::representation::{AltId, ChoiceFamily as F, ChoiceKey, SiteId};
    for family in [F::ExpressionInlining, F::ConstantArguments, F::HelperSharing,
        F::ParameterizedHelpers, F::DataEncoding] {
        let tactic = family.spec().tactic.spec().name;
        for codec in ["raw", "gzip", "brotli"] {
            let allowed = js(&format!("objective.codecs='{codec}'\npolicy.tactics.{tactic}='on'"));
            let mut output = OutputTactics::from_policy(&allowed);
            let key = ChoiceKey { family, site: SiteId::Symbol(1) };
            output.choices = output.choices.with(key, AltId(1));
            assert!(output.check_policy(&allowed).is_ok(), "{family:?}");
            let denied = js(&format!("objective.codecs='{codec}'\npolicy.tactics.{tactic}='off'"));
            assert!(output.check_policy(&denied).is_err());
            output.choices = output.choices.with(key, AltId(255));
            assert!(output.check_policy(&allowed).is_err());
        }
    }
    let allowed = js("policy.tactics.helper-sharing='on'");
    let mut output = OutputTactics::from_policy(&allowed);
    output.families.parameterized_helpers = false;
    output.choices = output.choices.with(ChoiceKey { family: F::ParameterizedHelpers,
        site: SiteId::Formed(1) }, AltId(1));
    assert!(output.check_policy(&js("effort.level=16")).is_err());
    output.choices = crate::representation::ChoiceMap::SEEDS.with(ChoiceKey {
        family: F::ProductLayout, site: SiteId::Cell(0) }, AltId(0));
    assert!(output.check_policy(&allowed).is_err(), "source proofs cannot be target flags");
}

#[test]
fn q1_extra_joints_default_at_14_with_explicit_early_override() {
    for level in [0, 13, 14, 15, 16] {
        for permission in ["auto", "off", "on"] {
            let policy = js(&format!("effort.level={level}\npolicy.tactics.representation-joints='{permission}'"));
            assert_eq!(policy.tactic(TacticId::RepresentationJoints).enabled,
                permission == "on" || (permission == "auto" && level >= 14));
        }
    }
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
    let blocked = js("policy.tactics.recurring-reconstruction='on'");
    assert_eq!(row(&blocked, "recurring-reconstruction")["status"], "prerequisite-disabled");
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
                matches!(producer, P::SharedRules | P::NativeFormation)
            );
        }
    }
}

#[test]
fn registry_risk_classes_are_an_admission_boundary_even_for_explicit_on() {
    for tactic in TacticId::ALL {
        let prerequisites = tactic.spec().prerequisites.iter()
            .map(|id| format!("policy.tactics.{}='on'\n", id.spec().name)).collect::<String>();
        let policy = js(&format!(
            "policy.version=3\n{prerequisites}policy.tactics.{}='on'",
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
