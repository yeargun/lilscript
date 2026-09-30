use super::*;
use crate::build::{compile_source, ServiceOptions, ServiceTarget};
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::js::selection::{Objective, Objectives, Plan, Style};
use crate::output_budget::AllocationBudget;
use serde_json::{json, Value};
use std::process::Command;
use std::time::Instant;

const CHILD: &str = "LILSCRIPT_PHASE_TIMING_TEST_CHILD";
const TEST: &str = "timing::tests::semantic_phase_timing_is_observational_and_covers_refusal";
const ANSWER: &str = "export int answer(){return 17;}";
const VALLEY: &str = include_str!("program/fixtures/search-structural-valley/entry.lil");

fn configuration(proposals: usize, inlining: bool) -> crate::config::ProjectConfig {
    toml::from_str(&format!(
        "objective.codecs='brotli'\n[javascript]\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit=48\ncandidate_limit=8\ncandidate_beam_width=2\n[policy.search]\ncodec_schedule='staged'\nrender_batch=8\ndiversity_interval=4\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'\ninlining='{}'\nscalar-replacement='off'\ncall-specialization='off'\nconstant-folding='off'\nstring-pooling='off'",
        if inlining { "on" } else { "off" },
    )).unwrap()
}

fn phase_counts() -> Vec<u64> {
    PHASE_BUCKETS
        .iter()
        .map(|bucket| bucket.snapshot().1)
        .collect()
}

