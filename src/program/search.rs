//! Compilation-owned bounded exploration over immutable, compatible recipes.
//! Scoring owns complete bytes; eligibility owns the source, recipe and naming
//! provenance. These are separate from scheduling heuristics and handle order.
use super::*;
use crate::compilation_policy::{
    AdmissionError, BaselineSeal, CandidateCostEvidence, CodecSchedule, OptimizationObjective,
};
use crate::output_budget::{AllocationClass::Retained, RetainedCharge};
use crate::program::artifact_provenance::ProvenanceError;
use crate::program::implementation_identity::{
    ImplementationDescription, SharedImplementationIdentity,
};
use crate::program::search_opportunities::{Inventory, OpportunityView};
use crate::js::selection::{Objective, Objectives, Plan, Sizes, Style};
use std::cmp::Ordering;

#[path = "search_selection.rs"]
mod selection;
use selection::Portfolio;

#[cfg(test)]
#[path = "search_cursor_tests.rs"]
mod cursor_tests;
#[cfg(test)]
#[path = "search_fairness_tests.rs"]
mod fairness_tests;
#[cfg(test)]
#[path = "function_support_tests.rs"]
mod function_support_tests;

/// Qualification of proof queries is explicit, independent of cache occupancy.
/// Search/probe/frontier limits come from the resolved TOML policy. These plans
/// are finite caller ceilings, not an estimate of sufficient source resources.
#[derive(Debug, Clone, Copy)]
pub struct SearchRequest {
    pub objectives: Objectives,
    pub scalar: ScalarRequest,
    pub helper: HelperRequest,
    pub string: StringRequest,
    pub facts_cache: CacheLimits,
}

#[derive(Debug)]
pub enum SearchError {
    Candidate(CandidateError),
    Admission(AdmissionError),
    Limit(SearchLimit),
}
impl From<CandidateError> for SearchError {
    fn from(error: CandidateError) -> Self {
        Self::Candidate(error)
    }
}
impl From<PublicationError> for SearchError {
    fn from(error: PublicationError) -> Self {
        CandidateError::from(error).into()
    }
}
impl From<AllocationError> for SearchError {
    fn from(error: AllocationError) -> Self {
        CandidateError::from(error).into()
    }
}
impl From<BudgetError> for SearchError {
    fn from(error: BudgetError) -> Self {
        CandidateError::from(error).into()
    }
}
impl From<AdmissionError> for SearchError {
    fn from(error: AdmissionError) -> Self {
        Self::Admission(error)
    }
}
impl From<ProvenanceError> for SearchError {
    fn from(error: ProvenanceError) -> Self {
        match error {
            ProvenanceError::Allocation(error) => error.into(),
            ProvenanceError::Naming(error) => CandidateError::from(error).into(),
            ProvenanceError::Admission(error) => error.into(),
        }
    }
}

impl SearchError {
    /// A refused allocation leaves the existing immutable artifacts valid.
    /// This recognizes resource ownership boundaries, not individual tactics;
    /// semantic Unknown/Truncated answers are deliberately not included.
    fn optional_memory_refusal(&self) -> bool {
        matches!(
            self,
            Self::Candidate(CandidateError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            ))) | Self::Candidate(CandidateError::Publication(PublicationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Optional)
            ))) | Self::Candidate(CandidateError::LocalFacts(CompilationFactsError::Facts(
                FactsError::Budget(BudgetError::MemoryExhausted(WorkDomain::Optional))
            ))) | Self::Candidate(CandidateError::LocalFacts(
                CompilationFactsError::Publication(PublicationError::Budget(
                    BudgetError::MemoryExhausted(WorkDomain::Optional)
                ))
            ))
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchLimit {
    Alternatives,
    CodecProbes,
    ArtifactCount,
    ArtifactBytes,
    Frontier,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchCounters {
    /// Optional complete-artifact attempts, one per naming/output-mode pair.
    /// Failed attempts count; semantic hint visits and map unions do not.
    pub proposals: usize,
    /// Attempted source opportunities / compatible map unions. This ordinal
    /// rotates structural objectives independently of permitted naming styles.
    pub structural_attempts: usize,
    pub skipped_unknown: usize,
    pub skipped_truncated: usize,
    pub proof_queries: usize,
    pub unknown_proofs: usize,
    pub truncated_proofs: usize,
    /// Later cell hints covered by one already proved product component.
    pub equivalent_product_hints: usize,
    /// A parent already replaces every sealed creator with an inline recipe;
    /// adding shared transport would remain unused in compatible children.
    pub inactive_function_layouts: usize,
    pub inventory_truncated: bool,
    pub conflicting_choices: usize,
    /// A required baseline already owns the proposed component.
    pub redundant_choices: usize,
    pub duplicate_states: usize,
    pub structures: usize,
    pub renders: usize,
    pub codec_probes: usize,
    pub admitted_artifacts: usize,
    pub beam_evictions: usize,
    pub queued_artifacts: usize,
    pub pending_peak: usize,
    pub pressure_scores: usize,
    pub diversity_scores: usize,
    pub scoring_events: usize,
}

/// An observation of a committed, eligible complete artifact. A visitor may
/// copy it into caller-owned diagnostics; search keeps no unbounded history.
pub struct SearchObservation<'a> {
    pub candidate: CandidateId,
    pub recipe_fingerprint: u64,
    /// Full versioned recipe identity, borrowed from the retained source state.
    /// Fingerprints alone cannot establish equality or explain selected choices.
    pub recipe_descriptor: ImplementationDescription<'a>,
    pub naming: &'a Plan,
    pub output: OutputTactics,
    pub sizes: Sizes,
    pub javascript: &'a str,
    pub baseline: bool,
}

