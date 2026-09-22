//! One bounded metadata owner for queued artifacts and exact incumbents.
//! ArtifactArena owns immutable text and codec scores. Search owns disposal of
//! each entry's artifact; this container owns only its Vec backing.
use super::RevisionId;
use super::artifacts::{ArtifactId, QualifiedArtifact};
use crate::compilation_policy::{BudgetError, BudgetLedger, WorkKind};
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use std::mem::size_of;

#[derive(Debug)]
pub(super) struct Entry {
    pub(super) artifact: ArtifactId,
    pub(super) state: usize,
    pub(super) qualified: [Option<QualifiedArtifact>; 3],
    /// These measurements and identities remain immutable while stored. Only
    /// pending/scheduling metadata may change through get_mut.
    /// Primary allocation only. The Portfolio counts shared dependency text
    /// once across its existing entries; raw is the complete package score.
    pub(super) capacity: usize,
    pub(super) raw: usize,
    pub(super) render_work: u64,
    pub(super) ordinal: u64,
    pub(super) pending: bool,
}

/// A private, single-use insertion ticket. The owner may perform fallible work
/// after preparation, but must not mutate the entries before committing it.
#[derive(Debug)]
#[must_use = "insert the prepared entry or discard this unused ticket"]
pub(super) struct PreparedEntrySlot {
    owner: RevisionId,
    index: usize,
    capacity: usize,
    previous_count: usize,
    previous_bytes: usize,
    next_count: usize,
    next_bytes: usize,
}

#[derive(Debug)]
#[must_use = "Search must remove and dispose entries, then discard their backing"]
pub(super) struct Entries {
    owner: RevisionId,
    slots: Vec<Option<Entry>>,
    charge: Option<RetainedCharge<RevisionId>>,
    count: usize,
    text_bytes: usize,
}

impl Entries {
    pub(super) fn new(owner: RevisionId) -> Self {
        Self {
            owner,
            slots: Vec::new(),
            charge: None,
            count: 0,
            text_bytes: 0,
        }
    }

