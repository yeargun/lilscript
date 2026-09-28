//! The walk (plan M3.5; architecture §9.6 and law L7).
//!
//! From the level-0 artifact A0, the search's one mandatory render, each
//! requested objective walks one list of moves in a fixed order that no level
//! changes: the choice sites of the incumbent by stake (M9.1), the declared
//! family challengers (`js::Challenger`, M9.2, M9.3), then the joint moves (the
//! other literal spelling, the naming seeds). A move is formed from the same
//! candidate, rendered, admitted by the same admission as every artifact (the
//! target verifier at preparation, then `ArtifactArena::qualify` under the
//! policy), judged first by the proxy (Brotli at quality 5; gzip and raw are
//! their own proxies), which prunes a move worse than the incumbent by more
//! than the margin, and then by the objective's exact codec, which alone
//! keeps: a move replaces the incumbent only when the complete artifact is
//! strictly smaller and no entry's row grows.
//!
//! The level sets only counts (`WalkSchedule`): the positions the walk may
//! examine, the exact judgements it may make, and its passes. A pass walks
//! the list once; a pass that keeps a move is followed by another, up to the
//! level's number, so from level 14 the walk reaches its fixed point. Then
//! the tail: a restart from A0 under each other naming seed, walked in passes
//! of its own and kept only when its result is strictly smaller, and the
//! structural beam (`beam_move`) until M9.1's rest deletes it.
//!
//! Monotone by construction: nothing the walk reads depends on the level,
//! and every level's counts are at least the level below's, so the walk at
//! L+1 passes through level L's stopping point and every later change is a
//! strict exact win: size(L+1) <= size(L). Everything here is sequential and
//! deterministic: no thread count, time or allocation address enters a
//! decision.
//!
//! Why the list runs on the final artifact and not inside the search: a
//! rewrite that runs on every emission moves which plan the search ranks
//! first, and can lose on the fleet while winning on its own (the terminal
//! challenger law; migration 7.34–7.37 read +128 over 20 ports that way).
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
    /// The effort's prefix or exact budget was spent before it.
    Budget,
    /// Its proxy judgement was worse than the incumbent's by more than the
    /// margin: not judged exactly (architecture §9.4; the proxy never keeps).
    Pruned,
    /// The compilation's resources ran out before it; the stage ended.
    Stopped,
}

/// One declared challenger's trial, in schedule order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChallengerTrial {
    pub challenger: &'static str,
    /// The walk's pass, from 1.
    pub pass: usize,
    pub outcome: ChallengerOutcome,
    /// Its complete artifact's size under the objective's codec, when scored.
    pub size: Option<usize>,
    /// That size minus the incumbent's at the time (negative is smaller).
    pub delta: Option<i64>,
    /// The proxy judge's delta against the incumbent, when it judged.
    pub proxy: Option<i64>,
}

/// One whole-artifact joint move's trial (architecture §9.5), after the
/// site moves: the other literal spelling, or another naming seed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JointTrial {
    pub name: String,
    pub pass: usize,
    pub outcome: ChallengerOutcome,
    pub size: Option<usize>,
    pub delta: Option<i64>,
    pub proxy: Option<i64>,
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
    pub pass: usize,
    pub outcome: ChallengerOutcome,
    pub size: Option<usize>,
    pub delta: Option<i64>,
    pub proxy: Option<i64>,
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

/// The walk on one objective (architecture §9.6): its level's schedule,
/// what it examined, judged and kept. The list is the choice moves, the
/// declared challengers, then the joint moves; a pass walks it once, and a
/// pass that keeps a move is followed by another, up to the level's number.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TerminalObjective {
    pub codec: &'static str,
    /// The level's prefix p(L), exact budget e(L), proxy margin M and
    /// number of passes; an unbounded count is null.
    #[serde(serialize_with = "crate::compilation_policy::serialize_bound")]
    pub prefix: usize,
    #[serde(serialize_with = "crate::compilation_policy::serialize_bound")]
    pub exact: usize,
    pub margin: usize,
    #[serde(serialize_with = "crate::compilation_policy::serialize_bound")]
    pub pass_limit: usize,
    /// Passes over the list the walk made: it stops after one that keeps
    /// nothing.
    pub passes: usize,
    /// List positions examined, at most `prefix`.
    pub examined: usize,
    /// Exact judgements made (kept or rejected), at most `exact`.
    pub judged: usize,
    /// Moves the proxy pruned.
    pub pruned: usize,
    /// Challengers formed: every judged one, and those that rendered the
    /// incumbent's bytes or were refused.
    pub tried: usize,
    /// Challengers the codec judged exactly.
    pub scored: usize,
    /// Exact codec scores the walk spent.
    pub codec_probes: usize,
    /// The level-0 artifact's size under the codec, and the delivered one's.
    pub before: usize,
    pub after: usize,
    /// The delivered artifact's families and raw spelling, by name.
    pub spelling: Vec<&'static str>,
    /// The delivered artifact's naming seed.
    pub style: String,
    /// Every declared challenger, in schedule order, for each pass.
    pub trials: Vec<ChallengerTrial>,
    /// Choice alternatives formed.
    pub choices_tried: usize,
    /// Choice alternatives the codec judged exactly.
    pub choices_scored: usize,
    /// Formations made only to read the incumbent's choice sites (0 or 1).
    pub surveys: usize,
    /// Every choice alternative offered, in schedule order: sites by the
    /// largest estimated saving, then each site's alternatives by theirs.
    pub choice_trials: Vec<ChoiceTrial>,
    /// Every choice site of the delivered artifact.
    pub choices: Vec<ChoiceOutcome>,
    /// Joint moves formed.
    pub joints_tried: usize,
    /// The joint moves after the site moves, in order: the literal
    /// spelling, then the naming seeds.
    pub joint_trials: Vec<JointTrial>,
    /// Restarts formed.
    pub restarts_tried: usize,
    /// The tail's restarts (from level 14), in the policy's seed order.
    pub restarts: Vec<RestartTrial>,
}

