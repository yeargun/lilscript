//! The terminal challenger stage through the public service (plan M5.4,
//! M9.2, M9.3): what it keeps, what it rejects, its report, its budget and its
//! determinism. The program is `tests/cases/objective_judged_spellings.lil`,
//! whose `.out` file is its oracle.
use super::*;
use serde_json::Value;
use std::process::Command;

const PROGRAM: &str = include_str!("../tests/cases/objective_judged_spellings.lil");
const EXPECTED: &str = include_str!("../tests/cases/objective_judged_spellings.out");

fn compile(codec: &str, extra: &str) -> ServiceCompilation {
    let config: ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncost_model='{codec}'\n{extra}"
    ))
    .unwrap();
    compile_source(PROGRAM, &config, ServiceOptions::default()).unwrap()
}

fn codec_of(name: &str) -> Objective {
    match name {
        "raw" => Objective::Raw,
        "gzip" => Objective::Gzip,
        _ => Objective::Brotli,
    }
}

/// The stage's report on the requested objective.
fn stage(compiled: &ServiceCompilation) -> &Value {
    let objectives = compiled.report()["search"]["terminal"]["objectives"]
        .as_array()
        .expect("the build reports its terminal stage");
    assert_eq!(objectives.len(), 1, "one requested objective, one stage");
    &objectives[0]
}

fn trials(stage: &Value) -> &[Value] {
    stage["trials"].as_array().unwrap()
}

fn outcome(trial: &Value) -> &str {
    trial["outcome"].as_str().unwrap()
}

fn delivered(compiled: &ServiceCompilation, codec: &str) -> usize {
    let artifact = compiled.javascript(codec_of(codec)).unwrap();
    let size = artifact.sizes().get(codec_of(codec)).unwrap();
    assert_eq!(
        size,
        crate::compression::measure(artifact.javascript().as_bytes(), codec_of(codec)).unwrap(),
        "the delivered bytes own their score"
    );
    size
}