struct State {
    candidate: CandidateId,
    identity: SharedImplementationIdentity,
    next: usize,
    pending: bool,
    /// Structural attempt ordinal; scoring has its own independent clock.
    /// New descendants start young so a parent's skip continuation can wait
    /// fairly without another queue or a retained expansion history.
    last_served: usize,
    best: [Option<usize>; 3],
    cheap_raw: Option<usize>,
}

#[derive(Clone, Copy)]
enum Seed {
    Unseen,
    /// Qualified to this search's pinned source, contract and fixed request.
    Unknown,
    /// No repeat under identical attempt ceilings. This is not a negative
    /// semantic answer; a search under stronger bounds must try it again.
    Truncated,
    /// Valid choice incompatible with the immutable required baseline map.
    Conflict,
    /// The immutable baseline already selects this component.
    Redundant,
    Ready(CandidateId),
    /// The same fixed product cut was already scheduled at this earlier hint.
    /// Only its Ready entry owns the proof checkpoint.
    Equivalent(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Evaluation {
    Baseline,
    ContinueBaseline,
    Structural,
}

/// The exclusive compilation borrow pins source meaning and the immutable JS
/// contract throughout result inspection. Drop releases all search-owned
/// checkpoints, proofs, artifacts and descriptors through their original ledger.
/// The Compilation-owned local-facts cache and artifact-arena slot capacity
/// remain reusable until Compilation releases them. Neither establishes
/// candidate eligibility or retains search-owned output bytes.
pub struct JavaScriptSearch<'a, 'src> {
    compilation: &'a mut Compilation<'src>,
    owner: RevisionId,
    header_charge: Option<RetainedCharge<RevisionId>>,
    states: Vec<Option<State>>,
    states_charge: Option<RetainedCharge<RevisionId>>,
    seeds: Vec<Seed>,
    seeds_charge: Option<RetainedCharge<RevisionId>>,
    inventory: Option<Inventory>,
    portfolio: Portfolio,
    counters: SearchCounters,
    sealed: Option<BaselineSeal>,
    stopped: Option<SearchError>,
    discovery_refusal: Option<SearchError>,
}

impl<'src> Compilation<'src> {
    pub fn search_javascript<'a>(
        &'a mut self,
        source: SemanticId,
        policy: &ResolvedPolicy,
        request: SearchRequest,
    ) -> Result<JavaScriptSearch<'a, 'src>, SearchError> {
        self.search_javascript_observed(source, policy, request, |_| {})
    }

    pub fn search_javascript_observed<'a>(
        &'a mut self,
        source: SemanticId,
        policy: &ResolvedPolicy,
        request: SearchRequest,
        mut observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<JavaScriptSearch<'a, 'src>, SearchError> {
        self.lookup(source)?;
        self.ledger.require_preparing_baseline()?;
        let objective = policy.objective().ok_or(CandidateError::NotJavaScript)?;
        // This first producer has exact transfer evidence only. Do not compile
        // under constraints it cannot establish, including an explicit zero.
        policy.admit_evidence(
            &[],
            CandidateCostEvidence::size_only(0),
            CandidateCostEvidence::size_only(0),
        )?;
        let seeds = Plan::seeds_for_policy(policy).map_err(CandidateError::from)?;
        let owner = RevisionId::fresh();
        let header_charge = {
            let mut budget = AllocationBudget::new(Some((&mut self.ledger, WorkDomain::Baseline)));
            let bytes = bytes::<JavaScriptSearch<'_, 'src>>(1)?;
            budget.retain(Retained, bytes)?;
            budget.detach_retained(owner, bytes)?
        };
        let mut search = JavaScriptSearch {
            compilation: self,
            owner,
            header_charge: Some(header_charge),
            states: Vec::new(),
            states_charge: None,
            seeds: Vec::new(),
            seeds_charge: None,
            inventory: None,
            portfolio: Portfolio::new(owner),
            counters: SearchCounters::default(),
            sealed: None,
            stopped: None,
            discovery_refusal: None,
        };
        search.grow_states(1, WorkDomain::Baseline)?;
        let direct = search
            .compilation
            .direct_javascript(source, policy, WorkDomain::Baseline)?;
        search.insert_state(direct, 0, WorkDomain::Baseline)?;
        let (baseline_renders, continuation) =
            search.evaluate_baseline(policy, request.objectives, seeds, &mut observe)?;
        if objective.optional_alternatives == 0 {
            return Ok(search);
        }
        let exploration = continuation.and_then(|()| {
            search.explore(
                policy,
                objective,
                request,
                seeds,
                baseline_renders,
                &mut observe,
            )
        });
        // A proposal ceiling ends discovery, not already admitted scoring.
        // A refused allocation ends discovery after its temporary owners unwind.
        // Existing queued bytes can still be affordable to score. This is one
        // transition into finalization, with no retry of the refused operation.
        // Work/deadline failures do not receive a new allowance.
        match exploration {
            Ok(()) => {
                search.stopped = search
                    .drain_pending(policy, request.objectives, &mut observe)
                    .err();
            }
            Err(SearchError::Limit(SearchLimit::Alternatives)) => {
                search.stopped = Some(
                    search
                        .drain_pending(policy, request.objectives, &mut observe)
                        .err()
                        .unwrap_or(SearchError::Limit(SearchLimit::Alternatives)),
                );
            }
            Err(error) if error.optional_memory_refusal() => {
                search.discovery_refusal = Some(error);
                search.stopped = search
                    .finish_discovery()
                    .and_then(|()| search.drain_pending(policy, request.objectives, &mut observe))
                    .err();
            }
            Err(error) => search.stopped = Some(error),
        }
        search.abandon_pending();
        Ok(search)
    }
}

impl JavaScriptSearch<'_, '_> {
    pub fn counters(&self) -> SearchCounters {
        self.counters
    }
    pub fn baseline_seal(&self) -> BaselineSeal {
        self.sealed.expect("only sealed results escape")
    }
    /// None means the bounded opportunity inventory/frontier was drained. It
    /// never certifies global optimality or complete language capability coverage.
    pub fn stopped(&self) -> Option<&SearchError> {
        self.stopped.as_ref().or(self.discovery_refusal.as_ref())
    }
    /// The allocation refusal that ended discovery, even if final scoring then
    /// reached another cap. No proof was made Unknown because it could not fit.
    pub fn discovery_refusal(&self) -> Option<&SearchError> {
        self.discovery_refusal.as_ref()
    }
    pub fn with_winner<R>(
        &self,
        objective: Objective,
        inspect: impl FnOnce(ArtifactView<'_>, &Plan) -> R,
    ) -> Option<R> {
        let winner = self
            .portfolio
            .entries
            .get(self.portfolio.selected[index(objective)]?)
            .unwrap();
        Some(
            self.compilation
                .with_artifact(winner.artifact, |artifact| {
                    inspect(
                        artifact,
                        self.compilation
                            .artifacts
                            .provenance(winner.artifact)
                            .expect("search owns winner provenance")
                            .naming(),
                    )
                })
                .expect("search owns every retained winner"),
        )
    }
    pub fn ledger(&self) -> &BudgetLedger {
        &self.compilation.ledger
    }

    /// Explicit terminal handoff without copying the selected output. If one
    /// artifact wins multiple objectives, taking it consumes all those aliases.
    /// Inspect all requested winners before taking shared output when needed.
    /// Returns None without consuming a package winner; use with_winner or
    /// take_winner_artifact to preserve its required second file.
    pub fn take_winner(&mut self, objective: Objective) -> Option<String> {
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        self.portfolio
            .take_winner(&mut self.compilation.artifacts, &mut budget, objective)
    }

    /// Transfer the existing complete artifact owner without copying its
    /// files. After dropping Search, inspect or discard this handle through
    /// the originating Compilation. Taking one artifact consumes all
    /// objective aliases of that winner.
    pub fn take_winner_artifact(&mut self, objective: Objective) -> Option<ArtifactId> {
        self.portfolio.take_winner_artifact(objective)
    }

    pub fn winner_qualification(&self, objective: Objective) -> Option<&QualifiedArtifact> {
        self.portfolio
            .entries
            .get(self.portfolio.selected[index(objective)]?)?
            .qualified[index(objective)]
        .as_ref()
    }

    /// Transfer the admitted complete artifact out of the search portfolio.
    /// As with take_winner_artifact, all aliases of the same bytes are consumed.
    pub fn take_qualified_winner(&mut self, objective: Objective) -> Option<QualifiedArtifact> {
        let qualified = *self.winner_qualification(objective)?;
        let artifact = self.take_winner_artifact(objective)?;
        debug_assert_eq!(qualified.artifact(), artifact);
        Some(qualified)
    }

    fn grow_states(&mut self, capacity: usize, domain: WorkDomain) -> Result<(), SearchError> {
        if capacity <= self.states.capacity() {
            return Ok(());
        }
        let mut budget = AllocationBudget::new(Some((&mut self.compilation.ledger, domain)));
        budget.work(WorkKind::Analysis, self.states.len() as u64)?;
        let mut states = budget.vector(Retained, capacity)?;
        let charge =
            budget.detach_retained(self.owner, bytes::<Option<State>>(states.capacity())?)?;
        states.append(&mut self.states);
        drop(std::mem::replace(&mut self.states, states));
        if let Some(old) = self.states_charge.replace(charge) {
            budget.with_ledger(|owner| old.discard(&self.owner, owner.unwrap().0).unwrap());
        }
        Ok(())
    }

    /// Admit only the next needed backing capacity. Pending artifact entries
    /// pin their source state even after it leaves the active structural beam.
    fn prepare_state_slot(&mut self, limit: usize) -> Result<(), SearchError> {
        self.compilation.ledger.charge(
            WorkDomain::Optional,
            WorkKind::Analysis,
            u64::try_from(self.states.len()).map_err(|_| AllocationError::Capacity)?,
        )?;
        if self.states.len() < self.states.capacity() || self.states.iter().any(Option::is_none) {
            return Ok(());
        }
        let capacity = self
            .states
            .capacity()
            .max(1)
            .checked_mul(2)
            .ok_or(AllocationError::Capacity)?
            .min(limit);
        if capacity <= self.states.capacity() {
            return Err(SearchError::Limit(SearchLimit::Frontier));
        }
        self.grow_states(capacity, WorkDomain::Optional)
    }

    /// Takes ownership even on failure; a partially inserted candidate never
    /// survives an unsuccessful descriptor admission.
    fn insert_state(
        &mut self,
        candidate: CandidateId,
        next: usize,
        domain: WorkDomain,
    ) -> Result<Option<usize>, SearchError> {
        let result = (|| {
            let identity = self
                .compilation
                .share_implementation_identity(candidate, domain)?;
            let mut budget = AllocationBudget::new(Some((&mut self.compilation.ledger, domain)));
            let checked = (|| {
                let mut free = None;
                for (position, state) in self.states.iter().enumerate() {
                    budget.work(WorkKind::Analysis, 1)?;
                    let Some(state) = state else {
                        free.get_or_insert(position);
                        continue;
                    };
                    if identity.fingerprint() == state.identity.fingerprint()
                        && identity.equivalent(&state.identity, &mut budget)?
                    {
                        return Ok(None);
                    }
                }
                let position = free.unwrap_or(self.states.len());
                if position >= self.states.capacity() {
                    return Err(SearchError::Limit(SearchLimit::Frontier));
                }
                Ok(Some(position))
            })();
            match checked {
                Ok(Some(position)) => {
                    let state = Some(State {
                        candidate,
                        identity,
                        next,
                        pending: true,
                        last_served: self.counters.structural_attempts,
                        best: [None; 3],
                        cheap_raw: None,
                    });
                    if position == self.states.len() {
                        self.states.push(state);
                    } else {
                        self.states[position] = state;
                    }
                    self.counters.structures += 1;
                    Ok(Some(position))
                }
                other => {
                    budget.with_ledger(|ledger| {
                        identity
                            .discard(self.compilation.store, ledger.unwrap().0)
                            .unwrap()
                    });
                    other
                }
            }
        })();
        if !matches!(result, Ok(Some(_))) {
            self.compilation.discard(candidate.semantic_id())?;
        }
        if matches!(result, Ok(None)) {
            self.counters.duplicate_states += 1;
        }
        result
    }

    fn evaluate_baseline(
        &mut self,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        styles: &[Style],
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<(usize, Result<(), SearchError>), SearchError> {
        let objective = policy.objective().unwrap();
        let candidate = self.states[0].as_ref().unwrap().candidate;
        let Self {
            compilation,
            states,
            portfolio,
            counters,
            sealed,
            ..
        } = self;
        let (baseline_renders, continuation) = compilation.with_javascript_target_in(
            candidate,
            policy,
            super::OutputTactics::from_policy(policy),
            WorkDomain::Baseline,
            |target| -> Result<_, SearchError> {
                let alternative = target.with_output_in(WorkDomain::Baseline, |output| {
                    Self::evaluate_output(
                        output,
                        states,
                        portfolio,
                        counters,
                        0,
                        policy,
                        objectives,
                        styles,
                        Evaluation::Baseline,
                        observe,
                    )
                })??;
                let baseline_renders = counters.renders;
                if objective.optional_alternatives == 0 || (styles.len() <= 1 && !alternative) {
                    return Ok((baseline_renders, None));
                }
                *sealed = Some(
                    target
                        .budget
                        .with_ledger(|ledger| ledger.unwrap().0.seal_baseline())?,
                );
                let continuation = (|| {
                    optional_preflight(*counters, objective, objectives)?;
                    target.with_output_in(WorkDomain::Optional, |output| {
                        Self::evaluate_output(
                            output,
                            states,
                            portfolio,
                            counters,
                            0,
                            policy,
                            objectives,
                            styles,
                            Evaluation::ContinueBaseline,
                            observe,
                        )
                    })??;
                    Ok(())
                })();
                Ok((baseline_renders, Some(continuation)))
            },
        )??;
        // With no direct continuation, seal after all transient target storage
        // drops. Otherwise the same immutable target was charged through both
        // phases and is now released before inventory or structural exploration.
        match continuation {
            Some(result) => Ok((baseline_renders, result)),
            None => {
                *sealed = Some(compilation.ledger.seal_baseline()?);
                Ok((baseline_renders, Ok(())))
            }
        }
    }

    fn evaluate_structure(
        &mut self,
        state: usize,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        styles: &[Style],
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<(), SearchError> {
        let objective = policy.objective().unwrap();
        if styles.is_empty() {
            return Ok(());
        }
        self.preflight_optional(objective, objectives)?;
        let candidate = self.states[state].as_ref().unwrap().candidate;
        let Self {
            compilation,
            states,
            portfolio,
            counters,
            ..
        } = self;
        compilation.with_javascript_output_in(
            candidate,
            policy,
            WorkDomain::Optional,
            |output| {
                Self::evaluate_output(
                    output,
                    states,
                    portfolio,
                    counters,
                    state,
                    policy,
                    objectives,
                    styles,
                    Evaluation::Structural,
                    observe,
                )
            },
        )??;
        Ok(())
    }

    fn evaluate_output(
        output: &mut super::BudgetedJavaScriptOutput<'_, '_>,
        states: &mut [Option<State>],
        portfolio: &mut Portfolio,
        counters: &mut SearchCounters,
        state: usize,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        styles: &[Style],
        evaluation: Evaluation,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<bool, SearchError> {
        let objective = policy.objective().unwrap();
        let baseline = evaluation == Evaluation::Baseline;
        let has_alternative = output.has_literal_alternative()?;
        // One prepared Module/Basis serves every pair. The mandatory route
        // renders one default artifact. Optional work first visits the usual
        // default naming seeds, then Original spellings only for live clients.
        for mode in [None, Some(LiteralOutput::Original)] {
            if mode.is_some() && (baseline || !has_alternative) {
                break;
            }
            // Self-named functions trade raw bytes for repeated names, which a
            // codec compresses almost for free: the policy's own objective
            // decides, so the schedule is the same for every objective.
            let raw_spelling = objective.codec == Objective::Raw;
            for (ordinal, &style) in styles.iter().enumerate() {
                if baseline && ordinal != 0 {
                    break;
                }
                if evaluation == Evaluation::ContinueBaseline && mode.is_none() && ordinal == 0 {
                    continue;
                }
                if !baseline {
                    optional_preflight(*counters, objective, objectives)?;
                    counters.proposals += 1;
                }
                let available = if baseline {
                    usize::MAX
                } else {
                    objective
                        .retained_candidate_bytes
                        .max(portfolio.baseline_capacity)
                };
                let plan = Plan::spelled(style, raw_spelling);
                counters.renders += 1;
                let before_render = output.with_allocation_budget(|budget| {
                    budget.with_ledger(|ledger| ledger.unwrap().0.work_by_kind(WorkKind::Render))
                });
                let artifact = match mode {
                    None => output.render_bounded(&plan, available)?,
                    Some(literals) => {
                        output.render_bounded_with_literals(&plan, literals, available)?
                    }
                };
                let render_work = output
                    .with_allocation_budget(|budget| {
                        budget
                            .with_ledger(|ledger| ledger.unwrap().0.work_by_kind(WorkKind::Render))
                    })
                    .checked_sub(before_render)
                    .expect("one monotonic render-work owner");
                let (raw, capacity) =
                    output.with_artifact(artifact, |view| (view.sizes.raw, view.retained_capacity))?;
                if capacity > available {
                    return Err(SearchError::Limit(SearchLimit::ArtifactBytes));
                }
                let position = portfolio.stage(output, state, artifact, render_work)?;
                let hint = &mut states[state].as_mut().unwrap().cheap_raw;
                *hint = Some(hint.map_or(raw, |old| old.min(raw)));
                output.with_retained_arena(|arena, budget| {
                    portfolio.consider(
                        arena,
                        budget,
                        states,
                        policy.contract(),
                        policy,
                        objectives,
                        position,
                        baseline,
                        counters,
                        observe,
                    )
                })?;
            }
        }
        Ok(has_alternative)
    }

    fn preflight_optional(
        &self,
        objective: OptimizationObjective,
        objectives: Objectives,
    ) -> Result<(), SearchError> {
        optional_preflight(self.counters, objective, objectives)
    }

    fn score_next(
        &mut self,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<bool, SearchError> {
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        self.portfolio.score_next(
            &mut self.compilation.artifacts,
            &mut budget,
            &mut self.states,
            &self.compilation.javascript.as_ref().unwrap().contract,
            policy,
            objectives,
            &mut self.counters,
            observe,
        )
    }

    fn drain_pending(
        &mut self,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<(), SearchError> {
        while self.score_next(policy, objectives, observe)? {}
        Ok(())
    }

    fn abandon_pending(&mut self) {
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        self.portfolio
            .abandon_pending(&mut self.compilation.artifacts, &mut budget);
    }

    /// Retain only the semantic states needed by already produced artifacts.
    /// Physical ownership cleanup needs no new memory. Choosing unpinned states
    /// is still admitted analysis work; a spent work/deadline budget stops here.
    fn finish_discovery(&mut self) -> Result<(), SearchError> {
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        self.portfolio
            .discard_provisional(&mut self.compilation.artifacts, &mut budget);
        drop(budget);
        self.discard_discovery_owners();
        for i in 0..self.states.len() {
            let mut budget =
                AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
            budget.work(WorkKind::Analysis, 1)?;
            let Some(state) = &mut self.states[i] else {
                continue;
            };
            let pinned = self.portfolio.pins(i, &mut budget)?;
            drop(budget);
            if pinned {
                state.pending = false;
            } else if let Some(state) = self.states[i].take() {
                self.compilation
                    .discard(state.candidate.semantic_id())
                    .expect("search owns state");
                state
                    .identity
                    .discard(self.compilation.store, &mut self.compilation.ledger)
                    .unwrap();
            }
        }
        Ok(())
    }

    fn discard_discovery_owners(&mut self) {
        for seed in &self.seeds {
            if let Seed::Ready(candidate) = seed {
                self.compilation
                    .discard(candidate.semantic_id())
                    .expect("search owns proof seed");
            }
        }
        drop(std::mem::take(&mut self.seeds));
        if let Some(charge) = self.seeds_charge.take() {
            charge
                .discard(&self.owner, &mut self.compilation.ledger)
                .unwrap();
        }
        if let Some(inventory) = self.inventory.take() {
            inventory
                .discard(self.owner, &mut self.compilation.ledger)
                .unwrap();
        }
    }

    fn explore(
        &mut self,
        policy: &ResolvedPolicy,
        objective: OptimizationObjective,
        request: SearchRequest,
        styles: &[Style],
        baseline_renders: usize,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<(), SearchError> {
        let state_limit = objective
            .beam_width
            .max(1)
            .checked_add(objective.retained_candidates.max(1))
            .and_then(|capacity| capacity.checked_add(5))
            .ok_or(AllocationError::Capacity)?;
        let staged = objective.search.codec_schedule == CodecSchedule::Staged
            && request.objectives != Objectives::One(Objective::Raw);
        let mut next_score_at = baseline_renders
            .checked_add(objective.search.render_batch)
            .ok_or(AllocationError::Capacity)?;
        // Naming the direct recipe can spend the last artifact/probe slot.
        // Discovery has no useful output then; drain the existing portfolio
        // through the same finalization transition before scanning the source.
        self.preflight_optional(objective, request.objectives)?;
        let source = self
            .compilation
            .candidate_slot(self.states[0].as_ref().unwrap().candidate)?;
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        self.counters.inventory_truncated = true;
        self.inventory = Some(Inventory::build(
            &self.compilation.slots[source]
                .checkpoint
                .as_ref()
                .unwrap()
                .semantic
                .program,
            policy,
            // Artifact attempts and source hints are different work. A small
            // render allowance must not hide a useful late source opportunity.
            // The finite input, work ledger and admitted buffers bound this
            // inventory; exhaustion preserves the scored baseline.
            usize::MAX,
            self.owner,
            &mut budget,
        )?);
        let count = self.inventory.as_ref().unwrap().len();
        self.counters.inventory_truncated = self.inventory.as_ref().unwrap().truncated();
        self.seeds = budget.filled(Retained, count, Seed::Unseen)?;
        self.seeds_charge =
            Some(budget.detach_retained(self.owner, bytes::<Seed>(self.seeds.capacity())?)?);
        drop(budget);
        if count == 0 {
            self.states[0].as_mut().unwrap().pending = false;
            return Ok(());
        }
        loop {
            // Batch sizes depend only on the fixed schedule. A prepared target
            // may finish its naming seeds before this event boundary.
            if staged && self.counters.renders >= next_score_at {
                self.score_next(policy, request.objectives, observe)?;
                next_score_at = self
                    .counters
                    .renders
                    .checked_add(objective.search.render_batch)
                    .ok_or(AllocationError::Capacity)?;
            }
            self.reclaim_states()?;
            let Some(parent) =
                self.next_state(request.objectives, objective.search.diversity_interval)?
            else {
                return Ok(());
            };
            self.preflight_optional(objective, request.objectives)?;
            self.counters.structural_attempts = self
                .counters
                .structural_attempts
                .checked_add(1)
                .ok_or(AllocationError::Capacity)?;
            self.states[parent].as_mut().unwrap().last_served =
                self.counters.structural_attempts;
            let base = self.states[parent].as_ref().unwrap().candidate;
            let step = self.states[parent].as_ref().unwrap().next;
            self.states[parent].as_mut().unwrap().next += 1;
            if step + 1 >= count {
                self.states[parent].as_mut().unwrap().pending = false;
            }
            let Some(seed) = self.proof(step, policy, request)? else {
                continue;
            };
            if let Some(OpportunityView::Function(body)) =
                Self::opportunity(self.inventory.as_ref().unwrap(), step)
            {
                let base_slot = self.compilation.candidate_slot(base)?;
                let seed_slot = self.compilation.candidate_slot(seed)?;
                let base_map = self.compilation.slots[base_slot]
                    .checkpoint
                    .as_ref()
                    .unwrap()
                    .implementations
                    .as_ref()
                    .unwrap();
                let seed_map = self.compilation.slots[seed_slot]
                    .checkpoint
                    .as_ref()
                    .unwrap()
                    .implementations
                    .as_ref()
                    .unwrap();
                let mut budget = AllocationBudget::new(Some((
                    &mut self.compilation.ledger,
                    WorkDomain::Optional,
                )));
                let layout = seed_map
                    .function_for_body(body, &mut budget)?
                    .expect("function proof includes the requested body");
                if super::super::demand::shared_transport_support(
                    base_map,
                    layout,
                    policy.javascript_contract().unwrap().execution,
                    |n| budget.work(WorkKind::Analysis, n as u64),
                )? == super::super::demand::SharedTransportSupport::AllCreatorsInline
                {
                    self.counters.inactive_function_layouts += 1;
                    continue;
                }
            }
            let next = step + 1;
            // Admission before publishing a child means a failed growth never
            // leaves an otherwise unowned candidate checkpoint behind.
            self.prepare_state_slot(state_limit)?;
            let combined =
                match self
                    .compilation
                    .combine_javascript(base, seed, policy, WorkDomain::Optional)
                {
                    Ok(candidate) => candidate,
                    Err(CandidateError::ConflictingChoice) => {
                        self.counters.conflicting_choices += 1;
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
            let Some(child) = self.insert_state(combined, next, WorkDomain::Optional)? else {
                continue;
            };
            self.states[child].as_mut().unwrap().pending = next < count;
            self.evaluate_structure(child, policy, request.objectives, styles, observe)?;
            self.trim_frontier(objective.beam_width.max(1), request.objectives)?;
        }
    }

    fn opportunity(inventory: &Inventory, step: usize) -> Option<OpportunityView<'_>> {
        inventory.get(step)
    }

    fn proof(
        &mut self,
        step: usize,
        policy: &ResolvedPolicy,
        request: SearchRequest,
    ) -> Result<Option<CandidateId>, SearchError> {
        match self.seeds[step] {
            Seed::Ready(candidate) => return Ok(Some(candidate)),
            Seed::Equivalent(original) => {
                let Seed::Ready(candidate) = self.seeds[original] else {
                    unreachable!("equivalent hints borrow an already published proof")
                };
                return Ok(Some(candidate));
            }
            Seed::Unknown | Seed::Truncated | Seed::Conflict | Seed::Redundant => return Ok(None),
            Seed::Unseen => {}
        }
        if !matches!(
            Self::opportunity(self.inventory.as_ref().unwrap(), step),
            Some(
                OpportunityView::Scalar(_)
                    | OpportunityView::Product(_)
                    | OpportunityView::Function(_)
            )
        ) && self.compilation.local_facts.is_none()
        {
            self.compilation
                .enable_local_facts(request.facts_cache, WorkDomain::Optional)
                .map_err(CandidateError::from)?;
        }
        self.counters.proof_queries += 1;
        let direct = self.states[0].as_ref().unwrap().candidate;
        let result = (|| -> Result<Seed, CandidateError> {
            Ok(
                match Self::opportunity(self.inventory.as_ref().unwrap(), step)
                    .unwrap()
                {
                    OpportunityView::Scalar(cell) => match self
                        .compilation
                        .scalar_javascript(
                            direct,
                            cell,
                            request.scalar,
                            policy,
                            WorkDomain::Optional,
                        )?
                        .outcome
                    {
                        ScalarOutcome::Published(candidate) => Seed::Ready(candidate),
                        ScalarOutcome::Unknown(_) => {
                            self.counters.unknown_proofs += 1;
                            Seed::Unknown
                        }
                        ScalarOutcome::Truncated(_) => {
                            self.counters.truncated_proofs += 1;
                            Seed::Truncated
                        }
                    },
                    OpportunityView::Product(cell) => match self
                        .compilation
                        .scalar_product_javascript(
                            direct,
                            cell,
                            request.scalar,
                            policy,
                            WorkDomain::Optional,
                        )?
                        .outcome
                    {
                        ProductOutcome::Published(candidate) => Seed::Ready(candidate),
                        ProductOutcome::Unknown(_) => {
                            self.counters.unknown_proofs += 1;
                            Seed::Unknown
                        }
                        ProductOutcome::Truncated(_) => {
                            self.counters.truncated_proofs += 1;
                            Seed::Truncated
                        }
                    },
                    OpportunityView::Function(body) => match self
                        .compilation
                        .scalar_function_javascript(
                            direct,
                            body,
                            request.scalar,
                            policy,
                            WorkDomain::Optional,
                        )?
                        .outcome
                    {
                        FunctionOutcome::Published(candidate) => Seed::Ready(candidate),
                        FunctionOutcome::Unknown(_) => {
                            self.counters.unknown_proofs += 1;
                            Seed::Unknown
                        }
                        FunctionOutcome::Truncated(_) => {
                            self.counters.truncated_proofs += 1;
                            Seed::Truncated
                        }
                    },
                    OpportunityView::Inline(cell) => match self
                        .compilation
                        .inline_helper_javascript(
                            direct,
                            cell,
                            request.helper,
                            policy,
                            WorkDomain::Optional,
                        )?
                        .outcome
                    {
                        HelperOutcome::Published(candidate) => Seed::Ready(candidate),
                        HelperOutcome::Unknown(_) => {
                            self.counters.unknown_proofs += 1;
                            Seed::Unknown
                        }
                        HelperOutcome::Truncated(_) => {
                            self.counters.truncated_proofs += 1;
                            Seed::Truncated
                        }
                    },
                    OpportunityView::String {
                        definitions,
                        choice,
                    } => match self
                        .compilation
                        .represent_string_javascript(
                            direct,
                            definitions,
                            choice,
                            request.string,
                            policy,
                            WorkDomain::Optional,
                        )?
                        .outcome
                    {
                        StringOutcome::Published(candidate) => Seed::Ready(candidate),
                        StringOutcome::Unknown(_) => {
                            self.counters.unknown_proofs += 1;
                            Seed::Unknown
                        }
                        StringOutcome::Truncated(_) => {
                            self.counters.truncated_proofs += 1;
                            Seed::Truncated
                        }
                    },
                },
            )
        })();
        let result = match result {
            Ok(result) => result,
            Err(CandidateError::ConflictingChoice) => {
                self.counters.conflicting_choices += 1;
                Seed::Conflict
            }
            Err(CandidateError::DuplicateRoot) => {
                self.counters.redundant_choices += 1;
                Seed::Redundant
            }
            Err(error) => return Err(error.into()),
        };
        self.seeds[step] = result;
        if matches!(
            Self::opportunity(self.inventory.as_ref().unwrap(), step),
            Some(OpportunityView::Product(_))
        ) {
            if let Seed::Ready(candidate) = result {
                self.reuse_product_hints(step, candidate)?;
            }
        }
        Ok(match result {
            Seed::Ready(candidate) => Some(candidate),
            _ => None,
        })
    }

    fn reuse_product_hints(
        &mut self,
        step: usize,
        candidate: CandidateId,
    ) -> Result<(), SearchError> {
        let inventory = self.inventory.as_ref().unwrap();
        let Some(OpportunityView::Product(cell)) =
            Self::opportunity(inventory, step)
        else {
            unreachable!("product reuse follows a product opportunity");
        };
        let slot = self.compilation.candidate_slot(candidate)?;
        let map = self.compilation.slots[slot]
            .checkpoint
            .as_ref()
            .unwrap()
            .implementations
            .as_ref()
            .unwrap();
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        let family = map
            .product_for_cell(cell, &mut budget)?
            .expect("product proof includes the requested cell");
        let count = family.cells().len();
        if count <= 1 {
            return Ok(());
        }
        // Probe the existing sorted inventory once for each covered cell.
        // A nonempty required map may also own unrelated product components.
        let work = count
            .checked_mul((usize::BITS - inventory.len().leading_zeros()) as usize + 1)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(AllocationError::Capacity)?;
        budget.work(WorkKind::Analysis, work)?;
        let mut reused = 0;
        for cell in family.cells() {
            let Some(index) = inventory.product_index(cell.cell) else {
                continue;
            };
            if index > step && matches!(self.seeds[index], Seed::Unseen) {
                self.seeds[index] = Seed::Equivalent(step);
                reused += 1;
            }
        }
        self.counters.equivalent_product_hints += reused;
        Ok(())
    }

    // Objective-guided turns share this frontier with periodic age turns.
    // Structural attempts and artifact scoring use the same configured
    // interval on separate clocks; neither counter stands in for the other.
    fn next_state(
        &mut self,
        objectives: Objectives,
        diversity_interval: usize,
    ) -> Result<Option<usize>, SearchError> {
        let count = objectives.iter().count();
        let preferred = objectives
            .iter()
            .nth(self.counters.structural_attempts % count)
            .unwrap();
        let mut best: Option<usize> = None;
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        normalize_cursors(
            &mut self.states,
            &self.seeds,
            &mut self.counters,
            &mut budget,
        )?;
        if self.counters.structural_attempts % diversity_interval == diversity_interval - 1 {
            return Ok(oldest_state(&self.states, &mut budget)?);
        }
        for (i, state) in self.states.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            let Some(state) = state.as_ref().filter(|s| s.pending) else {
                continue;
            };
            let replace = if let Some(old) = best {
                state_order(
                    state,
                    self.states[old].as_ref().unwrap(),
                    preferred,
                    self.portfolio.baseline[index(preferred)],
                    self.portfolio.baseline_raw,
                    &mut budget,
                )? == Ordering::Less
            } else {
                true
            };
            if replace {
                best = Some(i);
            }
        }
        Ok(best)
    }
    fn trim_frontier(&mut self, width: usize, objectives: Objectives) -> Result<(), SearchError> {
        let preferred = objectives
            .iter()
            .nth(self.counters.structural_attempts % objectives.iter().count())
            .unwrap();
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        normalize_cursors(
            &mut self.states,
            &self.seeds,
            &mut self.counters,
            &mut budget,
        )?;
        // Keep one waiting cursor inside the existing beam until it can get
        // an age turn. Even width one makes finite progress: it keeps that
        // continuation while newer descendants may lose expansion eligibility.
        // Normalize first so an exhausted tail cannot occupy this slot.
        let protected = oldest_state(&self.states, &mut budget)?;
        let mut count = 0;
        let mut worst: Option<usize> = None;
        for (i, state) in self.states.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            let Some(state) = state.as_ref().filter(|s| s.pending) else {
                continue;
            };
            count += 1;
            if Some(i) == protected {
                continue;
            }
            let replace = if let Some(old) = worst {
                state_order(
                    state,
                    self.states[old].as_ref().unwrap(),
                    preferred,
                    self.portfolio.baseline[index(preferred)],
                    self.portfolio.baseline_raw,
                    &mut budget,
                )? == Ordering::Greater
            } else {
                true
            };
            if replace {
                worst = Some(i);
            }
        }
        if count > width {
            self.states[worst.unwrap()].as_mut().unwrap().pending = false;
            self.counters.beam_evictions += 1;
        }
        drop(budget);
        self.reclaim_states()?;
        Ok(())
    }
    fn reclaim_states(&mut self) -> Result<(), SearchError> {
        for i in 1..self.states.len() {
            let mut budget =
                AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
            budget.work(WorkKind::Analysis, 1)?;
            if !self.states[i].as_ref().is_some_and(|state| !state.pending) {
                continue;
            }
            let pinned = self.portfolio.pins(i, &mut budget)?;
            drop(budget);
            if !pinned {
                let state = self.states[i].take().unwrap();
                self.compilation
                    .discard(state.candidate.semantic_id())
                    .expect("search owns state");
                state
                    .identity
                    .discard(self.compilation.store, &mut self.compilation.ledger)
                    .unwrap();
            }
        }
        Ok(())
    }
}

fn normalize_cursors(
    states: &mut [Option<State>],
    seeds: &[Seed],
    counters: &mut SearchCounters,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    for state in states {
        budget.work(WorkKind::Analysis, 1)?;
        let Some(state) = state.as_mut().filter(|state| state.pending) else {
            continue;
        };
        // These answers are qualified by the pinned source/contract/request,
        // not by parent layout. Skipping them spends work but no proposal or
        // structural-attempt ordinal. Conflicts still belong to each parent.
        while state.next < seeds.len() {
            budget.work(WorkKind::Analysis, 1)?;
            match seeds[state.next] {
                Seed::Unknown => counters.skipped_unknown += 1,
                Seed::Truncated => counters.skipped_truncated += 1,
                Seed::Equivalent(_) | Seed::Conflict | Seed::Redundant => {}
                Seed::Unseen | Seed::Ready(_) => break,
            }
            state.next += 1;
        }
        if state.next == seeds.len() {
            state.pending = false;
        }
    }
    Ok(())
}

fn oldest_state(
    states: &[Option<State>],
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<usize>, AllocationError> {
    let mut oldest: Option<usize> = None;
    for (i, state) in states.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        let Some(state) = state.as_ref().filter(|state| state.pending) else {
            continue;
        };
        let order = if let Some(previous) = oldest {
            let previous = states[previous].as_ref().unwrap();
            budget.work(WorkKind::Analysis, 1)?;
            match state.last_served.cmp(&previous.last_served) {
                Ordering::Equal => state.identity.compare(&previous.identity, budget)?,
                order => order,
            }
        } else {
            Ordering::Less
        };
        if order == Ordering::Less {
            oldest = Some(i);
        }
    }
    Ok(oldest)
}

fn state_order(
    left: &State,
    right: &State,
    codec: Objective,
    baseline_codec: Option<usize>,
    baseline_raw: Option<usize>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Ordering, AllocationError> {
    budget.work(WorkKind::Analysis, 1)?;
    // Compare dimensionless ratios. An unscored raw hint is never treated as
    // a compressed score; both are relative to their matching direct baseline.
    let ratio = |state: &State| {
        state.best[index(codec)]
            .zip(baseline_codec)
            .or_else(|| state.cheap_raw.zip(baseline_raw))
    };
    let cost = match (ratio(left), ratio(right)) {
        (Some((left, left_base)), Some((right, right_base))) => ((left as u128)
            * (right_base.max(1) as u128))
            .cmp(&((right as u128) * (left_base.max(1) as u128))),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    if cost == Ordering::Equal {
        left.identity.compare(&right.identity, budget)
    } else {
        Ok(cost)
    }
}

fn optional_preflight(
    counters: SearchCounters,
    objective: OptimizationObjective,
    objectives: Objectives,
) -> Result<(), SearchError> {
    if counters.proposals >= objective.optional_alternatives {
        return Err(SearchError::Limit(SearchLimit::Alternatives));
    }
    // New immutable text has no non-raw score yet. Preserve the existing
    // complete-objective preflight before spending proof/formation work.
    let missing = objectives
        .iter()
        .filter(|&codec| codec != Objective::Raw)
        .count();
    if missing
        > objective
            .optional_codec_probes
            .saturating_sub(counters.codec_probes)
    {
        return Err(SearchError::Limit(SearchLimit::CodecProbes));
    }
    Ok(())
}
fn index(codec: Objective) -> usize {
    match codec {
        Objective::Raw => 0,
        Objective::Gzip => 1,
        Objective::Brotli => 2,
    }
}
fn codec(index: usize) -> Objective {
    [Objective::Raw, Objective::Gzip, Objective::Brotli][index]
}
fn bytes<T>(count: usize) -> Result<u64, AllocationError> {
    count
        .checked_mul(size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(AllocationError::Capacity)
}

impl Drop for JavaScriptSearch<'_, '_> {
    fn drop(&mut self) {
        let mut budget =
            AllocationBudget::new(Some((&mut self.compilation.ledger, WorkDomain::Optional)));
        self.portfolio
            .discard_all(&mut self.compilation.artifacts, &mut budget);
        drop(budget);
        for state in self.states.iter_mut().filter_map(Option::take) {
            self.compilation
                .discard(state.candidate.semantic_id())
                .expect("search owns state");
            state
                .identity
                .discard(self.compilation.store, &mut self.compilation.ledger)
                .unwrap();
        }
        self.discard_discovery_owners();
        drop(std::mem::take(&mut self.states));
        for charge in [self.states_charge.take(), self.header_charge.take()]
            .into_iter()
            .flatten()
        {
            charge
                .discard(&self.owner, &mut self.compilation.ledger)
                .unwrap();
        }
    }
}