fn compile_case(label: &str, source: &str, proposals: usize, inlining: bool) -> Value {
    let before = phase_counts();
    let phases_before = PHASE_BUCKETS.map(|bucket| bucket.snapshot().0);
    let started = Instant::now();
    let objectives = Objectives::All;
    let compiled = compile_source(
        source,
        &configuration(proposals, inlining),
        ServiceOptions {
            objectives: Some(objectives),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let wall_ns = started.elapsed().as_nanos();
    let phases: Vec<_> = PHASE_BUCKETS.iter().enumerate().map(|(index, bucket)| {
        let (nanos, calls, bytes) = bucket.snapshot();
        if !matches!(bucket.name, "codec_lookup" | "codec_reuse") {
            assert_eq!(bytes, 0, "phase counters do not claim byte measurements");
        }
        json!({"name":bucket.name,"calls":calls-before[index],"elapsed_ns":nanos-phases_before[index]})
    }).collect();
    if enabled() {
        // The terminal stage builds one demand plan and one formed head per
        // objective it forms challengers for, then one tail, target and
        // render per challenger, and scores the ones that differ.
        let stages = compiled.report()["search"]["terminal"]["objectives"]
            .as_array()
            .unwrap();
        // Choice alternatives and joint moves form and render like
        // challengers; a survey forms a tail only, to read the incumbent's
        // choice sites.
        let formed = |stage: &Value| {
            stage["tried"].as_u64().unwrap()
                + stage["choices_tried"].as_u64().unwrap()
                + stage["joints_tried"].as_u64().unwrap()
                + stage["restarts_tried"].as_u64().unwrap()
        };
        let tried: u64 = stages.iter().map(formed).sum();
        let surveys: u64 = stages
            .iter()
            .map(|stage| stage["surveys"].as_u64().unwrap())
            .sum();
        // Each start the portfolio holds (the search's winner, the level-0
        // artifact) is walked in formations of its own, from one head, and
        // forms the head with the other int32 hints if a challenger asks.
        let heads: u64 = stages
            .iter()
            .map(|stage| {
                let starts = stage["starts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|start| matches!(start["name"].as_str(), Some("search" | "level-0")))
                    .count() as u64;
                let heads = stage["heads"].as_u64().unwrap();
                assert!((starts..=2 * starts).contains(&heads), "{stage}");
                heads
            })
            .sum();
        let terminal_encodes: u64 = stages
            .iter()
            .map(|stage| stage["codec_probes"].as_u64().unwrap())
            .sum();
        let renders = compiled.report()["search"]["renders"].as_u64().unwrap() + tried;
        assert_eq!(phases[5]["calls"], renders);
        assert_eq!(phases[6]["calls"], renders);
        // The level-0 artifact is measured once under each requested codec.
        let baseline_encodes = objectives
            .iter()
            .filter(|codec| *codec != Objective::Raw)
            .count() as u64;
        let encodes = compiled.report()["search"]["codec_probes"]
            .as_u64()
            .unwrap()
            + baseline_encodes
            + terminal_encodes;
        let calls = |name: &str| {
            phases.iter().find(|phase| phase["name"] == name).unwrap()["calls"]
                .as_u64()
                .unwrap()
        };
        let physical = calls("canonical_gzip") + calls("canonical_brotli");
        let reuse = calls("codec_reuse");
        // Search counts logical judgments. Physical reuse includes proxy
        // hits too, so only some of its hits replace canonical encodes.
        assert!((physical..=physical + reuse).contains(&encodes), "{label}");
        assert_eq!(
            calls("codec_lookup"),
            physical + calls("proxy_brotli") + reuse
        );
        if !inlining {
            let expected = if proposals == 0 {
                [1; 7]
            } else {
                [1, 1, 2, 2, 2, 3, 3]
            };
            let terminal = [
                heads,
                heads + tried + surveys,
                tried,
                tried,
                tried,
                tried,
                tried,
            ];
            assert_eq!(
                phases[..7]
                    .iter()
                    .map(|row| row["calls"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .zip(terminal)
                    .map(|(search, terminal)| search + terminal)
                    .collect::<Vec<_>>()
            );
        } else {
            assert!(phases[0]["calls"].as_u64().unwrap() > 1);
            assert!(
                compiled.report()["search"]["proof_queries"]
                    .as_u64()
                    .unwrap()
                    > 0
            );
        }
    } else {
        assert_eq!(before, phase_counts());
    }
    assert_eq!(
        compiled.report()["resources"]["retained_bytes_after_handoff"],
        0
    );
    let artifacts: Vec<_> = objectives
        .iter()
        .map(|codec| {
            let artifact = compiled.javascript(codec).unwrap();
            json!({"objective":format!("{codec:?}"),"javascript":artifact.javascript(),
            "sha256":artifact.sha256(),"score":artifact.sizes().get(codec).unwrap()})
        })
        .collect();
    json!({"label":label,"phases":phases,"wall_ns":wall_ns,
        "deterministic":{"artifacts":artifacts,"search":compiled.report()["search"],
            "resources":compiled.report()["resources"]}})
}

fn check_refusal_and_native() {
    use crate::js::extract::OutputError;
    use crate::js::{Expr, Literal, Module, Statement};
    let before = phase_counts();
    let mut invalid = Module::default();
    invalid.scopes.clear();
    assert!(invalid.verify().unwrap_err().contains("root scope"));
    let mut module = Module::default();
    let value = module.expression(Expr::Literal(Literal::Number(1.0)), None);
    module.regions[0]
        .statements
        .push(Statement::Evaluate(value));
    let policy = configuration(0, false)
        .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let mut ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 1_000_000,
            optional_work: 1_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 1_000_000,
        },
    )
    .unwrap();
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_admitted(&policy, &mut budget)
            .unwrap();
        assert!(matches!(
            output.render_admitted(&Plan::new(Style::Global), 0, 17u64),
            Err(OutputError::ByteLimit)
        ));
    }
    assert_eq!(ledger.retained_bytes(), 0);
    let after = phase_counts();
    let delta: Vec<_> = after.iter().zip(&before).map(|(a, b)| a - b).collect();
    assert_eq!(
        delta,
        if enabled() {
            PHASE_BUCKETS.map(|bucket| match bucket.name {
                "target_verify" => 2,
                "target_edition" | "target_basis" | "target_names" | "target_print" => 1,
                _ => 0,
            })
        } else {
            [0; PHASE_BUCKETS.len()]
        }
    );
    let refused = compile_source(
        ANSWER,
        &configuration(0, false),
        ServiceOptions {
            target: ServiceTarget::Native,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert!(refused.message.contains("native exported ABI"));
    let native = compile_source(
        "int answer(){return 17;}print(answer());",
        &configuration(0, false),
        ServiceOptions {
            target: ServiceTarget::Native,
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert_eq!(native.report()["resources"]["codec_work"], 0);
    assert_eq!(
        native.report()["resources"]["retained_bytes_after_handoff"],
        0
    );
    assert_eq!(
        phase_counts(),
        after,
        "native does not enter JS target phases"
    );
    let raw = compile_source(
        ANSWER,
        &configuration(0, false),
        ServiceOptions {
            objectives: Some(Objectives::One(Objective::Raw)),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert_eq!(raw.report()["search"]["codec_probes"], 0);
    assert_eq!(
        &phase_counts()[7..9],
        &after[7..9],
        "raw has no encoder attempt"
    );
}

#[test]
fn semantic_phase_timing_is_observational_and_covers_refusal() {
    if std::env::var_os(CHILD).is_none() {
        let mut snapshots = Vec::new();
        for enabled in [false, true] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1");
            if enabled {
                command.env("LILSCRIPT_TIMING", "1");
            } else {
                command.env_remove("LILSCRIPT_TIMING");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            let rows: Vec<Value> = text
                .lines()
                .filter_map(|line| line.strip_prefix("semantic-phase-timing "))
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["enabled"], enabled);
            snapshots.push(rows.into_iter().next().unwrap());
        }
        for index in 0..3 {
            assert_eq!(
                snapshots[0]["cases"][index]["deterministic"],
                snapshots[1]["cases"][index]["deterministic"]
            );
        }
        eprintln!("semantic-phase-timing-pair {}", json!(snapshots));
        return;
    }
    let cases = [
        compile_case("direct", ANSWER, 0, false),
        compile_case("naming", ANSWER, 8, false),
        compile_case("structural", VALLEY, 24, true),
    ];
    check_refusal_and_native();
    static GUARD: Bucket = Bucket::new("guard-test");
    let fail = || -> Result<(), ()> {
        let _scope = GUARD.scope(0);
        Err(())
    };
    assert!(fail().is_err());
    assert!(std::panic::catch_unwind(|| {
        let _scope = GUARD.scope(0);
        panic!("intentional timing guard unwind");
    })
    .is_err());
    assert_eq!(GUARD.snapshot().1, if enabled() { 2 } else { 0 });
    let report = report(0).map(|text| serde_json::from_str::<Value>(&text).unwrap());
    assert_eq!(report.is_some(), enabled());
    if let Some(report) = &report {
        for bucket in PHASE_BUCKETS {
            assert_eq!(
                report[format!("{}_calls", bucket.name)],
                bucket.snapshot().1
            );
            assert!(report[format!("{}_ms", bucket.name)].as_f64().unwrap() >= 0.0);
            assert!(report.get(format!("{}_mb", bucket.name)).is_none());
        }
        let rule_calls = report.as_object().unwrap().iter()
            .filter(|(name, _)| {
                name.starts_with("js_rule_") && name.ends_with("_calls") && name.as_str() != "js_rule_calls"
            })
            .map(|(_, calls)| calls.as_u64().unwrap())
            .sum::<u64>();
        assert!(rule_calls > 0);
        assert_eq!(rule_calls, report["js_rule_calls"].as_u64().unwrap());
    }
    println!(
        "semantic-phase-timing {}",
        json!({"enabled":enabled(),"cases":cases,"report":report})
    );
}
