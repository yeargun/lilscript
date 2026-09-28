//! The walk through the public service (plan M3.5, M9.2, M9.3): what it
//! keeps, what it rejects and prunes, its report, its counts and its
//! determinism. The program is `tests/cases/objective_judged_spellings.lil`,
//! whose `.out` file is its oracle.
use super::*;
use serde_json::Value;
use std::process::Command;

const PROGRAM: &str = include_str!("../tests/cases/objective_judged_spellings.lil");
const EXPECTED: &str = include_str!("../tests/cases/objective_judged_spellings.out");

fn compile(codec: &str, extra: &str) -> ServiceCompilation {
    let config: ProjectConfig =
        toml::from_str(&format!("[javascript]\ncost_model='{codec}'\n{extra}")).unwrap();
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

fn joint_trials(stage: &Value) -> &[Value] {
    stage["joint_trials"].as_array().unwrap()
}

/// The report's arithmetic: every kept move shrank the incumbent it was
/// applied to, every rejected one did not, the delivered artifact is the
/// level-0 artifact plus the kept deltas, and the walk's counts are its
/// trials' (architecture §9.6).
fn check_stage(compiled: &ServiceCompilation, codec: &str) {
    let stage = stage(compiled);
    assert_eq!(stage["codec"], codec);
    let margin = stage["margin"].as_i64().unwrap();
    // A schedule count the level leaves unbounded is null.
    let bound = |key: &str| stage[key].as_u64().unwrap_or(u64::MAX);
    let names: Vec<&str> = trials(stage)
        .iter()
        .filter(|trial| trial["pass"] == 1)
        .map(|trial| trial["challenger"].as_str().unwrap())
        .collect();
    let declared: Vec<&str> = crate::js::Challenger::ORDER
        .iter()
        .map(|challenger| challenger.name())
        .collect();
    assert_eq!(names, declared, "every declared challenger, in order, in the first pass");
    let before = stage["before"].as_i64().unwrap();
    // One list (architecture §9.6), walked in passes: the choice moves
    // (M9.1), the declared challengers, then the joint moves, each from the
    // incumbent before it. The tail's restarts each walk passes of their own
    // from their start, and replace the best result only when smaller.
    let choice_trials = stage["choice_trials"].as_array().unwrap();
    let restarts = stage["restarts"].as_array().unwrap();
    let passes = stage["passes"].as_u64().unwrap();
    let in_pass = |list: &'static str, pass: u64| {
        stage[list]
            .as_array()
            .unwrap()
            .iter()
            .filter(move |trial| trial["pass"] == pass)
    };
    let pass_trials = |pass: u64| -> Vec<&Value> {
        in_pass("choice_trials", pass)
            .chain(in_pass("trials", pass))
            .chain(in_pass("joint_trials", pass))
            .collect()
    };
    // Each walk's passes: the main walk's, then each restart's.
    let firsts: Vec<u64> = std::iter::once(1)
        .chain(restarts.iter().map(|restart| restart["pass"].as_u64().unwrap()))
        .chain(std::iter::once(passes + 1))
        .collect();
    assert!(firsts.windows(2).all(|pair| pair[0] <= pair[1]), "{stage}");
    let mut walk: Vec<&Value> = Vec::new();
    let mut best = before;
    for (segment, bounds) in firsts.windows(2).enumerate() {
        let restart = segment.checked_sub(1).map(|index| &restarts[index]);
        let mut incumbent = match restart {
            None => before,
            Some(restart) => {
                walk.push(restart);
                match restart["start"].as_i64() {
                    Some(start) => start,
                    None => {
                        assert_eq!(bounds[0], bounds[1], "an unformed restart walks no pass");
                        assert!(restart["size"].is_null() && restart["delta"].is_null());
                        if outcome(restart) == "pruned" {
                            assert!(restart["proxy"].as_i64().unwrap() > margin, "{restart}");
                        }
                        continue;
                    }
                }
            }
        };
        let segment_passes = bounds[0]..bounds[1];
        assert!(
            (segment_passes.end - segment_passes.start) <= stage["pass_limit"].as_u64().unwrap_or(u64::MAX),
            "{stage}"
        );
        for pass in segment_passes.clone() {
            let trials = pass_trials(pass);
            // A pass follows only one that kept a move.
            if pass + 1 < segment_passes.end {
                assert!(
                    trials.iter().any(|trial| outcome(trial) == "kept"),
                    "pass {pass} kept nothing, yet the walk went on: {stage}"
                );
            }
            for trial in trials {
                walk.push(trial);
                match outcome(trial) {
                    "kept" => {
                        let delta = trial["delta"].as_i64().unwrap();
                        assert!(delta < 0, "a kept move is strictly smaller: {trial}");
                        assert_eq!(trial["size"].as_i64().unwrap(), incumbent + delta);
                        incumbent += delta;
                    }
                    "rejected" => {
                        let delta = trial["delta"].as_i64().unwrap();
                        assert!(delta >= 0, "a smaller move is never rejected: {trial}");
                        assert_eq!(trial["size"].as_i64().unwrap(), incumbent + delta);
                    }
                    // The proxy judged it worse than the incumbent by more
                    // than the margin; the exact codec never ran.
                    "pruned" => {
                        assert!(trial["proxy"].as_i64().unwrap() > margin, "{trial}");
                        assert!(trial["size"].is_null() && trial["delta"].is_null());
                    }
                    _ => assert!(trial["size"].is_null() && trial["delta"].is_null()),
                }
            }
        }
        match restart {
            None => best = incumbent,
            Some(restart) => {
                assert_eq!(restart["size"].as_i64().unwrap(), incumbent, "{restart}");
                let delta = restart["delta"].as_i64().unwrap();
                assert_eq!(delta, incumbent - best, "{restart}");
                match outcome(restart) {
                    "kept" => {
                        assert!(delta < 0, "{restart}");
                        best = incumbent;
                    }
                    other => assert_eq!((other, delta >= 0), ("rejected", true), "{restart}"),
                }
            }
        }
    }
    assert_eq!(
        walk.len(),
        choice_trials.len() + trials(stage).len() + joint_trials(stage).len() + restarts.len(),
        "every trial belongs to a pass the walk made: {stage}"
    );
    // Once the walk's prefix or exact budget closes, nothing after is
    // formed.
    if let Some(first) = walk.iter().position(|trial| outcome(trial) == "budget") {
        assert!(walk[first..].iter().all(|trial| outcome(trial) == "budget"));
        assert!(
            stage["examined"] == stage["prefix"] || stage["judged"] == stage["exact"],
            "{stage}"
        );
    }
    let all = |outcomes: &[&str]| {
        walk.iter()
            .filter(|trial| outcomes.contains(&outcome(trial)))
            .count()
    };
    assert_eq!(all(&["stopped"]), 0, "nothing ran out: {stage}");
    assert_eq!(
        stage["examined"].as_u64().unwrap() as usize,
        walk.len() - all(&["budget"])
    );
    assert!(stage["examined"].as_u64().unwrap() <= bound("prefix"));
    // A restart's start is measured exactly, one judgement.
    assert_eq!(
        stage["judged"].as_u64().unwrap() as usize,
        all(&["kept", "rejected"])
    );
    assert!(stage["judged"].as_u64().unwrap() <= bound("exact"));
    assert_eq!(stage["pruned"].as_u64().unwrap() as usize, all(&["pruned"]));
    assert_eq!(stage["after"].as_i64().unwrap(), best);
    assert!(best <= before, "the walk never worsens the level-0 artifact");
    assert_eq!(delivered(compiled, codec) as i64, best);
    let count = |outcomes: &[&str]| {
        trials(stage)
            .iter()
            .filter(|trial| outcomes.contains(&outcome(trial)))
            .count()
    };
    assert_eq!(
        stage["tried"].as_u64().unwrap() as usize,
        count(&["kept", "rejected", "identical", "refused", "pruned"])
    );
    let scored = count(&["kept", "rejected"]);
    assert_eq!(stage["scored"].as_u64().unwrap() as usize, scored);
    let choices = |outcomes: &[&str]| {
        choice_trials
            .iter()
            .filter(|trial| outcomes.contains(&outcome(trial)))
            .count()
    };
    assert_eq!(
        stage["choices_tried"].as_u64().unwrap() as usize,
        choices(&["kept", "rejected", "identical", "refused", "pruned"])
    );
    let scored = choices(&["kept", "rejected"]);
    assert_eq!(stage["choices_scored"].as_u64().unwrap() as usize, scored);
    // Every delivered choice site names an alternative it offers.
    for site in stage["choices"].as_array().unwrap() {
        let offered: Vec<&str> = site["offered"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| pair[0].as_str().unwrap())
            .collect();
        assert!(
            offered.contains(&site["delivered"].as_str().unwrap()),
            "{site}"
        );
        assert!(offered.contains(&site["seed"].as_str().unwrap()), "{site}");
    }
}

