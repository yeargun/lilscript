//! A lazily built recipe descriptor shared by candidates, search and artifacts.
use super::*;
use crate::program::rewrite_lineage::RewriteLineage;
use std::sync::Arc;

#[derive(Debug)]
struct SharedIdentity {
    owner: RevisionId,
    snapshot: RevisionId,
    meaning: RevisionId,
    lineage: RewriteLineage,
    identity: ImplementationIdentity,
    fingerprint: u64,
    header_charge: RetainedCharge<RevisionId>,
}

#[derive(Debug)]
#[must_use = "retain with the admitting compilation or explicitly discard"]
pub(in crate::program) struct SharedImplementationIdentity(Arc<SharedIdentity>);

impl SharedImplementationIdentity {
    pub(in crate::program) fn ensure(
        cached: &mut Option<Self>,
        map: Option<&ImplementationMap>,
        snapshot: RevisionId,
        meaning: RevisionId,
        lineage: &RewriteLineage,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        if cached.is_none() {
            *cached = Some(Self::build(map, snapshot, meaning, lineage, owner, budget)?);
        }
        Ok(())
    }

    pub(in crate::program) fn build(
        map: Option<&ImplementationMap>,
        snapshot: RevisionId,
        meaning: RevisionId,
        lineage: &RewriteLineage,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut phase = budget.scope();
        let bytes = (size_of::<SharedIdentity>() + 2 * size_of::<usize>()) as u64;
        phase.retain(Retained, bytes)?;
        if !lineage.description().is_empty() {
            phase.work(WorkKind::Analysis, 8)?;
        }
        let lineage = lineage.share(&mut phase)?;
        let identity = match ImplementationIdentity::build(map, owner, &mut phase) {
            Ok(identity) => identity,
            Err(error) => {
                if !lineage.description().is_empty() {
                    phase.with_ledger(|ledger| lineage.discard(ledger.unwrap().0));
                }
                return Err(error);
            }
        };
        // Header admission precedes construction. Its exact detach introduces
        // no new refusal after the word/resource owner has been completed.
        let header_charge = phase
            .detach_retained(owner, bytes)
            .expect("admitted identity header");
        let mut fingerprint = identity.fingerprint();
        if !lineage.description().is_empty() {
            for byte in lineage.description().fingerprint().to_le_bytes() {
                fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(HASH_PRIME);
            }
        }
        Ok(Self(Arc::new(SharedIdentity {
            owner,
            snapshot,
            meaning,
            lineage,
            identity,
            fingerprint,
            header_charge,
        })))
    }

    pub(in crate::program) fn share(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        Ok(Self(Arc::clone(&self.0)))
    }

    pub(in crate::program) fn meaning(&self) -> RevisionId {
        self.0.meaning
    }
    pub(in crate::program) fn snapshot(&self) -> RevisionId {
        self.0.snapshot
    }
    pub(in crate::program) fn tactics(&self) -> &[crate::compilation_policy::TacticUse] {
        self.0.lineage.tactics()
    }

    pub(in crate::program) fn owner(&self) -> RevisionId {
        self.0.owner
    }

    pub(in crate::program) fn fingerprint(&self) -> u64 {
        self.0.fingerprint
    }

