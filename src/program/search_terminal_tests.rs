//! Monotone selection (plan M5.4, law L7), observed on the search itself.
//!
//! What the search guarantees, and how:
//! * **Within a search, an incumbent never worsens.** `Portfolio::score`
//!   replaces a codec's incumbent only with a strictly smaller admitted
//!   artifact, so each winner is the minimum of every artifact scored for its
//!   codec; the terminal stage then replaces it only with a strictly smaller
//!   one again.
//! * **Every effort level's result is at most the search-off result.** The
//!   mandatory baseline (the direct recipe, the policy's first naming seed and
//!   literal mode, the objective's seed families) does not depend on effort,
//!   and it is the first incumbent of every search.
//! * **The terminal stage is monotone in effort.** It tries a prefix of one
//!   declared schedule whose length is the effort's budget, sequentially,
//!   so from the same search winner a higher level keeps at least as much.
//! * **Not guaranteed today:** between two search-enabled levels whose
//!   search winners differ. Beam width, retained-candidate capacity and the
//!   effort-gated tactics (naming search at 8, call specialization at 11)
//!   shape the search's trajectory, so a higher level can explore a different
//!   frontier. Replaying the lower level's schedule as the first phase of the
//!   higher one closes this; it belongs with the calibrated effort ladder
//!   (M9.10), which re-derives those parameters.
use super::*;
#[path = "search_proxy_tests.rs"]
mod proxy_modes;
#[path = "search_deferred_naming_tests.rs"]
mod deferred_naming;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};

#[test]
fn accounting_errors_cannot_masquerade_as_a_completed_shorter_search() {
    for error in [
        BudgetError::WorkExhausted(WorkDomain::Optional),
        BudgetError::MemoryExhausted(WorkDomain::Optional),
        BudgetError::DeadlineExceeded,
    ] {
        assert!(matches!(
            refusal(CandidateError::Budget(error)),
            Ok(Judgement::Stopped)
        ));
    }
    for error in [
        BudgetError::InvalidPlan,
        BudgetError::BaselineExceedsLimit,
        BudgetError::BaselineNotSealed,
        BudgetError::InvalidBaselinePhase,
        BudgetError::BaselineAllocationSealed,
        BudgetError::InvalidRelease,
        BudgetError::AnalysisAttemptMismatch,
        BudgetError::InvalidAnalysisReceipt,
    ] {
        assert!(matches!(
            refusal(CandidateError::Budget(error)),
            Err(SearchError::Candidate(CandidateError::Budget(found))) if found == error
        ));
    }
}
use crate::js::selection::{Objective, Objectives};
use crate::program::facts::CacheLimits;

const PROGRAM: &str = include_str!("../../tests/cases/objective_judged_spellings.lil");

fn policy(codec: &str, level: u8) -> ResolvedPolicy {
    policy_with_search(codec, level, "")
}

