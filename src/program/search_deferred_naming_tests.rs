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
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "objective.codecs='{codec}'\neffort.level={level}\n[target.javascript]\nformat='bare'\n[policy.search]\ndeferred_naming_starts={deferred}\n{extra}"
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
