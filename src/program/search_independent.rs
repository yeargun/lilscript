//! Independent objective portfolios over one checked program and ledger.
//! Nested exclusive borrows retain all mandatory owners until the one seal;
//! unwinding the bounded output stack searches in canonical output/codec order.
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
        requests: &[(&ResolvedPolicy, SearchRequest)],
        decisions: &[Option<SavedDecision>; 3],
        mut observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<[Option<IndependentSearchResult>; 3], SearchError> {
        let expanded: Vec<_> = requests.iter().map(|&(policy, request)| (policy, request, decisions)).collect();
        let results = self.search_javascript_outputs(source, &expanded, observe)?;
        let mut by_codec = [None, None, None];
        for ((policy, _), result) in requests.iter().zip(results) {
            by_codec[index(policy.objective().unwrap().codec)] = result;
        }
        Ok(by_codec)
    }

    pub(crate) fn search_javascript_outputs(
        &mut self,
        source: SemanticId,
        requests: &[(&ResolvedPolicy, SearchRequest, &[Option<SavedDecision>; 3])],
        mut observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<Vec<Option<IndependentSearchResult>>, SearchError> {
        assert!((1..=crate::config::MAX_DELIVERY_OUTPUTS * 3).contains(&requests.len()));
        for (policy, request, _) in requests {
            assert_eq!(request.objectives, Objectives::One(policy.objective().unwrap().codec));
        }
        let mut results = (0..requests.len()).map(|_| None).collect::<Vec<_>>();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.prepare_independent_objectives(
                source,
                requests,
                requests.len(),
                &mut results,
                &mut observe,
            )
        }));
        match outcome {
            Ok(Ok(_)) => Ok(results),
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
        requests: &[(&ResolvedPolicy, SearchRequest, &[Option<SavedDecision>; 3])],
        total: usize,
        results: &mut [Option<IndependentSearchResult>],
        observe: &mut dyn FnMut(SearchObservation<'_>),
    ) -> Result<BaselineSeal, SearchError> {
        let Some((&(policy, request, decisions), earlier)) = requests.split_last() else {
            // Every requested incumbent and score now exists. No later
            // objective needs to reopen Baseline or guess a memory reserve.
            return Ok(self.ledger.seal_baseline()?);
        };
        let codec = policy.objective().unwrap().codec;
        let slot = requests.len() - 1;
        let observer = std::cell::RefCell::new(observe);
        let mut share = None;
        let mut allowance = 0;
        let mut seal = None;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut search = self.prepare_effort_search(source, policy, request,
                &mut |compilation| {
                    let ready = compilation.prepare_independent_objectives(source, earlier, total,
                        results, &mut |event| observer.borrow_mut()(event))?;
                    seal = Some(ready);
                    share = Some(compilation.ledger.begin_objective_share(total + 1 - requests.len()));
                    allowance = compilation.ledger.optional_search_work().1;
                    Ok(ready)
                }, &mut |event| observer.borrow_mut()(event))?;
            search.challenge_with_decisions(policy, request, decisions)?;
            let used = search.compilation.ledger.optional_search_work().0;
            let stopped = search.stopped().map(|error| format!("{error:?}"));
            let winner = search.take_qualified_winner(codec)
                .expect("every objective retains its qualified baseline or improvement");
            results[slot] = Some(IndependentSearchResult {
                winner, counters: search.counters(), terminal: search.terminal.take().unwrap(), stopped,
                optional_work_allowance: allowance, optional_work_used: used,
            });
            Ok::<_, SearchError>(())
        }));
        if let Some(share) = share { self.ledger.end_objective_share(share); }
        match outcome {
            Ok(result) => result?,
            Err(payload) => resume_unwind(payload),
        }
        Ok(seal.expect("all mandatory objectives precede optional work"))
    }
}

impl JavaScriptSearch<'_, '_> {
    pub(super) fn continue_independent_baseline(
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
