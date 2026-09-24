//! Native delivery shares the compilation artifact arena and allocation owner.
//! C/header byte counts describe emitted source files, not linked executable size.
use super::*;
use crate::program::native::{NativeArtifacts, NativeError};
use crate::program::rewrite_lineage::{RewriteDescription, RewriteLineage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Qualification {
    snapshot: RevisionId,
    meaning: RevisionId,
    abi_version: u32,
    callback_abi_version: u32,
    c_bytes: usize,
    header_bytes: usize,
    cost: CandidateCostEvidence,
    policy_fingerprint: [u8; 32],
}

/// A policy-admitted immutable pair of native C/header source files.
#[derive(Debug, Clone, Copy)]
pub struct QualifiedNativeArtifact {
    artifact: Handle,
    qualification: Qualification,
}
impl QualifiedNativeArtifact {
    /// Exact total C/header source bytes plus qualified static runtime estimates.
    /// This does not measure the separately compiled native executable.
    pub fn cost(self) -> CandidateCostEvidence {
        self.qualification.cost
    }
    pub fn c_bytes(self) -> usize {
        self.qualification.c_bytes
    }
    pub fn header_bytes(self) -> usize {
        self.qualification.header_bytes
    }
    pub fn abi_version(self) -> u32 {
        self.qualification.abi_version
    }
    pub fn callback_abi_version(self) -> u32 {
        self.qualification.callback_abi_version
    }
    pub fn policy_fingerprint(self) -> [u8; 32] {
        self.qualification.policy_fingerprint
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeArtifactView<'a> {
    pub c: &'a str,
    pub header: &'a str,
    pub retained_capacity: usize,
    pub snapshot: RevisionId,
    pub meaning: RevisionId,
    pub rewrites: RewriteDescription<'a>,
}

pub(super) struct NativeRecord {
    files: NativeArtifacts,
    qualification: Qualification,
    lineage: RewriteLineage,
    charge: RetainedCharge<RevisionId>,
}
impl NativeRecord {
    pub(super) fn discard(self, owner: RevisionId, budget: &mut AllocationBudget<'_>) {
        let Self {
            files,
            charge,
            lineage,
            ..
        } = self;
        drop(files);
        budget.with_ledger(|ledger| lineage.discard(ledger.unwrap().0));
        release(charge, owner, budget);
    }
}

impl ArtifactArena {
    pub(in crate::program) fn retain_native(
        &mut self,
        files: NativeArtifacts,
        snapshot: RevisionId,
        meaning: RevisionId,
        lineage: &RewriteLineage,
        callback_abi_version: u32,
        policy: &ResolvedPolicy,
        runtime: ArtifactRuntimeEvidence,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<QualifiedNativeArtifact, NativeError> {
        budget.work(WorkKind::Analysis, 1)?;
        let CompilationContract::Native { abi_version } = policy.contract() else {
            return Err(NativeError::WrongTarget);
        };
        let bytes = files
            .c
            .len()
            .checked_add(files.header.len())
            .ok_or(AllocationError::Capacity)?;
        let capacity = files
            .c
            .capacity()
            .checked_add(files.header.capacity())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(AllocationError::Capacity)?;
        let cost = runtime.cost(bytes)?;
        policy
            .admit_evidence(lineage.tactics(), cost, cost)
            .map_err(NativeError::Admission)?;
        let qualification = Qualification {
            snapshot,
            meaning,
            abi_version: *abi_version,
            callback_abi_version,
            c_bytes: files.c.len(),
            header_bytes: files.header.len(),
            cost,
            policy_fingerprint: policy.fingerprint(),
        };
        self.prepare_insert(budget)?;
        let lineage = lineage.share(budget)?;
        let charge = match budget.detach_retained(self.owner, capacity) {
            Ok(charge) => charge,
            Err(error) => {
                budget.with_ledger(|ledger| lineage.discard(ledger.unwrap().0));
                return Err(error.into());
            }
        };
        let artifact = self.insert_record(StoredRecord::Native(NativeRecord {
            files,
            qualification,
            lineage,
            charge,
        }));
        Ok(QualifiedNativeArtifact {
            artifact,
            qualification,
        })
    }

    fn native_record(
        &self,
        receipt: &QualifiedNativeArtifact,
    ) -> Result<&NativeRecord, NativeError> {
        let index = self.index(receipt.artifact)?;
        let Some(StoredRecord::Native(record)) = &self.slots[index].record else {
            return Err(CandidateError::Artifact("not a native artifact").into());
        };
        if record.qualification != receipt.qualification {
            return Err(
                CandidateError::Artifact("qualified native artifact identity changed").into(),
            );
        }
        Ok(record)
    }

    pub(in crate::program) fn with_qualified_native<R>(
        &self,
        receipt: &QualifiedNativeArtifact,
        inspect: impl FnOnce(NativeArtifactView<'_>) -> R,
    ) -> Result<R, NativeError> {
        let record = self.native_record(receipt)?;
        Ok(inspect(NativeArtifactView {
            c: &record.files.c,
            header: &record.files.header,
            retained_capacity: record.files.c.capacity() + record.files.header.capacity(),
            snapshot: record.qualification.snapshot,
            meaning: record.qualification.meaning,
            rewrites: record.lineage.description(),
        }))
    }

    pub(in crate::program) fn take_qualified_native(
        &mut self,
        receipt: QualifiedNativeArtifact,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(String, String), NativeError> {
        self.native_record(&receipt)?;
        let StoredRecord::Native(record) = self.remove_record(receipt.artifact)? else {
            unreachable!("validated native artifact")
        };
        let NativeRecord {
            files,
            charge,
            lineage,
            ..
        } = record;
        budget.with_ledger(|ledger| lineage.discard(ledger.unwrap().0));
        release(charge, self.owner, budget);
        Ok((files.c, files.header))
    }
}

#[cfg(test)]
#[path = "native_artifact_tests.rs"]
mod tests;