fn policy_with_search(codec: &str, level: u8, search: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig =
        toml::from_str(&format!("objective.codecs='{codec}'\neffort.level={level}\n[policy.search]\n{search}")).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn request(objectives: Objectives) -> SearchRequest {
    let local_facts = LocalFactsRequest {
        work_quota: 1_000_000,
        result_bytes: 512_000,
    };
    SearchRequest {
        objectives,
        scalar: ScalarRequest {
            max_work: 2_000_000,
            scratch_bytes: 4_000_000,
            output_bytes: 2_000_000,
        },
        helper: HelperRequest {
            max_work: 2_000_000,
            scratch_bytes: 4_000_000,
            output_bytes: 2_000_000,
            local_facts,
        },
        string: StringRequest {
            max_work: 2_000_000,
            scratch_bytes: 4_000_000,
            output_bytes: 2_000_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 128,
            bytes: 16_000_000,
            result_bytes: 512_000,
        },
    }
}

struct Run {
    /// Every scored artifact's size per codec, in scoring order.
    scored: Vec<[Option<usize>; 3]>,
    baseline: String,
    winners: [Option<(usize, String)>; 3],
    report: TerminalReport,
    logical_work: [u64; 2],
    cache_hits: usize,
}

fn search(policy: &ResolvedPolicy, objectives: Objectives, challenge: bool) -> Run {
    search_source(PROGRAM, policy, objectives, challenge)
}

fn search_source(
    text: &str,
    policy: &ResolvedPolicy,
    objectives: Objectives,
    challenge: bool,
) -> Run {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, text).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: 200_000_000,
            retained_bytes: 256_000_000,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 128 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let mut scored = Vec::new();
    let mut baseline = String::new();
    let run = {
        let mut search = compilation
            .search_javascript_observed(source, policy, request(objectives), |observation| {
                if observation.baseline {
                    baseline = observation.javascript.to_string();
                }
                scored.push(
                    [Objective::Raw, Objective::Gzip, Objective::Brotli]
                        .map(|codec| observation.sizes.get(codec)),
                );
            })
            .unwrap();
        let report = if challenge {
            search
                .challenge(policy, request(objectives))
                .unwrap()
                .clone()
        } else {
            TerminalReport::default()
        };
        // Level 0 measures no codec (M3.5): its size is taken here.
        let winners = [Objective::Raw, Objective::Gzip, Objective::Brotli].map(|codec| {
            search.with_winner(codec, |view, _| {
                let size = view.sizes.get(codec).unwrap_or_else(|| {
                    crate::compression::measure(view.javascript.as_bytes(), codec).unwrap()
                });
                (size, view.javascript.to_string())
            })
        });
        (winners, report)
    };
    let logical_work = [WorkDomain::Baseline, WorkDomain::Optional]
        .map(|domain| compilation.ledger().work_used(domain));
    let cache_hits = compilation.artifacts.measurement_hits();
    assert_eq!(compilation.finish().retained_bytes(), 0);
    Run {
        scored,
        baseline,
        winners: run.0,
        report: run.1,
        logical_work,
        cache_hits,
    }
}

#[test]
fn codec_reuse_preserves_logical_search_and_final_bytes_for_every_objective() {
    for codec in ["raw", "gzip", "brotli"] {
        let policy = policy(codec, 15);
        let objective = Objectives::One(policy.objective().unwrap().codec);
        let warm = search(&policy, objective, true);
        let cold =
            super::artifacts::compression_cache::without_reuse(|| search(&policy, objective, true));
        assert_eq!(warm.logical_work, cold.logical_work, "{codec}");
        assert_eq!(cold.cache_hits, 0);
        if codec != "raw" {
            assert!(
                warm.cache_hits > 0,
                "fixture must exercise a hit for {codec}"
            );
        }
        assert_eq!(warm.baseline, cold.baseline, "{codec}");
        assert_eq!(warm.scored, cold.scored, "{codec}");
        assert_eq!(warm.winners, cold.winners, "{codec}");
        assert_eq!(warm.report, cold.report, "{codec}");
    }
}

#[test]
fn q2_normalization_reuse_preserves_logical_search_and_objective_bytes() {
    for codec in ["raw", "gzip", "brotli"] {
        let enabled = policy(codec, 13);
        let disabled = enabled.clone().with_cache(crate::config::CacheConfig {
            normalization_reuse: false, ..Default::default()
        });
        let objective = Objectives::One(enabled.objective().unwrap().codec);
        let on = search(&enabled, objective, true);
        let off = search(&disabled, objective, true);
        assert_eq!(on.logical_work, off.logical_work, "{codec}");
        assert_eq!(on.scored, off.scored, "{codec}");
        assert_eq!(on.winners, off.winners, "{codec}");
        assert_eq!(on.report, off.report, "{codec}");
    }
}

