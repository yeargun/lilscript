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
//! The fast tiers replay a fixed move sequence, and each walk retains its
//! best admitted result. A wider structural frontier or a shared hard limit
//! can change which moves later effort levels reach; universal non-growth
//! across levels is not established. Decisions are sequential and deterministic
//! given the request and admitted resources, independent of thread count and
//! allocation addresses.
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
    /// Its assignment was judged earlier in this walk: the earlier verdict
    /// stands, and it is not formed again (the walk's memo).
    Recalled,
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
    /// The audit lane's exact delta of a pruned move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit: Option<i64>,
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
    /// The audit lane's exact delta of a pruned move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit: Option<i64>,
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
    /// The audit lane's exact delta of a pruned move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit: Option<i64>,
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
    /// The level-0 artifact's size under the codec, the structural search's
    /// winner's (the level-0 artifact's where no search runs), and the
    /// delivered one's.
    pub before: usize,
    pub searched: usize,
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
    /// Heads formed: one for each start walked in formations of its own,
    /// and the head with the other `int32-hints` value wherever a
    /// challenger asked for it.
    pub heads: usize,
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
    /// Every start the objective walked, in order (AM2): the structural
    /// search's winner, the level-0 artifact, and the level-0 artifact under
    /// each other naming seed (a restart).
    pub starts: Vec<StartTrial>,
    /// Where each lower one-pass level would have stopped, as the walk from
    /// the level-0 artifact passed it (the replay check, §9.6).
    pub stops: Vec<Stop>,
}

/// A lower level's stopping point on the walk from the level-0 artifact:
/// that level's build delivers exactly these bytes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Stop {
    pub level: u8,
    pub size: usize,
    /// The delivered text's SHA-256: one file's, or for several files, the
    /// digest of their names and digests in plan order.
    pub sha256: String,
}

/// A walk's move renders within the larger of this and the level-0
/// artifact's capacity. It does not depend on the level, so neither does
/// the walk.
const RENDER_BOUND: usize = 768 * 1024;

/// The digest a stop records of an artifact's delivered files.
fn delivered_digest(view: &ArtifactView<'_>) -> String {
    use sha2::{Digest, Sha256};
    if view.files.is_empty() {
        return format!("{:x}", Sha256::digest(view.javascript.as_bytes()));
    }
    let mut all = String::new();
    for file in view.files {
        all.push_str(&file.name);
        all.push('\0');
        all.push_str(&format!("{:x}", Sha256::digest(file.code.as_bytes())));
        all.push('\n');
    }
    format!("{:x}", Sha256::digest(all.as_bytes()))
}

/// The lower one-pass levels a walk from the level-0 artifact passes, with
/// that walk's counts when it began.
struct Replay {
    pending: Vec<(u8, crate::compilation_policy::WalkSchedule)>,
    examined: usize,
    judged: usize,
}

/// One start of an objective's walks: `search` (the structural search's
/// winner), `level-0`, `naming:<seed>` (the level-0 artifact under another
/// naming seed), `local-naming` (the selected result's final refinements),
/// or `deferred-naming:<seed>` (a pruned seed revisited after those refinements).
/// Its walk runs in passes from `pass` on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StartTrial {
    pub name: String,
    /// The first pass of its walk.
    pub pass: usize,
    /// Kept: its walk's result replaced the objective's winner; rejected: it
    /// did not; pruned, identical, refused or budget: a restart's start was
    /// not walked.
    pub outcome: ChallengerOutcome,
    /// Its start's size under the objective's codec, when it has one.
    pub start: Option<usize>,
    /// Its walk's result, and that minus the winner's at the time.
    pub size: Option<usize>,
    pub delta: Option<i64>,
    /// A restart's proxy delta against the level-0 artifact.
    pub proxy: Option<i64>,
    /// The audit lane's exact delta of a pruned move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit: Option<i64>,
}

/// Every requested objective's walks, bounded by the level's schedule.
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
        (spelling.self_named, Challenger::SelfNamed),
        (spelling.read_order, Challenger::ReadOrder),
        (
            families.compound_assignments,
            Challenger::CompoundAssignments,
        ),
        (families.quotes, Challenger::Quotes),
        (families.loop_heads, Challenger::LoopHeads),
        (families.logical_statements, Challenger::LogicalStatements),
        (families.block_inlining, Challenger::BlockInlining),
        (families.flat_blocks, Challenger::FlatBlocks),
        (families.string_pooling, Challenger::StringPooling),
        (
            families.string_array_packing,
            Challenger::StringArrayPacking,
        ),
        (
            statements.conditional_returns,
            Challenger::ConditionalReturns,
        ),
        (statements.logical_branches, Challenger::LogicalBranches),
        (families.int32_hints, Challenger::Int32Hints),
        (families.string_constants, Challenger::StringConstants),
    ]
    .into_iter()
    .filter_map(|(on, challenger)| on.then_some(challenger.name()))
    .chain(families.property_mangling.then_some("property-mangling"))
    .collect()
}

/// A resource refusal ends the stage; any other refusal ends one challenger.
fn exhausted(error: &CandidateError) -> bool {
    matches!(
        error,
        CandidateError::Budget(
            BudgetError::WorkExhausted(_)
                | BudgetError::MemoryExhausted(_)
                | BudgetError::DeadlineExceeded
        )
    )
}

/// A broken allocation/analysis owner is a compiler error, never an ordinary
/// search stop or an inapplicable representation. Keep these failures visible.
fn refusal(error: CandidateError) -> Result<Judgement, SearchError> {
    if exhausted(&error) {
        Ok(Judgement::Stopped)
    } else if matches!(error, CandidateError::Budget(_)) {
        Err(error.into())
    } else {
        Ok(Judgement::Refused)
    }
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
    /// The walk's memo holds the assignment's earlier verdict: not formed
    /// again. Its size, when it was measured or matched an incumbent.
    Recalled {
        size: Option<usize>,
    },
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
    /// Explicit policy for proxy rejection and its diagnostic exact scores.
    proxy_pruning: crate::compilation_policy::ProxyPruning,
}