/// One restart of the walk's tail: the level-0 artifact under another
/// naming seed, walked in passes of its own from `pass` on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RestartTrial {
    pub name: String,
    /// The first pass of its walk.
    pub pass: usize,
    /// Kept: its result replaced the best walk's; rejected: it did not;
    /// pruned, identical or refused: its start.
    pub outcome: ChallengerOutcome,
    /// Its start's size under the objective's codec, when measured.
    pub start: Option<usize>,
    /// Its walk's result, and that minus the best result at the time.
    pub size: Option<usize>,
    pub delta: Option<i64>,
    /// The proxy judge's delta of its start against the level-0 artifact.
    pub proxy: Option<i64>,
}

/// Bounded by the declared schedule: at most `Challenger::ORDER.len()`
/// challenger trials per requested objective, and one choice trial per
/// alternative of each site the delivered candidate offers.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct TerminalReport {
    pub objectives: Vec<TerminalObjective>,
    /// The reserved beam move, when the level reaches it.
    pub beam: Option<BeamReport>,
}

/// The beam move (architecture §9.6): the structural exploration at its
/// level-13 schedule, every recipe formed with the walk incumbent's tactics
/// and naming, kept only on a strict exact win.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BeamReport {
    /// Why it did not run, when it did not.
    pub skipped: Option<&'static str>,
    pub proposals: usize,
    pub structures: usize,
    pub renders: usize,
    pub codec_probes: usize,
    pub before: usize,
    pub after: usize,
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

/// The walk's current artifact and the assignment that forms it. The
/// level-0 artifact is the portfolio's selected entry; every later one is
/// retained and qualified by the walk (`qualified`), which discards it when
/// it is replaced and promotes the last one once, when the walk ends.
#[derive(Clone)]
struct Incumbent {
    artifact: ArtifactId,
    spelling: Spelling,
    choices: ChoiceMap,
    plan: Plan,
    literals: crate::js::LiteralOutput,
    size: usize,
    qualified: Option<QualifiedArtifact>,
}

/// One trial's verdict against the incumbent, with its proxy delta when the
/// proxy judged it.
enum Judgement {
    Kept {
        artifact: ArtifactId,
        size: usize,
        qualified: QualifiedArtifact,
    },
    Rejected {
        size: usize,
    },
    Pruned,
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

/// What the walk holds fixed while it judges one objective's moves.
struct Judge<'a> {
    policy: &'a ResolvedPolicy,
    codec: Objective,
    output: &'a OutputTactics,
    available: usize,
    /// The proxy margin M of the level's schedule.
    margin: i64,
    /// The search baseline's qualification, the base of every comparison.
    baseline: Option<QualifiedArtifact>,
}

