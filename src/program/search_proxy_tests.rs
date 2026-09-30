use super::*;

#[test]
fn library_proxy_policy_ignores_the_legacy_cli_environment() {
    const CHILD: &str = "LILSCRIPT_PROXY_POLICY_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let test = concat!(
            module_path!(),
            "::library_proxy_policy_ignores_the_legacy_cli_environment"
        );
        let test = test.split_once("::").unwrap().1;
        let mut snapshots = Vec::new();
        for present in [false, true] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", test, "--nocapture"])
                .env(CHILD, "1");
            if present {
                command.env("LILSCRIPT_WALK_AUDIT", "");
            } else {
                command.env_remove("LILSCRIPT_WALK_AUDIT");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            let rows: Vec<serde_json::Value> = stdout
                .lines()
                .filter_map(|line| line.strip_prefix("proxy-policy "))
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(rows.len(), 1);
            snapshots.push(rows.into_iter().next().unwrap());
        }
        assert_eq!(snapshots[0], snapshots[1]);
        return;
    }
    let config: crate::config::ProjectConfig =
        toml::from_str("objective.codecs='gzip'\neffort.level=13").unwrap();
    let compiled =
        crate::build::compile_source(PROGRAM, &config, crate::build::ServiceOptions::default())
            .unwrap();
    let terminal = &compiled.report()["search"]["terminal"]["objectives"][0];
    assert!(terminal["pruned"].as_u64().unwrap() > 0);
    println!(
        "proxy-policy {}",
        serde_json::json!({
            "policy": compiled.report()["javascript_policy"],
            "request": compiled.report()["request"],
            "search": compiled.report()["search"],
            "resources": compiled.report()["resources"],
            "javascript": compiled.javascript(Objective::Gzip).unwrap().javascript(),
        })
    );
}

#[test]
fn configured_proxy_modes_preserve_oracles_and_expose_their_actual_work() {
    let mut audited_prunes = 0;
    for codec in ["raw", "gzip", "brotli"] {
        let mut runs = Vec::new();
        for mode in ["on", "audit", "off"] {
            let config: crate::config::ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n[policy.search]\nproxy_pruning='{mode}'"
            ))
            .unwrap();
            let policy = config
                .resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                })
                .unwrap();
            let objective = policy.objective().unwrap().codec;
            let run = search(&policy, Objectives::One(objective), true);
            let winner = run.winners.iter().flatten().next().unwrap();
            let output = std::process::Command::new("node")
                .args(["--input-type=module", "-e", &winner.1])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{codec}/{mode}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                include_str!("../../tests/cases/objective_judged_spellings.out"),
                "{codec}/{mode}"
            );
            let report = &run.report.objectives[0];
            let audits: Vec<_> = report
                .trials
                .iter()
                .map(|trial| trial.audit)
                .chain(report.choice_trials.iter().map(|trial| trial.audit))
                .chain(report.joint_trials.iter().map(|trial| trial.audit))
                .chain(report.starts.iter().map(|trial| trial.audit))
                .flatten()
                .collect();
            if mode == "audit" {
                assert_eq!(audits.len(), report.pruned, "{codec}");
                audited_prunes += audits.len();
            } else {
                assert!(audits.is_empty(), "{codec}/{mode}");
            }
            if mode == "off" {
                assert_eq!(report.pruned, 0, "{codec}");
            }
            runs.push(run);
        }
        let [ordinary, audited, _unpruned]: [Run; 3] = runs.try_into().ok().unwrap();
        // This fixture completes comfortably within its hard limits. Auditing
        // changes neither the choices nor the useful exact-judgement count.
        assert_eq!(ordinary.winners, audited.winners, "{codec}");
        assert_eq!(
            ordinary.report.objectives[0].judged, audited.report.objectives[0].judged,
            "{codec}"
        );
        assert!(
            audited.logical_work[1] >= ordinary.logical_work[1],
            "{codec}"
        );
        if codec != "raw" {
            assert!(
                audited.report.objectives[0].codec_probes
                    >= ordinary.report.objectives[0].codec_probes,
                "{codec}"
            );
        }
        // Even an exact proxy can reject a larger restart whose later walk
        // would improve. No-prune mode may therefore change the trajectory;
        // its obligations above are exact admission and the execution oracle.
    }
    assert!(
        audited_prunes > 0,
        "fixture must exercise real proxy rejection"
    );
}