#[test]
fn q2_configured_codec_cache_preserves_each_objectives_search_and_bytes() {
    let directory = std::env::temp_dir().join(format!("lilscript-q2-cache-search-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    for codec in ["raw", "gzip", "brotli"] {
        let policy = policy(codec, 13);
        let objective = Objectives::One(policy.objective().unwrap().codec);
        let enabled = policy.clone().with_cache(crate::config::CacheConfig {
            build_reuse: true, normalization_reuse: true, codec_reuse: true, directory: Some(directory.clone()),
        });
        let disabled = policy.with_cache(crate::config::CacheConfig {
            build_reuse: true, normalization_reuse: true, codec_reuse: false, directory: Some(directory.clone()),
        });
        let cold = search(&enabled, objective, true);
        let warm = search(&enabled, objective, true);
        let off = search(&disabled, objective, true);
        assert_eq!(cold.logical_work, warm.logical_work, "{codec}");
        assert_eq!(cold.logical_work, off.logical_work, "{codec}");
        assert_eq!(cold.scored, warm.scored, "{codec}");
        assert_eq!(cold.scored, off.scored, "{codec}");
        assert_eq!(cold.winners, warm.winners, "{codec}");
        assert_eq!(cold.winners, off.winners, "{codec}");
        assert_eq!(cold.report, warm.report, "{codec}");
        assert_eq!(cold.report, off.report, "{codec}");
        assert_eq!(off.cache_hits, 0);
        if codec == "raw" { assert!(!directory.exists()); }
        else { assert!(warm.cache_hits > 0); }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn local_read_order_polish_protects_the_completed_search_under_each_objective() {
    let geometry = include_str!("../../comparison/algorithms/cases/collection-geometry/main.lil");
    for (codec, index) in [("raw", 0), ("gzip", 1), ("brotli", 2)] {
        // Isolate the original final refinement; deferred combinations have
        // their own execution and TOML-isolation checks.
        let resolved = policy_with_search(codec, 15, "deferred_naming_polish=false");
        let objective = Objectives::One(resolved.objective().unwrap().codec);
        for source in [PROGRAM, geometry] {
            let original = without_local_polish(|| search_source(source, &resolved, objective, true));
            let polished = search_source(source, &resolved, objective, true);
            assert!(polished.winners[index].as_ref().unwrap().0 <= original.winners[index].as_ref().unwrap().0,
                "local naming must retain the completed {codec} winner");
            let stage = &polished.report.objectives[0];
            assert_eq!(replay(stage), stage.after);
            let trials = stage.joint_trials.iter().filter(|trial| trial.name == "naming:local-read-order").collect::<Vec<_>>();
            assert_eq!(trials.len(), 1);
            // Compare the completed prefix before later joint, deferred
            // and compact-allocation stages in the counterfactual run.
            let original_stage = &original.report.objectives[0];
            let prefix_passes = original_stage.starts.iter()
                .find(|start| start.name.starts_with("deferred-naming") || start.name == "compact-allocation" || start.name == "representation-joints")
                .map_or(original_stage.passes, |start| start.pass - 1);
            assert!(trials[0].pass > prefix_passes);
            if source == geometry {
                let javascript = &polished.winners[index].as_ref().unwrap().1;
                let output = std::process::Command::new("node").args(["-e", &format!(
                    "globalThis.algorithmCount=()=>4;globalThis.algorithmInt=i=>i+1;{javascript}"
                )]).output().unwrap();
                assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
                assert_eq!(String::from_utf8(output.stdout).unwrap(), "3\n");
            }
        }
    }
}

#[test]
fn private_properties_polish_protects_the_completed_search_and_replays() {
    let source = include_str!("fixtures/private-fields.lil");
    for (codec, index) in [("raw", 0), ("gzip", 1), ("brotli", 2)] {
        // Isolate the original final refinement; deferred combinations have
        // their own execution and TOML-isolation checks.
        let resolved = policy_with_search(codec, 15, "deferred_naming_polish=false");
        let objective = Objectives::One(resolved.objective().unwrap().codec);
        let previous = without_property_polish(|| search_source(source, &resolved, objective, true));
        let renamed = search_source(source, &resolved, objective, true);
        assert!(renamed.winners[index].as_ref().unwrap().0 <= previous.winners[index].as_ref().unwrap().0);
        let stage = &renamed.report.objectives[0];
        assert_eq!(replay(stage), stage.after);
        assert_eq!(stage.joint_trials.iter().filter(|trial| trial.name == "properties:private-fields").count(), 1);
    }
}

#[test]
fn terminal_family_vetoes_hold_under_each_objective() {
    let source = r#"export string[] make() { return ["aa","bb","cc","dd","ee","ff","gg","hh"]; }"#;
    for codec in ["raw", "gzip", "brotli"] {
        let config: crate::config::ProjectConfig = toml::from_str(&format!(
            "objective.codecs='{codec}'\neffort.level=15\n[javascript]\nassume_pristine_builtins=true\n[policy.tactics]\nstring-pooling='off'\nstring-array-packing='off'\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'"
        )).unwrap();
        let resolved = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let objective = resolved.objective().unwrap().codec;
        let run = search_source(source, &resolved, Objectives::One(objective), true);
        let stage = &run.report.objectives[0];
        assert!(
            stage
                .trials
                .iter()
                .any(|trial| trial.challenger == "pooling-and-packing"
                    && trial.outcome == ChallengerOutcome::Vetoed),
            "{codec}: {stage:?}"
        );
        for (_, javascript) in run.winners.into_iter().flatten() {
            assert!(!javascript.contains(".split("), "{codec}: {javascript}");
        }
    }
}

#[test]
fn incumbents_never_worsen_within_a_search_or_its_terminal_stage() {
    let policy = policy("brotli", 15);
    for challenge in [false, true] {
        let run = search(&policy, Objectives::All, challenge);
        for (index, codec) in ["raw", "gzip", "brotli"].into_iter().enumerate() {
            let best = run
                .scored
                .iter()
                .filter_map(|sizes| sizes[index])
                .min()
                .unwrap();
            let (winner, javascript) = run.winners[index].clone().unwrap();
            assert_eq!(
                winner,
                crate::compression::measure(
                    javascript.as_bytes(),
                    [Objective::Raw, Objective::Gzip, Objective::Brotli][index]
                )
                .unwrap()
            );
            if !challenge {
                // Every replacement was strictly smaller: the winner is the
                // minimum of everything the search scored for this codec.
                assert_eq!(winner, best, "{codec}");
                continue;
            }
            let stage = run
                .report
                .objectives
                .iter()
                .find(|stage| stage.codec == codec)
                .unwrap();
            assert_eq!(
                stage.searched, best,
                "{codec}: the walks settle against the search's winner"
            );
            assert_eq!(stage.after, winner);
            assert!(stage.after <= stage.searched);
            assert_eq!(replay(stage), stage.after);
        }
    }
}

/// The walks' result, replayed from the report: each start's walk from its
/// start size, through its kept moves in pass order; a start's result
/// replaces the objective's winner (first the search's) only when it
/// reports a strict win.
fn replay(stage: &TerminalObjective) -> usize {
    let kept = |pass: usize| -> Vec<(i64, usize)> {
        let choices = stage
            .choice_trials
            .iter()
            .filter(|trial| trial.pass == pass && trial.outcome == ChallengerOutcome::Kept)
            .map(|trial| (trial.delta.unwrap(), trial.size.unwrap()));
        let challengers = stage
            .trials
            .iter()
            .filter(|trial| trial.pass == pass && trial.outcome == ChallengerOutcome::Kept)
            .map(|trial| (trial.delta.unwrap(), trial.size.unwrap()));
        let joints = stage
            .joint_trials
            .iter()
            .filter(|trial| trial.pass == pass && trial.outcome == ChallengerOutcome::Kept)
            .map(|trial| (trial.delta.unwrap(), trial.size.unwrap()));
        choices.chain(challengers).chain(joints).collect()
    };
    let walk = |passes: std::ops::Range<usize>, start: usize| {
        let mut incumbent = start;
        for pass in passes {
            for (delta, size) in kept(pass) {
                // Exact codec ties may reduce raw delivered bytes.
                assert!(delta <= 0);
                assert_eq!(size as i64, incumbent as i64 + delta);
                incumbent = size;
            }
        }
        incumbent
    };
    let firsts: Vec<usize> = stage
        .starts
        .iter()
        .map(|start| start.pass)
        .chain(std::iter::once(stage.passes + 1))
        .collect();
    let mut best = stage.searched;
    for (index, start) in stage.starts.iter().enumerate() {
        let Some(from) = start.start else {
            assert_eq!(
                firsts[index],
                firsts[index + 1],
                "an unformed restart walks no pass"
            );
            continue;
        };
        let end = walk(firsts[index]..firsts[index + 1], from);
        assert_eq!(start.size, Some(end), "{start:?}");
        assert_eq!(start.delta, Some(end as i64 - best as i64), "{start:?}");
        if start.outcome == ChallengerOutcome::Kept {
            assert!(end <= best);
            best = end;
        }
    }
    best
}

#[test]
fn every_effort_level_starts_from_the_same_baseline_and_ends_at_most_there() {
    let off = search(
        &policy("brotli", 0),
        Objectives::One(Objective::Brotli),
        true,
    );
    let floor = off.winners[2].clone().unwrap();
    assert_eq!(floor.1, off.baseline, "search off delivers its baseline");
    for level in [3, 8, 11, 13, 15, 16] {
        let run = search(
            &policy("brotli", level),
            Objectives::One(Objective::Brotli),
            true,
        );
        assert_eq!(run.baseline, off.baseline, "level {level}");
        assert!(
            run.winners[2].as_ref().unwrap().0 <= floor.0,
            "level {level}"
        );
    }
}

/// The stage forms each challenger from one formed head of the winner's
/// candidate: formed under the winner's own assignment, that path renders
/// the winner's exact bytes, so a challenger differs only by its family.
#[test]
fn a_terminal_formation_of_the_winners_own_assignment_is_the_winner() {
    for (codec, objective) in [("brotli", Objective::Brotli), ("raw", Objective::Raw)] {
        let policy = policy(codec, 15);
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, PROGRAM).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let ledger = BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: 200_000_000,
                retained_bytes: 256_000_000,
                terminal_work: 0,
            },
        )
        .unwrap();
        let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 128 }).unwrap();
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        {
            let mut search = compilation
                .search_javascript(source, &policy, request(Objectives::One(objective)))
                .unwrap();
            let entry = search
                .portfolio
                .entries
                .get(search.portfolio.selected[index(objective)].unwrap())
                .unwrap();
            let candidate = search.states[entry.state].as_ref().unwrap().candidate;
            let artifact = entry.artifact;
            let provenance = search.compilation.artifacts.provenance(artifact).unwrap();
            let (plan, output) = (
                provenance.naming().clone(),
                provenance.description().output().clone(),
            );
            let winner = search
                .compilation
                .with_artifact(artifact, |view| view.javascript.to_string())
                .unwrap();
            let formed = search
                .compilation
                .with_javascript_formations_in(
                    candidate,
                    &policy,
                    output.dead_code_elimination,
                    output.target_compaction,
                    output.rules,
                    output.families.head(),
                    WorkDomain::Optional,
                    |formations| {
                        formations.form(output.clone(), |target| {
                            let staged = target.render_bounded_with_literals(
                                &plan,
                                output.literals,
                                usize::MAX,
                            )?;
                            target.take_artifact(staged)
                        })
                    },
                )
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(formed, winner, "{codec}");
        }
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
}

