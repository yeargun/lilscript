//! Completed output owns its allocation admission, independently of a target.
//! One arena implementation serves callback-local attempts and retained output.
//! Handles never expose mutable bytes; codec scores belong to those exact bytes.
use super::artifact_provenance::{ArtifactProvenance, ProvenanceError};
use super::ids::RevisionId;
use super::implementation_identity::{ImplementationDescription, SharedImplementationIdentity};
use super::publication::{CandidateError, CandidateId, LiteralOutput, OutputTactics};
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{
    AdmissionError, BudgetLedger, CandidateCostEvidence, CompilationContract, ResolvedPolicy,
    TacticUse, WorkKind,
};
use crate::config::CompressionCostModel;
use crate::js::{
    extract::Output,
    selection::{Objectives, Plan, Sizes},
    BindingId,
};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError, RetainedCharge};
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "artifact_native.rs"]
mod native;
use native::NativeRecord;
pub use native::{NativeArtifactView, QualifiedNativeArtifact};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Handle {
    arena: RevisionId,
    slot: u32,
    generation: RevisionId,
}

/// Only the originating output callback can use this temporary artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopedArtifactId(Handle);

/// An immutable completed artifact retained by its originating Compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactId(Handle);

/// Static runtime evidence is independent of the exact transfer measurement.
/// Missing evidence remains unknown, including for a direct implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArtifactRuntimeEvidence {
    pub performance_score: Option<u64>,
    pub startup_work: Option<u64>,
    pub recurring_work: Option<u64>,
    pub runtime_memory_bytes: Option<u64>,
}
impl ArtifactRuntimeEvidence {
    fn cost(self, transfer_bytes: usize) -> Result<CandidateCostEvidence, CandidateError> {
        Ok(CandidateCostEvidence {
            transfer_bytes: u64::try_from(transfer_bytes).map_err(|_| AllocationError::Capacity)?,
            performance_score: self.performance_score,
            startup_work: self.startup_work,
            recurring_work: self.recurring_work,
            runtime_memory_bytes: self.runtime_memory_bytes,
        })
    }
}

