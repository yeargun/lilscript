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
        "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit=48\ncandidate_limit=8\ncandidate_beam_width=2\n[policy.search]\ncodec_schedule='staged'\nrender_batch=8\ndiversity_interval=4\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'\ninlining='{}'\nscalar-replacement='off'\ncall-specialization='off'\nconstant-folding='off'\nstring-pooling='off'",
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
    let compiled = compile_source(
        source,
        &configuration(proposals, inlining),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let wall_ns = started.elapsed().as_nanos();
    let phases: Vec<_> = PHASE_BUCKETS.iter().enumerate().map(|(index, bucket)| {
        let (nanos, calls, bytes) = bucket.snapshot();
        assert_eq!(bytes, 0, "phase counters do not claim byte measurements");
        json!({"name":bucket.name,"calls":calls-before[index],"elapsed_ns":nanos-phases_before[index]})
    }).collect();
    if enabled() {
        let renders = compiled.report()["search"]["renders"].as_u64().unwrap();
        assert_eq!(phases[5]["calls"], renders);
        assert_eq!(phases[6]["calls"], renders);
        let encodes = compiled.report()["search"]["codec_probes"]
            .as_u64()
            .unwrap()
            + 2;
        assert_eq!(
            phases[7]["calls"].as_u64().unwrap() + phases[8]["calls"].as_u64().unwrap(),
            encodes
        );
        if !inlining {
            let expected = if proposals == 0 {
                [1; 7]
            } else {
                [1, 1, 2, 2, 2, 3, 3]
            };
            assert_eq!(
                phases[..7]
                    .iter()
                    .map(|row| row["calls"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                expected
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
    let artifacts: Vec<_> = [Objective::Raw, Objective::Gzip, Objective::Brotli]
        .into_iter()
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
            [0, 0, 2, 1, 1, 1, 1, 0, 0]
        } else {
            [0; 9]
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
        &phase_counts()[7..],
        &after[7..],
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
    }
    println!(
        "semantic-phase-timing {}",
        json!({"enabled":enabled(),"cases":cases,"report":report})
    );
}