#[test]
fn the_choice_schedule_resets_every_site_at_once_then_orders_by_stake() {
    use crate::js::choices::ChoiceAlternative;
    use crate::js::{AltId, ChoiceFamily, ChoiceKey, ChoiceSite, SiteId};
    let site = |site: u32, savings: &[(u8, i64)], applied: u8| ChoiceSite {
            pinned: false,
        key: ChoiceKey {
            family: ChoiceFamily::DataEncoding,
            site: SiteId::Symbol(site),
        },
        name: format!("t{site}"),
        alternatives: savings
            .iter()
            .map(|&(alternative, saving)| ChoiceAlternative {
                alternative: AltId(alternative),
                name: "",
                saving,
            })
            .collect(),
        seed: AltId(applied),
        applied: AltId(applied),
    };
    let small = site(4, &[(0, 0), (1, 120), (2, 90)], 1);
    let large = site(9, &[(0, 0), (2, 11_000), (3, 19_000)], 3);
    let plain = site(2, &[(0, 0), (2, -40)], 0);
    let sites = [small, large, plain];
    let schedule = crate::representation::schedule(&sites, false);
    // Both encoded sites back to their literal at once, largest stake first.
    assert_eq!(schedule[0], [(1, AltId(0)), (0, AltId(0))]);
    // Then the large site's other alternatives, best estimate first, the
    // literal (its undo) last; then the small one's. A negative raw estimate
    // still competes: it may improve the selected compressed objective.
    assert_eq!(
        schedule[1..],
        [
            vec![(1, AltId(2))],
            vec![(1, AltId(0))],
            vec![(0, AltId(2))],
            vec![(0, AltId(0))],
            vec![(2, AltId(2))],
        ]
    );
    // One encoded site needs no joint move: its literal is its own trial.
    assert_eq!(crate::representation::schedule(&sites[..1], false)[0], [(0, AltId(2))]);
}

