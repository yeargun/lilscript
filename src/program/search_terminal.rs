//! The terminal challenger stage (plan M5.4; architecture §9 and law L7).
//!
//! After the search has chosen each requested objective's winner, the stage
//! offers that final candidate a declared, ordered list of challengers
//! (`js::Challenger`): each a named alternative assignment of the output
//! families and the naming plan's raw spelling (M9.2, M9.3). A challenger is
//! formed from the same candidate, rendered with the incumbent's naming style,
//! source names and literal mode, admitted by the same admission as every
//! searched artifact (the target verifier at preparation, then
//! `ArtifactArena::qualify` under the policy), and scored by the objective's
//! exact codec. It is kept only when the complete artifact is strictly
//! smaller, and then it is the incumbent the next challenger is applied to.
//!
//! Why here and not inside the search: a rewrite that runs on every emission
//! moves which plan the search ranks first, and can lose on the fleet while
//! winning on its own (the terminal challenger law; migration 7.34–7.37 read
//! +128 over 20 ports that way). Applied to the final artifact and kept only
//! on an exact codec win, a challenger cannot regress it: nothing downstream
//! reads it. The old route's `emit_terminal_javascript_challengers` and
//! `finalize_javascript_candidates_with_terminal_objective_challengers`
//! (`d362338f:src/compiler.rs:5308-5480`) re-emitted the finalist's text with
//! other emitter options; this stage varies formation choices of the
//! candidate the search selected, on the tree.
//!
//! Monotone by construction (L7):
//! * within a search, an incumbent is replaced only by a strictly smaller
//!   admitted artifact (`Portfolio::score`, and here);
//! * the stage walks one fixed schedule until the codec has judged as many
//!   challengers as the policy's effort budget allows, so from the same final
//!   candidate a higher effort can only keep more;
//! * everything here is sequential and deterministic: no thread count, time
//!   or allocation address enters a decision.
use super::*;
use crate::js::{Challenger, ChoiceMap, ChoiceSite, Spelling};

/// What became of one declared challenger on one objective's final candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChallengerOutcome {
    /// The complete artifact shrank under the objective's codec: kept.
    Kept,
    /// Scored, and not smaller: discarded.
    Rejected,
    /// It rendered the incumbent's exact bytes: nothing to score.
    Identical,
    /// Its assignment prints the same program as the incumbent's or an
    /// earlier challenger's: not formed.
    Duplicate,
    /// A permission vetoes it (the output families need target compaction).
    Vetoed,
    /// Formation, rendering or admission refused it: discarded.
    Refused,
    /// The effort's challenger budget was spent before it.
    Budget,
    /// The compilation's resources ran out before it; the stage ended.
    Stopped,
}

/// One declared challenger's trial, in schedule order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChallengerTrial {
    pub challenger: &'static str,
    pub outcome: ChallengerOutcome,
    /// Its complete artifact's size under the objective's codec, when scored.
    pub size: Option<usize>,
    /// That size minus the incumbent's at the time (negative is smaller).
    pub delta: Option<i64>,
}

/// One choice alternative's trial (M9.1): a site of a choice family, by its
/// source name, taking one of its other alternatives.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChoiceTrial {
    pub family: crate::js::ChoiceFamily,
    pub site: String,
    pub alternative: &'static str,
    /// The estimator's raw bytes saved against the site's canonical form:
    /// the order of the schedule, never the verdict.
    pub estimate: i64,
    pub outcome: ChallengerOutcome,
    pub size: Option<usize>,
    pub delta: Option<i64>,
}

/// A choice site of the delivered artifact and the alternative it took.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChoiceOutcome {
    pub family: crate::js::ChoiceFamily,
    pub site: String,
    pub seed: &'static str,
    pub delivered: &'static str,
    /// Every alternative the site offers and its estimated saving.
    pub offered: Vec<(&'static str, i64)>,
}

