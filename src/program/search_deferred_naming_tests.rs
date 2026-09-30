use super::*;
use crate::build::{compile_source, ServiceCompilation, ServiceOptions};
use serde_json::Value;

// A generic catalog program whose initially larger Global naming start
// refines to a smaller gzip result. Its host-visible script roots matter.
const DISPATCH: &str = r#"
int dispatch(int kind, int value) {
    if (kind == 0) { return value; }
    if (kind == 1) { return value * 2; }
    if (kind == 2) { return value * 3 + 1; }
    return value - 1;
}
int total = 0;
for (int i = 0; i < 10; i++) { total += dispatch(i % 4, i + 3); }
print(total);
"#;

fn compile(codec: &str, level: u8, deferred: bool, extra: &str) -> ServiceCompilation {
    compile_mode(codec, level, &deferred.to_string(), extra)
}

fn compile_mode(codec: &str, level: u8, deferred: &str, extra: &str) -> ServiceCompilation {
    let deferred = if deferred.is_empty() {
        String::new()
    } else {
        format!("deferred_naming_starts={deferred}\n")
    };
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "objective.codecs='{codec}'\neffort.level={level}\n[target.javascript]\nformat='bare'\n[policy.search]\n{deferred}{extra}"
    ))
    .unwrap();
    compile_source(
        DISPATCH,
        &config,
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn deferred_naming_default_changes_at_fourteen_without_spending_work_at_thirteen() {
    for level in [13, 14, 15] {
        let default = compile_mode("gzip", level, "", "");
        let explicit = compile("gzip", level, level >= 14, "");
        verify(&default, Objective::Gzip);
        assert_eq!(
            default.javascript(Objective::Gzip).unwrap().javascript(),
            explicit.javascript(Objective::Gzip).unwrap().javascript()
        );
        assert_eq!(default.report()["search"], explicit.report()["search"]);
        assert_eq!(
            default.report()["resources"],
            explicit.report()["resources"]
        );
        assert_eq!(
            stage(&default)["starts"]
                .as_array()
                .unwrap()
                .iter()
                .any(deferred),
            level >= 14
        );
    }
}

fn stage(compiled: &ServiceCompilation) -> &Value {
    &compiled.report()["search"]["terminal"]["objectives"][0]
}

fn deferred(start: &Value) -> bool {
    start["name"]
        .as_str()
        .unwrap()
        .starts_with("deferred-naming")
}

fn verify(compiled: &ServiceCompilation, codec: Objective) {
    let javascript = compiled.javascript(codec).unwrap().javascript();
    let output = std::process::Command::new("node")
        .args(["-e", javascript])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"127\n");
    assert_eq!(
        compiled.report()["resources"]["retained_bytes_after_handoff"],
        0
    );
}

fn protected_prefix(before: &Value, after: &Value) {
    for name in ["starts", "trials", "choice_trials", "joint_trials", "stops"] {
        let prefix = before[name].as_array().unwrap();
        assert_eq!(
            &after[name].as_array().unwrap()[..prefix.len()],
            prefix,
            "{name}"
        );
    }
    assert!(after["after"].as_u64().unwrap() <= before["after"].as_u64().unwrap());
}