/// An exact artifact admitted under a resolved policy. The receipt may remain
/// as a baseline after its bytes are discarded; delivery still needs live bytes.
#[derive(Debug, Clone, Copy)]
pub struct QualifiedArtifact {
    artifact: ArtifactId,
    snapshot: RevisionId,
    meaning: RevisionId,
    codec: CompressionCostModel,
    cost: CandidateCostEvidence,
    policy_fingerprint: [u8; 32],
}
impl QualifiedArtifact {
    pub fn artifact(self) -> ArtifactId {
        self.artifact
    }
    pub fn codec(self) -> CompressionCostModel {
        self.codec
    }
    pub fn cost(self) -> CandidateCostEvidence {
        self.cost
    }
    pub fn policy_fingerprint(self) -> [u8; 32] {
        self.policy_fingerprint
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArtifactView<'a> {
    /// The entry file.
    pub javascript: &'a str,
    /// Chunk files loaded by the entry; every one is part of the scored bytes.
    pub chunks: &'a [ArtifactChunk],
    /// Module artifacts must be loaded as ECMAScript modules, including when
    /// their export list is empty. Script artifacts promise no strict entry.
    pub execution: JavaScriptExecution,
    /// Origin handle; it need not remain live. `implementation` owns its recipe.
    pub candidate: CandidateId,
    /// Full immutable structural selection, independent of candidate lifetime.
    pub implementation: ImplementationDescription<'a>,
    pub recipe_fingerprint: u64,
    /// Actual choices used for these immutable bytes, including canonicalized
    /// inactive literal alternatives. Permissions alone are not provenance.
    pub output: OutputTactics,
    pub sizes: Sizes,
    pub retained_capacity: usize,
}

/// Canonical gzip/Brotli streams are nonempty, even for an empty input. Zero
/// therefore denotes an unmeasured compressed coordinate without excluding
/// any valid positive usize score or changing package/relative size ceilings.
struct CachedSizes {
    raw: usize,
    gzip9: AtomicUsize,
    brotli11: AtomicUsize,
}
impl CachedSizes {
    fn new(raw: usize) -> Self {
        Self {
            raw,
            gzip9: AtomicUsize::new(0),
            brotli11: AtomicUsize::new(0),
        }
    }
    fn get(&self) -> Sizes {
        Sizes {
            raw: self.raw,
            gzip9: self.measured(CompressionCostModel::Gzip),
            brotli11: self.measured(CompressionCostModel::Brotli),
        }
    }
    fn measured(&self, codec: CompressionCostModel) -> Option<usize> {
        let slot = match codec {
            CompressionCostModel::Raw => return Some(self.raw),
            CompressionCostModel::Gzip => &self.gzip9,
            CompressionCostModel::Brotli => &self.brotli11,
        };
        // Only this independent derived integer is published. Artifact bytes
        // and provenance already have their enclosing owner's lifetime; no
        // other data is synchronized through the score slot.
        let size = slot.load(Ordering::Relaxed);
        (size != 0).then_some(size)
    }
    fn publish(&self, codec: CompressionCostModel, size: usize) -> Result<usize, CandidateError> {
        let slot = match codec {
            CompressionCostModel::Raw => unreachable!("raw size is complete at construction"),
            CompressionCostModel::Gzip => &self.gzip9,
            CompressionCostModel::Brotli => &self.brotli11,
        };
        if size == 0 {
            return Err(CandidateError::Codec(
                "canonical compressed artifact is empty",
            ));
        }
        match slot.compare_exchange(0, size, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => Ok(size),
            Err(previous) if previous == size => Ok(previous),
            Err(_) => Err(CandidateError::Codec("immutable artifact score changed")),
        }
    }
}

pub(crate) use crate::js::delivery::BundleSpec;

/// One chunk file of a multi-file artifact, scored with its entry.
pub struct ArtifactChunk {
    pub name: String,
    pub modules: Vec<u32>,
    /// Chunk files this one imports, by name.
    pub dependencies: Vec<String>,
    /// Lazy chunks this one loads with `import()`, by name.
    pub dynamic_dependencies: Vec<String>,
    /// Loaded only by `import()`.
    pub lazy: bool,
    /// Modules importing this chunk's module, when split counts them.
    pub importers: usize,
    pub code: String,
    charge: RetainedCharge<RevisionId>,
}

impl std::fmt::Debug for ArtifactChunk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ArtifactChunk")
            .field("name", &self.name)
            .field("modules", &self.modules)
            .field("bytes", &self.code.len())
            .finish()
    }
}

/// A delivered chunk file, detached from artifact storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredChunk {
    pub name: String,
    pub modules: Vec<u32>,
    pub dependencies: Vec<String>,
    pub dynamic_dependencies: Vec<String>,
    pub lazy: bool,
    /// Modules importing this chunk's module, when split counts them.
    pub importers: usize,
    pub code: String,
}

/// What a multi-file delivery's entry links to: the chunks it imports and
/// loads with `import()`, and the lazy chunks it preloads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryLinks {
    pub dependencies: Vec<String>,
    pub dynamic_dependencies: Vec<String>,
    pub preload: Vec<String>,
}

/// An exact multi-file delivery: the entry, what it links to, and every
/// chunk file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredBundle {
    pub entry: String,
    pub entry_links: EntryLinks,
    pub chunks: Vec<DeliveredChunk>,
}