    pub(super) fn prepare_insert(
        &mut self,
        text_capacity: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<PreparedEntrySlot, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        let next_count = self.count.checked_add(1).ok_or(AllocationError::Capacity)?;
        let next_bytes = self
            .text_bytes
            .checked_add(text_capacity)
            .ok_or(AllocationError::Capacity)?;
        let mut index = self.slots.len();
        for (candidate, slot) in self.slots.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if slot.is_none() {
                index = candidate;
                break;
            }
        }
        if index == self.slots.len() && index == self.slots.capacity() {
            let capacity = self
                .slots
                .capacity()
                .max(1)
                .checked_mul(2)
                .ok_or(AllocationError::Capacity)?;
            let mut growth = budget.scope();
            growth.work(
                WorkKind::Analysis,
                u64::try_from(self.slots.len()).map_err(|_| AllocationError::Capacity)?,
            )?;
            let mut slots = growth.vector(Retained, capacity)?;
            let bytes = slots
                .capacity()
                .checked_mul(size_of::<Option<Entry>>())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or(AllocationError::Capacity)?;
            let charge = growth.detach_retained(self.owner, bytes)?;
            // All old backing is still charged while the exact new buffer is
            // allocated. After admission, moving entries cannot allocate/fail.
            slots.append(&mut self.slots);
            drop(std::mem::replace(&mut self.slots, slots));
            if let Some(old) = self.charge.replace(charge) {
                growth.with_ledger(|ledger| {
                    old.discard(
                        &self.owner,
                        ledger.expect("entry backing requires its ledger").0,
                    )
                    .unwrap_or_else(|_| panic!("search entry backing owner invariant"));
                });
            }
        }
        Ok(PreparedEntrySlot {
            owner: self.owner,
            index,
            capacity: text_capacity,
            previous_count: self.count,
            previous_bytes: self.text_bytes,
            next_count,
            next_bytes,
        })
    }

    pub(super) fn insert_prepared(&mut self, slot: PreparedEntrySlot, entry: Entry) -> usize {
        assert_eq!(slot.owner, self.owner, "prepared entry owner invariant");
        assert_eq!(
            slot.capacity, entry.capacity,
            "prepared text capacity invariant"
        );
        assert_eq!(
            (slot.previous_count, slot.previous_bytes),
            (self.count, self.text_bytes),
            "entries changed between preparation and insertion"
        );
        // Complete raw transfer includes a dependency; it need not fit inside
        // this entry's primary String capacity. Both text owners were admitted
        // and bounded by the output/Portfolio before publication.
        if slot.index == self.slots.len() {
            assert!(
                self.slots.len() < self.slots.capacity(),
                "entry insertion was not admitted"
            );
            self.slots.push(Some(entry));
        } else {
            assert!(
                self.slots[slot.index].is_none(),
                "prepared entry slot was reused"
            );
            self.slots[slot.index] = Some(entry);
        }
        self.count = slot.next_count;
        self.text_bytes = slot.next_bytes;
        slot.index
    }

    pub(super) fn get(&self, index: usize) -> Option<&Entry> {
        self.slots.get(index).and_then(Option::as_ref)
    }
    /// Artifact identity, provenance, capacity and raw length remain fixed.
    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut Entry> {
        self.slots.get_mut(index).and_then(Option::as_mut)
    }
    pub(super) fn pins(
        &self,
        state: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        // Inspect physical slots so holes cost work too; unused capacity does not.
        for slot in &self.slots {
            budget.work(WorkKind::Analysis, 1)?;
            if slot.as_ref().is_some_and(|entry| entry.state == state) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// The caller admits this bounded scan before traversing it.
    pub(super) fn iter(&self) -> impl Iterator<Item = (usize, &Entry)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_ref().map(|entry| (index, entry)))
    }
    pub(super) fn remove(&mut self, index: usize) -> Entry {
        let entry = self
            .slots
            .get_mut(index)
            .and_then(Option::take)
            .expect("Search owns each removed entry");
        self.count = self.count.checked_sub(1).expect("entry count invariant");
        self.text_bytes = self
            .text_bytes
            .checked_sub(entry.capacity)
            .expect("entry capacity invariant");
        entry
    }
    pub(super) fn len(&self) -> usize {
        self.count
    }
    pub(super) fn capacity(&self) -> usize {
        self.slots.capacity()
    }
    pub(super) fn retained_text_bytes(&self) -> usize {
        self.text_bytes
    }
    #[cfg(test)]
    pub(super) fn backing_bytes(&self) -> u64 {
        self.charge.as_ref().map_or(0, RetainedCharge::bytes)
    }

    pub(super) fn discard(
        self,
        owner: RevisionId,
        ledger: &mut BudgetLedger,
    ) -> Result<(), (Self, AllocationError)> {
        if owner != self.owner {
            return Err((self, AllocationError::WrongOwner));
        }
        if self.count != 0 {
            return Err((self, AllocationError::Budget(BudgetError::InvalidRelease)));
        }
        let Self { slots, charge, .. } = self;
        drop(slots);
        if let Some(charge) = charge {
            charge
                .discard(&owner, ledger)
                .unwrap_or_else(|_| panic!("search entry backing owner invariant"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{
        BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
    };
    use crate::semantic_program::publication::{
        BudgetedJavaScriptOutput, CheckpointLimit, Compilation,
    };
    use crate::structured_js::selection::{Plan, Style};

    const WORK: u64 = 10_000_000;
    const MEMORY: u64 = 4_000_000;

    fn with_output(
        inspect: impl FnOnce(&mut BudgetedJavaScriptOutput<'_, '_>, &ResolvedPolicy, RevisionId),
    ) {
        let arena = bumpalo::Bump::new();
        let syntax =
            crate::parse_source(&arena, "export int value(int input){return (input&255)+1;}")
                .unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = crate::semantic_program::from_checked_source(&syntax, &semantics).unwrap();
        let config: crate::config::ProjectConfig = toml::from_str(
            "[javascript]\nstrip_console=false\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'",
        )
        .unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
        .unwrap();
        let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 8 }).unwrap();
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compilation
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        // This private metadata owner uses the compilation's actual ledger;
        // artifact handles themselves come only from its public render path.
        let owner = RevisionId::fresh();
        compilation
            .with_javascript_output(direct, &policy, |output| inspect(output, &policy, owner))
            .unwrap();
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }

    fn with_ledger<R>(
        output: &BudgetedJavaScriptOutput<'_, '_>,
        inspect: impl FnOnce(&mut BudgetLedger) -> R,
    ) -> R {
        output.with_allocation_budget(|budget| {
            budget.with_ledger(|ledger| inspect(ledger.unwrap().0))
        })
    }

    fn insert(
        entries: &mut Entries,
        output: &mut BudgetedJavaScriptOutput<'_, '_>,
        _policy: &ResolvedPolicy,
        style: Style,
        ordinal: u64,
    ) -> usize {
        let plan = Plan::new(style);
        let artifact = output.render(&plan).unwrap();
        let (raw, capacity) = output
            .with_artifact(artifact, |view| {
                (view.javascript.len(), view.retained_capacity)
            })
            .unwrap();
        let slot = output
            .with_allocation_budget(|budget| entries.prepare_insert(capacity, budget))
            .unwrap();
        let artifact = output.retain_artifact(artifact).unwrap();
        entries.insert_prepared(
            slot,
            Entry {
                artifact,
                state: usize::try_from(ordinal).unwrap(),
                qualified: [None; 3],
                capacity,
                raw,
                render_work: 1,
                ordinal,
                pending: true,
            },
        )
    }

    fn dispose(output: &mut BudgetedJavaScriptOutput<'_, '_>, _owner: RevisionId, entry: Entry) {
        output
            .with_retained_arena(|arena, budget| arena.discard(entry.artifact, budget))
            .unwrap();
    }

    fn finish(mut entries: Entries, output: &mut BudgetedJavaScriptOutput<'_, '_>) {
        for index in 0..entries.capacity() {
            if entries.get(index).is_some() {
                let entry = entries.remove(index);
                dispose(output, entries.owner, entry);
            }
        }
        let owner = entries.owner;
        with_ledger(output, |ledger| entries.discard(owner, ledger).unwrap());
    }

    #[test]
    fn real_artifact_slots_reuse_without_copying_text_or_codec_scores() {
        with_output(|output, policy, owner| {
            let mut entries = Entries::new(owner);
            let first = insert(&mut entries, output, policy, Style::Global, 0);
            let second = insert(&mut entries, output, policy, Style::Source, 1);
            let backing = entries.backing_bytes();
            let capacity = entries.capacity();
            let second_id = entries.get(second).unwrap().artifact;
            let second_text = output
                .with_retained_arena(|arena, _| {
                    arena.with_artifact(second_id, |view| {
                        assert_eq!(view.sizes.gzip9, None);
                        assert_eq!(view.sizes.brotli11, None);
                        view.javascript.to_owned()
                    })
                })
                .unwrap();
            dispose(output, owner, entries.remove(first));
            let replacement = insert(&mut entries, output, policy, Style::Scoped, 2);
            assert_eq!(replacement, first);
            assert_eq!(entries.capacity(), capacity);
            assert_eq!(entries.backing_bytes(), backing);
            assert_eq!(entries.len(), 2);
            assert_eq!(
                entries.retained_text_bytes(),
                entries.iter().map(|(_, e)| e.capacity).sum::<usize>()
            );
            entries.get_mut(second).unwrap().pending = false;
            assert!(!entries.get(second).unwrap().pending);
            output
                .with_retained_arena(|arena, _| {
                    arena.with_artifact(second_id, |view| {
                        assert_eq!(view.javascript, second_text);
                        assert_eq!(entries.get(second).unwrap().raw, view.javascript.len());
                    })
                })
                .unwrap();
            finish(entries, output);
        });
    }

    #[test]
    fn backing_growth_charges_coexisting_buffers_and_releases_original_domain() {
        with_output(|output, policy, owner| {
            let mut entries = Entries::new(owner);
            insert(&mut entries, output, policy, Style::Global, 0);
            insert(&mut entries, output, policy, Style::Source, 1);
            assert_eq!(entries.len(), entries.capacity());
            let old = entries.backing_bytes();
            let before = with_ledger(output, |ledger| {
                (
                    ledger.retained_bytes(),
                    ledger.retained_bytes_in(WorkDomain::Baseline),
                    ledger.retained_bytes_in(WorkDomain::Optional),
                )
            });
            // Leave the ticket unused: preparation owns admitted metadata even
            // when later candidate admission declines before membership changes.
            let ticket = with_ledger(output, |ledger| {
                entries.prepare_insert(
                    0,
                    &mut AllocationBudget::new(Some((ledger, WorkDomain::Optional))),
                )
            })
            .unwrap();
            drop(ticket);
            let new = entries.backing_bytes();
            assert_eq!(new, old * 2);
            with_ledger(output, |ledger| {
                assert_eq!(ledger.retained_bytes(), before.0 + new - old);
                assert_eq!(
                    ledger.retained_bytes_in(WorkDomain::Baseline),
                    before.1 - old
                );
                assert_eq!(
                    ledger.retained_bytes_in(WorkDomain::Optional),
                    before.2 + new
                );
                assert!(ledger.peak_retained_bytes() >= before.0 + new);
            });
            assert_eq!(entries.len(), 2);
            finish(entries, output);
            with_ledger(output, |ledger| {
                assert_eq!(ledger.retained_bytes_in(WorkDomain::Optional), before.2);
            });
        });
    }

    #[test]
    fn denied_growth_and_aggregate_overflow_preserve_existing_artifacts() {
        with_output(|output, policy, owner| {
            let mut entries = Entries::new(owner);
            let first = insert(&mut entries, output, policy, Style::Global, 0);
            insert(&mut entries, output, policy, Style::Source, 1);
            let id = entries.get(first).unwrap().artifact;
            let before = (
                entries.capacity(),
                entries.backing_bytes(),
                entries.retained_text_bytes(),
            );
            let new_buffer = before.1 * 2;
            with_ledger(output, |ledger| {
                let live = ledger.retained_bytes();
                let padding = MEMORY - live - (new_buffer - 1);
                ledger.retain(WorkDomain::Baseline, padding).unwrap();
                let filled = ledger.retained_bytes();
                let denied = entries.prepare_insert(
                    0,
                    &mut AllocationBudget::new(Some((ledger, WorkDomain::Optional))),
                );
                assert!(matches!(
                    denied,
                    Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                        WorkDomain::Optional
                    )))
                ));
                assert_eq!(ledger.retained_bytes(), filled);
                ledger.release(WorkDomain::Baseline, padding).unwrap();
                let overflow = entries.prepare_insert(
                    usize::MAX,
                    &mut AllocationBudget::new(Some((ledger, WorkDomain::Optional))),
                );
                assert!(matches!(overflow, Err(AllocationError::Capacity)));
                assert_eq!(ledger.retained_bytes(), live);
                let remaining_work = WORK - ledger.work_used(WorkDomain::Optional);
                ledger
                    .charge(WorkDomain::Optional, WorkKind::Analysis, remaining_work)
                    .unwrap();
                let denied = entries.prepare_insert(
                    0,
                    &mut AllocationBudget::new(Some((ledger, WorkDomain::Optional))),
                );
                assert!(matches!(
                    denied,
                    Err(AllocationError::Budget(BudgetError::WorkExhausted(
                        WorkDomain::Optional
                    )))
                ));
                assert_eq!(ledger.retained_bytes(), live);
            });
            assert_eq!(
                (
                    entries.capacity(),
                    entries.backing_bytes(),
                    entries.retained_text_bytes()
                ),
                before
            );
            assert_eq!(entries.len(), 2);
            output
                .with_retained_arena(|arena, _| {
                    arena.with_artifact(id, |view| assert!(!view.javascript.is_empty()))
                })
                .unwrap();
            finish(entries, output);
        });
    }

    #[test]
    fn rejected_disposal_returns_all_owned_entries_then_empty_release_succeeds() {
        with_output(|output, policy, owner| {
            let mut entries = Entries::new(owner);
            let index = insert(&mut entries, output, policy, Style::Global, 0);
            let bytes = entries.backing_bytes();
            let (returned, error) = with_ledger(output, |ledger| {
                entries.discard(RevisionId::fresh(), ledger).unwrap_err()
            });
            assert_eq!(error, AllocationError::WrongOwner);
            entries = returned;
            let (returned, error) =
                with_ledger(output, |ledger| entries.discard(owner, ledger).unwrap_err());
            assert_eq!(error, AllocationError::Budget(BudgetError::InvalidRelease));
            entries = returned;
            assert_eq!(entries.len(), 1);
            assert_eq!(entries.backing_bytes(), bytes);
            dispose(output, owner, entries.remove(index));
            let live = with_ledger(output, |ledger| ledger.retained_bytes());
            with_ledger(output, |ledger| {
                entries.discard(owner, ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), live - bytes);
            });
        });
    }

    #[test]
    fn pin_lookup_admits_each_visited_slot_and_stops_at_the_first_match() {
        for holes in [false, true] {
            for state in [0, 1, 2, 99] {
                let found = state < 3 && (!holes || state == 2);
                let visits = if found { state + 1 } else { 3 };
                for allowance in 0..=visits {
                    with_output(|output, policy, owner| {
                        let mut entries = Entries::new(owner);
                        for ordinal in 0..3 {
                            insert(&mut entries, output, policy, Style::Global, ordinal);
                        }
                        assert_eq!(entries.capacity(), 4);
                        if holes {
                            for index in 0..2 {
                                dispose(output, owner, entries.remove(index));
                            }
                        }
                        let identities: Vec<_> = entries
                            .iter()
                            .map(|(index, entry)| (index, entry.artifact))
                            .collect();
                        with_ledger(output, |ledger| {
                            let remaining = WORK - ledger.work_used(WorkDomain::Optional);
                            ledger
                                .charge(
                                    WorkDomain::Optional,
                                    WorkKind::Analysis,
                                    remaining - allowance as u64,
                                )
                                .unwrap();
                            let retained = ledger.retained_bytes();
                            let result = entries.pins(
                                state,
                                &mut AllocationBudget::new(Some((ledger, WorkDomain::Optional))),
                            );
                            if allowance < visits {
                                assert_eq!(
                                    result,
                                    Err(AllocationError::Budget(BudgetError::WorkExhausted(
                                        WorkDomain::Optional
                                    )))
                                );
                            } else {
                                assert_eq!(result, Ok(found));
                            }
                            assert_eq!(ledger.work_used(WorkDomain::Optional), WORK);
                            assert_eq!(ledger.retained_bytes(), retained);
                        });
                        assert_eq!(
                            entries
                                .iter()
                                .map(|(index, entry)| (index, entry.artifact))
                                .collect::<Vec<_>>(),
                            identities
                        );
                        finish(entries, output);
                    });
                }
            }
        }
    }
}
