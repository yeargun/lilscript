//! One owner for pending artifacts and exact incumbents. Staging, delayed
//! scoring and immediate scoring use the same bytes, provenance and promotion.
use super::*;
use crate::compilation_policy::CodecSchedule;
use crate::semantic_program::artifacts::ArtifactArena;
use crate::semantic_program::search_entries::{Entries, Entry};

#[cfg(test)]
#[path = "search_memory_tests.rs"]
mod memory_tests;
#[cfg(test)]
#[path = "search_resource_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "search_score_reuse_tests.rs"]
mod reuse_tests;

pub(super) struct Portfolio {
    pub(super) entries: Entries,
    pub(super) selected: [Option<usize>; 3],
    pub(super) baseline: [Option<usize>; 3],
    baseline_qualified: [Option<QualifiedArtifact>; 3],
    pub(super) baseline_capacity: usize,
    pub(super) baseline_raw: Option<usize>,
    owner: RevisionId,
    ordinal: u64,
    score_events: usize,
    // Physical dependency-bearing entries only; preserves the Whole fast path.
    resource_entries: usize,
}
impl Portfolio {
    pub(super) fn new(owner: RevisionId) -> Self {
        Self {
            entries: Entries::new(owner),
            selected: [None; 3],
            baseline: [None; 3],
            baseline_qualified: [None; 3],
            baseline_capacity: 0,
            baseline_raw: None,
            owner,
            ordinal: 0,
            score_events: 0,
            resource_entries: 0,
        }
    }
    /// The pool owns primary strings and shares fixed producer strings. Count
    /// each actual dependency allocation once among the selected entry subset.
    /// This is a bounded scan of existing owners, not a resource index. Equal
    /// bytes in distinct allocations still count separately; semantic identity
    /// and codec costs do not use this allocation equality.
    fn retained_package_bytes(
        &self,
        arena: &ArtifactArena,
        budget: &mut AllocationBudget<'_>,
        primary_bytes: usize,
        include: impl Fn(usize) -> bool,
    ) -> Result<usize, SearchError> {
        if self.resource_entries == 0 {
            return Ok(primary_bytes);
        }
        let mut bytes = primary_bytes;
        budget.work(WorkKind::Analysis, self.entries.capacity() as u64)?;
        for (position, entry) in self.entries.iter() {
            if !include(position) {
                continue;
            }
            let Some(dependency) = arena.dependency(entry.artifact)? else {
                continue;
            };
            let mut first = true;
            budget.work(WorkKind::Analysis, self.entries.capacity() as u64)?;
            for (previous, other) in self.entries.iter() {
                if previous == position {
                    break;
                }
                if include(previous)
                    && arena
                        .dependency(other.artifact)?
                        .is_some_and(|other| dependency.same_owner(other))
                {
                    first = false;
                    break;
                }
            }
            if first {
                bytes = bytes
                    .checked_add(dependency.view().retained_capacity)
                    .ok_or(AllocationError::Capacity)?;
            }
        }
        Ok(bytes)
    }