    pub(in crate::program) fn description(&self) -> ImplementationDescription<'_> {
        let mut description = self.0.identity.description();
        description.rewrites = self.0.lineage.description();
        description.snapshot = Some(self.0.snapshot);
        description.meaning = Some(self.0.meaning);
        description
    }

    #[cfg(test)]
    pub(in crate::program) fn words(&self) -> &[u32] {
        self.0.identity.words()
    }

    pub(in crate::program) fn equivalent(
        &self,
        other: &Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        if self.snapshot() != other.snapshot() {
            return Ok(false);
        }
        if self
            .0
            .lineage
            .description()
            .compare(other.0.lineage.description(), budget)?
            != Ordering::Equal
        {
            return Ok(false);
        }
        self.0.identity.equivalent(&other.0.identity, budget)
    }

    pub(in crate::program) fn compare(
        &self,
        other: &Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Ordering, AllocationError> {
        let order = self
            .0
            .lineage
            .description()
            .compare(other.0.lineage.description(), budget)?;
        if order != Ordering::Equal {
            return Ok(order);
        }
        self.0.identity.compare(&other.0.identity, budget)
    }

    /// Intrinsic backing, excluding independently shared resource payloads.
    pub(in crate::program) fn retained_bytes(&self) -> u64 {
        self.0.header_charge.bytes()
            + self.0.identity.retained_bytes()
            + self.0.lineage.retained_bytes()
    }

    pub(in crate::program) fn discard(
        self,
        owner: RevisionId,
        ledger: &mut BudgetLedger,
    ) -> Result<(), (Self, AllocationError)> {
        if self.0.owner != owner {
            return Err((self, AllocationError::WrongOwner));
        }
        if let Ok(shared) = Arc::try_unwrap(self.0) {
            shared.lineage.discard(ledger);
            shared.identity.discard(owner, ledger).unwrap();
            shared.header_charge.discard(&owner, ledger).unwrap();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetError, BudgetPlan, ResourceLimits, WorkDomain};

    fn ledger(memory: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 10_000,
                optional_work: 10_000,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap()
    }

    #[test]
    fn sharing_keeps_one_backing_and_releases_only_after_last_owner() {
        let mut ledger = ledger(10_000);
        let owner = RevisionId::fresh();
        let identity = SharedImplementationIdentity::build(
            None,
            RevisionId::fresh(),
            RevisionId::fresh(),
            &RewriteLineage::default(),
            owner,
            &mut AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional))),
        )
        .unwrap();
        let retained = ledger.retained_bytes();
        assert_eq!(retained, identity.retained_bytes());
        let shared = identity
            .share(&mut AllocationBudget::new(Some((
                &mut ledger,
                WorkDomain::Baseline,
            ))))
            .unwrap();
        assert_eq!(identity.words().as_ptr(), shared.words().as_ptr());
        assert_eq!(ledger.retained_bytes(), retained);
        let identity = identity
            .discard(RevisionId::fresh(), &mut ledger)
            .unwrap_err()
            .0;
        assert_eq!(ledger.retained_bytes(), retained);
        identity.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), retained);
        shared.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn equal_words_never_establish_snapshot_equivalence_and_share_refusal_is_atomic() {
        let mut ledger = ledger(10_000);
        let owner = RevisionId::fresh();
        let [left, right] = std::array::from_fn(|_| {
            SharedImplementationIdentity::build(
                None,
                RevisionId::fresh(),
                RevisionId::fresh(),
                &RewriteLineage::default(),
                owner,
                &mut AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional))),
            )
            .unwrap()
        });
        assert_eq!(left.fingerprint(), right.fingerprint());
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        assert!(!left.equivalent(&right, &mut budget).unwrap());
        assert_eq!(
            left.compare(&right, &mut budget).unwrap(),
            Ordering::Equal,
            "stable ordering must not depend on revision allocation order"
        );
        drop(budget);
        let retained = ledger.retained_bytes();
        let remaining = 10_000 - ledger.work_used(WorkDomain::Optional);
        ledger
            .charge(WorkDomain::Optional, WorkKind::Analysis, remaining)
            .unwrap();
        assert!(matches!(
            left.share(&mut AllocationBudget::new(Some((
                &mut ledger,
                WorkDomain::Optional
            )))),
            Err(AllocationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(ledger.retained_bytes(), retained);
        left.discard(owner, &mut ledger).unwrap();
        right.discard(owner, &mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn refused_word_allocation_releases_the_previously_admitted_shared_header() {
        let header = (size_of::<SharedIdentity>() + 2 * size_of::<usize>()) as u64;
        let mut ledger = ledger(header + 23);
        assert!(matches!(
            SharedImplementationIdentity::build(
                None,
                RevisionId::fresh(),
                RevisionId::fresh(),
                &RewriteLineage::default(),
                RevisionId::fresh(),
                &mut AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)))
            ),
            Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.peak_retained_bytes(), header);
    }
}
