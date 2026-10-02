//! Protected effort checkpoints. All baseline owners are created before the
//! one seal; lower tiers finish before the wider frontier spends optional work.
//! Only complete, freshly qualified artifacts survive between tiers.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EffortCheckpoint {
    pub level: u8,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fast_stops: Vec<(&'static str, Vec<terminal::Stop>)>,
    pub objectives: Vec<(&'static str, usize, String)>,
    pub proposals: usize,
    pub renders: usize,
    pub judgments: usize,
    pub optional_work: u64,
    pub stopped: Option<String>,
}

impl<'src> Compilation<'src> {
    pub(super) fn prepare_effort_search<'a>(
        &'a mut self,
        source: SemanticId,
        policy: &ResolvedPolicy,
        request: SearchRequest,
        before_optional: &mut dyn FnMut(
            &mut Compilation<'src>,
        ) -> Result<BaselineSeal, SearchError>,
        observe: &mut dyn FnMut(SearchObservation<'_>),
    ) -> Result<JavaScriptSearch<'a, 'src>, SearchError> {
        let objective = policy.objective().ok_or(CandidateError::NotJavaScript)?;
        let seeds = Plan::seeds_for_policy(policy).map_err(CandidateError::from)?;
        let mut search = self.prepare_javascript_search(source, policy)?;
        let (renders, continuation) = search.evaluate_baseline(
            policy,
            request.objectives,
            &seeds[..1],
            objective.walk.starts,
            false,
            &mut |event| observe(event),
        )?;
        continuation?;
        let mut seal = None;
        let lower_policy = policy.preceding_effort();
        let mut qualifications = [None, None, None];
        if lower_policy.is_some() {
            for codec in request.objectives.iter() {
                qualifications[index(codec)] = Some(search.compilation.prepare_artifact_requalification(policy)?);
            }
        }
        let previous = if let Some(lower_policy) = lower_policy {
            let result = (|| {
                let mut lower = search.compilation.prepare_effort_search(
                    source,
                    &lower_policy,
                    request,
                    &mut |compilation| {
                        let receipt = before_optional(compilation)?;
                        seal = Some(receipt);
                        Ok(receipt)
                    },
                    &mut |_| {},
                )?;
                lower.challenge(&lower_policy, request)?;
                let mut row = EffortCheckpoint {
                    level: lower_policy.effort(),
                    fast_stops: Vec::new(),
                    objectives: Vec::new(),
                    proposals: lower.counters().proposals,
                    renders: lower.counters().renders,
                    judgments: lower.terminal_report().map_or(0, |report| {
                        report
                            .objectives
                            .iter()
                            .map(|objective| objective.judged)
                            .sum()
                    }),
                    optional_work: lower.compilation.ledger.optional_search_work().0,
                    stopped: lower.stopped().map(|error| format!("{error:?}")),
                };
                let mut receipts = [None; 3];
                let mut choices = [Vec::new(), Vec::new(), Vec::new()];
                for codec in request.objectives.iter() {
                    let receipt = *lower
                        .winner_qualification(codec)
                        .expect("lower tier keeps an incumbent");
                    lower.with_winner(codec, |view, _| {
                        row.objectives.push((
                            codec.name(),
                            view.sizes.get(codec).unwrap(),
                            terminal::delivered_digest(&view),
                        ));
                    });
                    receipts[index(codec)] = Some(receipt);
                    choices[index(codec)] = lower
                        .terminal_report()
                        .and_then(|report| {
                            report
                                .objectives
                                .iter()
                                .find(|stage| stage.codec == codec.name())
                        })
                        .map_or_else(Vec::new, |stage| stage.choices.clone());
                }
                // Fast-tier replay belongs to the actual level-12 policy.
                // A higher tier can enable new target tactics in its baseline.
                if lower_policy.effort() == 12 {
                    for &(codec, size, ref sha256) in &row.objectives {
                        let mut stops = lower
                            .terminal_report()
                            .and_then(|report| {
                                report.objectives.iter().find(|stage| stage.codec == codec)
                            })
                            .map_or_else(Vec::new, |stage| stage.stops.clone());
                        stops.push(terminal::Stop {
                            level: 12,
                            size,
                            sha256: sha256.clone(),
                        });
                        row.fast_stops.push((codec, stops));
                    }
                }
                let checkpoints = std::mem::take(&mut lower.effort_checkpoints);
                for codec in request.objectives.iter() {
                    let receipt = receipts[index(codec)].unwrap();
                    if receipts[..index(codec)]
                        .iter()
                        .flatten()
                        .any(|old| old.artifact() == receipt.artifact())
                    {
                        continue;
                    }
                    assert_eq!(
                        lower.take_qualified_winner(codec).unwrap().artifact(),
                        receipt.artifact()
                    );
                }
                Ok::<_, SearchError>((receipts, choices, checkpoints, row))
            })();
            match result {
                Ok(previous) => Some(previous),
                Err(error)
                    if seal.is_some()
                        && (terminal::resource(&error) || error.optional_memory_refusal()) =>
                {
                    search.effort_refusal = Some(format!("{error:?}"));
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        search.sealed = Some(match seal {
            Some(seal) => seal,
            None => before_optional(search.compilation)?,
        });
        if let Some((receipts, choices, mut checkpoints, row)) = previous {
            // Own every handoff before a fallible requalification or comparison.
            search.protected = receipts;
            search.protected_choices = choices;
            checkpoints.push(row);
            search.effort_checkpoints = checkpoints;
            search.qualify_protected(policy, request.objectives, qualifications)?;
        }
        if objective.walk.starts && objective.optional_alternatives != 0 {
            let continuation =
                search.continue_independent_baseline(policy, request, seeds, &mut |event| {
                    observe(event)
                });
            search.explore_after_baseline(
                policy,
                request,
                seeds,
                renders,
                continuation,
                &mut |event| observe(event),
            );
        }
        search.settle_protected(policy, request.objectives)?;
        Ok(search)
    }
}

impl JavaScriptSearch<'_, '_> {
    fn qualify_protected(
        &mut self,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        mut qualifications: [Option<crate::program::artifacts::QualificationCredit>; 3],
    ) -> Result<(), SearchError> {
        for codec in objectives.iter() {
            let Some(previous) = self.protected[index(codec)] else {
                continue;
            };
            let baseline = self.portfolio.baseline_qualification(codec).copied();
            self.protected[index(codec)] = Some(self.compilation.requalify_prepared_artifact(
                previous,
                policy,
                ArtifactRuntimeEvidence::default(),
                baseline.as_ref(),
                qualifications[index(codec)].take().expect("effort handoff was funded before exploration"),
            )?);
        }
        Ok(())
    }

    pub(super) fn settle_protected(
        &mut self,
        policy: &ResolvedPolicy,
        objectives: Objectives,
    ) -> Result<(), SearchError> {
        for codec in objectives.iter() {
            let i = index(codec);
            self.protected_selected[i] = false;
            let Some(previous) = self.protected[i] else {
                continue;
            };
            let current = self
                .portfolio
                .entries
                .get(self.portfolio.selected[i].unwrap())
                .unwrap();
            let current_receipt = current.qualified[i].unwrap();
            let base = self
                .portfolio
                .baseline_qualification(codec)
                .map_or(previous.cost(), |base| base.cost());
            let order = policy
                .compare_evidence(current_receipt.cost(), previous.cost(), base)?
                .ok_or(CandidateError::NotJavaScript)?;
            let new_raw = self
                .compilation
                .artifacts
                .with_artifact(current.artifact, |view| view.sizes.raw)?;
            let old_raw = self
                .compilation
                .artifacts
                .with_artifact(previous.artifact(), |view| view.sizes.raw)?;
            let improves =
                order == Ordering::Less || order == Ordering::Equal && new_raw <= old_raw;
            let dominant = if improves {
                self.compilation.artifacts.compare_rows(
                    current.artifact,
                    previous.artifact(),
                    codec,
                    &mut AllocationBudget::new(Some((
                        &mut self.compilation.ledger,
                        WorkDomain::Optional,
                    ))),
                    |new, old| new.len() == old.len() && new.iter().zip(old).all(|(a, b)| a <= b),
                )
            } else {
                Ok(false)
            };
            match dominant {
                Ok(dominant) => self.protected_selected[i] = !dominant,
                Err(error) => {
                    let error = SearchError::from(error);
                    if !terminal::resource(&error) && !error.optional_memory_refusal() {
                        return Err(error);
                    }
                    self.effort_refusal = Some(format!("{error:?}"));
                    self.protected_selected[i] = true;
                }
            }
        }
        Ok(())
    }

    pub(super) fn remove_protected(&mut self, codec: Objective) -> Option<ArtifactId> {
        let artifact = self.protected[index(codec)]?.artifact();
        for i in 0..3 {
            if self.protected[i].is_some_and(|entry| entry.artifact() == artifact) {
                self.protected[i] = None;
                self.protected_selected[i] = false;
                self.protected_choices[i].clear();
            }
        }
        Some(artifact)
    }

    pub(super) fn take_protected_winner(&mut self, codec: Objective) -> Option<ArtifactId> {
        let artifact = self.protected[index(codec)]?.artifact();
        for i in 0..3 {
            if self.protected_selected[i]
                && self.protected[i].is_some_and(|entry| entry.artifact() == artifact)
            {
                self.portfolio.selected[i] = None;
            }
        }
        self.remove_protected(codec)
    }

    pub(super) fn discard_protected(&mut self) {
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            if let Some(artifact) = self.remove_protected(codec) {
                self.compilation
                    .discard_artifact(artifact)
                    .expect("search owns effort handoffs");
            }
        }
    }
}
