//! Lazy unordered pairs of published proof seeds. Only cursors are retained;
//! candidates, compatibility and exact scores stay in their existing owners.
use super::RevisionId;
use crate::compilation_policy::{BudgetLedger, WorkKind};
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use std::mem::size_of;

#[derive(Clone, Copy, Default)]
struct Cursor {
    ready_at: usize,
    next: usize,
}

pub(super) struct SeedPairs {
    cursors: Vec<Cursor>,
    ready: usize,
    charge: RetainedCharge<RevisionId>,
}

impl SeedPairs {
    pub(super) fn new(
        count: usize,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut phase = budget.scope();
        let cursors = phase.filled(Retained, count, Cursor::default())?;
        let bytes = cursors
            .capacity()
            .checked_mul(size_of::<Cursor>())
            .ok_or(AllocationError::Capacity)?;
        let charge = phase.detach_retained(owner, bytes as u64)?;
        Ok(Self {
            cursors,
            ready: 0,
            charge,
        })
    }

    pub(super) fn publish(
        &mut self,
        seed: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        let cursor = &mut self.cursors[seed];
        assert_eq!(cursor.ready_at, 0, "each seed proof publishes once");
        // There can be at most cursors.len() successful publications.
        self.ready += 1;
        cursor.ready_at = self.ready;
        Ok(())
    }

    pub(super) fn next(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<(usize, usize)>, AllocationError> {
        for seed in 0..self.cursors.len() {
            budget.work(WorkKind::Analysis, 1)?;
            let ready_at = self.cursors[seed].ready_at;
            if ready_at == 0 {
                continue;
            }
            while self.cursors[seed].next < self.cursors.len() {
                budget.work(WorkKind::Analysis, 1)?;
                let other = self.cursors[seed].next;
                self.cursors[seed].next += 1;
                let earlier = self.cursors[other].ready_at;
                // The later publication owns the pair, even when proofs arrive
                // out of source order. No seen-pair matrix or rescan is needed.
                if earlier != 0 && earlier < ready_at {
                    return Ok(Some((seed.min(other), seed.max(other))));
                }
            }
        }
        Ok(None)
    }

    pub(super) fn discard(self, owner: RevisionId, ledger: &mut BudgetLedger) {
        let Self {
            cursors, charge, ..
        } = self;
        drop(cursors);
        charge
            .discard(&owner, ledger)
            .expect("search owns pair cursors");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetError, BudgetPlan, ResourceLimits, WorkDomain};

    fn ledger(work: u64, memory: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 10_000,
                optional_work: work,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap()
    }

    #[test]
    fn publication_orders_and_late_proofs_visit_each_unordered_pair_once() {
        for order in [
            [0, 2, 4],
            [0, 4, 2],
            [2, 0, 4],
            [2, 4, 0],
            [4, 0, 2],
            [4, 2, 0],
        ] {
            let mut ledger = ledger(10_000, 10_000);
            let owner = RevisionId::fresh();
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut pairs = SeedPairs::new(5, owner, &mut budget).unwrap();
            assert_eq!(pairs.next(&mut budget).unwrap(), None);
            let mut seen = Vec::new();
            for (count, seed) in order.into_iter().enumerate() {
                pairs.publish(seed, &mut budget).unwrap();
                while let Some(pair) = pairs.next(&mut budget).unwrap() {
                    seen.push(pair);
                }
                assert_eq!(seen.len(), count * (count + 1) / 2);
                assert_eq!(pairs.next(&mut budget).unwrap(), None);
            }
            seen.sort();
            assert_eq!(seen, [(0, 2), (0, 4), (2, 4)]);
            drop(budget);
            assert_eq!(ledger.retained_bytes(), (5 * size_of::<Cursor>()) as u64);
            pairs.discard(owner, &mut ledger);
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }

    #[test]
    fn refused_allocation_preserves_preexisting_retained_bytes() {
        for (work, memory, expected) in [
            (0, 10_000, BudgetError::WorkExhausted(WorkDomain::Optional)),
            (
                10_000,
                65,
                BudgetError::MemoryExhausted(WorkDomain::Optional),
            ),
        ] {
            let mut ledger = ledger(work, memory);
            ledger.retain(WorkDomain::Baseline, 64).unwrap();
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            assert!(
                matches!(SeedPairs::new(3, RevisionId::fresh(), &mut budget), Err(AllocationError::Budget(error)) if error == expected)
            );
            drop(budget);
            assert_eq!(ledger.retained_bytes(), 64);
            ledger.release(WorkDomain::Baseline, 64).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }

    #[test]
    fn scan_refusal_does_not_advance_the_unadmitted_cursor() {
        for allowance in [0, 1, 2, 3] {
            let mut ledger = ledger(10_000, 10_000);
            let owner = RevisionId::fresh();
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut pairs = SeedPairs::new(3, owner, &mut budget).unwrap();
            pairs.publish(0, &mut budget).unwrap();
            pairs.publish(2, &mut budget).unwrap();
            drop(budget);
            let retained = ledger.retained_bytes();
            let remaining = 10_000 - ledger.work_used(WorkDomain::Optional);
            ledger
                .charge(
                    WorkDomain::Optional,
                    WorkKind::Analysis,
                    remaining - allowance,
                )
                .unwrap();
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            assert!(matches!(
                pairs.next(&mut budget),
                Err(AllocationError::Budget(BudgetError::WorkExhausted(
                    WorkDomain::Optional
                )))
            ));
            assert_eq!(pairs.cursors[0].next, allowance.saturating_sub(1) as usize);
            assert_eq!(pairs.cursors[2].next, 0);
            drop(budget);
            assert_eq!(ledger.retained_bytes(), retained);
            pairs.discard(owner, &mut ledger);
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }
}
