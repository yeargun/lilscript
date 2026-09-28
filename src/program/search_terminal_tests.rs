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
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Objectives};
use crate::program::facts::CacheLimits;

const PROGRAM: &str = include_str!("../../tests/cases/objective_judged_spellings.lil");

fn policy(codec: &str, level: u8) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\ncost_model='{codec}'\noptimization_level={level}"
    ))
    .unwrap();
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
}

fn search(policy: &ResolvedPolicy, objectives: Objectives, challenge: bool) -> Run {
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
            search.challenge(policy, request(objectives)).unwrap().clone()
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
    assert_eq!(compilation.finish().retained_bytes(), 0);
    Run {
        scored,
        baseline,
        winners: run.0,
        report: run.1,
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
                assert!(delta < 0);
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
            assert_eq!(firsts[index], firsts[index + 1], "an unformed restart walks no pass");
            continue;
        };
        let end = walk(firsts[index]..firsts[index + 1], from);
        assert_eq!(start.size, Some(end), "{start:?}");
        assert_eq!(start.delta, Some(end as i64 - best as i64), "{start:?}");
        if start.outcome == ChallengerOutcome::Kept {
            assert!(end < best);
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
    use crate::js::{AltId, ChoiceFamily, ChoiceKey, ChoiceSite};
    let site = |site: u32, savings: &[(u8, i64)], applied: u8| ChoiceSite {
        key: ChoiceKey {
            family: ChoiceFamily::DataEncoding,
            site,
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
    let schedule = choice_schedule(&sites);
    // Both encoded sites back to their literal at once, largest stake first.
    assert_eq!(schedule[0], [(1, AltId(0)), (0, AltId(0))]);
    // Then the large site's other alternatives, best estimate first, the
    // literal (its undo) last; then the small one's; the plain site offers
    // nothing that saves.
    assert_eq!(
        schedule[1..],
        [
            vec![(1, AltId(2))],
            vec![(1, AltId(0))],
            vec![(0, AltId(2))],
            vec![(0, AltId(0))],
        ]
    );
    // One encoded site needs no joint move: its literal is its own trial.
    assert_eq!(choice_schedule(&sites[..1])[0], [(0, AltId(2))]);
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
