//! Optional resource ownership stays inside the existing artifact arena.
//! Completed producer text moves once, after admission; ordinary artifacts
//! retain their original String allocation and no shared wrapper.
use super::*;
use crate::compilation_policy::{AdmissionError, CandidateCostEvidence, ResolvedPolicy};
use crate::semantic_program::artifact_provenance::{ArtifactProvenance, ProvenanceError};
use crate::semantic_program::physical_export::SharedPhysicalExport;
use std::sync::Arc;

#[derive(Default)]
pub(super) enum ArtifactResource {
    #[default]
    Whole,
    Producer(Box<ProducedResource>),
    Consumer(FrozenArtifact),
}

#[derive(Clone, Copy, Default)]
pub(in crate::semantic_program) enum ResourceOutput<'a> {
    #[default]
    Whole,
    Producer {
        contract: &'a SharedPhysicalExport,
    },
    Consumer(&'a FrozenArtifact),
}
impl ResourceOutput<'_> {
    pub(super) fn retain_actual(
        self,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<ArtifactResource, CandidateError> {
        Ok(match self {
            Self::Whole => ArtifactResource::Whole,
            Self::Producer { contract } => {
                let mut phase = budget.scope();
                let bytes = size_of::<ProducedResource>() as u64;
                phase.retain(AllocationClass::Retained, bytes)?;
                let contract = contract.share(&mut phase)?;
                // Both owners are already admitted. Detaching this exact
                // header cannot encounter a new work/memory refusal.
                let charge = phase
                    .detach_retained(owner, bytes)
                    .expect("admitted producer header");
                ArtifactResource::Producer(Box::new(ProducedResource {
                    contract,
                    owner,
                    charge,
                }))
            }
            Self::Consumer(producer) => ArtifactResource::Consumer(producer.share(budget)?),
        })
    }
    pub(super) fn dependency_bytes(self) -> usize {
        match self {
            Self::Consumer(producer) => producer.record().text.len(),
            _ => 0,
        }
    }
}
impl ArtifactResource {
    pub(super) fn dependency(&self) -> Option<&FrozenArtifact> {
        match self {
            Self::Consumer(producer) => Some(producer),
            _ => None,
        }
    }
    pub(super) fn discard(self, budget: &mut AllocationBudget<'_>) {
        let result = match self {
            Self::Whole => Ok(()),
            Self::Producer(produced) => produced.discard(budget),
            Self::Consumer(producer) => producer.discard(budget),
        };
        result.unwrap_or_else(|_| panic!("artifact resource allocation owner invariant"));
    }
}

/// A second actual ESM file required by the primary resource. Its canonical
/// sizes are measured separately; no concatenated compression stream is used.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactResourceView<'a> {
    pub specifier: &'a str,
    pub javascript: &'a str,
    /// Actual canonical choices of this delivered immutable resource.
    pub output: OutputTactics,
    pub sizes: Sizes,
    pub retained_capacity: usize,
}

struct SharedArtifact {
    owner: RevisionId,
    // None exists only in a unique, unpublished PreparedFreeze.
    record: Option<Record>,
    charge: RetainedCharge<RevisionId>,
}

#[must_use = "commit after publication admission or discard through the original ledger"]
pub(in crate::semantic_program) struct PreparedFreeze {
    source: ArtifactId,
    shared: Arc<SharedArtifact>,
}
impl PreparedFreeze {
    pub(in crate::semantic_program) fn discard(
        self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let shared = Arc::try_unwrap(self.shared)
            .unwrap_or_else(|_| panic!("unpublished freeze must remain unique"));
        debug_assert!(shared.record.is_none());
        release(shared.charge, shared.owner, budget);
        Ok(())
    }
}

#[must_use = "retain through the originating compilation or explicitly discard"]
pub(in crate::semantic_program) struct FrozenArtifact(Arc<SharedArtifact>);
impl std::fmt::Debug for FrozenArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenArtifact")
            .field("candidate", &self.record().candidate)
            .field("bytes", &self.record().text.len())
            .finish()
    }
}
impl FrozenArtifact {
    pub(in crate::semantic_program) fn description(
        &self,
    ) -> crate::semantic_program::implementation_identity::FrozenProducerDescription<'_> {
        crate::semantic_program::implementation_identity::FrozenProducerDescription::new(
            self.contract().borrow().description(),
            &self.record().text,
            self.record().identity.description().recipe_words(),
            self.record().identity.description().rewrites(),
            self.record().identity.description().snapshot_identity(),
            self.record().identity.description().meaning_identity(),
            self.record().provenance.description(),
        )
    }
    fn record(&self) -> &Record {
        self.0
            .record
            .as_ref()
            .expect("only complete producers can be shared")
    }
    pub(in crate::semantic_program) fn contract(&self) -> &SharedPhysicalExport {
        &self.produced().contract
    }
    fn produced(&self) -> &ProducedResource {
        let ArtifactResource::Producer(produced) = &self.record().resource else {
            unreachable!("frozen producer owns its actual formation contract")
        };
        produced
    }
    pub(in crate::semantic_program) fn check_permissions(
        &self,
        policy: &ResolvedPolicy,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), CandidateError> {
        self.record()
            .provenance
            .check_permissions(policy, budget)
            .map_err(provenance_error)
    }
    pub(in crate::semantic_program) fn check_policy(
        &self,
        consumer: &ArtifactProvenance,
        policy: &ResolvedPolicy,
        cost: CandidateCostEvidence,
        baseline: CandidateCostEvidence,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), CandidateError> {
        consumer
            .admit_with_dependency(&self.record().provenance, policy, cost, baseline, budget)
            .map_err(provenance_error)
    }
    pub(in crate::semantic_program) fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(in crate::semantic_program) fn retained_bytes(&self) -> u64 {
        self.0.charge.bytes()
            + self.record().charge.bytes()
            + self.produced().charge.bytes()
            + self.record().provenance.retained_bytes()
            + self.record().identity.retained_bytes()
    }
    pub(in crate::semantic_program) fn share(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.work(WorkKind::Edit, 1)?;
        Ok(Self(Arc::clone(&self.0)))
    }
    pub(in crate::semantic_program) fn discard(
        self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if let Ok(shared) = Arc::try_unwrap(self.0) {
            shared
                .record
                .expect("frozen producer is complete")
                .discard(shared.owner, budget);
            release(shared.charge, shared.owner, budget);
        }
        Ok(())
    }
    pub(in crate::semantic_program) fn view(&self) -> ArtifactResourceView<'_> {
        let record = self.record();
        ArtifactResourceView {
            specifier: self.contract().borrow().source_specifier(),
            javascript: &record.text,
            output: record.output,
            sizes: record.sizes.get(),
            retained_capacity: record.text.capacity(),
        }
    }
    pub(super) fn measure(
        &self,
        codec: CompressionCostModel,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, CandidateError> {
        self.record().measure(codec, budget)
    }
}