/// The stage on one objective: its budget, what it tried and what it kept.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TerminalObjective {
    pub codec: &'static str,
    /// Challengers the effort allows the codec to judge
    /// (`OptimizationObjective::terminal_challengers`).
    pub budget: usize,
    /// Challengers formed: every scored one, and those that rendered the
    /// incumbent's bytes or were refused.
    pub tried: usize,
    /// Challengers the codec judged (kept or rejected), at most `budget`.
    pub scored: usize,
    /// Exact codec scores the stage spent, on challengers and choices.
    pub codec_probes: usize,
    /// The search winner's size under the codec, and the delivered one's.
    pub before: usize,
    pub after: usize,
    /// The delivered artifact's families and raw spelling, by name.
    pub spelling: Vec<&'static str>,
    /// Every declared challenger, in schedule order.
    pub trials: Vec<ChallengerTrial>,
    /// Choice alternatives the effort allows the codec to judge
    /// (`OptimizationObjective::terminal_choices`), counted apart from the
    /// challengers.
    pub choice_budget: usize,
    /// Choice alternatives formed: every judged one, and those that
    /// rendered the incumbent's bytes or were refused.
    pub choices_tried: usize,
    /// Choice alternatives the codec judged, at most `choice_budget`.
    pub choices_scored: usize,
    /// Formations made only to read the incumbent's choice sites (0 or 1).
    pub surveys: usize,
    /// Every choice alternative offered, in schedule order: sites by the
    /// largest estimated saving, then each site's alternatives by theirs.
    pub choice_trials: Vec<ChoiceTrial>,
    /// Every choice site of the delivered artifact.
    pub choices: Vec<ChoiceOutcome>,
}

/// Bounded by the declared schedule: at most `Challenger::ORDER.len()`
/// challenger trials per requested objective, and one choice trial per
/// alternative of each site the delivered candidate offers.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct TerminalReport {
    pub objectives: Vec<TerminalObjective>,
}

fn codec_name(codec: Objective) -> &'static str {
    match codec {
        Objective::Raw => "raw",
        Objective::Gzip => "gzip",
        Objective::Brotli => "brotli",
    }
}

/// The names of what an assignment turns on, in schedule order.
fn spelling_names(spelling: Spelling) -> Vec<&'static str> {
    let families = spelling.families;
    let statements = families.statements;
    [
        (statements.conditional_values, Challenger::ConditionalValues),
        (statements.exit_points, Challenger::ExitPoints),
        (statements.loop_fusion, Challenger::LoopFusion),
        (spelling.raw_spelling, Challenger::RawSpelling),
        (families.loop_heads, Challenger::LoopHeads),
        (families.logical_statements, Challenger::LogicalStatements),
        (families.block_inlining, Challenger::BlockInlining),
        (families.flat_blocks, Challenger::FlatBlocks),
        (families.string_pooling, Challenger::StringPooling),
        (
            statements.conditional_returns,
            Challenger::ConditionalReturns,
        ),
        (statements.logical_branches, Challenger::LogicalBranches),
    ]
    .into_iter()
    .filter_map(|(on, challenger)| on.then_some(challenger.name()))
    .collect()
}

/// A resource refusal ends the stage; any other refusal ends one challenger.
fn exhausted(error: &CandidateError) -> bool {
    matches!(error, CandidateError::Budget(_))
}

/// A resource refusal from the portfolio's own admission.
fn resource(error: &SearchError) -> bool {
    matches!(error, SearchError::Candidate(error) if exhausted(error))
        || matches!(
            error,
            SearchError::Limit(SearchLimit::ArtifactCount | SearchLimit::ArtifactBytes)
        )
}

struct Incumbent {
    artifact: ArtifactId,
    spelling: Spelling,
    choices: ChoiceMap,
    size: usize,
}

/// One trial's verdict against the incumbent.
enum Judgement {
    Kept { artifact: ArtifactId, size: usize },
    Rejected { size: usize },
    Identical,
    Refused,
    Stopped,
}

/// Dominance (design §10, L7): a challenger may replace the incumbent only
/// when no entry's row grows, so a smaller sum never buys one entry's bytes
/// with another's. Rows compare entry by entry; plans of other entries do
/// not compare.
pub(super) fn dominates(rows: &[u64], held: &[u64]) -> bool {
    rows.len() == held.len() && rows.iter().zip(held).all(|(row, held)| row <= held)
}

/// What the stage holds fixed while it judges one objective's challengers.
struct Judge<'a> {
    policy: &'a ResolvedPolicy,
    codec: Objective,
    output: &'a OutputTactics,
    plan: &'a Plan,
    available: usize,
}