    pub(super) fn pins(
        &self,
        state: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        self.entries.pins(state, budget)
    }
    pub(super) fn stage(
        &mut self,
        output: &mut BudgetedJavaScriptOutput<'_, '_>,
        state: usize,
        artifact: ScopedArtifactId,
        render_work: u64,
    ) -> Result<usize, SearchError> {
        let (capacity, raw, resource) = output.with_artifact(artifact, |view| {
            (
                view.retained_capacity,
                view.sizes.raw,
                view.dependency.is_some(),
            )
        })?;
        let resource_entries = self
            .resource_entries
            .checked_add(usize::from(resource))
            .ok_or(AllocationError::Capacity)?;
        let ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or(AllocationError::Capacity)?;
        let ticket = output
            .with_allocation_budget(|budget| self.entries.prepare_insert(capacity, budget))?;
        let artifact = output.retain_artifact(artifact)?;
        // Every fallible operation is complete before moving either owner.
        let position = self.entries.insert_prepared(
            ticket,
            Entry {
                artifact,
                state,
                qualified: [None; 3],
                capacity,
                raw,
                render_work,
                ordinal: self.ordinal,
                pending: false,
            },
        );
        self.ordinal = ordinal;
        self.resource_entries = resource_entries;
        Ok(position)
    }
    /// A staged queue jointly consumes the retained count/byte pool. When the
    /// pool fills, score its oldest pending member through this same protocol.
    /// This pressure event makes useful progress without an extra byte store.
    /// A single provisional trial may replace a full incumbent portfolio.
    pub(super) fn consider(
        &mut self,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
        states: &mut [Option<State>],
        formation_contract: &CompilationContract,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        position: usize,
        baseline: bool,
        counters: &mut SearchCounters,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<(), SearchError> {
        let objective = policy.objective().unwrap();
        if baseline
            || objectives == Objectives::One(Objective::Raw)
            || objective.search.codec_schedule == CodecSchedule::Immediate
        {
            return self.score(
                arena,
                budget,
                states,
                formation_contract,
                policy,
                objectives,
                position,
                baseline,
                counters,
                observe,
            );
        }
        let limit = objective
            .retained_candidate_bytes
            .max(self.baseline_capacity);
        loop {
            budget.work(WorkKind::Analysis, 1)?;
            if self.entries.len() <= objective.retained_candidates.max(1)
                && self.retained_package_bytes(
                    arena,
                    budget,
                    self.entries.retained_text_bytes(),
                    |_| true,
                )? <= limit
            {
                self.entries.get_mut(position).unwrap().pending = true;
                counters.queued_artifacts += 1;
                counters.pending_peak = counters.pending_peak.max(self.pending_count(budget)?);
                return Ok(());
            }
            let Some(oldest) = self.oldest(budget)? else {
                return self.score(
                    arena,
                    budget,
                    states,
                    formation_contract,
                    policy,
                    objectives,
                    position,
                    false,
                    counters,
                    observe,
                );
            };
            counters.pressure_scores += 1;
            self.score(
                arena,
                budget,
                states,
                formation_contract,
                policy,
                objectives,
                oldest,
                false,
                counters,
                observe,
            )?;
        }
    }
    pub(super) fn score_next(
        &mut self,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
        states: &mut [Option<State>],
        formation_contract: &CompilationContract,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        counters: &mut SearchCounters,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<bool, SearchError> {
        let schedule = policy.objective().unwrap().search;
        let age_turn =
            self.score_events % schedule.diversity_interval == schedule.diversity_interval - 1;
        let selected = if age_turn {
            self.oldest(budget)?
        } else {
            self.promising(arena, states, budget)?
        };
        let Some(position) = selected else {
            return Ok(false);
        };
        self.score(
            arena,
            budget,
            states,
            formation_contract,
            policy,
            objectives,
            position,
            false,
            counters,
            observe,
        )?;
        if age_turn {
            counters.diversity_scores += 1;
        }
        Ok(true)
    }
    fn pending_count(&self, budget: &mut AllocationBudget<'_>) -> Result<usize, AllocationError> {
        let mut count = 0;
        budget.work(WorkKind::Analysis, self.entries.capacity() as u64)?;
        for (_, entry) in self.entries.iter() {
            count += usize::from(entry.pending);
        }
        Ok(count)
    }
    fn oldest(&self, budget: &mut AllocationBudget<'_>) -> Result<Option<usize>, AllocationError> {
        let mut selected: Option<(usize, u64)> = None;
        budget.work(WorkKind::Analysis, self.entries.capacity() as u64)?;
        for (position, entry) in self.entries.iter() {
            if entry.pending && selected.is_none_or(|(_, ordinal)| entry.ordinal < ordinal) {
                selected = Some((position, entry.ordinal));
            }
        }
        Ok(selected.map(|(position, _)| position))
    }
    fn promising(
        &self,
        arena: &ArtifactArena,
        states: &[Option<State>],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<usize>, SearchError> {
        let baseline_raw = self
            .baseline_raw
            .expect("mandatory complete artifact establishes the raw scheduling reference");
        let mut selected: Option<usize> = None;
        budget.work(WorkKind::Analysis, self.entries.capacity() as u64)?;
        for (position, entry) in self.entries.iter() {
            if !entry.pending {
                continue;
            }
            let preferred = if let Some(old) = selected {
                let old = self.entries.get(old).unwrap();
                let mut order = hint_order(entry, old, baseline_raw);
                if order == Ordering::Equal {
                    order = states[entry.state]
                        .as_ref()
                        .unwrap()
                        .identity
                        .compare(&states[old.state].as_ref().unwrap().identity, budget)?;
                }
                if order == Ordering::Equal {
                    order = arena
                        .provenance(entry.artifact)?
                        .compare_output(arena.provenance(old.artifact)?, budget)?;
                }
                order == Ordering::Less
            } else {
                true
            };
            if preferred {
                selected = Some(position);
            }
        }
        Ok(selected)
    }
    pub(super) fn score(
        &mut self,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
        states: &mut [Option<State>],
        formation_contract: &CompilationContract,
        policy: &ResolvedPolicy,
        objectives: Objectives,
        position: usize,
        baseline: bool,
        counters: &mut SearchCounters,
        observe: &mut impl FnMut(SearchObservation<'_>),
    ) -> Result<(), SearchError> {
        // Scoring consumes a pending trial before its first admission. A failed
        // codec, partial All score or promotion never re-enters the queue when
        // a memory refusal ends discovery and starts finalization.
        self.entries.get_mut(position).unwrap().pending = false;
        let objective = policy.objective().unwrap();
        let artifact = self.entries.get(position).unwrap().artifact;
        budget.work(WorkKind::Analysis, 1)?;
        let known = arena.reuse_scores(artifact, objectives, budget)?;
        let missing = objectives
            .iter()
            .filter(|&codec| known.get(codec).is_none())
            .count();
        if !baseline
            && missing
                > objective
                    .optional_codec_probes
                    .saturating_sub(counters.codec_probes)
        {
            return Err(SearchError::Limit(SearchLimit::CodecProbes));
        }
        for codec in objectives.iter() {
            if !baseline && known.get(codec).is_none() {
                counters.codec_probes += 1;
            }
            arena.measure(artifact, codec, budget)?;
        }
        let sizes = arena.with_artifact(artifact, |view| view.sizes)?;
        let entry = self.entries.get(position).unwrap();
        let state = entry.state;
        let mut replace = [false; 3];
        let mut qualified = [None; 3];
        for codec in objectives.iter() {
            let i = index(codec);
            let admission = arena.qualify(
                artifact,
                formation_contract,
                policy,
                codec,
                ArtifactRuntimeEvidence::default(),
                self.baseline_qualified[i].as_ref(),
                budget,
            )?;
            let cost = admission.cost();
            let base = self.baseline_qualified[i].map_or(cost, |baseline| baseline.cost());
            qualified[i] = Some(admission);
            let Some(incumbent) = self.selected[i].map(|slot| self.entries.get(slot).unwrap())
            else {
                replace[i] = true;
                continue;
            };
            budget.work(WorkKind::Analysis, 1)?;
            let previous = arena.with_artifact(incumbent.artifact, |view| {
                CandidateCostEvidence::size_only(view.sizes.get(codec).unwrap() as u64)
            })?;
            let mut order = policy
                .compare_evidence(cost, previous, base)?
                .ok_or(CandidateError::NotJavaScript)?;
            if order == Ordering::Equal {
                order = states[state]
                    .as_ref()
                    .unwrap()
                    .identity
                    .compare(&states[incumbent.state].as_ref().unwrap().identity, budget)?;
            }
            if order == Ordering::Equal {
                order = arena
                    .provenance(entry.artifact)?
                    .compare_output(arena.provenance(incumbent.artifact)?, budget)?;
            }
            replace[i] = order == Ordering::Less;
        }
        if baseline {
            self.baseline_capacity =
                self.retained_package_bytes(arena, budget, entry.capacity, |slot| {
                    slot == position
                })?;
            self.baseline_raw = Some(entry.raw);
            self.baseline = std::array::from_fn(|i| {
                objectives
                    .iter()
                    .any(|codec| index(codec) == i)
                    .then(|| sizes.get(codec(i)).unwrap())
            });
            self.baseline_qualified = qualified;
        }
        if replace.iter().any(|&yes| yes) {
            let mut count = 1usize;
            let mut primary = entry.capacity;
            budget.work(WorkKind::Analysis, self.entries.capacity() as u64)?;
            for (slot, other) in self.entries.iter() {
                if slot != position && (0..3).any(|i| !replace[i] && self.selected[i] == Some(slot))
                {
                    count += 1;
                    primary = primary
                        .checked_add(other.capacity)
                        .ok_or(AllocationError::Capacity)?;
                }
            }
            let retained = self.retained_package_bytes(arena, budget, primary, |slot| {
                slot == position || (0..3).any(|i| !replace[i] && self.selected[i] == Some(slot))
            })?;
            if !baseline && count > objective.retained_candidates.max(1) {
                return Err(SearchError::Limit(SearchLimit::ArtifactCount));
            }
            if !baseline
                && retained
                    > objective
                        .retained_candidate_bytes
                        .max(self.baseline_capacity)
            {
                return Err(SearchError::Limit(SearchLimit::ArtifactBytes));
            }
        }
        // No fallible admission remains before the selected union changes.
        let old_selected = self.selected;
        self.entries.get_mut(position).unwrap().pending = false;
        self.entries.get_mut(position).unwrap().qualified = qualified;
        for i in 0..3 {
            if replace[i] {
                self.selected[i] = Some(position);
            }
        }
        // A pressure score runs while a separately staged trial is provisional.
        // Only displaced incumbents belong to this promotion's cleanup set.
        for slot in old_selected.into_iter().flatten() {
            if slot != position
                && !self.selected.contains(&Some(slot))
                && self.entries.get(slot).is_some()
            {
                self.discard_entry(slot, arena, budget);
            }
        }
        for codec in objectives.iter() {
            let best = &mut states[state].as_mut().unwrap().best[index(codec)];
            let value = sizes.get(codec).unwrap();
            *best = Some(best.map_or(value, |previous| previous.min(value)));
        }
        counters.admitted_artifacts += 1;
        if !baseline {
            self.score_events += 1;
            counters.scoring_events += 1;
        }
        arena.with_artifact(artifact, |view| {
            observe(SearchObservation {
                candidate: view.candidate,
                recipe_fingerprint: view.recipe_fingerprint,
                recipe_descriptor: view.implementation,
                naming: arena
                    .provenance(artifact)
                    .expect("search owns artifact provenance")
                    .naming(),
                output: view.output,
                sizes,
                javascript: view.javascript,
                dependency: view.dependency,
                baseline,
            })
        })?;
        if !self.selected.contains(&Some(position)) {
            self.discard_entry(position, arena, budget);
        }
        Ok(())
    }
    fn discard_entry(
        &mut self,
        position: usize,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
    ) {
        let artifact = self.entries.get(position).unwrap().artifact;
        let resource = arena
            .dependency(artifact)
            .expect("search owns entry artifact")
            .is_some();
        let entry = self.entries.remove(position);
        self.resource_entries -= usize::from(resource);
        arena
            .discard(entry.artifact, budget)
            .expect("search owns every entry artifact");
    }
    pub(super) fn abandon_pending(
        &mut self,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
    ) {
        for position in 0..self.entries.capacity() {
            if self.entries.get(position).is_some() && !self.selected.contains(&Some(position)) {
                self.discard_entry(position, arena, budget);
            }
        }
    }
    pub(super) fn discard_provisional(
        &mut self,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
    ) {
        for position in 0..self.entries.capacity() {
            if self
                .entries
                .get(position)
                .is_some_and(|entry| !entry.pending)
                && !self.selected.contains(&Some(position))
            {
                self.discard_entry(position, arena, budget);
            }
        }
    }
    pub(super) fn discard_all(
        &mut self,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
    ) {
        for position in 0..self.entries.capacity() {
            if self.entries.get(position).is_some() {
                self.discard_entry(position, arena, budget);
            }
        }
        self.selected = [None; 3];
        debug_assert_eq!(self.resource_entries, 0);
        let entries = std::mem::replace(&mut self.entries, Entries::new(self.owner));
        budget.with_ledger(|ledger| entries.discard(self.owner, ledger.unwrap().0).unwrap());
    }
    pub(super) fn take_winner_artifact(
        &mut self,
        arena: &ArtifactArena,
        _budget: &mut AllocationBudget<'_>,
        objective: Objective,
    ) -> Option<ArtifactId> {
        let position = self.selected[index(objective)]?;
        let artifact = self.entries.get(position).unwrap().artifact;
        let resource = arena
            .dependency(artifact)
            .expect("search owns selected artifact")
            .is_some();
        for slot in &mut self.selected {
            if *slot == Some(position) {
                *slot = None;
            }
        }
        let entry = self.entries.remove(position);
        self.resource_entries -= usize::from(resource);
        Some(entry.artifact)
    }

    pub(super) fn take_winner(
        &mut self,
        arena: &mut ArtifactArena,
        budget: &mut AllocationBudget<'_>,
        objective: Objective,
    ) -> Option<String> {
        let position = self.selected[index(objective)]?;
        let artifact = self.entries.get(position).unwrap().artifact;
        if arena
            .dependency(artifact)
            .expect("search owns selected artifact")
            .is_some()
        {
            return None;
        }
        for slot in &mut self.selected {
            if *slot == Some(position) {
                *slot = None;
            }
        }
        let entry = self.entries.remove(position);
        let text = arena
            .take(entry.artifact, budget)
            .expect("search owns selected artifact");
        Some(text)
    }
}

/// Raw benefit per observed render work is a scheduling proxy only. Losses
/// remain available through the deterministic age lane, never proven dominated.
fn hint_order(left: &Entry, right: &Entry, baseline: usize) -> Ordering {
    let left_benefit = baseline.checked_sub(left.raw);
    let right_benefit = baseline.checked_sub(right.raw);
    match (left_benefit, right_benefit) {
        (Some(left_gain), Some(right_gain)) => {
            let left_ratio = (left_gain as u128) * u128::from(right.render_work.max(1));
            let right_ratio = (right_gain as u128) * u128::from(left.render_work.max(1));
            right_ratio
                .cmp(&left_ratio)
                .then_with(|| left.render_work.cmp(&right.render_work))
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left
            .raw
            .cmp(&right.raw)
            .then_with(|| left.render_work.cmp(&right.render_work)),
    }
}