impl ArtifactArena {
    /// Borrow the actual text owner for bounded pool accounting. Allocation
    /// equality here is not semantic/artifact identity and never admits reuse.
    pub(in crate::semantic_program) fn dependency(
        &self,
        id: ArtifactId,
    ) -> Result<Option<&FrozenArtifact>, CandidateError> {
        Ok(self.get(id.0)?.resource.dependency())
    }
    pub(in crate::semantic_program) fn producer_contract(
        &self,
        id: ArtifactId,
    ) -> Result<&SharedPhysicalExport, CandidateError> {
        match &self.get(id.0)?.resource {
            ArtifactResource::Producer(produced) => Ok(&produced.contract),
            _ => Err(CandidateError::Artifact(
                "artifact was not formed as a fixed producer",
            )),
        }
    }
    pub(in crate::semantic_program) fn check_producer_permissions(
        &self,
        id: ArtifactId,
        policy: &ResolvedPolicy,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), CandidateError> {
        match &self.get(id.0)?.resource {
            ArtifactResource::Producer(_) => self
                .get(id.0)?
                .provenance
                .check_permissions(policy, budget)
                .map_err(provenance_error),
            _ => Err(CandidateError::Artifact(
                "artifact was not formed as a fixed producer",
            )),
        }
    }
    pub(in crate::semantic_program) fn prepare_freeze(
        &self,
        id: ArtifactId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<PreparedFreeze, CandidateError> {
        budget.work(WorkKind::Edit, 1)?;
        self.producer_contract(id)?;
        let mut phase = budget.scope();
        let bytes = (size_of::<SharedArtifact>() + 2 * size_of::<usize>()) as u64;
        phase.retain(AllocationClass::Retained, bytes)?;
        let charge = phase.detach_retained(self.owner, bytes)?;
        let shared = Arc::new(SharedArtifact {
            owner: self.owner,
            record: None,
            charge,
        });
        Ok(PreparedFreeze { source: id, shared })
    }
    /// No callback or fallible admission may intervene between the caller's
    /// final validation and this transfer plus checkpoint installation.
    pub(in crate::semantic_program) fn commit_freeze(
        &mut self,
        mut prepared: PreparedFreeze,
    ) -> FrozenArtifact {
        let record = self
            .remove(prepared.source.0)
            .expect("preadmitted producer artifact remains retained");
        let shared = Arc::get_mut(&mut prepared.shared).expect("freeze commits before sharing");
        debug_assert!(shared.record.is_none());
        shared.record = Some(record);
        FrozenArtifact(prepared.shared)
    }
}

pub(super) struct ProducedResource {
    contract: SharedPhysicalExport,
    owner: RevisionId,
    charge: RetainedCharge<RevisionId>,
}
impl ProducedResource {
    fn discard(self: Box<Self>, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        let Self {
            contract,
            owner,
            charge,
        } = *self;
        contract.discard(budget)?;
        release(charge, owner, budget);
        Ok(())
    }
}
pub(super) fn discard_provenance(
    provenance: ArtifactProvenance,
    owner: RevisionId,
    budget: &mut AllocationBudget<'_>,
) {
    budget.with_ledger(|ledger| {
        let (ledger, _) = ledger.expect("artifact provenance belongs to compilation ledger");
        provenance
            .discard(owner, ledger)
            .unwrap_or_else(|_| panic!("artifact provenance allocation owner invariant"));
    });
}
pub(super) fn provenance_error(error: ProvenanceError) -> CandidateError {
    match error {
        ProvenanceError::Allocation(error) => error.into(),
        ProvenanceError::Naming(error) => error.into(),
        ProvenanceError::Admission(AdmissionError::ForbiddenTactic(tactic)) => {
            CandidateError::ForbiddenTactic(tactic)
        }
        ProvenanceError::Admission(error) => CandidateError::Admission(error),
    }
}
