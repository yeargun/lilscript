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
        toml::from_str(&format!("objective.codecs='{codec}'\n{extra}")).unwrap();
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
    let searched = stage["searched"].as_i64().unwrap();
    assert!(searched <= before, "the search never worsens the level-0 artifact");
    // One list (architecture §9.6), walked in passes: the choice moves
    // (M9.1), the declared challengers, then the joint moves, each from the
    // incumbent before it. Each start (AM2) walks passes of its own from its
    // size, and its result replaces the objective's winner, first the
    // search's, only when smaller.
    let choice_trials = stage["choice_trials"].as_array().unwrap();
    let starts = stage["starts"].as_array().unwrap();
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
    let firsts: Vec<u64> = starts
        .iter()
        .map(|start| start["pass"].as_u64().unwrap())
        .chain(std::iter::once(passes + 1))
        .collect();
    assert!(firsts.windows(2).all(|pair| pair[0] <= pair[1]), "{stage}");
    assert!(
        starts.is_empty() || firsts[0] == 1,
        "the first start walks the first pass: {stage}"
    );
    // The walk in order; `true` marks a start's record.
    let mut walk: Vec<(bool, &Value)> = Vec::new();
    let mut best = searched;
    for (index, start) in starts.iter().enumerate() {
        walk.push((true, start));
        let bounds = firsts[index]..firsts[index + 1];
        let Some(from) = start["start"].as_i64() else {
            assert!(bounds.is_empty(), "an unformed restart walks no pass");
            assert!(start["size"].is_null() && start["delta"].is_null());
            if outcome(start) == "pruned" {
                assert!(start["proxy"].as_i64().unwrap() > margin, "{start}");
            }
            continue;
        };
        assert!(
            (bounds.end - bounds.start) <= stage["pass_limit"].as_u64().unwrap_or(u64::MAX),
            "{stage}"
        );
        let mut incumbent = from;
        for pass in bounds.clone() {
            let trials = pass_trials(pass);
            // A pass follows only one that kept a move.
            if pass + 1 < bounds.end {
                assert!(
                    trials.iter().any(|trial| outcome(trial) == "kept"),
                    "pass {pass} kept nothing, yet the walk went on: {stage}"
                );
            }
            for trial in trials {
                walk.push((false, trial));
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
                    // The walk's memo: an assignment judged before keeps its
                    // verdict, never smaller than the incumbent.
                    "recalled" => {
                        if let Some(size) = trial["size"].as_i64() {
                            assert!(size >= incumbent, "{trial}");
                            assert_eq!(trial["delta"].as_i64().unwrap(), size - incumbent);
                        }
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
        assert_eq!(start["size"].as_i64().unwrap(), incumbent, "{start}");
        let delta = start["delta"].as_i64().unwrap();
        assert_eq!(delta, incumbent - best, "{start}");
        match outcome(start) {
            "kept" => {
                assert!(delta < 0, "{start}");
                best = incumbent;
            }
            other => assert!(
                matches!(other, "rejected" | "identical") && delta >= 0,
                "{start}"
            ),
        }
    }
    assert_eq!(
        walk.len(),
        choice_trials.len() + trials(stage).len() + joint_trials(stage).len() + starts.len(),
        "every trial belongs to a pass the walk made: {stage}"
    );
    // Once the walk's prefix or exact budget closes, nothing after is
    // formed.
    if let Some(first) = walk.iter().position(|(_, trial)| outcome(trial) == "budget") {
        assert!(walk[first..].iter().all(|(_, trial)| outcome(trial) == "budget"));
        assert!(
            stage["examined"] == stage["prefix"] || stage["judged"] == stage["exact"],
            "{stage}"
        );
    }
    let count = |starts: bool, outcomes: &[&str]| {
        walk.iter()
            .filter(|(start, trial)| *start == starts && outcomes.contains(&outcome(trial)))
            .count()
    };
    // A restart is any start the portfolio does not hold (AM2): a naming
    // seed, the other int32 hints, the other objective's family seed.
    let restart = |trial: &Value| {
        trial["name"]
            .as_str()
            .is_some_and(|name| !matches!(name, "search" | "level-0" | "beam"))
    };
    let restarts = |outcomes: &[&str]| {
        walk.iter()
            .filter(|(start, trial)| *start && restart(trial) && outcomes.contains(&outcome(trial)))
            .count()
    };
    assert_eq!(count(false, &["stopped"]) + count(true, &["stopped"]), 0, "nothing ran out: {stage}");
    // A move takes a position, and so does a restart's start; a start the
    // portfolio holds does not.
    let moves = walk.iter().filter(|(start, _)| !*start).count();
    let restarted = walk.iter().filter(|(start, trial)| *start && restart(trial)).count();
    assert_eq!(
        stage["examined"].as_u64().unwrap() as usize,
        moves - count(false, &["budget"]) + restarted - restarts(&["budget"]),
        "{stage}"
    );
    assert!(stage["examined"].as_u64().unwrap() <= bound("prefix"));
    // A judged move and a measured restart start are one exact judgement
    // each; a recalled move is none.
    let measured_restarts = walk
        .iter()
        .filter(|(start, trial)| *start && restart(trial) && trial["start"].is_i64())
        .count();
    assert_eq!(
        stage["judged"].as_u64().unwrap() as usize,
        count(false, &["kept", "rejected"]) + measured_restarts,
        "{stage}"
    );
    assert!(stage["judged"].as_u64().unwrap() <= bound("exact"));
    assert_eq!(
        stage["pruned"].as_u64().unwrap() as usize,
        count(false, &["pruned"]) + restarts(&["pruned"])
    );
    assert_eq!(stage["after"].as_i64().unwrap(), best);
    assert!(best <= searched, "the walks never worsen the search's winner");
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
    let compiled = compile("brotli", "effort.level=15");
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
    let raw = compile("raw", "effort.level=15");
    let brotli = compile("brotli", "effort.level=15");
    let gzip = compile("gzip", "effort.level=15");
    for (compiled, codec) in [(&raw, "raw"), (&gzip, "gzip"), (&brotli, "brotli")] {
        check_spellings(compiled, codec);
    }
    let spelling = |compiled: &ServiceCompilation| stage(compiled)["spelling"].clone();
    // Each objective starts from its own seed and keeps what its own codec
    // measures smaller. Two answers may coincide (since R10 and R11 the raw
    // and Brotli objectives keep the same nine families here), but the
    // three codecs do not all judge alike.
    assert!(
        spelling(&raw) != spelling(&gzip) || spelling(&gzip) != spelling(&brotli),
        "{}",
        spelling(&raw)
    );
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
    use sha2::{Digest, Sha256};
    let mut previous: Option<(i64, i64)> = None;
    let mut digests = std::collections::BTreeMap::new();
    let mut stops = Vec::new();
    for level in 1..=15u8 {
        let compiled = compile("brotli", &format!("effort.level={level}"));
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
        let javascript = compiled.javascript(Objective::Brotli).unwrap().javascript();
        digests.insert(level, format!("{:x}", Sha256::digest(javascript.as_bytes())));
        stops.extend(
            stage["stops"]
                .as_array()
                .unwrap()
                .iter()
                .map(|stop| (level, stop.clone())),
        );
    }
    // Level 0 delivers the level-0 artifact: no walk and no codec.
    let off = compile("brotli", "effort.level=0");
    assert!(off.report()["search"]["terminal"]["objectives"]
        .as_array()
        .unwrap()
        .is_empty());
    let artifact = off.javascript(Objective::Brotli).unwrap();
    assert_eq!(artifact.sizes().get(Objective::Brotli), None, "level 0 measures no codec");
    digests.insert(0, format!("{:x}", Sha256::digest(artifact.javascript().as_bytes())));
    // The replay check (§9.6): wherever a walk from the level-0 artifact
    // passed a lower one-pass level's stopping point, that level's build
    // delivers exactly the recorded bytes.
    assert!(!stops.is_empty());
    for (level, stop) in &stops {
        let lower = stop["level"].as_u64().unwrap() as u8;
        assert!(lower < *level && lower <= 12, "{level}: {stop}");
        assert_eq!(
            stop["sha256"].as_str().unwrap(),
            digests[&lower],
            "the level-{level} walk's stop for level {lower}"
        );
    }
    // Level 13 records every lower level's stop.
    let recorded: std::collections::BTreeSet<u64> = stops
        .iter()
        .filter(|(level, _)| *level == 13)
        .map(|(_, stop)| stop["level"].as_u64().unwrap())
        .collect();
    assert_eq!(recorded, (0..=12).collect());
}

#[test]
fn a_vetoed_family_is_never_formed() {
    let compiled = compile(
        "brotli",
        "effort.level=15\n[policy.tactics]\ntarget-compaction='off'",
    );
    check_spellings(&compiled, "brotli");
    for trial in trials(stage(&compiled)) {
        match trial["challenger"].as_str().unwrap() {
            // The naming plan's members need no formation permission.
            "self-named-functions" | "read-order" => assert_ne!(outcome(trial), "vetoed"),
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
    let first = key(&compile("brotli", "effort.level=15"));
    assert_eq!(first, key(&compile("brotli", "effort.level=15")));
    let concurrent: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| key(&compile("brotli", "effort.level=15"))))
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
            "objective.codecs='{codec}'\neffort.level=15\n\
             [javascript]\nassume_pristine_builtins=true\n"
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
        "objective.codecs='brotli'\neffort.level=0\n\
         [javascript]\nassume_pristine_builtins=true\n",
    )
    .unwrap();
    let compiled = compile_source(TABLES, &config, ServiceOptions::default()).unwrap();
    assert!(compiled.report()["search"]["terminal"]["objectives"]
        .as_array()
        .unwrap()
        .is_empty());
}