#[test]
fn deferred_naming_protects_the_completed_prefix_and_recovers_an_uphill_start() {
    for (name, codec) in [
        ("raw", Objective::Raw),
        ("gzip", Objective::Gzip),
        ("brotli", Objective::Brotli),
    ] {
        let before = compile(name, 13, false, "");
        let after = compile(name, 13, true, "");
        verify(&before, codec);
        verify(&after, codec);
        let old = stage(&before);
        let new = stage(&after);
        protected_prefix(old, new);
        let original = old["starts"].as_array().unwrap();
        let revisited: Vec<_> = new["starts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| deferred(s))
            .collect();
        let pruned: Vec<_> = original
            .iter()
            .filter(|s| s["outcome"] == "pruned")
            .collect();
        assert_eq!(revisited.len(), pruned.len());
        for start in revisited {
            let name = start["name"]
                .as_str()
                .unwrap()
                .strip_prefix("deferred-")
                .unwrap();
            assert!(pruned.iter().any(|s| s["name"] == name));
            assert_ne!(start["outcome"], "pruned");
            assert!(start["pass"].as_u64().unwrap() > old["passes"].as_u64().unwrap());
        }
        assert!(
            after.report()["resources"]["optional_work"]
                .as_u64()
                .unwrap()
                >= before.report()["resources"]["optional_work"]
                    .as_u64()
                    .unwrap()
        );
        if name == "gzip" {
            assert!(new["after"].as_u64().unwrap() < old["after"].as_u64().unwrap());
            assert!(new["starts"].as_array().unwrap().iter().any(|s| deferred(s)
                && s["outcome"] == "kept"
                && s["proxy"].as_i64().unwrap() > 0));
        }
        if pruned.is_empty() {
            assert_eq!(old, new);
            assert_eq!(before.report()["resources"], after.report()["resources"]);
        }
    }
}

#[test]
fn deferred_naming_obeys_fast_tiers_naming_vetoes_and_no_prune_mode() {
    for (level, extra) in [
        (12, ""),
        (13, "[policy.tactics]\nnaming-search='off'"),
        (13, "[policy.tactics]\nidentifier-mangling='off'"),
        (13, "proxy_pruning='off'"),
    ] {
        let before = compile("gzip", level, false, extra);
        let after = compile("gzip", level, true, extra);
        verify(&after, Objective::Gzip);
        assert_eq!(
            before.javascript(Objective::Gzip).unwrap().javascript(),
            after.javascript(Objective::Gzip).unwrap().javascript()
        );
        assert_eq!(before.report()["search"], after.report()["search"]);
        assert_eq!(before.report()["resources"], after.report()["resources"]);
        assert!(!stage(&after)["starts"]
            .as_array()
            .unwrap()
            .iter()
            .any(deferred));
    }
}

#[test]
fn deferred_naming_exhaustion_keeps_the_completed_winner() {
    let full = compile("gzip", 13, false, "");
    let resources = &full.report()["resources"];
    let work =
        resources["baseline_work"].as_u64().unwrap() + resources["optional_work"].as_u64().unwrap();
    let mut stopped = false;
    for additional in [0, 256, 4096] {
        let extra = format!("[policy.resources]\nlogical_work={}", work + additional);
        let before = compile("gzip", 13, false, &extra);
        let after = compile("gzip", 13, true, &extra);
        verify(&after, Objective::Gzip);
        protected_prefix(stage(&before), stage(&after));
        stopped |= stage(&after)["starts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| deferred(s) && s["outcome"] == "stopped");
        assert!(
            after.report()["resources"]["baseline_work"]
                .as_u64()
                .unwrap()
                + after.report()["resources"]["optional_work"]
                    .as_u64()
                    .unwrap()
                <= work + additional
        );
    }
    assert!(
        stopped,
        "fixture must exercise refusal of the additional tail"
    );
}

#[test]
fn deferred_naming_combines_private_fields_and_local_names_with_real_toml_vetoes() {
    let source = include_str!("fixtures/private-fields.lil");
    let mut exercised = false;
    for (name, codec) in [
        ("raw", Objective::Raw),
        ("gzip", Objective::Gzip),
        ("brotli", Objective::Brotli),
    ] {
        let compile = |starts, polish, extra| {
            let config = toml::from_str(&format!(
                "objective.codecs='{name}'\neffort.level=14\n[policy.search]\ndeferred_naming_starts={starts}\ndeferred_naming_polish={polish}\n[policy.tactics]\nscalar-replacement='off'\ninlining='off'\n{extra}"
            ))
            .unwrap();
            compile_source(source, &config, ServiceOptions::default()).unwrap()
        };
        let protected = compile(false, false, "");
        let ordinary = compile(true, false, "");
        let combined = compile(true, true, "");
        let vetoed = compile(true, true, "property-mangling='off'");
        for result in [&protected, &ordinary, &combined, &vetoed] {
            let javascript = result.javascript(codec).unwrap().javascript();
            let output = std::process::Command::new("node")
                .args([
                    "--input-type=module",
                    "-e",
                    &format!("{javascript}\nconst f=make(3);console.log(f(1),f(2),f(-4));"),
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, b"14 16 12\n");
            assert_eq!(
                result.report()["resources"]["retained_bytes_after_handoff"],
                0
            );
        }
        protected_prefix(stage(&protected), stage(&combined));
        let refinement = |stage: &Value, family: &str| -> Vec<Value> {
            let Some(start) = stage["starts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| deferred(s))
            else {
                return Vec::new();
            };
            let pass = start["pass"].as_u64().unwrap();
            stage["joint_trials"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|trial| trial["name"] == family && trial["pass"].as_u64().unwrap() >= pass)
                .cloned()
                .collect()
        };
        for family in ["naming:local-read-order", "properties:private-fields"] {
            assert!(refinement(stage(&ordinary), family).is_empty());
            let trials = refinement(stage(&combined), family);
            exercised |= !trials.is_empty();
            if stage(&combined)["starts"]
                .as_array()
                .unwrap()
                .iter()
                .any(deferred)
            {
                assert!(!trials.is_empty(), "{name}/{family}");
            }
        }
        assert!(stage(&vetoed)["joint_trials"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|trial| trial["name"] == "properties:private-fields")
            .all(|trial| trial["outcome"] == "vetoed"));
        assert!(vetoed
            .javascript(codec)
            .unwrap()
            .javascript()
            .contains("accumulatedValue"));
    }
    assert!(
        exercised,
        "fixture must reach a deferred naming/field combination"
    );
}