impl Judge<'_> {
    /// Form the candidate under `spelling` and `choices` and render it under
    /// `plan` with `literals`; then judge it against `reference`: identical
    /// bytes are `Identical`, and a proxy worse by more than the margin is
    /// `Pruned`. Otherwise the objective's exact codec measures it and
    /// admission qualifies it. Returns the retained artifact with its exact
    /// size and qualification, the proxy delta, and whether an exact codec
    /// score was spent; on any other verdict the artifact is discarded.
    fn measure(
        &self,
        formations: &mut Formations<'_, '_>,
        spelling: Spelling,
        choices: &ChoiceMap,
        plan: &Plan,
        literals: crate::js::LiteralOutput,
        reference: &Incumbent,
    ) -> Result<(Result<(ArtifactId, usize, QualifiedArtifact), Judgement>, Option<i64>, bool), SearchError>
    {
        let Self {
            policy,
            codec,
            output,
            available,
            margin,
            baseline,
        } = *self;
        let tactics = OutputTactics {
            families: spelling.families,
            choices: choices.clone(),
            literals,
            ..output.clone()
        };
        let named = Plan {
            style: plan.style,
            source_names: plan.source_names.clone(),
            raw_spelling: spelling.raw_spelling,
        };
        let rendered = formations
            .form(tactics, |target| {
                let staged = target.render_bounded_with_literals(&named, literals, available)?;
                target.retain_artifact(staged)
            })
            .and_then(|retained| retained);
        let challenged = match rendered {
            Ok(challenged) => challenged,
            Err(error) if exhausted(&error) => return Ok((Err(Judgement::Stopped), None, false)),
            Err(_) => return Ok((Err(Judgement::Refused), None, false)),
        };
        // The proxy judges first: a move clearly worse than the reference is
        // pruned before the exact codec runs. Where the proxy is the exact
        // codec (gzip, a delivery plan's files), its measurement is the
        // move's exact probe.
        let exact_proxy = codec != Objective::Raw;
        let proxied = formations.with_arena(|arena, _, budget| {
            let result = (|| -> Result<Option<(i64, bool)>, CandidateError> {
                if arena.same_output(challenged, reference.artifact, budget)? {
                    return Ok(None);
                }
                let exact = exact_proxy && arena.proxy_is_exact(challenged, codec)?;
                let challenger = arena.measure_proxy(challenged, codec, budget)?;
                let held = arena.measure_proxy(reference.artifact, codec, budget)?;
                Ok(Some((challenger as i64 - held as i64, exact)))
            })();
            let prune = match &result {
                Ok(Some((delta, _))) => *delta > margin,
                Ok(None) => true,
                Err(_) => true,
            };
            if prune {
                arena
                    .discard(challenged, budget)
                    .expect("the walk owns its move's artifact");
            }
            result
        });
        let proxy = match proxied {
            Ok(Some((delta, exact))) if delta > margin => {
                return Ok((Err(Judgement::Pruned), Some(delta), exact))
            }
            Ok(Some((delta, _))) => Some(delta),
            Ok(None) => return Ok((Err(Judgement::Identical), None, false)),
            Err(error) if exhausted(&error) => return Ok((Err(Judgement::Stopped), None, false)),
            Err(_) => return Ok((Err(Judgement::Refused), None, false)),
        };
        let scored = formations.with_arena(|arena, contract, budget| {
            let result = (|| -> Result<(usize, QualifiedArtifact), CandidateError> {
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
                Ok((size, qualified))
            })();
            if result.is_err() {
                arena
                    .discard(challenged, budget)
                    .expect("the walk owns its move's artifact");
            }
            result
        });
        let probed = codec != Objective::Raw;
        match scored {
            Ok((size, qualified)) => Ok((Ok((challenged, size, qualified)), proxy, probed)),
            Err(error) if exhausted(&error) => Ok((Err(Judgement::Stopped), proxy, probed)),
            Err(_) => Ok((Err(Judgement::Refused), proxy, probed)),
        }
    }

    /// Whether `challenger` replaces `held`: strictly smaller as a complete
    /// artifact under the objective, and no entry's row grows (dominance,
    /// design §10; one file is one row).
    fn wins(
        &self,
        formations: &mut Formations<'_, '_>,
        challenger: ArtifactId,
        qualified: QualifiedArtifact,
        held: &Incumbent,
    ) -> Result<bool, SearchError> {
        let codec = self.codec;
        let rows = formations.with_arena(|arena, _, budget| {
            Ok::<_, CandidateError>((
                arena.rows(challenger, codec, budget)?,
                arena.rows(held.artifact, codec, budget)?,
            ))
        })?;
        let base = self
            .baseline
            .map_or(qualified.cost(), |baseline| baseline.cost());
        let order = self
            .policy
            .compare_evidence(
                qualified.cost(),
                CandidateCostEvidence::size_only(held.size as u64),
                base,
            )
            .map_err(SearchError::from)?
            .ok_or(CandidateError::NotJavaScript)?;
        Ok(order == Ordering::Less && dominates(&rows.0, &rows.1))
    }

    /// Form, render and judge one move against the incumbent: the proxy
    /// first, pruning a move worse than the incumbent by more than the
    /// margin; then the objective's exact codec, which alone keeps. A kept
    /// move's artifact is retained for the walk; any other is discarded.
    /// Returns the verdict, the proxy delta and whether an exact codec score
    /// was spent.
    fn judge(
        &self,
        formations: &mut Formations<'_, '_>,
        spelling: Spelling,
        choices: &ChoiceMap,
        plan: &Plan,
        literals: crate::js::LiteralOutput,
        incumbent: &Incumbent,
    ) -> Result<(Judgement, Option<i64>, bool), SearchError> {
        let (measured, proxy, probed) =
            self.measure(formations, spelling, choices, plan, literals, incumbent)?;
        let (challenged, size, qualified) = match measured {
            Ok(measured) => measured,
            Err(judgement) => return Ok((judgement, proxy, probed)),
        };
        let wins = self.wins(formations, challenged, qualified, incumbent);
        if !matches!(wins, Ok(true)) {
            formations.with_arena(|arena, _, budget| {
                arena
                    .discard(challenged, budget)
                    .expect("the walk owns its move's artifact")
            });
        }
        match wins {
            Ok(true) => Ok((
                Judgement::Kept {
                    artifact: challenged,
                    size,
                    qualified,
                },
                proxy,
                probed,
            )),
            Ok(false) => Ok((Judgement::Rejected { size }, proxy, probed)),
            Err(error) if error.optional_memory_refusal() || resource(&error) => {
                Ok((Judgement::Stopped, proxy, probed))
            }
            Err(error) => Err(error),
        }
    }
}