impl Judge<'_> {
    /// Form the candidate under `families` and `choices`, render it with the
    /// incumbent's naming and `raw_spelling`, admit it and score it under the
    /// objective's codec; it replaces the incumbent only when the complete
    /// artifact is strictly smaller and no entry's row grows. Returns the
    /// verdict and whether an exact codec score was spent.
    fn judge(
        &self,
        formations: &mut Formations<'_, '_>,
        portfolio: &mut Portfolio,
        spelling: Spelling,
        choices: &ChoiceMap,
        incumbent: &Incumbent,
    ) -> Result<(Judgement, bool), SearchError> {
        let Self {
            policy,
            codec,
            output,
            plan,
            available,
        } = *self;
        let tactics = OutputTactics {
            families: spelling.families,
            choices: choices.clone(),
            ..output.clone()
        };
        let named = Plan {
            style: plan.style,
            source_names: plan.source_names.clone(),
            raw_spelling: spelling.raw_spelling,
        };
        let rendered = formations
            .form(tactics, |target| {
                let staged =
                    target.render_bounded_with_literals(&named, output.literals, available)?;
                target.retain_artifact(staged)
            })
            .and_then(|retained| retained);
        let challenged = match rendered {
            Ok(challenged) => challenged,
            Err(error) if exhausted(&error) => return Ok((Judgement::Stopped, false)),
            Err(_) => return Ok((Judgement::Refused, false)),
        };
        let baseline = portfolio.baseline_qualification(codec).copied();
        let scored = formations.with_arena(|arena, contract, budget| {
            let result =
                (|| -> Result<Option<(usize, QualifiedArtifact, bool)>, CandidateError> {
                    if arena.same_output(challenged, incumbent.artifact, budget)? {
                        return Ok(None);
                    }
                    let size = arena.measure(challenged, codec, budget)?;
                    let qualified = arena.qualify(
                        challenged,
                        contract,
                        policy,
                        codec,
                        ArtifactRuntimeEvidence::default(),
                        baseline.as_ref(),
                        budget,
                    )?;
                    // Dominance (design §10): no entry's row may grow, so a
                    // smaller sum never trades one entry's bytes for
                    // another's. One file is one row: strictly smaller.
                    let rows = arena.rows(challenged, codec, budget)?;
                    let held = arena.rows(incumbent.artifact, codec, budget)?;
                    Ok(Some((size, qualified, dominates(&rows, &held))))
                })();
            if !matches!(result, Ok(Some(_))) {
                arena
                    .discard(challenged, budget)
                    .expect("the stage owns its challenger");
            }
            result
        });
        let probed = codec != Objective::Raw && !matches!(scored, Ok(None));
        let (size, qualified, dominates) = match scored {
            Ok(Some(scored)) => scored,
            Ok(None) => return Ok((Judgement::Identical, probed)),
            Err(error) if exhausted(&error) => return Ok((Judgement::Stopped, probed)),
            Err(_) => return Ok((Judgement::Refused, probed)),
        };
        let base = baseline.map_or(qualified.cost(), |baseline| baseline.cost());
        let smaller = policy
            .compare_evidence(
                qualified.cost(),
                CandidateCostEvidence::size_only(incumbent.size as u64),
                base,
            )
            .map_err(SearchError::from)
            .and_then(|order| Ok(order.ok_or(CandidateError::NotJavaScript)?))
            .map(|order| order == Ordering::Less && dominates);
        // Promotion admits a portfolio entry before it changes anything; a
        // refusal leaves the incumbent in place. The challenger goes unless
        // it became the incumbent.
        let kept = formations.with_arena(|arena, _, budget| {
            let kept = smaller.and_then(|smaller| {
                if !smaller {
                    return Ok(false);
                }
                portfolio
                    .promote_terminal(arena, budget, codec, challenged, qualified)
                    .map(|_| true)
            });
            if !matches!(kept, Ok(true)) {
                arena
                    .discard(challenged, budget)
                    .expect("the stage owns its challenger");
            }
            kept
        });
        match kept {
            Ok(true) => Ok((
                Judgement::Kept {
                    artifact: challenged,
                    size,
                },
                probed,
            )),
            Ok(false) => Ok((Judgement::Rejected { size }, probed)),
            Err(error) if error.optional_memory_refusal() || resource(&error) => {
                Ok((Judgement::Stopped, probed))
            }
            Err(error) => Err(error),
        }
    }
}