struct Record {
    text: String,
    /// Chunk files delivered with the entry text; empty for one file.
    chunks: Vec<ArtifactChunk>,
    /// What the entry links to, charged with the entry text.
    entry_links: EntryLinks,
    execution: JavaScriptExecution,
    candidate: CandidateId,
    identity: SharedImplementationIdentity,
    provenance: ArtifactProvenance,
    output: OutputTactics,
    // Only derived codec scores are mutable; source bytes and provenance are
    // immutable.
    sizes: CachedSizes,
    charge: RetainedCharge<RevisionId>,
}
impl Record {
    fn view(&self) -> ArtifactView<'_> {
        ArtifactView {
            javascript: &self.text,
            chunks: &self.chunks,
            execution: self.execution,
            candidate: self.candidate,
            implementation: self.identity.description(),
            recipe_fingerprint: self.identity.fingerprint(),
            output: self.output,
            sizes: self.sizes.get(),
            retained_capacity: self.text.capacity(),
        }
    }
    fn measure(
        &self,
        codec: CompressionCostModel,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, CandidateError> {
        // A cache lookup is work, but it neither allocates nor invokes a codec.
        budget.work(WorkKind::Codec, 1)?;
        if let Some(size) = self.sizes.measured(codec) {
            return Ok(size);
        }
        let mut size = crate::compression::measure_admitted(self.text.as_bytes(), codec, budget)?;
        for chunk in &self.chunks {
            size = size
                .checked_add(crate::compression::measure_admitted(
                    chunk.code.as_bytes(),
                    codec,
                    budget,
                )?)
                .ok_or(AllocationError::Capacity)?;
        }
        // A refusal in any file, a zero score, or an overflowing package
        // sum never publishes a partial coordinate. Other completed codecs
        // retain their independent monotone entries.
        self.sizes.publish(codec, size)
    }
    fn take(
        self,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> (String, EntryLinks, Vec<DeliveredChunk>) {
        let Self {
            text,
            chunks,
            entry_links,
            charge,
            provenance,
            identity,
            ..
        } = self;
        discard_provenance(provenance, owner, budget);
        discard_identity(identity, owner, budget);
        release(charge, owner, budget);
        let chunks = chunks
            .into_iter()
            .map(|chunk| {
                release(chunk.charge, owner, budget);
                DeliveredChunk {
                    name: chunk.name,
                    modules: chunk.modules,
                    dependencies: chunk.dependencies,
                    dynamic_dependencies: chunk.dynamic_dependencies,
                    lazy: chunk.lazy,
                    importers: chunk.importers,
                    code: chunk.code,
                }
            })
            .collect();
        (text, entry_links, chunks)
    }
    fn discard(self, owner: RevisionId, budget: &mut AllocationBudget<'_>) {
        let Self {
            text,
            chunks,
            charge,
            provenance,
            identity,
            ..
        } = self;
        drop(text);
        for chunk in chunks {
            drop(chunk.code);
            release(chunk.charge, owner, budget);
        }
        discard_provenance(provenance, owner, budget);
        discard_identity(identity, owner, budget);
        release(charge, owner, budget);
    }
}

fn discard_provenance(
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

fn provenance_error(error: ProvenanceError) -> CandidateError {
    match error {
        ProvenanceError::Allocation(error) => error.into(),
        ProvenanceError::Naming(error) => error.into(),
        ProvenanceError::Admission(AdmissionError::ForbiddenTactic(tactic)) => {
            CandidateError::ForbiddenTactic(tactic)
        }
        ProvenanceError::Admission(error) => CandidateError::Admission(error),
    }
}

fn equal_bytes(
    left: &str,
    right: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, CandidateError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    budget.work(
        WorkKind::Analysis,
        u64::try_from(left.len()).map_err(|_| AllocationError::Capacity)?,
    )?;
    Ok(left == right)
}

fn same_streams(
    left: ArtifactView<'_>,
    right: ArtifactView<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, CandidateError> {
    if left.sizes.raw != right.sizes.raw || left.javascript.len() != right.javascript.len() {
        return Ok(false);
    }
    if !equal_bytes(left.javascript, right.javascript, budget)? {
        return Ok(false);
    }
    if left.chunks.len() != right.chunks.len() {
        return Ok(false);
    }
    for (left, right) in left.chunks.iter().zip(right.chunks) {
        if !equal_bytes(&left.name, &right.name, budget)?
            || !equal_bytes(&left.code, &right.code, budget)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

enum StoredRecord {
    JavaScript(Record),
    Native(NativeRecord),
}
impl StoredRecord {
    fn discard(self, owner: RevisionId, budget: &mut AllocationBudget<'_>) {
        match self {
            Self::JavaScript(record) => record.discard(owner, budget),
            Self::Native(record) => record.discard(owner, budget),
        }
    }
}

struct Slot {
    generation: RevisionId,
    record: Option<StoredRecord>,
    next_free: Option<u32>,
}

pub(super) struct ArtifactArena {
    identity: RevisionId,
    owner: RevisionId,
    slots: Vec<Slot>,
    charge: Option<RetainedCharge<RevisionId>>,
    free: Option<u32>,
}
impl ArtifactArena {
    pub(super) fn new(owner: RevisionId) -> Self {
        Self {
            identity: RevisionId::fresh(),
            owner,
            slots: Vec::new(),
            charge: None,
            free: None,
        }
    }
    fn index(&self, id: Handle) -> Result<usize, CandidateError> {
        if id.arena != self.identity {
            return Err(CandidateError::Artifact(
                "artifact belongs to another output owner",
            ));
        }
        let index = id.slot as usize;
        if self
            .slots
            .get(index)
            .is_none_or(|slot| slot.generation != id.generation || slot.record.is_none())
        {
            return Err(CandidateError::Artifact("artifact is no longer retained"));
        }
        Ok(index)
    }
    fn get(&self, id: Handle) -> Result<&Record, CandidateError> {
        match self.slots[self.index(id)?].record.as_ref().unwrap() {
            StoredRecord::JavaScript(record) => Ok(record),
            StoredRecord::Native(_) => Err(CandidateError::Artifact("not a JavaScript artifact")),
        }
    }
    fn get_mut(&mut self, id: Handle) -> Result<&mut Record, CandidateError> {
        let index = self.index(id)?;
        match self.slots[index].record.as_mut().unwrap() {
            StoredRecord::JavaScript(record) => Ok(record),
            StoredRecord::Native(_) => Err(CandidateError::Artifact("not a JavaScript artifact")),
        }
    }
    /// Admit insertion and relocation before the producer detaches any bytes.
    /// Existing storage may belong to a different work domain, so construct a
    /// fresh buffer while the old token remains charged, then move and release.
    fn prepare_insert(&mut self, budget: &mut AllocationBudget<'_>) -> Result<(), CandidateError> {
        budget.work(WorkKind::Render, 1)?;
        if self.free.is_some() || self.slots.len() < self.slots.capacity() {
            return Ok(());
        }
        let capacity = self
            .slots
            .capacity()
            .max(1)
            .checked_mul(2)
            .ok_or(AllocationError::Capacity)?
            .min(u32::MAX as usize);
        if capacity <= self.slots.len() {
            return Err(AllocationError::Capacity.into());
        }
        let mut growth = budget.scope();
        growth.work(
            WorkKind::Render,
            u64::try_from(self.slots.len()).map_err(|_| AllocationError::Capacity)?,
        )?;
        let mut slots = growth.vector::<Slot>(AllocationClass::Retained, capacity)?;
        let bytes = slots
            .capacity()
            .checked_mul(size_of::<Slot>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(AllocationError::Capacity)?;
        let charge = growth.detach_retained(self.owner, bytes)?;
        slots.append(&mut self.slots);
        let old = std::mem::replace(&mut self.slots, slots);
        drop(old);
        if let Some(old_charge) = self.charge.replace(charge) {
            release(old_charge, self.owner, &mut growth);
        }
        Ok(())
    }
    /// Infallible after prepare_insert; no callback can observe a partial move.
    fn insert(&mut self, record: Record) -> Handle {
        self.insert_record(StoredRecord::JavaScript(record))
    }
    fn insert_record(&mut self, record: StoredRecord) -> Handle {
        let generation = RevisionId::fresh();
        let slot = if let Some(index) = self.free {
            let slot = &mut self.slots[index as usize];
            self.free = slot.next_free;
            slot.generation = generation;
            slot.record = Some(record);
            slot.next_free = None;
            index
        } else {
            let index = self.slots.len() as u32;
            debug_assert!(self.slots.len() < self.slots.capacity());
            self.slots.push(Slot {
                generation,
                record: Some(record),
                next_free: None,
            });
            index
        };
        Handle {
            arena: self.identity,
            slot,
            generation,
        }
    }
    fn remove(&mut self, id: Handle) -> Result<Record, CandidateError> {
        self.get(id)?;
        let StoredRecord::JavaScript(record) = self.remove_record(id)? else {
            unreachable!("validated JavaScript artifact")
        };
        Ok(record)
    }
    fn remove_record(&mut self, id: Handle) -> Result<StoredRecord, CandidateError> {
        let index = self.index(id)?;
        let slot = &mut self.slots[index];
        let record = slot.record.take().unwrap();
        slot.next_free = self.free;
        self.free = Some(index as u32);
        Ok(record)
    }
    fn clear(&mut self, budget: &mut AllocationBudget<'_>) {
        for slot in self.slots.drain(..) {
            if let Some(record) = slot.record {
                record.discard(self.owner, budget);
            }
        }
        drop(std::mem::take(&mut self.slots));
        if let Some(charge) = self.charge.take() {
            release(charge, self.owner, budget);
        }
        self.free = None;
    }
    pub(super) fn with_artifact<R>(
        &self,
        id: ArtifactId,
        inspect: impl FnOnce(ArtifactView<'_>) -> R,
    ) -> Result<R, CandidateError> {
        Ok(inspect(self.get(id.0)?.view()))
    }
    pub(super) fn measure(
        &mut self,
        id: ArtifactId,
        codec: CompressionCostModel,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, CandidateError> {
        self.get_mut(id.0)?.measure(codec, budget)
    }
    /// Whether two retained artifacts deliver the same files, byte for byte.
    pub(super) fn same_output(
        &self,
        left: ArtifactId,
        right: ArtifactId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, CandidateError> {
        budget.work(WorkKind::Analysis, 1)?;
        same_streams(self.get(left.0)?.view(), self.get(right.0)?.view(), budget)
    }
    pub(super) fn provenance(&self, id: ArtifactId) -> Result<&ArtifactProvenance, CandidateError> {
        Ok(&self.get(id.0)?.provenance)
    }

    pub(super) fn qualify(
        &self,
        id: ArtifactId,
        formation_contract: &CompilationContract,
        policy: &ResolvedPolicy,
        codec: CompressionCostModel,
        runtime: ArtifactRuntimeEvidence,
        baseline: Option<&QualifiedArtifact>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<QualifiedArtifact, CandidateError> {
        budget.work(WorkKind::Analysis, 1)?;
        for contract in [formation_contract, policy.contract()] {
            if let CompilationContract::JavaScript {
                preserved_properties,
                ..
            } = contract
            {
                budget.work(
                    WorkKind::Analysis,
                    u64::try_from(preserved_properties.len())
                        .map_err(|_| AllocationError::Capacity)?,
                )?;
                for name in preserved_properties {
                    budget.work(
                        WorkKind::Analysis,
                        u64::try_from(name.len()).map_err(|_| AllocationError::Capacity)?,
                    )?;
                }
            }
        }
        if formation_contract != policy.contract() {
            return Err(CandidateError::ContractMismatch);
        }
        let record = self.get(id.0)?;
        let size = record
            .sizes
            .measured(codec)
            .ok_or(CandidateError::Artifact(
                "requested artifact codec is unmeasured",
            ))?;
        let cost = runtime.cost(size)?;
        let base = if let Some(baseline) = baseline {
            if baseline.artifact.0.arena != self.identity
                || baseline.meaning != record.identity.meaning()
                || baseline.codec != codec
            {
                return Err(CandidateError::Artifact(
                    "baseline belongs to a different artifact contract",
                ));
            }
            baseline.cost
        } else {
            cost
        };
        record
            .provenance
            .admit(policy, cost, base, budget)
            .map_err(provenance_error)?;
        Ok(QualifiedArtifact {
            artifact: id,
            snapshot: record.identity.snapshot(),
            meaning: record.identity.meaning(),
            codec,
            cost,
            policy_fingerprint: policy.fingerprint(),
        })
    }

    pub(super) fn check_qualified(
        &self,
        qualified: &QualifiedArtifact,
    ) -> Result<(), CandidateError> {
        let record = self.get(qualified.artifact.0)?;
        if record.identity.snapshot() != qualified.snapshot
            || record.identity.meaning() != qualified.meaning
            || record
                .sizes
                .measured(qualified.codec)
                .and_then(|size| u64::try_from(size).ok())
                != Some(qualified.cost.transfer_bytes)
        {
            return Err(CandidateError::Artifact(
                "qualified artifact identity changed",
            ));
        }
        Ok(())
    }
    /// Reuse only exact scores from live retained records. Byte equality does
    /// not share candidate eligibility, provenance or structural continuations.
    pub(super) fn reuse_scores(
        &self,
        id: ArtifactId,
        objectives: Objectives,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Sizes, CandidateError> {
        budget.work(WorkKind::Analysis, 1)?;
        let target = self.get(id.0)?;
        let original = target.sizes.get();
        let mut known = original;
        if objectives.iter().all(|codec| known.get(codec).is_some()) {
            return Ok(known);
        }
        for slot in &self.slots {
            budget.work(WorkKind::Analysis, 1)?;
            let Some(StoredRecord::JavaScript(donor)) = &slot.record else {
                continue;
            };
            let measured = donor.sizes.get();
            if !objectives
                .iter()
                .any(|codec| known.get(codec).is_none() && measured.get(codec).is_some())
                || !same_streams(target.view(), donor.view(), budget)?
            {
                continue;
            }
            for codec in objectives.iter() {
                match codec {
                    CompressionCostModel::Raw => {}
                    CompressionCostModel::Gzip => known.gzip9 = known.gzip9.or(measured.gzip9),
                    CompressionCostModel::Brotli => {
                        known.brotli11 = known.brotli11.or(measured.brotli11)
                    }
                }
            }
            if objectives.iter().all(|codec| known.get(codec).is_some()) {
                break;
            }
        }
        // A refused comparison publishes nothing, even after finding one codec.
        for codec in objectives.iter() {
            if original.get(codec).is_none() {
                if let Some(size) = known.get(codec) {
                    target.sizes.publish(codec, size)?;
                }
            }
        }
        Ok(known)
    }
    pub(super) fn take(
        &mut self,
        id: ArtifactId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<String, CandidateError> {
        if !self.get(id.0)?.chunks.is_empty() {
            return Err(CandidateError::Artifact(
                "a multi-file artifact is delivered with all of its chunks",
            ));
        }
        Ok(self.remove(id.0)?.take(self.owner, budget).0)
    }
    /// The entry text and every chunk file of one artifact.
    pub(super) fn take_bundle(
        &mut self,
        id: ArtifactId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<DeliveredBundle, CandidateError> {
        let (entry, entry_links, chunks) = self.remove(id.0)?.take(self.owner, budget);
        Ok(DeliveredBundle {
            entry,
            entry_links,
            chunks,
        })
    }
    pub(super) fn discard(
        &mut self,
        id: ArtifactId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), CandidateError> {
        self.remove(id.0)?.discard(self.owner, budget);
        Ok(())
    }
    pub(super) fn finish(mut self, ledger: &mut BudgetLedger) {
        self.clear(&mut AllocationBudget::new(Some((
            ledger,
            crate::compilation_policy::WorkDomain::Baseline,
        ))));
    }
}

fn release(
    charge: RetainedCharge<RevisionId>,
    owner: RevisionId,
    budget: &mut AllocationBudget<'_>,
) {
    budget.with_ledger(|ledger| {
        let (ledger, _) = ledger.expect("artifact storage always has a compilation ledger");
        charge
            .discard(&owner, ledger)
            .unwrap_or_else(|_| panic!("artifact allocation owner invariant"));
    });
}

fn discard_identity(
    identity: SharedImplementationIdentity,
    owner: RevisionId,
    budget: &mut AllocationBudget<'_>,
) {
    budget.with_ledger(|ledger| {
        identity.discard(owner, ledger.unwrap().0).unwrap();
    });
}

/// The only production rendering boundary. Temporary artifacts are released on
/// callback return or unwind; retain_artifact explicitly transfers an incumbent.
/// The application may copy a borrowed view, but those external allocations are
/// outside the compiler's memory contract, as is an explicit terminal handoff.
///
/// The prepared facade cannot outlive its target or retain a Compilation borrow:
/// ```compile_fail
/// use lilscript::program::publication::{Compilation, CandidateId};
/// use lilscript::compilation_policy::ResolvedPolicy;
/// fn escape(compilation: &mut Compilation<'_>, candidate: CandidateId, policy: &ResolvedPolicy) {
///     let escaped = compilation.with_javascript_output(candidate, policy, |output| output);
/// }
/// ```
/// A borrowed artifact cannot silently become a retained, uncharged output:
/// ```compile_fail
/// use lilscript::program::publication::{Compilation, ArtifactId};
/// fn escape(compilation: &Compilation<'_>, artifact: ArtifactId) {
///     let escaped = compilation.with_artifact(artifact, |view| view.javascript);
/// }
/// ```
/// A view's borrow excludes mutation of its containing staging arena:
/// ```compile_fail
/// use lilscript::program::publication::{BudgetedJavaScriptOutput, ScopedArtifactId};
/// fn mutate(output: &mut BudgetedJavaScriptOutput<'_, '_>, artifact: ScopedArtifactId) {
///     output.with_artifact(artifact, |view| {
///         output.discard_artifact(artifact).unwrap();
///         println!("{}", view.javascript);
///     }).unwrap();
/// }
/// ```
pub struct BudgetedJavaScriptOutput<'scope, 'target> {
    output: &'scope Output<'target>,
    staging: ArtifactArena,
    retained: &'scope mut ArtifactArena,
    candidate: CandidateId,
    identity: &'scope SharedImplementationIdentity,
    structural: &'scope [TacticUse],
    policy: &'scope ResolvedPolicy,
    execution: JavaScriptExecution,
    choices: OutputTactics,
    /// Multi-file delivery, when the contract asks for it.
    bundle: Option<&'scope BundleSpec>,
}
impl<'scope, 'target> BudgetedJavaScriptOutput<'scope, 'target> {
    pub(super) fn with_bundle(mut self, bundle: Option<&'scope BundleSpec>) -> Self {
        self.bundle = bundle;
        self
    }
    pub(super) fn new(
        output: &'scope Output<'target>,
        retained: &'scope mut ArtifactArena,
        candidate: CandidateId,
        identity: &'scope SharedImplementationIdentity,
        structural: &'scope [TacticUse],
        policy: &'scope ResolvedPolicy,
        execution: JavaScriptExecution,
        choices: OutputTactics,
    ) -> Self {
        Self {
            output,
            staging: ArtifactArena::new(retained.owner),
            retained,
            candidate,
            identity,
            structural,
            policy,
            execution,
            choices,
            bundle: None,
        }
    }
    pub fn render(&mut self, plan: &Plan) -> Result<ScopedArtifactId, CandidateError> {
        self.render_bounded(plan, usize::MAX)
    }
    pub fn render_bounded(
        &mut self,
        plan: &Plan,
        byte_limit: usize,
    ) -> Result<ScopedArtifactId, CandidateError> {
        self.render_bounded_with_literals(plan, self.choices.literals, byte_limit)
    }
    /// All modes borrow the same verified target and naming basis. Only inert
    /// occurrences certified by Formation can differ between these renders.
    pub fn render_bounded_with_literals(
        &mut self,
        plan: &Plan,
        literals: LiteralOutput,
        byte_limit: usize,
    ) -> Result<ScopedArtifactId, CandidateError> {
        if literals == LiteralOutput::Observed && !self.choices.target_compaction {
            return Err(CandidateError::ForbiddenTactic(
                crate::compilation_policy::TacticId::TargetCompaction,
            ));
        }
        self.output
            .with_allocation_budget(|budget| self.staging.prepare_insert(budget))?;
        let (text, charge, literals, chunks, entry_links) = match self.bundle {
            Some(bundle) => {
                let (rendered, literals) = self.output.render_bundle_with_literals_admitted(
                    plan,
                    literals,
                    byte_limit,
                    self.staging.owner,
                    bundle,
                )?;
                let (text, charge) = rendered.entry;
                let chunks = rendered
                    .chunks
                    .into_iter()
                    .map(|chunk| ArtifactChunk {
                        name: chunk.name,
                        modules: chunk.modules,
                        dependencies: chunk.dependencies,
                        dynamic_dependencies: chunk.dynamic_dependencies,
                        lazy: chunk.lazy,
                        importers: chunk.importers,
                        code: chunk.code,
                        charge: chunk.charge,
                    })
                    .collect::<Vec<_>>();
                let links = EntryLinks {
                    dependencies: rendered.entry_dependencies,
                    dynamic_dependencies: rendered.entry_dynamic_dependencies,
                    preload: rendered.preload,
                };
                (text, charge, literals, chunks, links)
            }
            None => {
                let (text, charge, literals) = self.output.render_with_literals_admitted(
                    plan,
                    literals,
                    byte_limit,
                    self.staging.owner,
                )?;
                (text, charge, literals, Vec::new(), EntryLinks::default())
            }
        };
        let actual_output = OutputTactics {
            literals,
            ..self.choices
        };
        let retained = match self.output.with_allocation_budget(|budget| {
            let provenance = ArtifactProvenance::build(
                self.structural.iter().chain(self.identity.tactics()),
                actual_output,
                plan,
                self.policy,
                self.staging.owner,
                budget,
            )
            .map_err(provenance_error)?;
            match self.identity.share(budget) {
                Ok(identity) => Ok((provenance, identity)),
                Err(error) => {
                    discard_provenance(provenance, self.staging.owner, budget);
                    Err(error.into())
                }
            }
        }) {
            Ok(retained) => retained,
            Err(error) => {
                drop(text);
                self.output.with_allocation_budget(|budget| {
                    release(charge, self.staging.owner, budget);
                    for chunk in chunks {
                        release(chunk.charge, self.staging.owner, budget);
                    }
                });
                return Err(error);
            }
        };
        let (provenance, identity) = retained;
        let chunk_bytes: usize = chunks.iter().map(|chunk| chunk.code.len()).sum();
        let sizes = CachedSizes::new(text.len() + chunk_bytes);
        Ok(ScopedArtifactId(self.staging.insert(Record {
            text,
            chunks,
            entry_links,
            execution: self.execution,
            candidate: self.candidate,
            identity,
            provenance,
            output: actual_output,
            sizes,
            charge,
        })))
    }
    pub fn has_literal_alternative(&self) -> Result<bool, CandidateError> {
        Ok(self.output.has_literal_alternative_admitted()?)
    }
    pub fn source_candidates(&self) -> Result<&[BindingId], CandidateError> {
        Ok(self.output.source_candidates_admitted()?)
    }
    /// Search uses this output owner's existing resource allowance.
    pub(crate) fn with_allocation_budget<R>(
        &self,
        inspect: impl FnOnce(&mut AllocationBudget<'_>) -> R,
    ) -> R {
        self.output.with_allocation_budget(inspect)
    }
    /// Use the same retained-byte and score service while a prepared target is
    /// alive. Immediate and deferred search share one admission/promotion path.
    pub(super) fn with_retained_arena<R>(
        &mut self,
        inspect: impl FnOnce(&mut ArtifactArena, &mut AllocationBudget<'_>) -> R,
    ) -> R {
        self.output
            .with_allocation_budget(|budget| inspect(self.retained, budget))
    }
    pub fn with_artifact<R>(
        &self,
        id: ScopedArtifactId,
        inspect: impl FnOnce(ArtifactView<'_>) -> R,
    ) -> Result<R, CandidateError> {
        Ok(inspect(self.staging.get(id.0)?.view()))
    }
    pub fn measure(
        &mut self,
        id: ScopedArtifactId,
        codec: CompressionCostModel,
    ) -> Result<usize, CandidateError> {
        self.output
            .with_allocation_budget(|budget| self.staging.get_mut(id.0)?.measure(codec, budget))
    }
    pub fn retain_artifact(&mut self, id: ScopedArtifactId) -> Result<ArtifactId, CandidateError> {
        self.staging.index(id.0)?;
        self.output
            .with_allocation_budget(|budget| self.retained.prepare_insert(budget))?;
        Ok(ArtifactId(self.retained.insert(self.staging.remove(id.0)?)))
    }
    pub fn take_artifact(&mut self, id: ScopedArtifactId) -> Result<String, CandidateError> {
        if !self.staging.get(id.0)?.chunks.is_empty() {
            return Err(CandidateError::Artifact(
                "a multi-file artifact is delivered with all of its chunks",
            ));
        }
        let record = self.staging.remove(id.0)?;
        Ok(self
            .output
            .with_allocation_budget(|budget| record.take(self.staging.owner, budget).0))
    }
    pub fn discard_artifact(&mut self, id: ScopedArtifactId) -> Result<(), CandidateError> {
        let record = self.staging.remove(id.0)?;
        self.output
            .with_allocation_budget(|budget| record.discard(self.staging.owner, budget));
        Ok(())
    }
}
impl Drop for BudgetedJavaScriptOutput<'_, '_> {
    fn drop(&mut self) {
        self.output
            .with_allocation_budget(|budget| self.staging.clear(budget));
    }
}

#[cfg(test)]
#[path = "artifact_scores_tests.rs"]
mod scores_tests;

#[cfg(test)]
#[path = "artifact_reuse_tests.rs"]
mod reuse_tests;

#[cfg(test)]
#[path = "artifact_admission_tests.rs"]
mod admission_tests;
