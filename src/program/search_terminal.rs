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
use crate::js::{Challenger, Spelling};

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
    /// Exact codec scores the stage spent.
    pub codec_probes: usize,
    /// The search winner's size under the codec, and the delivered one's.
    pub before: usize,
    pub after: usize,
    /// The delivered artifact's families and raw spelling, by name.
    pub spelling: Vec<&'static str>,
    /// Every declared challenger, in schedule order.
    pub trials: Vec<ChallengerTrial>,
}

/// Bounded by the declared schedule: at most `Challenger::ORDER.len()` trials
/// per requested objective.
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
    size: usize,
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
        let output = provenance.description().output();
        let before = arena
            .with_artifact(artifact, |view| view.sizes.get(codec))?
            .expect("a selected winner is measured under its codec");
        let seed = Spelling {
            families: output.families,
            raw_spelling: plan.raw_spelling,
        };
        let budget = objective.terminal_challengers;
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
        };
        let trial = |challenger: Challenger, outcome| ChallengerTrial {
            challenger: challenger.name(),
            outcome,
            size: None,
            delta: None,
        };
        if budget == 0 {
            report.trials.extend(
                Challenger::ORDER
                    .into_iter()
                    .map(|challenger| trial(challenger, ChallengerOutcome::Budget)),
            );
            return Ok(Some(report));
        }
        // When no challenger passes the duplicate and permission filters
        // from the seed, nothing is ever formed (the incumbent can only
        // change through a formed one): record that without building the
        // candidate's demand and head.
        let mut seen = vec![seed.effective()];
        let mut dry = Vec::with_capacity(Challenger::ORDER.len());
        let mut formable = false;
        for challenger in Challenger::ORDER {
            let next = challenger.apply(codec, seed);
            let outcome = if seen.contains(&next.effective()) {
                ChallengerOutcome::Duplicate
            } else if (OutputTactics {
                families: next.families,
                ..output
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
        if !formable {
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
            size: before,
        };
        let mut stopped = false;
        let formed = compilation.with_javascript_formations_in(
            candidate,
            policy,
            output.dead_code_elimination,
            output.target_compaction,
            WorkDomain::Optional,
            |formations| -> Result<(), SearchError> {
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
                    let choices = OutputTactics {
                        families: next.families,
                        ..output
                    };
                    if choices.check_policy(policy).is_err() {
                        report
                            .trials
                            .push(trial(challenger, ChallengerOutcome::Vetoed));
                        continue;
                    }
                    report.tried += 1;
                    let named = Plan {
                        style: plan.style,
                        source_names: plan.source_names.clone(),
                        raw_spelling: next.raw_spelling,
                    };
                    let rendered = formations
                        .form(choices, |target| {
                            let staged = target.render_bounded_with_literals(
                                &named,
                                output.literals,
                                available,
                            )?;
                            target.retain_artifact(staged)
                        })
                        .and_then(|retained| retained);
                    let challenged = match rendered {
                        Ok(challenged) => challenged,
                        Err(error) => {
                            stopped = exhausted(&error);
                            report.trials.push(trial(
                                challenger,
                                if stopped {
                                    ChallengerOutcome::Stopped
                                } else {
                                    ChallengerOutcome::Refused
                                },
                            ));
                            continue;
                        }
                    };
                    let baseline = portfolio.baseline_qualification(codec).copied();
                    let scored = formations.with_arena(|arena, contract, budget| {
                        let result =
                            (|| -> Result<Option<(usize, QualifiedArtifact)>, CandidateError> {
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
                                Ok(Some((size, qualified)))
                            })();
                        if !matches!(result, Ok(Some(_))) {
                            arena
                                .discard(challenged, budget)
                                .expect("the stage owns its challenger");
                        }
                        result
                    });
                    if codec != Objective::Raw && !matches!(scored, Ok(None)) {
                        report.codec_probes += 1;
                    }
                    let (size, qualified) = match scored {
                        Ok(Some(scored)) => scored,
                        Ok(None) => {
                            report
                                .trials
                                .push(trial(challenger, ChallengerOutcome::Identical));
                            continue;
                        }
                        Err(error) => {
                            stopped = exhausted(&error);
                            report.trials.push(trial(
                                challenger,
                                if stopped {
                                    ChallengerOutcome::Stopped
                                } else {
                                    ChallengerOutcome::Refused
                                },
                            ));
                            continue;
                        }
                    };
                    report.scored += 1;
                    let delta = size as i64 - incumbent.size as i64;
                    let base = baseline.map_or(qualified.cost(), |baseline| baseline.cost());
                    let smaller = policy
                        .compare_evidence(
                            qualified.cost(),
                            CandidateCostEvidence::size_only(incumbent.size as u64),
                            base,
                        )
                        .map_err(SearchError::from)
                        .and_then(|order| Ok(order.ok_or(CandidateError::NotJavaScript)?))
                        .map(|order| order == Ordering::Less);
                    // Promotion admits a portfolio entry before it changes
                    // anything; a refusal leaves the incumbent in place. The
                    // challenger goes unless it became the incumbent.
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
                    let outcome = match kept {
                        Ok(true) => {
                            incumbent = Incumbent {
                                artifact: challenged,
                                spelling: next,
                                size,
                            };
                            ChallengerOutcome::Kept
                        }
                        Ok(false) => ChallengerOutcome::Rejected,
                        Err(error) if error.optional_memory_refusal() || resource(&error) => {
                            stopped = true;
                            report
                                .trials
                                .push(trial(challenger, ChallengerOutcome::Stopped));
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    report.trials.push(ChallengerTrial {
                        challenger: challenger.name(),
                        outcome,
                        size: Some(size),
                        delta: Some(delta),
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