/// The proxy judge's reading of one move: its delta against the reference
/// and its own proxy size, which the walk's memo keeps.
#[derive(Debug, Clone, Copy)]
struct Proxy {
    delta: i64,
    size: usize,
    /// With proxy auditing, a pruned move's exact delta against the
    /// reference: a negative one is a potential miss, still subject to the
    /// artifact's admission constraints before it could be a winner.
    audit: Option<i64>,
}

impl Judge<'_> {
    /// Form the candidate under `spelling` and `choices` and render it under
    /// `plan` with `literals`; then judge it against `reference`: identical
    /// bytes are `Identical`, and a proxy worse by more than the margin is
    /// `Pruned`. Otherwise the objective's exact codec measures it and
    /// admission qualifies it. Returns the retained artifact with its exact
    /// size and qualification, the proxy reading, and whether an exact codec
    /// score was spent; on any other verdict the artifact is discarded.
    fn measure(
        &self,
        formations: &mut Formations<'_, '_>,
        spelling: Spelling,
        choices: &ChoiceMap,
        plan: &Plan,
        literals: crate::js::LiteralOutput,
        reference: &Incumbent,
        allow_pruning: bool,
    ) -> Result<
        (
            Result<(ArtifactId, usize, QualifiedArtifact), Judgement>,
            Option<Proxy>,
            bool,
        ),
        SearchError,
    > {
        let Self {
            policy,
            codec,
            output,
            available,
            margin,
            baseline,
            proxy_pruning,
        } = *self;
        let pruning = allow_pruning && proxy_pruning != crate::compilation_policy::ProxyPruning::Off;
        let audit = allow_pruning && proxy_pruning == crate::compilation_policy::ProxyPruning::Audit;
        let tactics = OutputTactics {
            families: spelling.families,
            choices: choices.clone(),
            literals,
            ..output.clone()
        };
        let named = Plan {
            style: plan.style,
            alphabet: plan.alphabet,
            source_names: plan.source_names.clone(),
            self_named: spelling.self_named,
            read_order: spelling.read_order,
            local_read_order: plan.local_read_order,
        };
        let rendered = formations
            .form(tactics, |target| {
                let staged = target.render_bounded_with_literals(&named, literals, available)?;
                target.retain_artifact(staged)
            })
            .and_then(|retained| retained);
        let challenged = match rendered {
            Ok(challenged) => challenged,
            Err(error) => return Ok((Err(refusal(error)?), None, false)),
        };
        // The proxy judges first: a move clearly worse than the reference is
        // pruned before the exact codec runs. Where the proxy is the exact
        // codec (gzip, a delivery plan's files), its measurement is the
        // move's exact probe.
        let exact_proxy = codec != Objective::Raw;
        let proxied = formations.with_arena(|arena, _, budget| {
            let result = (|| -> Result<Option<(Proxy, bool)>, CandidateError> {
                if arena.same_output(challenged, reference.artifact, budget)? {
                    return Ok(None);
                }
                let exact = exact_proxy && arena.proxy_is_exact(challenged, codec)?;
                let challenger = arena.measure_proxy(challenged, codec, budget)?;
                let held = arena.measure_proxy(reference.artifact, codec, budget)?;
                let mut proxy = Proxy {
                    delta: challenger as i64 - held as i64,
                    size: challenger,
                    audit: None,
                };
                if audit && proxy.delta > margin {
                    let exact = arena.measure(challenged, codec, budget)?;
                    proxy.audit = Some(exact as i64 - reference.size as i64);
                }
                let probed = exact || (codec != Objective::Raw && proxy.audit.is_some());
                Ok(Some((proxy, probed)))
            })();
            let prune = match &result {
                Ok(Some((proxy, _))) => pruning && proxy.delta > margin,
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
            Ok(Some((proxy, exact))) if pruning && proxy.delta > margin => {
                return Ok((Err(Judgement::Pruned), Some(proxy), exact))
            }
            Ok(Some((proxy, _))) => Some(proxy),
            Ok(None) => return Ok((Err(Judgement::Identical), None, false)),
            Err(error) => return Ok((Err(refusal(error)?), None, false)),
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
            Err(error) => Ok((Err(refusal(error)?), proxy, probed)),
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
        held: (ArtifactId, usize),
    ) -> Result<bool, SearchError> {
        let codec = self.codec;
        let (held, held_size) = held;
        let rows = formations.with_arena(|arena, _, budget| {
            Ok::<_, CandidateError>((
                arena.rows(challenger, codec, budget)?,
                arena.rows(held, codec, budget)?,
            ))
        })?;
        let base = self
            .baseline
            .map_or(qualified.cost(), |baseline| baseline.cost());
        let order = self
            .policy
            .compare_evidence(
                qualified.cost(),
                CandidateCostEvidence::size_only(held_size as u64),
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
    /// Returns the verdict, the proxy reading and whether an exact codec
    /// score was spent.
    fn judge(
        &self,
        formations: &mut Formations<'_, '_>,
        spelling: Spelling,
        choices: &ChoiceMap,
        plan: &Plan,
        literals: crate::js::LiteralOutput,
        incumbent: &Incumbent,
    ) -> Result<(Judgement, Option<Proxy>, bool), SearchError> {
        let (measured, proxy, probed) =
            self.measure(formations, spelling, choices, plan, literals, incumbent, true)?;
        let (challenged, size, qualified) = match measured {
            Ok(measured) => measured,
            Err(judgement) => return Ok((judgement, proxy, probed)),
        };
        let wins = self.wins(
            formations,
            challenged,
            qualified,
            (incumbent.artifact, incumbent.size),
        );
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

/// What the walk's memo holds of one assignment it judged.
#[derive(Debug, Clone, Copy)]
enum Recall {
    /// Measured exactly at this size.
    Measured(usize),
    /// Pruned; the proxy size of its artifact.
    Pruned(usize),
    /// It rendered the bytes of an incumbent of this size.
    Identical(usize),
    Refused,
}

/// One assignment the walk forms: the output families and raw spelling, the
/// choice sites' alternatives, the naming plan and the literal spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Assignment {
    spelling: Spelling,
    choices: ChoiceMap,
    style: Style,
    alphabet: crate::js::selection::Alphabet,
    source_names: Vec<crate::js::BindingId>,
    local_read_order: bool,
    literals: crate::js::LiteralOutput,
}

/// One start's walks inside its candidate's formations (architecture §9.6,
/// AM2): the list walked in passes from an incumbent, restarts from the
/// same candidate, and the settlement of each result against the
/// objective's winner, with the walk's counts in its report.
struct Walker<'w, 'scope, 'src> {
    formations: &'w mut Formations<'scope, 'src>,
    portfolio: &'w mut Portfolio,
    judge: &'w Judge<'w>,
    report: &'w mut TerminalObjective,
    walk: crate::compilation_policy::WalkSchedule,
    codec: Objective,
    /// The source state of the start's candidate, which a promoted result
    /// belongs to.
    state: usize,
    output: &'w OutputTactics,
    /// The naming seeds the policy permits.
    naming: &'w [Style],
    choices_permitted: bool,
    /// The compilation's resources ran out: every later position is
    /// `Stopped`.
    stopped: bool,
    /// Every assignment this start's walks judged, with its verdict. Sizes
    /// only fall along a walk, so a measured assignment that was not smaller
    /// then is not smaller now; a repeat is not formed again.
    memo: Vec<(Assignment, Recall)>,
    /// On the walk from the level-0 artifact: the lower levels whose
    /// stopping points it records.
    replay: Option<Replay>,
}

impl Walker<'_, '_, '_> {
    /// Whether the walk may examine one more position.
    fn open(&self) -> bool {
        self.report.examined < self.walk.prefix && self.report.judged < self.walk.exact
    }

    /// Record the stopping point of every lower level whose walk would not
    /// examine the next position: its prefix or exact budget is spent.
    fn replay(&mut self, incumbent: &Incumbent) -> Result<(), SearchError> {
        let Some(replay) = &self.replay else {
            return Ok(());
        };
        let examined = self.report.examined - replay.examined;
        let judged = self.report.judged - replay.judged;
        let stopping: Vec<u8> = replay
            .pending
            .iter()
            .filter(|(_, schedule)| examined >= schedule.prefix || judged >= schedule.exact)
            .map(|&(level, _)| level)
            .collect();
        self.record_stops(&stopping, incumbent)
    }

    /// The walk from the level-0 artifact finished its first pass: every
    /// lower one-pass level still pending stops here.
    fn finish_replay(&mut self, incumbent: &Incumbent) -> Result<(), SearchError> {
        let Some(replay) = &self.replay else {
            return Ok(());
        };
        let levels: Vec<u8> = replay.pending.iter().map(|&(level, _)| level).collect();
        self.record_stops(&levels, incumbent)?;
        self.replay = None;
        Ok(())
    }

    fn record_stops(&mut self, levels: &[u8], incumbent: &Incumbent) -> Result<(), SearchError> {
        if levels.is_empty() {
            return Ok(());
        }
        let sha256 = self.formations.with_arena(|arena, _, _| {
            arena.with_artifact(incumbent.artifact, |view| delivered_digest(&view))
        })?;
        for &level in levels {
            self.report.stops.push(Stop {
                level,
                size: incumbent.size,
                sha256: sha256.clone(),
            });
        }
        if let Some(replay) = &mut self.replay {
            replay.pending.retain(|(level, _)| !levels.contains(level));
        }
        Ok(())
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
    /// an artifact the portfolio holds.
    fn discard(&mut self, incumbent: Incumbent) {
        if incumbent.qualified.is_some() {
            self.formations.with_arena(|arena, _, budget| {
                arena
                    .discard(incumbent.artifact, budget)
                    .expect("the walk owns its incumbents")
            });
        }
    }

    /// Judge one move from `incumbent`, through the memo: a repeated
    /// assignment keeps its earlier verdict without being formed, unless it
    /// was measured smaller than the incumbent (a dominance rejection) or
    /// its recorded proxy no longer exceeds the margin.
    fn judge_move(
        &mut self,
        spelling: Spelling,
        choices: &ChoiceMap,
        plan: &Plan,
        literals: crate::js::LiteralOutput,
        incumbent: &Incumbent,
    ) -> Result<(Judgement, Option<Proxy>, bool), SearchError> {
        let key = Assignment {
            spelling: spelling.effective(),
            choices: choices.clone(),
            style: plan.style,
            alphabet: plan.alphabet,
            source_names: plan.source_names.clone(),
            literals,
            local_read_order: plan.local_read_order,
        };
        let recalled = self
            .memo
            .iter()
            .find(|(assignment, _)| *assignment == key)
            .map(|&(_, recall)| recall);
        match recalled {
            Some(Recall::Measured(size) | Recall::Identical(size)) if size >= incumbent.size => {
                return Ok((Judgement::Recalled { size: Some(size) }, None, false));
            }
            Some(Recall::Refused) => {
                return Ok((Judgement::Recalled { size: None }, None, false));
            }
            Some(Recall::Pruned(size)) => {
                let codec = self.codec;
                let held = self.formations.with_arena(|arena, _, budget| {
                    arena.measure_proxy(incumbent.artifact, codec, budget)
                })?;
                let delta = size as i64 - held as i64;
                if delta > self.judge.margin {
                    let proxy = Proxy {
                        delta,
                        size,
                        audit: None,
                    };
                    return Ok((Judgement::Recalled { size: None }, Some(proxy), false));
                }
            }
            _ => {}
        }
        let (judgement, proxy, probed) = self.judge.judge(
            self.formations,
            spelling,
            choices,
            plan,
            literals,
            incumbent,
        )?;
        let recall = match &judgement {
            Judgement::Kept { size, .. } | Judgement::Rejected { size } => {
                Some(Recall::Measured(*size))
            }
            Judgement::Pruned => proxy.map(|proxy| Recall::Pruned(proxy.size)),
            Judgement::Identical => Some(Recall::Identical(incumbent.size)),
            Judgement::Refused => Some(Recall::Refused),
            Judgement::Stopped | Judgement::Recalled { .. } => None,
        };
        if let Some(recall) = recall {
            match self
                .memo
                .iter_mut()
                .find(|(assignment, _)| *assignment == key)
            {
                Some(entry) => entry.1 = recall,
                None => self.memo.push((key, recall)),
            }
        }
        Ok((judgement, proxy, probed))
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
                | self.joint_moves(incumbent, pass, JointPhase::Ordinary)?;
            if passes == 1 {
                self.finish_replay(incumbent)?;
            }
            if !kept {
                break;
            }
        }
        // A walk whose budget closed before its first pass began stops at
        // its start.
        self.finish_replay(incumbent)
    }

    /// The choice moves (M9.1): the incumbent's choice sites, surveyed, in
    /// the choice schedule. Whether one was kept.
    fn choice_moves(
        &mut self,
        incumbent: &mut Incumbent,
        pass: usize,
    ) -> Result<bool, SearchError> {
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
            Err(error) => {
                self.stopped |= matches!(refusal(error)?, Judgement::Stopped);
                return Ok(false);
            }
        };
        let mut kept = false;
        for moves in choice_schedule(&sites) {
            self.replay(incumbent)?;
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
                audit: None,
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
                let choices = moved.iter().fold(
                    incumbent.choices.clone(),
                    |choices, &(site, alternative)| choices.with(sites[site].key, alternative),
                );
                let (judgement, proxy, probed) = self.judge_move(
                    incumbent.spelling,
                    &choices,
                    &incumbent.plan,
                    incumbent.literals,
                    incumbent,
                )?;
                self.count(&judgement, probed);
                if !matches!(judgement, Judgement::Recalled { .. } | Judgement::Stopped) {
                    self.report.choices_tried += 1;
                }
                record.proxy = proxy.map(|proxy| proxy.delta);
                record.audit = proxy.and_then(|proxy| proxy.audit);
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
                    Judgement::Recalled { size } => {
                        record.outcome = ChallengerOutcome::Recalled;
                        record.size = size;
                        record.delta = size.map(|size| size as i64 - incumbent.size as i64);
                    }
                    Judgement::Pruned => record.outcome = ChallengerOutcome::Pruned,
                    Judgement::Identical => record.outcome = ChallengerOutcome::Identical,
                    Judgement::Refused => record.outcome = ChallengerOutcome::Refused,
                    Judgement::Stopped => {
                        self.stopped = true;
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
            audit: None,
        };
        let mut kept = false;
        let mut seen = vec![incumbent.spelling.effective()];
        for challenger in Challenger::ORDER {
            self.replay(incumbent)?;
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
            let mut next = challenger.apply(self.codec, incumbent.spelling);
            if challenger == Challenger::OtherSeed && self.output.target_compaction {
                next.families = next.families.permitted(self.judge.policy);
            }
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
            // A family that prints nothing in this candidate prints the
            // incumbent's program: nothing to form.
            if matches!(challenger, Challenger::Int32Hints) && self.formations.hints_inert() {
                self.report
                    .trials
                    .push(trial(challenger, ChallengerOutcome::Duplicate));
                continue;
            }
            let (judgement, proxy, probed) = self.judge_move(
                next,
                &incumbent.choices,
                &incumbent.plan,
                incumbent.literals,
                incumbent,
            )?;
            self.count(&judgement, probed);
            if !matches!(judgement, Judgement::Recalled { .. } | Judgement::Stopped) {
                self.report.tried += 1;
            }
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
                Judgement::Recalled { size } => (
                    ChallengerOutcome::Recalled,
                    size,
                    size.map(|size| size as i64 - incumbent.size as i64),
                ),
                Judgement::Pruned => (ChallengerOutcome::Pruned, None, None),
                Judgement::Identical => (ChallengerOutcome::Identical, None, None),
                Judgement::Refused => (ChallengerOutcome::Refused, None, None),
                Judgement::Stopped => {
                    self.stopped = true;
                    (ChallengerOutcome::Stopped, None, None)
                }
            };
            self.report.trials.push(ChallengerTrial {
                challenger: challenger.name(),
                pass,
                outcome,
                size,
                delta,
                proxy: proxy.map(|proxy| proxy.delta),
                audit: proxy.and_then(|proxy| proxy.audit),
            });
        }
        Ok(kept)
    }

    /// The joint moves, after every site move (architecture §9.5): the other
    /// literal spelling (only target compaction may observe weak literals),
    /// then each naming seed the policy permits other than the incumbent's.
    /// Whether one was kept.
    fn joint_moves(
        &mut self,
        incumbent: &mut Incumbent,
        pass: usize,
        phase: JointPhase,
    ) -> Result<bool, SearchError> {
        use crate::js::LiteralOutput;
        let other = match incumbent.literals {
            LiteralOutput::Original => LiteralOutput::Observed,
            LiteralOutput::Observed => LiteralOutput::Original,
        };
        #[derive(Clone, Copy)]
        enum Joint {
            Literals,
            Style(Style),
            SequentialAlphabet,
            ObservedAlphabet,
            LocalReadOrder,
            PrivateProperties,
        }
        let mut joints: Vec<(String, Joint)> = Vec::new();
        if matches!(phase, JointPhase::LocalBindings) {
            joints.push(("naming:local-read-order".into(), Joint::LocalReadOrder));
        } else if matches!(phase, JointPhase::PrivateProperties) {
            joints.push(("properties:private-fields".into(), Joint::PrivateProperties));
        } else {
            if other == LiteralOutput::Original || self.output.target_compaction {
                joints.push((format!("literals:{other:?}"), Joint::Literals));
            }
            for &style in self.naming {
                if style != incumbent.plan.style {
                    joints.push((format!("naming:{style:?}"), Joint::Style(style)));
                }
            }
            joints.push(("alphabet:sequential".into(), Joint::SequentialAlphabet));
            joints.push(("alphabet:frequency".into(), Joint::ObservedAlphabet));
        }
        let mut kept = false;
        for (name, joint) in joints {
            self.replay(incumbent)?;
            let mut record = JointTrial {
                name,
                pass,
                outcome: ChallengerOutcome::Budget,
                size: None,
                delta: None,
                proxy: None,
                audit: None,
            };
            if self.stopped {
                record.outcome = ChallengerOutcome::Stopped;
            } else if self.open() {
                self.report.examined += 1;
                // A naming move keeps the incumbent's literal spelling, which
                // the literal move may just have changed.
                let literals = if matches!(joint, Joint::Literals) {
                    other
                } else {
                    incumbent.literals
                };
                let mut plan = incumbent.plan.clone();
                let mut spelling = incumbent.spelling;
                match joint {
                    Joint::Literals => (),
                    Joint::Style(style) => {
                        plan.style = style;
                        plan.source_names.clear();
                        if style != Style::Scoped {
                            plan.local_read_order = false;
                        }
                    }
                    Joint::PrivateProperties => {
                        if !self.judge.policy.tactic(TacticId::PropertyMangling).enabled
                            || !self.output.target_compaction
                        {
                            record.outcome = ChallengerOutcome::Vetoed;
                            self.report.joint_trials.push(record);
                            continue;
                        }
                        if self.formations.properties_inert() {
                            record.outcome = ChallengerOutcome::Duplicate;
                            self.report.joint_trials.push(record);
                            continue;
                        }
                        spelling.families.property_mangling ^= true;
                    }
                    Joint::LocalReadOrder => {
                        let policy = self.judge.policy;
                        if !policy.tactic(TacticId::IdentifierMangling).enabled
                            || !policy.tactic(TacticId::NamingSearch).enabled
                        {
                            record.outcome = ChallengerOutcome::Vetoed;
                            self.report.joint_trials.push(record);
                            continue;
                        }
                        plan.local_read_order =
                            plan.style != Style::Scoped || !plan.local_read_order;
                        plan.style = Style::Scoped;
                    }
                    Joint::SequentialAlphabet | Joint::ObservedAlphabet => {
                        let policy = self.judge.policy;
                        if !policy.tactic(TacticId::IdentifierMangling).enabled
                            || !policy.tactic(TacticId::NamingSearch).enabled
                            || !policy.tactic(TacticId::NamingAlphabet).enabled
                        {
                            record.outcome = ChallengerOutcome::Vetoed;
                            self.report.joint_trials.push(record);
                            continue;
                        }
                        use crate::js::selection::Alphabet;
                        let alphabet = match joint {
                            Joint::SequentialAlphabet => Ok(Alphabet::default()),
                            _ => self.formations.with_arena(|arena, _, budget| {
                                arena
                                    .with_artifact(incumbent.artifact, |view| {
                                        Alphabet::observed(
                                            std::iter::once(view.javascript.as_bytes()).chain(
                                                view.files.iter().map(|file| file.code.as_bytes()),
                                            ),
                                            budget,
                                        )
                                    })?
                                    .map_err(CandidateError::from)
                            }),
                        };
                        plan.alphabet = match alphabet {
                            Ok(alphabet) => alphabet,
                            Err(error) => {
                                record.outcome = if matches!(refusal(error)?, Judgement::Stopped) {
                                    self.stopped = true;
                                    ChallengerOutcome::Stopped
                                } else {
                                    ChallengerOutcome::Refused
                                };
                                self.report.joint_trials.push(record);
                                continue;
                            }
                        };
                        if plan.alphabet == incumbent.plan.alphabet {
                            record.outcome = ChallengerOutcome::Duplicate;
                            self.report.joint_trials.push(record);
                            continue;
                        }
                    }
                }
                plan.self_named = incumbent.spelling.self_named;
                plan.read_order = incumbent.spelling.read_order;
                let (judgement, proxy, probed) =
                    self.judge_move(spelling, &incumbent.choices, &plan, literals, incumbent)?;
                self.count(&judgement, probed);
                if !matches!(judgement, Judgement::Recalled { .. } | Judgement::Stopped) {
                    self.report.joints_tried += 1;
                }
                record.proxy = proxy.map(|proxy| proxy.delta);
                record.audit = proxy.and_then(|proxy| proxy.audit);
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
                            spelling,
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
                    Judgement::Recalled { size } => {
                        record.size = size;
                        record.delta = size.map(|size| size as i64 - incumbent.size as i64);
                        ChallengerOutcome::Recalled
                    }
                    Judgement::Pruned => ChallengerOutcome::Pruned,
                    Judgement::Identical => ChallengerOutcome::Identical,
                    Judgement::Refused => ChallengerOutcome::Refused,
                    Judgement::Stopped => {
                        self.stopped = true;
                        ChallengerOutcome::Stopped
                    }
                };
            }
            self.report.joint_trials.push(record);
        }
        Ok(kept)
    }

    /// Settle a start's result against the objective's winner (the
    /// portfolio's selected entry): a result the walk retains replaces the
    /// winner on a strict exact win and is discarded otherwise. The start's
    /// record, `starts[index]`, takes the verdict.
    fn settle(&mut self, slot: usize, result: Incumbent) -> Result<(), SearchError> {
        let codec = self.codec;
        let winner =
            self.portfolio.selected[index(codec)].expect("an objective walked has a winner");
        let held = self.portfolio.entries.get(winner).unwrap().artifact;
        let held_size = self
            .formations
            .with_arena(|arena, _, _| arena.with_artifact(held, |view| view.sizes.get(codec)))?
            .expect("a winner is measured under its codec");
        let delta = result.size as i64 - held_size as i64;
        let outcome = match result.qualified {
            // The start itself, unchanged: the portfolio holds it.
            None if result.artifact == held => ChallengerOutcome::Identical,
            None => ChallengerOutcome::Rejected,
            Some(qualified) => {
                let wins = self.judge.wins(
                    self.formations,
                    result.artifact,
                    qualified,
                    (held, held_size),
                );
                let promoted = match wins {
                    Ok(true) => {
                        let (portfolio, state) = (&mut *self.portfolio, self.state);
                        self.formations.with_arena(|arena, _, budget| {
                            portfolio
                                .promote_terminal(
                                    arena,
                                    budget,
                                    codec,
                                    state,
                                    result.artifact,
                                    qualified,
                                )
                                .map(|_| true)
                        })
                    }
                    other => other,
                };
                match promoted {
                    Ok(true) => ChallengerOutcome::Kept,
                    Ok(false) => {
                        self.discard(result.clone());
                        ChallengerOutcome::Rejected
                    }
                    Err(error) if error.optional_memory_refusal() || resource(&error) => {
                        self.stopped = true;
                        self.discard(result.clone());
                        ChallengerOutcome::Stopped
                    }
                    Err(error) => {
                        self.discard(result.clone());
                        return Err(error);
                    }
                }
            }
        };
        let record = &mut self.report.starts[slot];
        record.outcome = outcome;
        record.size = Some(result.size);
        record.delta = Some(delta);
        Ok(())
    }

    /// Walk from a start the portfolio holds, then settle its result.
    fn walk_start(
        &mut self,
        name: &str,
        start: Incumbent,
        local_naming: bool,
    ) -> Result<(), SearchError> {
        let slot = self.report.starts.len();
        self.report.starts.push(StartTrial {
            name: name.to_string(),
            pass: self.report.passes + 1,
            outcome: ChallengerOutcome::Budget,
            start: Some(start.size),
            size: None,
            delta: None,
            proxy: None,
            audit: None,
        });
        let mut result = start;
        if local_naming {
            self.polish(&mut result)?;
        } else {
            self.passes(&mut result)?;
        }
        self.settle(slot, result)
    }

    /// The same final refinements serve the selected winner and a deferred
    /// seed. Every kept choice is measured under this objective; permissions
    /// and resource refusal are enforced by the ordinary joint-move owner.
    fn polish(&mut self, result: &mut Incumbent) -> Result<(), SearchError> {
        self.report.passes += 1;
        if self.joint_moves(result, self.report.passes, JointPhase::LocalBindings)? {
            self.passes(result)?;
        }
        #[cfg(test)]
        let properties = !SKIP_PROPERTY_POLISH.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let properties = true;
        if properties
            && self.joint_moves(result, self.report.passes, JointPhase::PrivateProperties)?
        {
            self.passes(result)?;
        }
        Ok(())
    }

    /// A restart (AM2): `origin` under another naming seed, walked in passes
    /// of its own and settled like any start. Its start is judged by the
    /// proxy against `origin` and then measured exactly. A pass-by-pass walk
    /// cannot reach an assignment whose naming loses alone and wins with the
    /// families it enables. Deferred starts bypass only this initial proxy
    /// rejection, after the earlier search has completed. Returns whether
    /// the initial proxy rejected the start, so it can be deferred once.
    fn restart(
        &mut self,
        origin: &Incumbent,
        style: Style,
        deferred: bool,
    ) -> Result<bool, SearchError> {
        let mut record = StartTrial {
            name: if deferred {
                format!("deferred-naming:{style:?}")
            } else {
                format!("naming:{style:?}")
            },
            pass: self.report.passes + 1,
            outcome: ChallengerOutcome::Budget,
            start: None,
            size: None,
            delta: None,
            proxy: None,
            audit: None,
        };
        if self.stopped {
            record.outcome = ChallengerOutcome::Stopped;
            self.report.starts.push(record);
            return Ok(false);
        }
        if !self.open() {
            self.report.starts.push(record);
            return Ok(false);
        }
        self.report.examined += 1;
        self.report.restarts_tried += 1;
        let plan = Plan {
            style,
            alphabet: origin.plan.alphabet,
            source_names: Vec::new(),
            self_named: origin.spelling.self_named,
            read_order: origin.spelling.read_order,
            local_read_order: style == Style::Scoped && origin.plan.local_read_order,
        };
        let (measured, proxy, probed) = self.judge.measure(
            self.formations,
            origin.spelling,
            &origin.choices,
            &plan,
            origin.literals,
            origin,
            !deferred,
        )?;
        self.report.codec_probes += usize::from(probed);
        record.proxy = proxy.map(|proxy| proxy.delta);
        record.audit = proxy.and_then(|proxy| proxy.audit);
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
                self.report.starts.push(record);
                return Ok(matches!(judgement, Judgement::Pruned));
            }
        };
        // The start's exact measurement is the restart's judgement.
        self.report.judged += 1;
        record.start = Some(size);
        let slot = self.report.starts.len();
        self.report.starts.push(record);
        let mut result = Incumbent {
            artifact,
            plan,
            size,
            qualified: Some(qualified),
            ..origin.clone()
        };
        self.passes(&mut result)?;
        if deferred
            && self
                .judge
                .policy
                .objective()
                .expect("JavaScript objective")
                .search
                .deferred_naming_polish
            && !self.stopped
            && self.open()
        {
            self.polish(&mut result)?;
        }
        self.settle(slot, result)?;
        Ok(false)
    }
}

