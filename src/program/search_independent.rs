//! Independent objective portfolios over one checked program and ledger.
//! Nested exclusive borrows retain all mandatory owners until the one seal;
//! unwinding the bounded three-frame stack searches in canonical codec order.
use super::*;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

pub(crate) struct IndependentSearchResult {
    pub winner: QualifiedArtifact,
    pub counters: SearchCounters,
    pub terminal: TerminalReport,
    pub stopped: Option<String>,
    pub optional_work_allowance: u64,
    pub optional_work_used: u64,
}

impl<'src> Compilation<'src> {
    pub(crate) fn search_javascript_independent(
        &mut self,
        source: SemanticId,
        requests: [(&ResolvedPolicy, SearchRequest); 3],
        mut observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<[IndependentSearchResult; 3], SearchError> {
        for (index, (policy, request)) in requests.iter().enumerate() {
            assert_eq!(request.objectives, Objectives::One(codec(index)));
            assert_eq!(policy.objective().unwrap().codec, codec(index));
        }
        let mut results = [None, None, None];
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.prepare_independent_objectives(source, &requests, &mut results, &mut observe)
        }));
        match outcome {
            Ok(Ok(_)) => Ok(results.map(Option::unwrap)),
            other => {
                // Active searches release their own owners on unwinding. Only
                // earlier completed handoffs remain outside those owners.
                for result in results.into_iter().flatten() {
                    self.discard_artifact(result.winner.artifact())
                        .expect("completed objective owns its handoff");
                }
                match other {
                    Ok(Err(error)) => Err(error),
                    Err(payload) => resume_unwind(payload),
                    Ok(Ok(_)) => unreachable!(),
                }
            }
        }
    }

    fn prepare_independent_objectives(
        &mut self,
        source: SemanticId,
        requests: &[(&ResolvedPolicy, SearchRequest)],
        results: &mut [Option<IndependentSearchResult>; 3],
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<BaselineSeal, SearchError> {
        let Some((&(policy, request), earlier)) = requests.split_last() else {
            // Every requested incumbent and score now exists. No later
            // objective needs to reopen Baseline or guess a memory reserve.
            return Ok(self.ledger.seal_baseline()?);
        };
        let objective = policy.objective().unwrap();
        let seeds = Plan::seeds_for_policy(policy).map_err(CandidateError::from)?;
        let mut search = self.prepare_javascript_search(source, policy)?;
        let (baseline_renders, continuation) = search.evaluate_baseline(
            policy,
            request.objectives,
            &seeds[..1],
            objective.walk.starts,
            false,
            observe,
        )?;
        continuation?;
        let seal = search
            .compilation
            .prepare_independent_objectives(source, earlier, results, observe)?;
        search.sealed = Some(seal);
        let slot = index(objective.codec);
        let share = search.compilation.ledger.begin_objective_share(3 - slot);
        let (_, allowance) = search.compilation.ledger.optional_search_work();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if objective.walk.starts && objective.optional_alternatives != 0 {
                let continuation =
                    search.continue_independent_baseline(policy, request, seeds, observe);
                search.explore_after_baseline(
                    policy,
                    request,
                    seeds,
                    baseline_renders,
                    continuation,
                    observe,
                );
            }
            search.challenge(policy, request)?;
            Ok::<_, SearchError>(())
        }));
        let (used, _) = search.compilation.ledger.optional_search_work();
        search.compilation.ledger.end_objective_share(share);
        match outcome {
            Ok(result) => result?,
            Err(payload) => resume_unwind(payload),
        }
        let stopped = search.stopped().map(|error| format!("{error:?}"));
        let winner = search
            .take_qualified_winner(objective.codec)
            .expect("every independent objective retains its qualified baseline or improvement");
        results[slot] = Some(IndependentSearchResult {
            winner,
            counters: search.counters(),
            terminal: search.terminal.take().unwrap(),
            stopped,
            optional_work_allowance: allowance,
            optional_work_used: used,
        });
        Ok(seal)
    }
}

impl JavaScriptSearch<'_, '_> {
    fn continue_independent_baseline(
        &mut self,
        policy: &ResolvedPolicy,
        request: SearchRequest,
        seeds: &[Style],
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<(), SearchError> {
        let candidate = self.states[0].as_ref().unwrap().candidate;
        optional_preflight(
            self.counters,
            policy.objective().unwrap(),
            request.objectives,
        )?;
        let Self {
            compilation,
            states,
            portfolio,
            counters,
            ..
        } = self;
        // Mandatory targets were dropped before sealing all objectives. Only
        // this objective's optional continuation is formed again; neither its
        // baseline nor another objective's score is added to the portfolio.
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
                    0,
                    policy,
                    request.objectives,
                    seeds,
                    Evaluation::ContinueBaseline,
                    observe,
                )
            },
        )??;
        Ok(())
    }
}