/// Dominance (design §10, §14.2): a challenger that shrinks the sum of rows
/// but grows one entry's row is refused; one row is the old scalar rule.
#[test]
fn dominance_refuses_a_smaller_sum_that_grows_one_row() {
    // One entry: strictly smaller wins, as before M3.3.
    assert!(dominates(&[90], &[100]));
    assert!(!dominates(&[110], &[100]));
    // Two entries: the sum shrinks (190 < 200), but entry 1 grows.
    assert!(!dominates(&[60, 130], &[100, 100]));
    // No row grows: kept when the sum is smaller as well.
    assert!(dominates(&[100, 90], &[100, 100]));
    // Plans of other entries never compare.
    assert!(!dominates(&[50], &[100, 100]));
}

#[test]
fn g1_compact_allocation_preserves_the_completed_search_and_delivers_its_score() {
    for (name, codec) in [("raw", Objective::Raw), ("gzip", Objective::Gzip), ("brotli", Objective::Brotli)] {
        let policy = policy_with_search(name, 13, "[policy.tactics]\nnaming-compaction='on'");
        let old = without_compact_allocation(|| search(&policy, Objectives::One(codec), true));
        let new = search(&policy, Objectives::One(codec), true);
        let before = &old.report.objectives[0];
        let after = &new.report.objectives[0];
        let a = serde_json::to_value(before).unwrap();
        let b = serde_json::to_value(after).unwrap();
        for field in ["starts", "trials", "choice_trials", "joint_trials", "stops"] {
            let prefix = a[field].as_array().unwrap();
            assert_eq!(&b[field].as_array().unwrap()[..prefix.len()], prefix, "{name}/{field}");
        }
        assert!(after.starts.iter().any(|s| s.name == "compact-allocation"));
        assert!(after.after <= before.after);
        assert_eq!(replay(after), after.after);
        let (size, javascript) = new.winners[index(codec)].as_ref().unwrap();
        assert_eq!(*size, after.after);
        let model = match codec { Objective::Raw => crate::config::CompressionCostModel::Raw,
            Objective::Gzip => crate::config::CompressionCostModel::Gzip,
            Objective::Brotli => crate::config::CompressionCostModel::Brotli };
        assert_eq!(crate::compression::measure(javascript.as_bytes(), model).unwrap(), *size);
    }
}
