//! Stable local proofs depend on an immutable body revision and, for
//! reachability, whether a live closure creates it. Repeated proofs admit the
//! same complete stage with physical reuse enabled or disabled.
use super::*;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};

#[derive(Clone, Copy)]
pub(super) enum LocalRule {
    Returns,
    Unreachable,
}
#[derive(Clone, Copy)]
struct Stable {
    revision: RevisionId,
    created: bool,
    admission: Option<crate::admission_replay::Receipt>,
}

pub(super) struct DirtyUnits {
    seen: Vec<[Option<Stable>; 2]>,
    enabled: bool,
}

impl DirtyUnits {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            seen: Vec::new(),
            enabled,
        }
    }
    pub(super) fn release(self, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        super::storage::release_vec(self.seen, Scratch, budget)
    }
    pub(super) fn prepare(
        &mut self,
        count: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if count > self.seen.len() {
            let added = count - self.seen.len();
            budget.reserve_vec(Scratch, &mut self.seen, added)?;
            self.seen.resize(count, [None; 2]);
        }
        Ok(())
    }
    pub(super) fn plan<T: Default>(
        &mut self,
        program: &Program<'_>,
        unit: UnitId,
        rule: LocalRule,
        created: bool,
        receipt: &mut RuleReceipt,
        budget: &mut AllocationBudget<'_>,
        build: impl FnOnce(&mut AllocationBudget<'_>) -> Result<T, AllocationError>,
        empty: impl FnOnce(&T) -> bool,
    ) -> Result<T, AllocationError> {
        debug_assert!(self.seen.len() >= program.units.len());
        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        let revision = program.units[unit.index()].revision();
        let slot = &mut self.seen[unit.index()][rule as usize];
        if let Some(stable) = slot.filter(|old| old.revision == revision && old.created == created)
        {
            let mut run = || {
                if self.enabled {
                    receipt.local_units_reused += 1;
                    Ok(T::default())
                } else {
                    receipt.local_units_visited += 1;
                    build(&mut AllocationBudget::new(None))
                }
            };
            return match stable.admission {
                Some(admission) => budget.replay(&admission, run),
                None => run(), // Inspection has no logical ledger to replay.
            };
        }
        receipt.local_units_visited += 1;
        let (result, admission) = budget.record(build)?;
        *slot = if empty(&result) {
            debug_assert!(admission.is_none_or(|stage| stage.live_bytes() == 0));
            Some(Stable {
                revision,
                created,
                admission,
            })
        } else {
            None
        };
        Ok(result)
    }
}