#[derive(Clone, Copy)]
enum JointPhase {
    Ordinary,
    LocalBindings,
    PrivateProperties,
}

#[derive(Clone, Copy)]
enum WalkPhase {
    Search {
        restarts: bool,
    },
    /// Extend the selected result after the earlier walks have converged.
    /// A losing trial cannot redirect their useful trajectories.
    LocalNaming,
    /// Revisit only starts actually pruned by the protected prefix.
    DeferredNaming(NamingStarts),
}

/// Fixed storage: no allocation or dependency on diagnostic start names.
#[derive(Clone, Copy, Default)]
struct NamingStarts {
    global: bool,
    scoped: bool,
    source: bool,
}

impl NamingStarts {
    fn insert(&mut self, style: Style) {
        match style {
            Style::Global => self.global = true,
            Style::Scoped => self.scoped = true,
            Style::Source => self.source = true,
        }
    }

    fn contains(self, style: Style) -> bool {
        match style {
            Style::Global => self.global,
            Style::Scoped => self.scoped,
            Style::Source => self.source,
        }
    }

    fn is_empty(self) -> bool {
        !self.global && !self.scoped && !self.source
    }
}

#[cfg(test)]
thread_local! {
    static SKIP_LOCAL_POLISH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SKIP_PROPERTY_POLISH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn without_property_polish<T>(run: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            SKIP_PROPERTY_POLISH.with(|value| value.set(self.0));
        }
    }
    let _reset = Reset(SKIP_PROPERTY_POLISH.with(|value| value.replace(true)));
    run()
}