fn execute(javascript: &str) -> String {
    let script = format!(
        "await import('data:text/javascript,'+encodeURIComponent({}));",
        serde_json::to_string(javascript).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for terminal stage observation tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// The report's arithmetic: every kept challenger shrank the incumbent it
/// was applied to, every rejected one did not, and the delivered artifact is
/// the search winner plus the kept deltas.
fn check_stage(compiled: &ServiceCompilation, codec: &str) {
    let stage = stage(compiled);
    assert_eq!(stage["codec"], codec);
    let names: Vec<&str> = trials(stage)
        .iter()
        .map(|trial| trial["challenger"].as_str().unwrap())
        .collect();
    let declared: Vec<&str> = crate::js::Challenger::ORDER
        .iter()
        .map(|challenger| challenger.name())
        .collect();
    assert_eq!(names, declared, "every declared challenger, in order");
    let before = stage["before"].as_i64().unwrap();
    let mut incumbent = before;
    for trial in trials(stage) {
        match outcome(trial) {
            "kept" => {
                let delta = trial["delta"].as_i64().unwrap();
                assert!(delta < 0, "a kept challenger is strictly smaller: {trial}");
                assert_eq!(trial["size"].as_i64().unwrap(), incumbent + delta);
                incumbent += delta;
            }
            "rejected" => {
                let delta = trial["delta"].as_i64().unwrap();
                assert!(
                    delta >= 0,
                    "a smaller challenger is never rejected: {trial}"
                );
                assert_eq!(trial["size"].as_i64().unwrap(), incumbent + delta);
            }
            _ => assert!(trial["size"].is_null() && trial["delta"].is_null()),
        }
    }
    assert_eq!(stage["after"].as_i64().unwrap(), incumbent);
    assert!(incumbent <= before, "the stage never worsens the incumbent");
    assert_eq!(delivered(compiled, codec) as i64, incumbent);
    let count = |outcomes: &[&str]| {
        trials(stage)
            .iter()
            .filter(|trial| outcomes.contains(&outcome(trial)))
            .count()
    };
    assert_eq!(
        stage["tried"].as_u64().unwrap() as usize,
        count(&["kept", "rejected", "identical", "refused"])
    );
    let scored = count(&["kept", "rejected"]);
    assert_eq!(stage["scored"].as_u64().unwrap() as usize, scored);
    assert!(scored <= stage["budget"].as_u64().unwrap() as usize);
    let javascript = compiled.javascript(codec_of(codec)).unwrap().javascript();
    assert_eq!(execute(javascript), EXPECTED, "{codec}: {javascript}");
}

#[test]
fn the_stage_keeps_a_challenger_that_shrinks_the_artifact_and_rejects_one_that_grows_it() {
    // Level 15 tries the whole schedule.
    let compiled = compile("brotli", "optimization_level=15");
    check_stage(&compiled, "brotli");
    let stage = stage(&compiled);
    let kept = trials(stage)
        .iter()
        .filter(|trial| outcome(trial) == "kept");
    let rejected = trials(stage)
        .iter()
        .filter(|trial| outcome(trial) == "rejected");
    assert!(kept.count() >= 1, "{stage}");
    assert!(rejected.count() >= 1, "{stage}");
    assert!(
        stage["after"].as_u64() < stage["before"].as_u64(),
        "{stage}"
    );
    assert!(trials(stage).iter().all(|trial| outcome(trial) != "budget"));
}

#[test]
fn the_objective_seeds_the_families_and_its_codec_judges_them() {
    let raw = compile("raw", "optimization_level=15");
    let brotli = compile("brotli", "optimization_level=15");
    let gzip = compile("gzip", "optimization_level=15");
    for (compiled, codec) in [(&raw, "raw"), (&gzip, "gzip"), (&brotli, "brotli")] {
        check_stage(compiled, codec);
    }
    let spelling = |compiled: &ServiceCompilation| stage(compiled)["spelling"].clone();
    // Each objective starts from its own seed and keeps what its own codec
    // measures smaller, so the three keep three different assignments (when
    // this landed: the raw objective turns its own seed's logical branches
    // off, Brotli keeps the raw seed with them, gzip keeps a codec subset).
    assert_ne!(spelling(&raw), spelling(&brotli));
    assert_ne!(spelling(&raw), spelling(&gzip));
    assert_ne!(spelling(&gzip), spelling(&brotli));
    // The raw objective's own seed is not its answer: the codec judged at
    // least one of its families off.
    assert!(trials(stage(&raw))
        .iter()
        .any(|trial| outcome(trial) == "kept"));
    // Every family is available to every objective: the raw objective is
    // offered the codecs' whole seed, a codec the raw one.
    for compiled in [&raw, &brotli] {
        let other = trials(stage(compiled))
            .iter()
            .find(|trial| trial["challenger"] == "other-objective-seed")
            .unwrap();
        assert!(matches!(outcome(other), "kept" | "rejected"), "{other}");
    }
}

#[test]
fn the_effort_sets_the_schedule_prefix_and_a_longer_one_never_ends_larger() {
    let mut previous: Option<(i64, i64)> = None;
    for (level, budget) in [(8u8, 3usize), (13, 7), (15, usize::MAX)] {
        let compiled = compile("brotli", &format!("optimization_level={level}"));
        check_stage(&compiled, "brotli");
        let stage = stage(&compiled);
        if budget != usize::MAX {
            assert_eq!(stage["budget"].as_u64().unwrap() as usize, budget);
        }
        let scored = stage["scored"].as_u64().unwrap() as usize;
        assert!(scored <= budget);
        // Once the codec has judged the budget, nothing further is formed.
        if let Some(first) = trials(stage)
            .iter()
            .position(|trial| outcome(trial) == "budget")
        {
            assert_eq!(scored, budget);
            assert!(trials(stage)[first..]
                .iter()
                .all(|trial| outcome(trial) == "budget"));
        }
        let (before, after) = (
            stage["before"].as_i64().unwrap(),
            stage["after"].as_i64().unwrap(),
        );
        if let Some((lower_before, lower_after)) = previous {
            // The same search winner at both levels (the program's search is
            // exhausted at each): from it, a longer prefix keeps at least as
            // much.
            assert_eq!(before, lower_before);
            assert!(
                after <= lower_after,
                "level {level}: {after} > {lower_after}"
            );
        }
        previous = Some((before, after));
    }
    // The search-off levels offer nothing.
    let off = compile("brotli", "optimization_level=0");
    let stage = stage(&off);
    assert_eq!(stage["budget"], 0);
    assert_eq!(stage["tried"], 0);
    assert!(trials(stage).iter().all(|trial| outcome(trial) == "budget"));
    assert_eq!(stage["before"], stage["after"]);
}

#[test]
fn a_vetoed_family_is_never_formed() {
    let compiled = compile(
        "brotli",
        "optimization_level=15\n[policy.tactics]\ntarget-compaction='off'",
    );
    check_stage(&compiled, "brotli");
    for trial in trials(stage(&compiled)) {
        match trial["challenger"].as_str().unwrap() {
            // The naming plan's spelling needs no formation permission.
            "raw-spelling" => assert_ne!(outcome(trial), "vetoed"),
            _ => assert!(matches!(outcome(trial), "vetoed" | "duplicate"), "{trial}"),
        }
    }
}

#[test]
fn the_stage_is_deterministic_across_runs_and_threads() {
    let key = |compiled: &ServiceCompilation| {
        (
            compiled
                .javascript(Objective::Brotli)
                .unwrap()
                .javascript()
                .to_string(),
            compiled.report()["search"]["terminal"].clone(),
        )
    };
    let first = key(&compile("brotli", "optimization_level=15"));
    assert_eq!(first, key(&compile("brotli", "optimization_level=15")));
    let concurrent: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| key(&compile("brotli", "optimization_level=15"))))
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect()
    });
    for result in concurrent {
        assert_eq!(result, first);
    }
}
