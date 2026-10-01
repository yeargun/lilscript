//! Local normalizers depend only on a unit's immutable body revision. Global
//! facts still invalidate through ProgramViews; a body-only proof cannot serve
//! a consumer of call, capture, cell or interface facts.
use super::*;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};

#[derive(Clone, Copy)]
pub(super) enum LocalRule {
    Returns,
    Unreachable,
}

pub(super) struct DirtyUnits {
    seen: Vec<[Option<(RevisionId, bool)>; 2]>,
    dirty: Vec<bool>,
    enabled: bool,
}

impl DirtyUnits {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            seen: Vec::new(),
            dirty: Vec::new(),
            enabled,
        }
    }

    pub(super) fn release(self, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        super::storage::release_vec(self.seen, Scratch, budget)?;
        super::storage::release_vec(self.dirty, Scratch, budget)
    }

    pub(super) fn select(
        &mut self,
        program: &Program<'_>,
        rule: LocalRule,
        created: Option<&[bool]>,
        receipt: &mut RuleReceipt,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<&[bool], AllocationError> {
        let count = program.units.len();
        if count > self.seen.len() {
            let added = count - self.seen.len();
            budget.reserve_vec(Scratch, &mut self.seen, added)?;
            budget.reserve_vec(Scratch, &mut self.dirty, added)?;
            self.seen.resize(count, [None; 2]);
            self.dirty.resize(count, true);
        }
        // Both modes use the same table admission and logical bookkeeping.
        budget.work(crate::compilation_policy::WorkKind::Analysis, count as u64)?;
        for (index, unit) in program.units.iter().enumerate() {
            let previous = &mut self.seen[index][rule as usize];
            let key = (
                unit.revision(),
                created.is_none_or(|created| created[index]),
            );
            let dirty = !self.enabled || *previous != Some(key);
            self.dirty[index] = dirty;
            if dirty {
                receipt.local_units_visited += 1;
            } else {
                receipt.local_units_reused += 1;
            }
            // Record the input. If this rule edits it, commit gives the unit a
            // new revision and the next round visits it again.
            *previous = Some(key);
        }
        Ok(&self.dirty)
    }
}