/// `check_stage`, and the delivered program prints the spellings case's
/// expected output.
fn check_spellings(compiled: &ServiceCompilation, codec: &str) {
    check_stage(compiled, codec);
    let javascript = compiled.javascript(codec_of(codec)).unwrap().javascript();
    assert_eq!(execute(javascript), EXPECTED, "{codec}: {javascript}");
}

#[test]
fn the_stage_keeps_a_challenger_that_shrinks_the_artifact_and_rejects_one_that_grows_it() {
    // Level 15 walks the whole list.
    let compiled = compile("brotli", "optimization_level=15");
    check_spellings(&compiled, "brotli");
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
        check_spellings(compiled, codec);
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
        assert!(
            matches!(outcome(other), "kept" | "rejected" | "pruned"),
            "{other}"
        );
    }
}

#[test]
fn the_effort_sets_the_schedule_prefix_and_a_longer_one_never_ends_larger() {
    let mut previous: Option<(i64, i64)> = None;
    for level in 1..=15u8 {
        let compiled = compile("brotli", &format!("optimization_level={level}"));
        check_spellings(&compiled, "brotli");
        let stage = stage(&compiled);
        // The level alone sets the walk's counts.
        let schedule = crate::compilation_policy::WalkSchedule::at(level, Objective::Brotli);
        // An unbounded count is null.
        let count = |value: &Value| value.as_u64().map_or(usize::MAX, |count| count as usize);
        assert!(!stage["prefix"].is_u64() || schedule.prefix != usize::MAX);
        assert_eq!(count(&stage["prefix"]), schedule.prefix);
        assert_eq!(count(&stage["exact"]), schedule.exact);
        assert_eq!(count(&stage["margin"]), schedule.margin);
        assert_eq!(count(&stage["pass_limit"]), schedule.passes);
        let (before, after) = (
            stage["before"].as_i64().unwrap(),
            stage["after"].as_i64().unwrap(),
        );
        if let Some((lower_before, lower_after)) = previous {
            // Every level starts from the level-0 artifact, and a longer
            // walk over the same list keeps at least as much (monotone by
            // construction).
            assert_eq!(before, lower_before);
            assert!(
                after <= lower_after,
                "level {level}: {after} > {lower_after}"
            );
        }
        previous = Some((before, after));
    }
    // Level 0 delivers the level-0 artifact: no walk.
    let off = compile("brotli", "optimization_level=0");
    let stage = stage(&off);
    assert_eq!(stage["prefix"], 0);
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
    check_spellings(&compiled, "brotli");
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

const TABLES: &str = include_str!("../tests/cases/data_tables.lil");
const TABLES_HOST: &str = include_str!("../tests/cases/data_tables.host.js");
const TABLES_EXPECTED: &str = include_str!("../tests/cases/data_tables.out");

#[test]
fn every_objective_judges_the_data_tables_and_delivers_them_exactly() {
    // Plan M9.1 and M9.8: each table site's alternatives are judged by the
    // objective's codec after the challengers, within the choice budget, and
    // whatever each objective keeps decodes to the literal's own graph.
    let mut delivered = Vec::new();
    for codec in ["raw", "gzip", "brotli"] {
        let config: ProjectConfig = toml::from_str(&format!(
            "[javascript]\nassume_pristine_builtins=true\n\
             cost_model='{codec}'\noptimization_level=15\n"
        ))
        .unwrap();
        let compiled = compile_source(TABLES, &config, ServiceOptions::default()).unwrap();
        check_stage(&compiled, codec);
        let stage = stage(&compiled);
        let sites = stage["choices"].as_array().unwrap();
        assert!(sites.len() >= 3, "{stage}");
        assert!(
            sites.iter().any(|site| site["delivered"] != "literal"),
            "{codec}: some table is encoded: {stage}"
        );
        // Every offered alternative other than the delivered one was judged.
        assert!(stage["choice_trials"]
            .as_array()
            .unwrap()
            .iter()
            .all(|trial| trial["outcome"] != "budget"));
        let javascript = compiled.javascript(codec_of(codec)).unwrap().javascript();
        let script = format!(
            "{TABLES_HOST}\nawait import('data:text/javascript,'+encodeURIComponent({}));",
            serde_json::to_string(javascript).unwrap(),
        );
        let output = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for terminal stage observation tests");
        assert!(output.status.success(), "{codec}: {javascript}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            TABLES_EXPECTED,
            "{codec}: {javascript}"
        );
        delivered.push(
            sites
                .iter()
                .map(|site| site["delivered"].as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
        );
    }
    // The raw objective's estimator seeds; the codecs judged some site
    // otherwise (when this landed, three objectives, three assignments).
    assert!(
        delivered[0] != delivered[1] || delivered[0] != delivered[2],
        "{delivered:?}"
    );
    // Level 0 has no walk: the seeds ship.
    let config: ProjectConfig = toml::from_str(
        "[javascript]\nassume_pristine_builtins=true\n\
         cost_model='brotli'\noptimization_level=0\n",
    )
    .unwrap();
    let compiled = compile_source(TABLES, &config, ServiceOptions::default()).unwrap();
    let stage = stage(&compiled);
    assert_eq!(stage["prefix"], 0);
    assert!(stage["choice_trials"].as_array().unwrap().is_empty());
}
