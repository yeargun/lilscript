//! Explicit two-file publication is a client of the ordinary checkpoint owner.
//! Producer bytes are detached only after every admission and validation; the
//! old artifact and all old checkpoints survive every recoverable refusal.
use super::*;
use crate::semantic_program::artifacts::PreparedFreeze;
use crate::semantic_program::fixed_resource::ResourceChoice;
use crate::semantic_program::implementations::FunctionEvidence;
use crate::semantic_program::physical_export::SharedPhysicalExport;

pub type ProducerOutcome = FunctionOutcome;
#[derive(Debug, Clone, Copy)]
pub struct ProducerPublication {
    pub outcome: ProducerOutcome,
    /// Present only when this request actually ran the complete family query.
    /// None means the selected candidate's existing proof was retained; its
    /// lookup, publication and storage costs remain in the ordinary ledger.
    pub receipt: Option<AnalysisWorkReceipt>,
}

impl Compilation<'_> {
    /// Select one sealed physical resource export. This does not render or
    /// freeze bytes; only this published choice can create a producer-tagged
    /// artifact through the ordinary output owner.
    pub fn producer_javascript(
        &mut self,
        base: CandidateId,
        cell: CellId,
        source_specifier: &str,
        export_name: &str,
        request: FunctionRequest,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<ProducerPublication, CandidateError> {
        use crate::semantic_program::function_layout::{self, FamilyOutcome, FamilyRequest};
        let base = self.candidate_slot(base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        self.check_existing_javascript_contract(policy, domain)?;
        let execution = policy
            .javascript_contract()
            .ok_or(CandidateError::InvalidRequest)?
            .execution;
        if execution != crate::compilation_contract::JavaScriptExecution::Module {
            return Err(CandidateError::InvalidRequest);
        }
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        let checkpoint = self.slots[base].checkpoint.as_ref().unwrap();
        let selected = checkpoint.implementations.as_ref().unwrap();
        if selected.resource().is_some() {
            return Err(CandidateError::ConflictingChoice);
        }
        check_candidate_policy(selected, policy)?;
        self.ledger.charge(domain, WorkKind::Analysis, 1)?;
        let declaration = checkpoint
            .semantic
            .program
            .cells
            .get(cell.index())
            .ok_or(CandidateError::InvalidRequest)?;
        let CellBinding::Function(body) = declaration.binding else {
            return Err(CandidateError::InvalidRequest);
        };
        let retained = {
            let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
            selected.share_function(body, &mut budget)?
        };
        let (proof, receipt) = if let Some(proof) = retained {
            (proof, None)
        } else {
            let analysis = function_layout::analyze_published(
                &checkpoint.semantic.program,
                &checkpoint.semantic.uses,
                body,
                FamilyRequest {
                    execution,
                    attempt: AnalysisAttempt {
                        plan: function_layout::FUNCTION_LAYOUT_PLAN,
                        algorithm_version: function_layout::FUNCTION_LAYOUT_VERSION,
                        work_quota: request.max_work,
                    },
                    scratch_bytes: request.scratch_bytes,
                    output_bytes: request.output_bytes,
                },
                &mut self.ledger,
                domain,
            )?;
            let proof = match analysis.outcome {
                FamilyOutcome::Unknown(reason) => {
                    return Ok(ProducerPublication {
                        outcome: ProducerOutcome::Unknown(reason),
                        receipt: Some(analysis.receipt),
                    })
                }
                FamilyOutcome::Truncated(limit) => {
                    return Ok(ProducerPublication {
                        outcome: ProducerOutcome::Truncated(limit),
                        receipt: Some(analysis.receipt),
                    })
                }
                FamilyOutcome::Complete(proof) => proof,
            };
            let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
            (
                FunctionEvidence::from_family(proof, &mut budget)?,
                Some(analysis.receipt),
            )
        };
        let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
        let contract = SharedPhysicalExport::derive(
            self.store,
            checkpoint.semantic.identity,
            &checkpoint.semantic.program,
            selected,
            cell,
            source_specifier,
            export_name,
            proof,
            &mut budget,
        )?;
        let mut map = match budget.with_ledger(|ledger| {
            let (ledger, domain) = ledger.unwrap();
            selected.prepare_resource_map(ledger, domain)
        }) {
            Ok(map) => map,
            Err(error) => {
                contract.discard(&mut budget)?;
                return Err(error.into());
            }
        };
        map.install_resource_prepared(ResourceChoice::Producer(contract));
        drop(budget);
        let outcome = ProducerOutcome::Published(
            self.publish_candidate(base, map, policy, domain, start, None)?,
        );
        Ok(ProducerPublication { outcome, receipt })
    }

    /// Consume exactly the bytes tagged by Producer formation. The resulting
    /// candidate owns the complete fixed producer dependency and a freshly
    /// prepared consumer contract; no freely clonable producer handle escapes.
    pub fn freeze_producer_javascript(
        &mut self,
        consumer_base: CandidateId,
        artifact: ArtifactId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<CandidateId, CandidateError> {
        let base = self.candidate_slot(consumer_base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        self.check_existing_javascript_contract(policy, domain)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        let mut map = None;
        let mut consumed = None;
        let mut freeze: Option<PreparedFreeze> = None;
        let prepared = (|| {
            let checkpoint = self.slots[base].checkpoint.as_ref().unwrap();
            let selected = checkpoint.implementations.as_ref().unwrap();
            if selected.resource().is_some() {
                return Err(CandidateError::ConflictingChoice);
            }
            let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
            self.artifacts
                .check_producer_permissions(artifact, policy, &mut budget)?;
            let actual = self.artifacts.producer_contract(artifact)?;
            if !ResourceChoice::validate_pending_consumer(
                actual.borrow(),
                actual.borrow(),
                checkpoint.semantic.identity,
                &checkpoint.semantic.program,
                &checkpoint.semantic.uses,
                selected,
                &mut budget,
            )? {
                return Err(CandidateError::StaleEvidence);
            }
            consumed = Some(actual.share(&mut budget)?);
            map = Some(budget.with_ledger(|ledger| {
                let (ledger, domain) = ledger.unwrap();
                selected.prepare_resource_map(ledger, domain)
            })?);
            freeze = Some(self.artifacts.prepare_freeze(artifact, &mut budget)?);
            drop(budget);
            self.prepare_candidate(base, map.as_ref().unwrap(), policy, domain)
        })();
        let (slot, charge) = match prepared {
            Ok(value) => value,
            Err(error) => {
                let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
                if let Some(freeze) = freeze {
                    freeze.discard(&mut budget)?;
                }
                if let Some(consumed) = consumed {
                    consumed.discard(&mut budget)?;
                }
                if let Some(map) = map {
                    budget.with_ledger(|ledger| map.discard(ledger.unwrap().0))?;
                }
                return Err(error);
            }
        };
        // No callback, budget check, source mutation, or fallible allocation
        // occurs between the first detach and final checkpoint installation.
        let producer = self.artifacts.commit_freeze(freeze.unwrap());
        let mut map = map.unwrap();
        map.install_resource_prepared(ResourceChoice::Consumer {
            producer,
            consumed: consumed.unwrap(),
        });
        Ok(self.install_candidate(base, map, slot, charge, domain, start, None))
    }

    /// Explicit replacement preserves the old consumer's consumed ABI. A
    /// Packed->Fields producer update therefore rejects before publication,
    /// even though semantic IDs/types and all unrelated facts are unchanged.
    pub fn retarget_fixed_producer_javascript(
        &mut self,
        base: CandidateId,
        producer_package: CandidateId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<CandidateId, CandidateError> {
        self.update_fixed_resource(base, producer_package, false, policy, domain)
    }
    /// Rebuild the consumer adapter explicitly against the actual replacement
    /// producer contract, then publish through the same checkpoint kernel.
    pub fn rebuild_fixed_consumer_javascript(
        &mut self,
        base: CandidateId,
        producer_package: CandidateId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<CandidateId, CandidateError> {
        self.update_fixed_resource(base, producer_package, true, policy, domain)
    }
    fn update_fixed_resource(
        &mut self,
        base: CandidateId,
        producer_package: CandidateId,
        refresh: bool,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<CandidateId, CandidateError> {
        let base = self.candidate_slot(base)?;
        let replacement = self.candidate_slot(producer_package)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        self.check_existing_javascript_contract(policy, domain)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        let old = self.slots[base].checkpoint.as_ref().unwrap();
        let replacement = self.slots[replacement].checkpoint.as_ref().unwrap();
        if old.semantic.identity != replacement.semantic.identity {
            return Err(CandidateError::StaleEvidence);
        }
        let selected = old.implementations.as_ref().unwrap();
        let Some(ResourceChoice::Consumer {
            consumed: prior, ..
        }) = selected.resource()
        else {
            return Err(CandidateError::InvalidRequest);
        };
        let Some(ResourceChoice::Consumer { producer, .. }) =
            replacement.implementations.as_ref().unwrap().resource()
        else {
            return Err(CandidateError::InvalidRequest);
        };
        let consumed = if refresh { producer.contract() } else { prior };
        let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
        if !ResourceChoice::validate_pending_consumer(
            producer.contract().borrow(),
            consumed.borrow(),
            old.semantic.identity,
            &old.semantic.program,
            &old.semantic.uses,
            selected,
            &mut budget,
        )? {
            return Err(CandidateError::StaleEvidence);
        }
        let consumed = consumed.share(&mut budget)?;
        let producer = match producer.share(&mut budget) {
            Ok(value) => value,
            Err(error) => {
                consumed.discard(&mut budget)?;
                return Err(error.into());
            }
        };
        let resource = ResourceChoice::Consumer { producer, consumed };
        let mut map = match budget.with_ledger(|ledger| {
            let (ledger, domain) = ledger.unwrap();
            selected.prepare_resource_map(ledger, domain)
        }) {
            Ok(value) => value,
            Err(error) => {
                resource.discard(&mut budget)?;
                return Err(error.into());
            }
        };
        map.install_resource_prepared(resource);
        drop(budget);
        self.publish_candidate(base, map, policy, domain, start, None)
    }
}

#[cfg(test)]
#[path = "fixed_resource_publication_tests.rs"]
mod tests;
