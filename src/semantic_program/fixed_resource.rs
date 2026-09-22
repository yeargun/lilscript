//! One optional resource choice in the existing implementation map.
//! There is no package/candidate registry, semantic clone, or physical alias graph.
use super::artifacts::FrozenArtifact;
use super::function_layout::ProductTransport;
use super::implementations::{ImplementationError, ImplementationMap, SharedFunction};
use super::physical_export::{PhysicalExport, SharedPhysicalExport};
use super::uses::UseIndex;
use super::{Program, RevisionId};
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationError};
use std::sync::Arc;

#[derive(Debug)]
pub(super) enum ResourceChoice {
    Producer(SharedPhysicalExport),
    Consumer {
        producer: FrozenArtifact,
        consumed: SharedPhysicalExport,
    },
}
impl ResourceChoice {
    pub(super) fn export(&self) -> &PhysicalExport {
        match self {
            Self::Producer(contract) => contract.borrow(),
            Self::Consumer { consumed, .. } => consumed.borrow(),
        }
    }
    pub(super) fn producer(&self) -> Option<&FrozenArtifact> {
        match self {
            Self::Consumer { producer, .. } => Some(producer),
            _ => None,
        }
    }
    pub(super) fn share(&self, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        match self {
            Self::Producer(contract) => Ok(Self::Producer(contract.share(budget)?)),
            Self::Consumer { producer, consumed } => {
                let producer = producer.share(budget)?;
                match consumed.share(budget) {
                    Ok(consumed) => Ok(Self::Consumer { producer, consumed }),
                    Err(error) => {
                        producer.discard(budget)?;
                        Err(error)
                    }
                }
            }
        }
    }
    pub(super) fn same_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Producer(a), Self::Producer(b)) => a.same_owner(b),
            (
                Self::Consumer {
                    producer: a,
                    consumed: ac,
                },
                Self::Consumer {
                    producer: b,
                    consumed: bc,
                },
            ) => a.same_owner(b) && ac.same_owner(bc),
            _ => false,
        }
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        match self {
            Self::Producer(contract) => contract.borrow().retained_bytes(),
            Self::Consumer { producer, consumed } => {
                producer.retained_bytes()
                    + consumed.borrow().retained_bytes()
                    + if producer.contract().same_owner(consumed) {
                        0
                    } else {
                        producer.contract().borrow().retained_bytes()
                            - consumed
                                .borrow()
                                .shared_proof_bytes(producer.contract().borrow())
                    }
            }
        }
    }
    pub(super) fn contains_function_owner(&self, owner: &Arc<SharedFunction>) -> bool {
        match self {
            Self::Producer(contract) => contract.borrow().contains_function_owner(owner),
            Self::Consumer { producer, consumed } => {
                consumed.borrow().contains_function_owner(owner)
                    || producer.contract().borrow().contains_function_owner(owner)
            }
        }
    }
    pub(super) fn discard(self, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        match self {
            Self::Producer(contract) => contract.discard(budget),
            Self::Consumer { producer, consumed } => {
                producer.discard(budget)?;
                consumed.discard(budget)
            }
        }
    }
    pub(super) fn validate(
        &self,
        identity: RevisionId,
        program: &Program<'_>,
        uses: &UseIndex,
        selected: &ImplementationMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        match self {
            Self::Producer(contract) => {
                let contract = contract.borrow();
                if !Self::validate_endpoint(contract, identity, program, uses, selected, budget)? {
                    return Ok(false);
                }
                let mut transport = ProductTransport::Packed;
                for function in selected.functions() {
                    budget.work(WorkKind::Analysis, 1)?;
                    if function.body() == contract.body() {
                        transport = function.transport(0);
                    }
                }
                Ok(contract.parameters()[0].transport() == transport)
            }
            Self::Consumer { producer, consumed } => Self::validate_pending_consumer(
                producer.contract().borrow(),
                consumed.borrow(),
                identity,
                program,
                uses,
                selected,
                budget,
            ),
        }
    }
    /// The same check is used before consuming bytes and for retained maps.
    pub(super) fn validate_pending_consumer(
        actual: &PhysicalExport,
        consumed: &PhysicalExport,
        identity: RevisionId,
        program: &Program<'_>,
        uses: &UseIndex,
        selected: &ImplementationMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        if !Self::validate_endpoint(consumed, identity, program, uses, selected, budget)? {
            return Ok(false);
        }
        budget.work(
            WorkKind::Analysis,
            actual.validation_work().ok_or(AllocationError::Capacity)?,
        )?;
        Ok(actual.valid_for_published(identity, program, uses)
            && consumed.same_abi(actual, budget)?)
    }
    fn validate_endpoint(
        contract: &PhysicalExport,
        identity: RevisionId,
        program: &Program<'_>,
        uses: &UseIndex,
        selected: &ImplementationMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(
            WorkKind::Analysis,
            contract
                .validation_work()
                .ok_or(AllocationError::Capacity)?,
        )?;
        if !contract.valid_for_published(identity, program, uses) {
            return Ok(false);
        }
        for helper in selected.helpers() {
            budget.work(WorkKind::Analysis, 1)?;
            if helper.root().cell == contract.cell() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

pub(super) fn union_resource<'a>(
    left: Option<&'a ResourceChoice>,
    right: Option<&'a ResourceChoice>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<&'a ResourceChoice>, ImplementationError> {
    if left.is_none() && right.is_none() {
        return Ok(None);
    }
    budget.work(WorkKind::Edit, 1)?;
    match (left, right) {
        (Some(a), Some(b)) if !a.same_owner(b) => Err(ImplementationError::ConflictingChoice),
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => unreachable!(),
    }
}