/// One objective's walk inside its candidate's formations (architecture
/// §9.6): the list walked in passes from an incumbent, with the walk's counts
/// in its report.
struct Walker<'w, 'scope, 'src> {
    formations: &'w mut Formations<'scope, 'src>,
    judge: &'w Judge<'w>,
    report: &'w mut TerminalObjective,
    walk: crate::compilation_policy::WalkSchedule,
    codec: Objective,
    output: &'w OutputTactics,
    /// The naming seeds the policy permits.
    naming: &'w [Style],
    choices_permitted: bool,
    /// The compilation's resources ran out: every later position is
    /// `Stopped`.
    stopped: bool,
}

impl Walker<'_, '_, '_> {
    /// Whether the walk may examine one more position.
    fn open(&self) -> bool {
        self.report.examined < self.walk.prefix && self.report.judged < self.walk.exact
    }

    /// Record one judged move in the counts.
    fn count(&mut self, judgement: &Judgement, probed: bool) {
        self.report.codec_probes += usize::from(probed);
        match judgement {
            Judgement::Kept { .. } | Judgement::Rejected { .. } => self.report.judged += 1,
            Judgement::Pruned => self.report.pruned += 1,
            _ => {}
        }
    }

    /// Put `next` in `incumbent`'s place, discarding the replaced artifact
    /// when the walk retains it.
    fn replace(&mut self, incumbent: &mut Incumbent, next: Incumbent) {
        let replaced = std::mem::replace(incumbent, next);
        self.discard(replaced);
    }

    /// Let go of an incumbent: the walk discards what it retains, and never
    /// the portfolio's level-0 artifact.
    fn discard(&mut self, incumbent: Incumbent) {
        if incumbent.qualified.is_some() {
            self.formations.with_arena(|arena, _, budget| {
                arena
                    .discard(incumbent.artifact, budget)
                    .expect("the walk owns its incumbents")
            });
        }
    }

    /// Walk the list in passes from `incumbent`, each from the incumbent the
    /// last one left, until a pass keeps nothing (the fixed point) or the
    /// level's passes or budgets run out.
    fn passes(&mut self, incumbent: &mut Incumbent) -> Result<(), SearchError> {
        let mut passes = 0;
        while passes < self.walk.passes && !self.stopped && self.open() {
            passes += 1;
            self.report.passes += 1;
            let pass = self.report.passes;
            let kept = self.choice_moves(incumbent, pass)?
                | self.challengers(incumbent, pass)?
                | self.joint_moves(incumbent, pass)?;
            if !kept {
                break;
            }
        }
        Ok(())
    }

    /// The choice moves (M9.1): the incumbent's choice sites, surveyed, in
    /// the choice schedule. Whether one was kept.
    fn choice_moves(&mut self, incumbent: &mut Incumbent, pass: usize) -> Result<bool, SearchError> {
        if !self.choices_permitted {
            return Ok(false);
        }
        self.report.surveys += 1;
        let surveyed = self.formations.survey(&OutputTactics {
            families: incumbent.spelling.families,
            choices: incumbent.choices.clone(),
            ..self.output.clone()
        });
        let mut sites = match surveyed {
            Ok(sites) => sites,
            Err(error) if exhausted(&error) => {
                self.stopped = true;
                return Ok(false);
            }
            Err(_) => return Ok(false),
        };
        let mut kept = false;
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
                pass,
                outcome: ChallengerOutcome::Budget,
                size: None,
                delta: None,
                proxy: None,
            };
            let moved: Vec<(usize, crate::js::AltId)> = moves
                .iter()
                .copied()
                .filter(|&(site, alternative)| sites[site].applied != alternative)
                .collect();
            if self.stopped {
                record.outcome = ChallengerOutcome::Stopped;
            } else if !self.open() {
                record.outcome = ChallengerOutcome::Budget;
            } else if moved.is_empty() {
                // An earlier trial kept this assignment already.
                self.report.examined += 1;
                record.outcome = ChallengerOutcome::Duplicate;
            } else {
                self.report.examined += 1;
                self.report.choices_tried += 1;
                let choices = moved
                    .iter()
                    .fold(incumbent.choices.clone(), |choices, &(site, alternative)| {
                        choices.with(sites[site].key, alternative)
                    });
                let (judgement, proxy, probed) = self.judge.judge(
                    self.formations,
                    incumbent.spelling,
                    &choices,
                    &incumbent.plan,
                    incumbent.literals,
                    incumbent,
                )?;
                self.count(&judgement, probed);
                record.proxy = proxy;
                match judgement {
                    Judgement::Kept {
                        artifact,
                        size,
                        qualified,
                    } => {
                        kept = true;
                        self.report.choices_scored += 1;
                        record.outcome = ChallengerOutcome::Kept;
                        record.size = Some(size);
                        record.delta = Some(size as i64 - incumbent.size as i64);
                        let next = Incumbent {
                            artifact,
                            choices,
                            size,
                            qualified: Some(qualified),
                            ..incumbent.clone()
                        };
                        self.replace(incumbent, next);
                        for &(site, alternative) in &moved {
                            sites[site].applied = alternative;
                        }
                    }
                    Judgement::Rejected { size } => {
                        self.report.choices_scored += 1;
                        record.outcome = ChallengerOutcome::Rejected;
                        record.size = Some(size);
                        record.delta = Some(size as i64 - incumbent.size as i64);
                    }
                    Judgement::Pruned => record.outcome = ChallengerOutcome::Pruned,
                    Judgement::Identical => record.outcome = ChallengerOutcome::Identical,
                    Judgement::Refused => record.outcome = ChallengerOutcome::Refused,
                    Judgement::Stopped => {
                        self.stopped = true;
                        self.report.choices_tried -= 1;
                        record.outcome = ChallengerOutcome::Stopped;
                    }
                }
            }
            self.report.choice_trials.push(record);
        }
        self.report.choices = sites
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
        Ok(kept)
    }

    /// The declared challengers, each applied to the incumbent's spelling;
    /// a spelling is tried once per pass. Whether one was kept.
    fn challengers(&mut self, incumbent: &mut Incumbent, pass: usize) -> Result<bool, SearchError> {
        let trial = |challenger: Challenger, outcome| ChallengerTrial {
            challenger: challenger.name(),
            pass,
            outcome,
            size: None,
            delta: None,
            proxy: None,
        };
        let mut kept = false;
        let mut seen = vec![incumbent.spelling.effective()];
        for challenger in Challenger::ORDER {
            if self.stopped {
                self.report
                    .trials
                    .push(trial(challenger, ChallengerOutcome::Stopped));
                continue;
            }
            if !self.open() {
                self.report
                    .trials
                    .push(trial(challenger, ChallengerOutcome::Budget));
                continue;
            }
            self.report.examined += 1;
            let next = challenger.apply(self.codec, incumbent.spelling);
            if seen.contains(&next.effective()) {
                self.report
                    .trials
                    .push(trial(challenger, ChallengerOutcome::Duplicate));
                continue;
            }
            seen.push(next.effective());
            if (OutputTactics {
                families: next.families,
                ..self.output.clone()
            })
            .check_policy(self.judge.policy)
            .is_err()
            {
                self.report
                    .trials
                    .push(trial(challenger, ChallengerOutcome::Vetoed));
                continue;
            }
            self.report.tried += 1;
            let (judgement, proxy, probed) = self.judge.judge(
                self.formations,
                next,
                &incumbent.choices,
                &incumbent.plan,
                incumbent.literals,
                incumbent,
            )?;
            self.count(&judgement, probed);
            let (outcome, size, delta) = match judgement {
                Judgement::Kept {
                    artifact,
                    size,
                    qualified,
                } => {
                    kept = true;
                    self.report.scored += 1;
                    let delta = size as i64 - incumbent.size as i64;
                    let replaced = Incumbent {
                        artifact,
                        spelling: next,
                        size,
                        qualified: Some(qualified),
                        ..incumbent.clone()
                    };
                    self.replace(incumbent, replaced);
                    (ChallengerOutcome::Kept, Some(size), Some(delta))
                }
                Judgement::Rejected { size } => {
                    self.report.scored += 1;
                    let delta = size as i64 - incumbent.size as i64;
                    (ChallengerOutcome::Rejected, Some(size), Some(delta))
                }
                Judgement::Pruned => (ChallengerOutcome::Pruned, None, None),
                Judgement::Identical => (ChallengerOutcome::Identical, None, None),
                Judgement::Refused => (ChallengerOutcome::Refused, None, None),
                Judgement::Stopped => {
                    self.stopped = true;
                    self.report.tried -= 1;
                    (ChallengerOutcome::Stopped, None, None)
                }
            };
            self.report.trials.push(ChallengerTrial {
                challenger: challenger.name(),
                pass,
                outcome,
                size,
                delta,
                proxy,
            });
        }
        Ok(kept)
    }

    /// The joint moves, after every site move (architecture §9.5): the other
    /// literal spelling (only target compaction may observe weak literals),
    /// then each naming seed the policy permits other than the incumbent's.
    /// Whether one was kept.
    fn joint_moves(&mut self, incumbent: &mut Incumbent, pass: usize) -> Result<bool, SearchError> {
        use crate::js::LiteralOutput;
        let other = match incumbent.literals {
            LiteralOutput::Original => LiteralOutput::Observed,
            LiteralOutput::Observed => LiteralOutput::Original,
        };
        let mut joints: Vec<(String, Option<Style>)> = Vec::new();
        if other == LiteralOutput::Original || self.output.target_compaction {
            joints.push((format!("literals:{other:?}"), None));
        }
        for &style in self.naming {
            if style != incumbent.plan.style {
                joints.push((format!("naming:{style:?}"), Some(style)));
            }
        }
        let mut kept = false;
        for (name, style) in joints {
            let mut record = JointTrial {
                name,
                pass,
                outcome: ChallengerOutcome::Budget,
                size: None,
                delta: None,
                proxy: None,
            };
            if self.stopped {
                record.outcome = ChallengerOutcome::Stopped;
            } else if self.open() {
                self.report.examined += 1;
                self.report.joints_tried += 1;
                // A naming move keeps the incumbent's literal spelling, which
                // the literal move may just have changed.
                let literals = if style.is_some() {
                    incumbent.literals
                } else {
                    other
                };
                let plan = match style {
                    Some(style) => Plan {
                        style,
                        source_names: Vec::new(),
                        raw_spelling: incumbent.spelling.raw_spelling,
                    },
                    None => incumbent.plan.clone(),
                };
                let (judgement, proxy, probed) = self.judge.judge(
                    self.formations,
                    incumbent.spelling,
                    &incumbent.choices,
                    &plan,
                    literals,
                    incumbent,
                )?;
                self.count(&judgement, probed);
                record.proxy = proxy;
                record.outcome = match judgement {
                    Judgement::Kept {
                        artifact,
                        size,
                        qualified,
                    } => {
                        kept = true;
                        record.size = Some(size);
                        record.delta = Some(size as i64 - incumbent.size as i64);
                        let next = Incumbent {
                            artifact,
                            plan,
                            literals,
                            size,
                            qualified: Some(qualified),
                            ..incumbent.clone()
                        };
                        self.replace(incumbent, next);
                        ChallengerOutcome::Kept
                    }
                    Judgement::Rejected { size } => {
                        record.size = Some(size);
                        record.delta = Some(size as i64 - incumbent.size as i64);
                        ChallengerOutcome::Rejected
                    }
                    Judgement::Pruned => ChallengerOutcome::Pruned,
                    Judgement::Identical => ChallengerOutcome::Identical,
                    Judgement::Refused => ChallengerOutcome::Refused,
                    Judgement::Stopped => {
                        self.stopped = true;
                        self.report.joints_tried -= 1;
                        ChallengerOutcome::Stopped
                    }
                };
            }
            self.report.joint_trials.push(record);
        }
        Ok(kept)
    }

    /// A restart, the tail's joint move (from level 14): the level-0
    /// artifact under another naming seed, walked in passes of its own. Its
    /// start is judged by the proxy against the level-0 artifact and then
    /// measured exactly; its result replaces `best` only on a strict exact
    /// win. A pass-by-pass walk cannot reach an assignment whose naming loses
    /// alone and wins with the families it enables.
    fn restart(&mut self, origin: &Incumbent, best: &mut Incumbent, style: Style) -> Result<(), SearchError> {
        let mut record = RestartTrial {
            name: format!("naming:{style:?}"),
            pass: self.report.passes + 1,
            outcome: ChallengerOutcome::Budget,
            start: None,
            size: None,
            delta: None,
            proxy: None,
        };
        if self.stopped {
            record.outcome = ChallengerOutcome::Stopped;
            self.report.restarts.push(record);
            return Ok(());
        }
        if !self.open() {
            self.report.restarts.push(record);
            return Ok(());
        }
        self.report.examined += 1;
        self.report.restarts_tried += 1;
        let plan = Plan {
            style,
            source_names: Vec::new(),
            raw_spelling: origin.spelling.raw_spelling,
        };
        let (measured, proxy, probed) = self.judge.measure(
            self.formations,
            origin.spelling,
            &origin.choices,
            &plan,
            origin.literals,
            origin,
        )?;
        self.report.codec_probes += usize::from(probed);
        record.proxy = proxy;
        let (artifact, size, qualified) = match measured {
            Ok(measured) => measured,
            Err(judgement) => {
                record.outcome = match judgement {
                    Judgement::Pruned => {
                        self.report.pruned += 1;
                        ChallengerOutcome::Pruned
                    }
                    Judgement::Identical => ChallengerOutcome::Identical,
                    Judgement::Stopped => {
                        self.stopped = true;
                        self.report.restarts_tried -= 1;
                        ChallengerOutcome::Stopped
                    }
                    _ => ChallengerOutcome::Refused,
                };
                self.report.restarts.push(record);
                return Ok(());
            }
        };
        // The start's exact measurement is the restart's judgement.
        self.report.judged += 1;
        record.start = Some(size);
        let mut local = Incumbent {
            artifact,
            plan,
            size,
            qualified: Some(qualified),
            ..origin.clone()
        };
        let index = self.report.restarts.len();
        self.report.restarts.push(record);
        self.passes(&mut local)?;
        let wins = match local.qualified {
            Some(qualified) => self.judge.wins(self.formations, local.artifact, qualified, best),
            None => Ok(false),
        };
        let record = &mut self.report.restarts[index];
        record.size = Some(local.size);
        record.delta = Some(local.size as i64 - best.size as i64);
        match wins {
            Ok(true) => {
                record.outcome = ChallengerOutcome::Kept;
                self.replace(best, local);
            }
            Ok(false) => {
                record.outcome = ChallengerOutcome::Rejected;
                self.discard(local);
            }
            Err(error) if error.optional_memory_refusal() || resource(&error) => {
                record.outcome = ChallengerOutcome::Stopped;
                self.stopped = true;
                self.discard(local);
            }
            Err(error) => {
                self.discard(local);
                return Err(error);
            }
        }
        Ok(())
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
        request: SearchRequest,
        mut observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<&TerminalReport, SearchError> {
        if self.terminal.is_none() {
            let objective = policy.objective().ok_or(CandidateError::NotJavaScript)?;
            let mut report = TerminalReport::default();
            for codec in request.objectives.iter() {
                if let Some(trials) = self.challenge_objective(policy, objective, codec)? {
                    report.objectives.push(trials);
                }
            }
            report.beam = self.beam_move(policy, objective, request, &mut observe)?;
            if let (Some(beam), [walk]) = (&report.beam, report.objectives.as_mut_slice()) {
                walk.after = beam.after;
            }
            self.terminal = Some(report);
        }
        Ok(self.terminal.as_ref().unwrap())
    }

    /// The reserved beam move, the last of the walk's list: reached only at
    /// the levels whose schedule reserves it (§13.4), after every unreserved
    /// move, so a level that reaches it passes through the result of every
    /// level that does not.
    fn beam_move(
        &mut self,
        policy: &ResolvedPolicy,
        objective: OptimizationObjective,
        request: SearchRequest,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<Option<BeamReport>, SearchError> {
        if !objective.walk.tail {
            return Ok(None);
        }
        let skipped = |reason| {
            Ok(Some(BeamReport {
                skipped: Some(reason),
                proposals: 0,
                structures: 0,
                renders: 0,
                codec_probes: 0,
                before: 0,
                after: 0,
            }))
        };
        let Objectives::One(codec) = request.objectives else {
            // Each objective's incumbent has its own tactics; one exploration
            // cannot form every recipe with all of them.
            return skipped("several objectives");
        };
        if self.stopped.is_some() {
            return skipped("stopped");
        }
        let Some(position) = self.portfolio.selected[index(codec)] else {
            return skipped("no incumbent");
        };
        let artifact = self.portfolio.entries.get(position).unwrap().artifact;
        let arena = &self.compilation.artifacts;
        let provenance = arena.provenance(artifact)?;
        let plan = provenance.naming().clone();
        let seed = BeamSeed {
            tactics: provenance.description().output().clone(),
            style: plan.style,
            raw_spelling: plan.raw_spelling,
        };
        let before = arena
            .with_artifact(artifact, |view| view.sizes.get(codec))?
            .expect("an incumbent is measured under its codec");
        let counted = self.counters;
        self.beam = Some(seed);
        let styles = [plan.style];
        let exploration = self.explore(
            policy,
            objective,
            request,
            &styles,
            self.baseline_renders,
            observe,
        );
        // A proposal ceiling ends discovery, not already admitted scoring. A
        // refused allocation ends discovery after its temporary owners
        // unwind. Work and deadline failures do not receive a new allowance.
        match exploration {
            Ok(()) => {
                self.stopped = self.drain_pending(policy, request.objectives, observe).err();
            }
            Err(SearchError::Limit(SearchLimit::Alternatives)) => {
                self.stopped = Some(
                    self.drain_pending(policy, request.objectives, observe)
                        .err()
                        .unwrap_or(SearchError::Limit(SearchLimit::Alternatives)),
                );
            }
            Err(error) if error.optional_memory_refusal() => {
                self.discovery_refusal = Some(error);
                self.stopped = self
                    .finish_discovery()
                    .and_then(|()| self.drain_pending(policy, request.objectives, observe))
                    .err();
            }
            Err(error) => self.stopped = Some(error),
        }
        self.abandon_pending();
        self.beam = None;
        let after = self.portfolio.selected[index(codec)]
            .and_then(|position| self.portfolio.entries.get(position))
            .map(|entry| entry.artifact)
            .and_then(|artifact| {
                self.compilation
                    .artifacts
                    .with_artifact(artifact, |view| view.sizes.get(codec))
                    .ok()
                    .flatten()
            })
            .unwrap_or(before);
        Ok(Some(BeamReport {
            skipped: None,
            proposals: self.counters.proposals - counted.proposals,
            structures: self.counters.structures - counted.structures,
            renders: self.counters.renders - counted.renders,
            codec_probes: self.counters.codec_probes - counted.codec_probes,
            before,
            after,
        }))
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
        let walk = objective.walk;
        let available = objective
            .retained_candidate_bytes
            .max(self.portfolio.baseline_capacity);
        let mut report = TerminalObjective {
            codec: codec_name(codec),
            prefix: walk.prefix,
            exact: walk.exact,
            margin: walk.margin,
            pass_limit: walk.passes,
            passes: 0,
            examined: 0,
            judged: 0,
            pruned: 0,
            tried: 0,
            scored: 0,
            codec_probes: 0,
            before,
            after: before,
            spelling: spelling_names(seed),
            style: format!("{:?}", plan.style),
            trials: Vec::with_capacity(Challenger::ORDER.len()),
            choices_tried: 0,
            choices_scored: 0,
            surveys: 0,
            choice_trials: Vec::new(),
            choices: Vec::new(),
            joints_tried: 0,
            joint_trials: Vec::new(),
            restarts_tried: 0,
            restarts: Vec::new(),
        };
        let trial = |challenger: Challenger, outcome| ChallengerTrial {
            challenger: challenger.name(),
            pass: 1,
            outcome,
            size: None,
            delta: None,
            proxy: None,
        };
        if walk.prefix == 0 {
            report.trials.extend(
                Challenger::ORDER
                    .into_iter()
                    .map(|challenger| trial(challenger, ChallengerOutcome::Budget)),
            );
            return Ok(Some(report));
        }
        // The naming seeds the policy permits: whole-artifact moves after
        // every site move (architecture §9.5). A plan that retains source
        // names has none.
        let naming: Vec<Style> = if plan.source_names.is_empty() {
            Plan::seeds_for_policy(policy).map_or_else(|_| Vec::new(), <[Style]>::to_vec)
        } else {
            Vec::new()
        };
        let Self {
            compilation,
            portfolio,
            ..
        } = self;
        // The level-0 artifact: the portfolio's selected entry, which the
        // walk never discards.
        let origin = Incumbent {
            artifact,
            spelling: seed,
            choices: output.choices.clone(),
            plan,
            literals: output.literals,
            size: before,
            qualified: None,
        };
        let judge = Judge {
            policy,
            codec,
            output: &output,
            available,
            margin: i64::try_from(walk.margin).unwrap_or(i64::MAX),
            baseline: portfolio.baseline_qualification(codec).copied(),
        };
        let formed = compilation.with_javascript_formations_in(
            candidate,
            policy,
            output.dead_code_elimination,
            output.target_compaction,
            WorkDomain::Optional,
            |formations| -> Result<Incumbent, SearchError> {
                let mut walker = Walker {
                    formations,
                    judge: &judge,
                    report: &mut report,
                    walk,
                    codec,
                    output: &output,
                    naming: &naming,
                    choices_permitted: output.target_compaction,
                    stopped: false,
                };
                let mut best = origin.clone();
                walker.passes(&mut best)?;
                // The tail (from level 14): a restart under each other
                // naming seed, in the policy's order.
                if walk.tail {
                    for &style in walker.naming {
                        if style != origin.plan.style {
                            walker.restart(&origin, &mut best, style)?;
                        }
                    }
                }
                // The walk's result becomes the objective's winner; a
                // refusal to admit it keeps the level-0 artifact.
                if let Some(qualified) = best.qualified {
                    let promoted = walker.formations.with_arena(|arena, _, budget| {
                        portfolio.promote_terminal(arena, budget, codec, best.artifact, qualified)
                    });
                    match promoted {
                        Ok(_) => {}
                        Err(error) if error.optional_memory_refusal() || resource(&error) => {
                            walker.discard(best);
                            return Ok(origin.clone());
                        }
                        Err(error) => return Err(error),
                    }
                }
                Ok(best)
            },
        );
        let delivered = match formed {
            Ok(result) => result?,
            // The candidate's demand could not be admitted: nothing formed.
            Err(error) if exhausted(&error) => {
                report.trials.extend(
                    Challenger::ORDER
                        .into_iter()
                        .map(|challenger| trial(challenger, ChallengerOutcome::Stopped)),
                );
                origin
            }
            Err(error) => return Err(error.into()),
        };
        report.after = delivered.size;
        report.spelling = spelling_names(delivered.spelling);
        report.style = format!("{:?}", delivered.plan.style);
        debug_assert!(report.after <= report.before);
        Ok(Some(report))
    }
}

#[cfg(test)]
#[path = "search_terminal_tests.rs"]
mod tests;