#[cfg(test)]
pub(crate) fn without_local_polish<T>(run: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            SKIP_LOCAL_POLISH.with(|flag| flag.set(self.0));
        }
    }
    let _reset = Reset(SKIP_LOCAL_POLISH.with(|flag| flag.replace(true)));
    run()
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
    /// Walk every requested objective from its starts (AM2), once; later
    /// calls return the same report. The winners it replaces are the ones
    /// `winner_qualification` and `take_winner` deliver. A resource refusal
    /// keeps the winner and is reported, never an error: the search's
    /// admitted winners stay valid.
    pub fn challenge(
        &mut self,
        policy: &ResolvedPolicy,
        request: SearchRequest,
    ) -> Result<&TerminalReport, SearchError> {
        if self.terminal.is_none() {
            let objective = policy.objective().ok_or(CandidateError::NotJavaScript)?;
            let mut report = TerminalReport::default();
            let walked = request.objectives.iter().try_for_each(|codec| {
                if let Some(stage) = self.walk_objective(policy, objective, codec)? {
                    report.objectives.push(stage);
                }
                Ok::<_, SearchError>(())
            });
            // The pinned level-0 artifact goes once every walk has run.
            let mut budget =
                AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
            self.portfolio
                .unpin(&mut self.compilation.artifacts, &mut budget);
            walked?;
            self.terminal = Some(report);
        }
        Ok(self.terminal.as_ref().unwrap())
    }

    /// The walk's report, once `challenge` has run.
    pub fn terminal_report(&self) -> Option<&TerminalReport> {
        self.terminal.as_ref()
    }

    /// One objective's walks: from the structural search's winner, when
    /// the search ran and selected something other than the level-0
    /// artifact, then from the level-0 artifact with its restarts.
    fn walk_objective(
        &mut self,
        policy: &ResolvedPolicy,
        objective: OptimizationObjective,
        codec: Objective,
    ) -> Result<Option<TerminalObjective>, SearchError> {
        let Some(winner) = self.portfolio.selected[index(codec)] else {
            return Ok(None);
        };
        // Without a walk the level-0 artifact is delivered as it is, and no
        // codec measured it (M3.5).
        if objective.walk.prefix == 0 {
            return Ok(None);
        }
        let level0 = self.portfolio.pinned.unwrap_or(winner);
        let size = |search: &Self, position: usize| -> Result<usize, SearchError> {
            let artifact = search.portfolio.entries.get(position).unwrap().artifact;
            Ok(search
                .compilation
                .artifacts
                .with_artifact(artifact, |view| view.sizes.get(codec))?
                .expect("a selected entry is measured under its codec"))
        };
        let before = size(self, level0)?;
        let searched = size(self, winner)?;
        let walk = objective.walk;
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
            searched,
            after: searched,
            spelling: Vec::new(),
            style: String::new(),
            trials: Vec::with_capacity(Challenger::ORDER.len()),
            choices_tried: 0,
            choices_scored: 0,
            surveys: 0,
            heads: 0,
            choice_trials: Vec::new(),
            choices: Vec::new(),
            joints_tried: 0,
            joint_trials: Vec::new(),
            restarts_tried: 0,
            starts: Vec::new(),
            stops: Vec::new(),
        };
        if level0 != winner {
            self.walk_from(
                policy,
                objective,
                codec,
                "search",
                winner,
                WalkPhase::Search { restarts: false },
                &mut report,
            )?;
        }
        let deferred = self.walk_from(
            policy,
            objective,
            codec,
            "level-0",
            level0,
            WalkPhase::Search {
                restarts: walk.starts,
            },
            &mut report,
        )?;
        #[cfg(test)]
        let polish = !SKIP_LOCAL_POLISH.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let polish = true;
        // Fast tiers keep the existing prefix. The final naming start belongs
        // to the default-and-higher schedule together with its restarts.
        if polish && walk.starts {
            let selected =
                self.portfolio.selected[index(codec)].expect("an objective keeps its winner");
            self.walk_from(
                policy,
                objective,
                codec,
                "local-naming",
                selected,
                WalkPhase::LocalNaming,
                &mut report,
            )?;
        }
        if policy.deferred_naming_starts_enabled() && !deferred.is_empty() {
            self.walk_from(
                policy,
                objective,
                codec,
                "deferred-naming",
                level0,
                WalkPhase::DeferredNaming(deferred),
                &mut report,
            )?;
        }
        let delivered =
            self.portfolio.selected[index(codec)].expect("an objective keeps its winner");
        report.after = size(self, delivered)?;
        let artifact = self.portfolio.entries.get(delivered).unwrap().artifact;
        let provenance = self.compilation.artifacts.provenance(artifact)?;
        let plan = provenance.naming();
        report.spelling = spelling_names(Spelling {
            families: provenance.description().output().families,
            self_named: plan.self_named,
            read_order: plan.read_order,
        });
        report.style = format!("{:?}", plan.style);
        debug_assert!(report.after <= report.searched);
        Ok(Some(report))
    }

    /// Walk from the portfolio entry at `position` in its candidate's
    /// formations, settle the result against the objective's winner, and
    /// then, with `restarts`, restart from the same entry under each other
    /// naming seed. Returns initial naming starts rejected by the proxy.
    fn walk_from(
        &mut self,
        policy: &ResolvedPolicy,
        objective: OptimizationObjective,
        codec: Objective,
        name: &str,
        position: usize,
        phase: WalkPhase,
        report: &mut TerminalObjective,
    ) -> Result<NamingStarts, SearchError> {
        if matches!(phase, WalkPhase::LocalNaming) {
            let outcome = if report.examined >= objective.walk.prefix
                || report.judged >= objective.walk.exact
            {
                Some(ChallengerOutcome::Budget)
            } else if (!policy.tactic(TacticId::IdentifierMangling).enabled
                || !policy.tactic(TacticId::NamingSearch).enabled)
                && !policy.tactic(TacticId::PropertyMangling).enabled
            {
                Some(ChallengerOutcome::Vetoed)
            } else {
                None
            };
            if let Some(outcome) = outcome {
                report.examined += usize::from(outcome == ChallengerOutcome::Vetoed);
                report.joint_trials.push(JointTrial {
                    name: "naming:local-read-order".into(),
                    pass: report.passes + 1,
                    outcome,
                    size: None,
                    delta: None,
                    proxy: None,
                    audit: None,
                });
                return Ok(NamingStarts::default());
            }
        }
        let (artifact, state) = {
            let entry = self.portfolio.entries.get(position).unwrap();
            (entry.artifact, entry.state)
        };
        let candidate = self.states[state]
            .as_ref()
            .expect("a portfolio entry pins its source state")
            .candidate;
        let arena = &self.compilation.artifacts;
        let provenance = arena.provenance(artifact)?;
        let plan = provenance.naming().clone();
        let output = provenance.description().output().clone();
        let size = arena
            .with_artifact(artifact, |view| view.sizes.get(codec))?
            .expect("a portfolio entry is measured under its codec");
        // The naming seeds the policy permits: whole-artifact moves after
        // every site move (architecture §9.5). A plan that retains source
        // names has none.
        let naming: Vec<Style> = if plan.source_names.is_empty() {
            Plan::seeds_for_policy(policy).map_or_else(|_| Vec::new(), <[Style]>::to_vec)
        } else {
            Vec::new()
        };
        let origin = Incumbent {
            artifact,
            spelling: Spelling {
                families: output.families,
                self_named: plan.self_named,
                read_order: plan.read_order,
            },
            choices: output.choices.clone(),
            plan,
            literals: output.literals,
            size,
            qualified: None,
        };
        let walk = objective.walk;
        let available = RENDER_BOUND.max(self.portfolio.baseline_capacity);
        // The walk from the level-0 artifact passes every lower one-pass
        // level's stopping point (levels 0 to 12, below this one).
        let replay = (name == "level-0").then(|| Replay {
            pending: (0..policy.effort())
                .map(|level| {
                    (
                        level,
                        crate::compilation_policy::WalkSchedule::at(level, codec),
                    )
                })
                .filter(|(_, schedule)| schedule.passes <= 1)
                .collect(),
            examined: report.examined,
            judged: report.judged,
        });
        let Self {
            compilation,
            portfolio,
            ..
        } = self;
        let judge = Judge {
            policy,
            codec,
            output: &output,
            available,
            margin: i64::try_from(walk.margin).unwrap_or(i64::MAX),
            baseline: portfolio.baseline_qualification(codec).copied(),
            proxy_pruning: policy
                .objective()
                .expect("JavaScript objective")
                .search
                .proxy_pruning,
        };
        let formed = compilation.with_javascript_formations_in(
            candidate,
            policy,
            output.dead_code_elimination,
            output.target_compaction,
            output.rules,
            output.families.head(),
            WorkDomain::Optional,
            |formations| -> Result<NamingStarts, SearchError> {
                let mut walker = Walker {
                    formations,
                    portfolio,
                    judge: &judge,
                    report: &mut *report,
                    walk,
                    codec,
                    state,
                    output: &output,
                    naming: &naming,
                    choices_permitted: output.target_compaction,
                    stopped: false,
                    memo: Vec::new(),
                    replay,
                };
                let walked = (|| {
                    let mut pruned = NamingStarts::default();
                    match phase {
                        WalkPhase::DeferredNaming(pending) => {
                            for &style in &naming {
                                if pending.contains(style) {
                                    walker.restart(&origin, style, true)?;
                                }
                            }
                        }
                        _ => {
                            walker.walk_start(
                                name,
                                origin.clone(),
                                matches!(phase, WalkPhase::LocalNaming),
                            )?;
                            if let WalkPhase::Search { restarts: true } = phase {
                                for &style in &naming {
                                    if style != origin.plan.style
                                        && walker.restart(&origin, style, false)?
                                    {
                                        pruned.insert(style);
                                    }
                                }
                            }
                        }
                    }
                    Ok(pruned)
                })();
                walker.report.heads += 1 + walker.formations.other_heads_formed();
                walked
            },
        );
        match formed {
            Ok(result) => result,
            // The candidate's demand could not be admitted: nothing formed.
            Err(error) if exhausted(&error) => {
                report.starts.push(StartTrial {
                    name: name.to_string(),
                    pass: report.passes + 1,
                    outcome: ChallengerOutcome::Stopped,
                    start: Some(size),
                    size: None,
                    delta: None,
                    proxy: None,
                    audit: None,
                });
                Ok(NamingStarts::default())
            }
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
#[path = "search_terminal_tests.rs"]
mod tests;