/// The choice schedule of one surveyed artifact, as moves (each a set of
/// site assignments judged together). First, when two or more sites apply a
/// non-canonical alternative, every site back to its canonical form at once:
/// encodings share decoder text a codec matches across tables, so one site
/// at a time cannot leave a state where several pay together (the families'
/// `other-objective-seed` challenger is the same whole move). Then every
/// site, largest stake first, and each site's alternatives other than the one
/// it applies, best estimate first. An alternative the estimator says saves
/// nothing is offered only when it is the canonical form (the undo of a
/// seed).
fn choice_schedule(sites: &[ChoiceSite]) -> Vec<Vec<(usize, crate::js::AltId)>> {
    let canonical = crate::js::AltId(0);
    let mut order: Vec<usize> = (0..sites.len()).collect();
    order.sort_by(|&a, &b| {
        sites[b]
            .stake()
            .cmp(&sites[a].stake())
            .then(sites[a].key.cmp(&sites[b].key))
    });
    let mut schedule = Vec::new();
    let encoded: Vec<(usize, crate::js::AltId)> = order
        .iter()
        .copied()
        .filter(|&site| sites[site].applied != canonical)
        .map(|site| (site, canonical))
        .collect();
    if encoded.len() > 1 {
        schedule.push(encoded);
    }
    for site in order {
        let mut alternatives: Vec<_> = sites[site]
            .alternatives
            .iter()
            .filter(|offered| {
                offered.alternative != sites[site].applied
                    && (offered.saving > 0 || offered.alternative == canonical)
            })
            .collect();
        alternatives.sort_by(|a, b| {
            b.saving
                .cmp(&a.saving)
                .then(a.alternative.cmp(&b.alternative))
        });
        schedule.extend(
            alternatives
                .into_iter()
                .map(|offered| vec![(site, offered.alternative)]),
        );
    }
    schedule
}

impl JavaScriptSearch<'_, '_> {
    /// Run the terminal challenger stage on every requested objective's
    /// winner, once; later calls return the same report. The winners it
    /// replaces are the ones `winner_qualification` and `take_winner`
    /// deliver. A resource refusal keeps the incumbent and is reported, never
    /// an error: the search's admitted winners stay valid.
    pub fn challenge(
        &mut self,
        policy: &ResolvedPolicy,
        objectives: Objectives,
    ) -> Result<&TerminalReport, SearchError> {
        if self.terminal.is_none() {
            let objective = policy.objective().ok_or(CandidateError::NotJavaScript)?;
            let mut report = TerminalReport::default();
            for codec in objectives.iter() {
                if let Some(trials) = self.challenge_objective(policy, objective, codec)? {
                    report.objectives.push(trials);
                }
            }
            self.terminal = Some(report);
        }
        Ok(self.terminal.as_ref().unwrap())
    }

    /// The stage's report, once `challenge` has run.
    pub fn terminal_report(&self) -> Option<&TerminalReport> {
        self.terminal.as_ref()
    }

