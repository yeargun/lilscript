//! Deterministic admission stages for repeated immutable formations.
//!
//! The first fully funded execution measures work by kind, temporary peak and
//! surviving backing. Every repeated assignment admits that complete stage,
//! independently of whether it borrows the result or physically executes it.
//! Recording is constant space. A recovered budget refusal cannot establish
//! a reusable stage: it may have selected a resource-dependent fallback.
use crate::compilation_policy::{WorkDomain, WorkKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Recording {
    domain: WorkDomain,
    start: [u64; 4],
    domains: [u64; 2],
    live: u64,
    peak: u64,
    refused: bool,
}
impl Recording {
    pub(crate) fn new(domain: WorkDomain, start: [u64; 4], domains: [u64; 2]) -> Self {
        Self {
            domain,
            start,
            domains,
            live: 0,
            peak: 0,
            refused: false,
        }
    }
    pub(crate) fn refuse(&mut self) {
        self.refused = true;
    }
    pub(crate) fn domain(&mut self, domain: WorkDomain) {
        self.refused |= domain != self.domain;
    }
    pub(crate) fn retain(&mut self, domain: WorkDomain, bytes: u64) {
        self.domain(domain);
        match self.live.checked_add(bytes) {
            Some(live) => {
                self.live = live;
                self.peak = self.peak.max(live);
            }
            None => self.refuse(),
        }
    }
    pub(crate) fn release(&mut self, domain: WorkDomain, bytes: u64) {
        if bytes == 0 {
            return;
        }
        self.domain(domain);
        match self.live.checked_sub(bytes) {
            Some(live) => self.live = live,
            None => self.refuse(),
        }
    }
    pub(crate) fn scratch(&mut self, domain: WorkDomain, bytes: u64) {
        self.domain(domain);
        match self.live.checked_add(bytes) {
            Some(peak) => self.peak = self.peak.max(peak),
            None => self.refuse(),
        }
    }
    /// Nested fresh builders contribute their peak while the parent's prior
    /// backing remains live. Work counters already include the child's work.
    pub(crate) fn absorb(&mut self, child: &Self) {
        self.domain(child.domain);
        self.refused |= child.refused;
        match (
            self.live.checked_add(child.peak),
            self.live.checked_add(child.live),
        ) {
            (Some(peak), Some(live)) => {
                self.peak = self.peak.max(peak);
                self.live = live;
            }
            _ => self.refuse(),
        }
    }
    pub(crate) fn finish(self, end: [u64; 4], domains: [u64; 2]) -> Option<Receipt> {
        let other = if self.domain == WorkDomain::Baseline {
            1
        } else {
            0
        };
        if self.refused || self.domains[other] != domains[other] {
            let _timing = crate::timing::FORMATION_ADMISSION_REFUSED.scope(0);
            return None;
        }
        let mut work = [0; 4];
        for index in 0..4 {
            work[index] = end[index].checked_sub(self.start[index])?;
        }
        Some(Receipt {
            work,
            peak: self.peak,
            live: self.live,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Receipt {
    work: [u64; 4],
    peak: u64,
    live: u64,
}
impl Receipt {
    pub(crate) fn without_retained(mut self, bytes: u64) -> Option<Self> {
        self.live = self.live.checked_sub(bytes)?;
        Some(self)
    }

    pub(crate) fn work(&self) -> impl Iterator<Item = (WorkKind, u64)> {
        [
            WorkKind::Analysis,
            WorkKind::Edit,
            WorkKind::Render,
            WorkKind::Codec,
        ]
        .into_iter()
        .zip(self.work)
    }
    pub(crate) fn peak_bytes(&self) -> u64 {
        self.peak
    }
    pub(crate) fn live_bytes(&self) -> u64 {
        self.live
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits};
    use crate::output_budget::{
        AllocationBudget,
        AllocationClass::{Retained, Scratch},
        AllocationError,
    };

    fn ledger(work: u64, memory: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 0,
                optional_work: work,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap()
    }
    fn build(budget: &mut AllocationBudget<'_>) -> Result<Vec<u8>, AllocationError> {
        budget.retained_phase(|budget| {
            let mut result = budget.copy_slice(Retained, &[1u8; 10])?;
            for _ in 0..7 {
                budget.work(WorkKind::Analysis, 3)?;
            }
            {
                let mut phase = budget.scope();
                let scratch = phase.filled(Scratch, 20, 7u16)?;
                phase.work(WorkKind::Edit, 7)?;
                drop(scratch);
            }
            budget.extend_copy(Retained, &mut result, &[2u8; 25])?;
            budget.work(WorkKind::Render, 0)?;
            Ok(result)
        })
    }
    #[test]
    fn q2_complete_stage_admits_the_same_limits_before_cold_or_reused_execution() {
        let mut owner = ledger(1_000_000, 1_000_000);
        let mut storage = AllocationBudget::new(Some((&mut owner, WorkDomain::Optional)));
        let (value, receipt) = storage.record(build).unwrap();
        let receipt = receipt.unwrap();
        assert_eq!(receipt.live_bytes(), value.capacity() as u64);
        storage.with_ledger(|ledger| {
            let (ledger, _) = ledger.unwrap();
            assert_eq!(receipt.peak_bytes(), ledger.peak_retained_bytes());
            for (kind, units) in receipt.work() {
                assert_eq!(units, ledger.work_by_kind(kind));
            }
        });
        for work in (0..180).chain([1_000_000]) {
            for memory in [0, 9, 10, 34, 35, 39, 40, 49, 50, 64, 1000] {
                let run = |reuse| {
                    let mut bill = ledger(work, memory);
                    let mut executed = false;
                    let outcome = {
                        let mut budget =
                            AllocationBudget::new(Some((&mut bill, WorkDomain::Optional)));
                        budget.replay(&receipt, || {
                            executed = true;
                            if reuse {
                                Ok(value.clone())
                            } else {
                                build(&mut AllocationBudget::new(None))
                            }
                        })
                    };
                    assert_eq!(
                        executed,
                        outcome.is_ok(),
                        "execution needs complete admission"
                    );
                    (
                        outcome,
                        bill.work_used(WorkDomain::Optional),
                        [
                            WorkKind::Analysis,
                            WorkKind::Edit,
                            WorkKind::Render,
                            WorkKind::Codec,
                        ]
                        .map(|kind| bill.work_by_kind(kind)),
                        bill.peak_retained_bytes(),
                        bill.retained_bytes(),
                    )
                };
                assert_eq!(run(true), run(false), "work={work} memory={memory}");
            }
        }
        drop((value, storage));
        assert_eq!(owner.retained_bytes(), 0);
    }
    #[test]
    fn q2_recovered_refusals_cannot_certify_a_formation_and_unwind_clears_ownership() {
        let mut owner = ledger(1_000_000, 1_000_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Optional)));
            let (_, receipt) = budget
                .record(|budget| {
                    assert!(budget.work(WorkKind::Edit, u64::MAX).is_err());
                    Ok::<_, AllocationError>(())
                })
                .unwrap();
            assert!(receipt.is_none());
            let (_, receipt) = budget
                .record(|budget| {
                    assert!(budget.check_scratch(u64::MAX).is_err());
                    Ok::<_, AllocationError>(())
                })
                .unwrap();
            assert!(receipt.is_none());
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<((), _), AllocationError> = budget.record(|budget| {
                    let mut phase = budget.scope();
                    let _value = phase.copy_slice(Retained, &[4u64; 10])?;
                    panic!("formation builder unwind");
                });
            }));
            assert!(panic.is_err());
            assert_eq!(budget.retained_bytes(Retained), 0);
            let (value, receipt) = budget.record(build).unwrap();
            let receipt = receipt.unwrap();
            let before = budget.retained_bytes(Retained);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), AllocationError> =
                    budget.replay(&receipt, || panic!("physical replay unwind"));
            }));
            assert!(panic.is_err());
            assert_eq!(budget.retained_bytes(Retained), before);
            drop(value);
        }
        assert_eq!(owner.retained_bytes(), 0);
    }

    #[test]
    fn q2_nested_stages_preserve_parent_peak_work_and_refusal() {
        let mut owner = ledger(1_000_000, 1_000_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Optional)));
            let ((prefix, child), outer) = budget
                .record(|budget| {
                    let prefix = budget.copy_slice(Retained, &[0u8; 19])?;
                    let (child, inner) = budget.record(build)?;
                    let inner = inner.unwrap();
                    assert_eq!(inner.live_bytes(), child.capacity() as u64);
                    Ok::<_, AllocationError>((prefix, child))
                })
                .unwrap();
            let outer = outer.unwrap();
            budget.with_ledger(|ledger| {
                let (ledger, _) = ledger.unwrap();
                assert_eq!(outer.peak_bytes(), ledger.peak_retained_bytes());
                assert_eq!(outer.live_bytes(), ledger.retained_bytes());
                for (kind, units) in outer.work() {
                    assert_eq!(units, ledger.work_by_kind(kind));
                }
            });
            let (_, outer) = budget
                .record(|budget| {
                    let (_, child) = budget.record(|budget| {
                        assert!(budget.check_scratch(u64::MAX).is_err());
                        Ok::<_, AllocationError>(())
                    })?;
                    assert!(child.is_none());
                    Ok::<_, AllocationError>(())
                })
                .unwrap();
            assert!(outer.is_none());
            // A child unwind must restore the active parent's recorder.
            let (_, outer) = budget
                .record(|budget| {
                    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _: Result<((), _), AllocationError> = budget.record(|budget| {
                            let mut phase = budget.scope();
                            let _scratch = phase.copy_slice(Scratch, &[0u8; 71])?;
                            panic!("nested stage");
                        });
                    }));
                    assert!(panic.is_err());
                    budget.work(WorkKind::Analysis, 5)?;
                    Ok::<_, AllocationError>(())
                })
                .unwrap();
            let outer = outer.unwrap();
            assert_eq!(outer.peak_bytes(), 71);
            assert_eq!(outer.live_bytes(), 0);
            assert_eq!(
                outer
                    .work()
                    .find(|(kind, _)| *kind == WorkKind::Analysis)
                    .unwrap()
                    .1,
                5
            );
            drop((prefix, child));
        }
        assert_eq!(owner.retained_bytes(), 0);
    }
}