    fn challenge_objective(
        &mut self,
        policy: &ResolvedPolicy,
        objective: OptimizationObjective,
        codec: Objective,
    ) -> Result<Option<TerminalObjective>, SearchError> {
        let i = index(codec);
        let Some(position) = self.portfolio.selected[i] else {
            return Ok(None);
        };
        let entry = self.portfolio.entries.get(position).unwrap();
        let (artifact, state) = (entry.artifact, entry.state);
        let candidate = self.states[state]
            .as_ref()
            .expect("a selected entry pins its source state")
            .candidate;
        let arena = &self.compilation.artifacts;
        let provenance = arena.provenance(artifact)?;
        let plan = provenance.naming().clone();
        let output = provenance.description().output().clone();
        let before = arena
            .with_artifact(artifact, |view| view.sizes.get(codec))?
            .expect("a selected winner is measured under its codec");
        let seed = Spelling {
            families: output.families,
            raw_spelling: plan.raw_spelling,
        };
        let budget = objective.terminal_challengers;
        let choice_budget = if output.target_compaction {
            objective.terminal_choices
        } else {
            0
        };
        let available = objective
            .retained_candidate_bytes
            .max(self.portfolio.baseline_capacity);
        let mut report = TerminalObjective {
            codec: codec_name(codec),
            budget,
            tried: 0,
            scored: 0,
            codec_probes: 0,
            before,
            after: before,
            spelling: spelling_names(seed),
            trials: Vec::with_capacity(Challenger::ORDER.len()),
            choice_budget,
            choices_tried: 0,
            choices_scored: 0,
            surveys: 0,
            choice_trials: Vec::new(),
            choices: Vec::new(),
        };
        let trial = |challenger: Challenger, outcome| ChallengerTrial {
            challenger: challenger.name(),
            outcome,
            size: None,
            delta: None,
        };
        if budget == 0 && choice_budget == 0 {
            report.trials.extend(
                Challenger::ORDER
                    .into_iter()
                    .map(|challenger| trial(challenger, ChallengerOutcome::Budget)),
            );
            return Ok(Some(report));
        }
        // When no challenger passes the duplicate and permission filters
        // from the seed and no choice may be judged, nothing is ever formed
        // (the incumbent can only change through a formed one): record that
        // without building the candidate's demand and head.
        let mut seen = vec![seed.effective()];
        let mut dry = Vec::with_capacity(Challenger::ORDER.len());
        let mut formable = false;
        for challenger in Challenger::ORDER {
            let next = challenger.apply(codec, seed);
            let outcome = if budget == 0 {
                ChallengerOutcome::Budget
            } else if seen.contains(&next.effective()) {
                ChallengerOutcome::Duplicate
            } else if (OutputTactics {
                families: next.families,
                ..output.clone()
            })
            .check_policy(policy)
            .is_err()
            {
                seen.push(next.effective());
                ChallengerOutcome::Vetoed
            } else {
                formable = true;
                break;
            };
            dry.push(trial(challenger, outcome));
        }
        if !formable && choice_budget == 0 {
            report.trials = dry;
            return Ok(Some(report));
        }
        let Self {
            compilation,
            portfolio,
            ..
        } = self;
        let mut incumbent = Incumbent {
            artifact,
            spelling: seed,
            choices: output.choices.clone(),
            size: before,
        };
        let judge = Judge {
            policy,
            codec,
            output: &output,
            plan: &plan,
            available,
        };
        let mut stopped = false;
        let formed = compilation.with_javascript_formations_in(
            candidate,
            policy,
            output.dead_code_elimination,
            output.target_compaction,
            WorkDomain::Optional,
            |formations| -> Result<(), SearchError> {
                'choices: {
                    if choice_budget == 0 {
                        break 'choices;
                    }
                    // The choice phase (M9.1), first: the sites the search
                    // winner's tree offers, each other alternative judged on the
                    // whole artifact within the effort's choice budget. Data
                    // layout is the larger stake; the spellings are then judged
                    // on the layout the artifact keeps.
                    report.surveys += 1;
                    let surveyed = formations.survey(&OutputTactics {
                        families: incumbent.spelling.families,
                        choices: incumbent.choices.clone(),
                        ..output.clone()
                    });
                    let mut sites = match surveyed {
                        Ok(sites) => sites,
                        Err(error) if exhausted(&error) => {
                            stopped = true;
                            break 'choices;
                        }
                        Err(_) => break 'choices,
                    };
                    for moves in choice_schedule(&sites) {
                        let (site, alternative) = moves[0];
                        let estimate = moves
                            .iter()
                            .map(|&(site, alternative)| {
                                sites[site]
                                    .alternatives
                                    .iter()
                                    .find(|offered| offered.alternative == alternative)
                                    .map_or(0, |offered| offered.saving)
                            })
                            .sum();
                        let joint = moves.len() > 1;
                        let mut record = ChoiceTrial {
                            family: sites[site].key.family,
                            site: if joint {
                                format!("{} sites", moves.len())
                            } else {
                                sites[site].name.clone()
                            },
                            alternative: if joint {
                                "canonical"
                            } else {
                                sites[site].name_of(alternative)
                            },
                            estimate,
                            outcome: ChallengerOutcome::Budget,
                            size: None,
                            delta: None,
                        };
                        let moved: Vec<(usize, crate::js::AltId)> = moves
                            .iter()
                            .copied()
                            .filter(|&(site, alternative)| sites[site].applied != alternative)
                            .collect();
                        if stopped {
                            record.outcome = ChallengerOutcome::Stopped;
                        } else if moved.is_empty() {
                            // An earlier trial kept this assignment already.
                            record.outcome = ChallengerOutcome::Duplicate;
                        } else if report.choices_scored < choice_budget {
                            report.choices_tried += 1;
                            let choices = moved.iter().fold(
                                incumbent.choices.clone(),
                                |choices, &(site, alternative)| {
                                    choices.with(sites[site].key, alternative)
                                },
                            );
                            let (judgement, probed) = judge.judge(
                                formations,
                                portfolio,
                                incumbent.spelling,
                                &choices,
                                &incumbent,
                            )?;
                            report.codec_probes += usize::from(probed);
                            match judgement {
                                Judgement::Kept { artifact, size } => {
                                    report.choices_scored += 1;
                                    record.outcome = ChallengerOutcome::Kept;
                                    record.size = Some(size);
                                    record.delta = Some(size as i64 - incumbent.size as i64);
                                    incumbent = Incumbent {
                                        artifact,
                                        spelling: incumbent.spelling,
                                        choices,
                                        size,
                                    };
                                    for &(site, alternative) in &moved {
                                        sites[site].applied = alternative;
                                    }
                                }
                                Judgement::Rejected { size } => {
                                    report.choices_scored += 1;
                                    record.outcome = ChallengerOutcome::Rejected;
                                    record.size = Some(size);
                                    record.delta = Some(size as i64 - incumbent.size as i64);
                                }
                                Judgement::Identical => {
                                    record.outcome = ChallengerOutcome::Identical
                                }
                                Judgement::Refused => record.outcome = ChallengerOutcome::Refused,
                                Judgement::Stopped => {
                                    stopped = true;
                                    report.choices_tried -= 1;
                                    record.outcome = ChallengerOutcome::Stopped;
                                }
                            }
                        }
                        report.choice_trials.push(record);
                    }
                    report.choices = sites
                        .iter()
                        .map(|site| ChoiceOutcome {
                            family: site.key.family,
                            site: site.name.clone(),
                            seed: site.name_of(site.seed),
                            delivered: site.name_of(site.applied),
                            offered: site
                                .alternatives
                                .iter()
                                .map(|offered| (offered.name, offered.saving))
                                .collect(),
                        })
                        .collect();
                }
                let mut seen = vec![seed.effective()];
                for challenger in Challenger::ORDER {
                    if stopped {
                        report
                            .trials
                            .push(trial(challenger, ChallengerOutcome::Stopped));
                        continue;
                    }
                    if report.scored >= budget {
                        report
                            .trials
                            .push(trial(challenger, ChallengerOutcome::Budget));
                        continue;
                    }
                    let next = challenger.apply(codec, incumbent.spelling);
                    if seen.contains(&next.effective()) {
                        report
                            .trials
                            .push(trial(challenger, ChallengerOutcome::Duplicate));
                        continue;
                    }
                    seen.push(next.effective());
                    if (OutputTactics {
                        families: next.families,
                        ..output.clone()
                    })
                    .check_policy(policy)
                    .is_err()
                    {
                        report
                            .trials
                            .push(trial(challenger, ChallengerOutcome::Vetoed));
                        continue;
                    }
                    report.tried += 1;
                    let (judgement, probed) =
                        judge.judge(formations, portfolio, next, &incumbent.choices, &incumbent)?;
                    report.codec_probes += usize::from(probed);
                    let (outcome, size, delta) = match judgement {
                        Judgement::Kept { artifact, size } => {
                            report.scored += 1;
                            let delta = size as i64 - incumbent.size as i64;
                            incumbent = Incumbent {
                                artifact,
                                spelling: next,
                                choices: incumbent.choices.clone(),
                                size,
                            };
                            (ChallengerOutcome::Kept, Some(size), Some(delta))
                        }
                        Judgement::Rejected { size } => {
                            report.scored += 1;
                            let delta = size as i64 - incumbent.size as i64;
                            (ChallengerOutcome::Rejected, Some(size), Some(delta))
                        }
                        Judgement::Identical => (ChallengerOutcome::Identical, None, None),
                        Judgement::Refused => (ChallengerOutcome::Refused, None, None),
                        Judgement::Stopped => {
                            stopped = true;
                            report.tried -= 1;
                            (ChallengerOutcome::Stopped, None, None)
                        }
                    };
                    report.trials.push(ChallengerTrial {
                        challenger: challenger.name(),
                        outcome,
                        size,
                        delta,
                    });
                }
                Ok(())
            },
        );
        match formed {
            Ok(result) => result?,
            // The candidate's demand could not be admitted: nothing formed.
            Err(error) if exhausted(&error) => {
                report.trials.extend(
                    Challenger::ORDER
                        .into_iter()
                        .map(|challenger| trial(challenger, ChallengerOutcome::Stopped)),
                );
            }
            Err(error) => return Err(error.into()),
        }
        report.after = incumbent.size;
        report.spelling = spelling_names(incumbent.spelling);
        debug_assert!(report.after <= report.before);
        Ok(Some(report))
    }
}

#[cfg(test)]
#[path = "search_terminal_tests.rs"]
mod tests;
