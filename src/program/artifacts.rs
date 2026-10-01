//! Completed output owns its allocation admission, independently of a target.
//! One arena implementation serves callback-local attempts and retained output.
//! Handles never expose mutable bytes; codec scores belong to those exact bytes.
use super::artifact_provenance::{ArtifactProvenance, ProvenanceError};
use super::ids::RevisionId;
use super::implementation_identity::{ImplementationDescription, SharedImplementationIdentity};
use super::publication::{CandidateError, CandidateId, LiteralOutput, OutputTactics};
use crate::admission_parse::StructureDigest;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{
    AdmissionError, BudgetLedger, CandidateCostEvidence, CompilationContract, ResolvedPolicy,
    TacticUse, WorkKind,
};
use crate::compression::CodecSettings;
use crate::config::CompressionCostModel;
use crate::js::{
    extract::Output,
    selection::{Objectives, Plan, Sizes},
    BindingId,
};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError, RetainedCharge};
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "artifact_compression.rs"]
pub(super) mod compression_cache;
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
    /// Whether `cost` carries the codec's exact size. Without a walk (level
    /// 0) no codec runs, and the artifact is admitted on its raw bytes
    /// (M3.5).
    exact: bool,
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
    pub fn exact(self) -> bool {
        self.exact
    }
    pub fn policy_fingerprint(self) -> [u8; 32] {
        self.policy_fingerprint
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactView<'a> {
    /// The one delivered file; empty when a delivery plan places the output
    /// in several files (`files`).
    pub javascript: &'a str,
    /// Every file of a delivery plan, in plan order; each is part of the
    /// scored bytes. Empty for one file.
    pub files: &'a [ArtifactFile],
    /// The plan's files, entries and links, when there is one.
    pub layout: Option<&'a crate::js::delivery::DeliveredLayout>,
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
    /// The scores: one file's sizes, or a plan's sum of entry rows (design
    /// §10), which is the sum of its files' sizes when every file is in one
    /// row.
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
    /// The walk's proxy judge for Brotli (architecture §9.4), measured once.
    brotli_proxy: AtomicUsize,
}
impl CachedSizes {
    fn new(raw: usize) -> Self {
        Self {
            raw,
            gzip9: AtomicUsize::new(0),
            brotli11: AtomicUsize::new(0),
            brotli_proxy: AtomicUsize::new(0),
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

/// One file of a delivery plan, scored with the others.
pub struct ArtifactFile {
    pub name: String,
    pub code: String,
    /// This file's own codec sizes, measured once.
    sizes: CachedSizes,
}

impl ArtifactFile {
    /// This file's measured sizes.
    pub fn sizes(&self) -> Sizes {
        self.sizes.get()
    }
}

impl std::fmt::Debug for ArtifactFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ArtifactFile")
            .field("name", &self.name)
            .field("bytes", &self.code.len())
            .finish()
    }
}

/// A delivered file of a plan, detached from artifact storage, with the
/// codec sizes the search measured (manifest v3 reads these; it never
/// encodes again).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeliveredFile {
    pub name: String,
    pub code: String,
    pub sizes: Sizes,
}

/// An exact multi-file delivery: every file and the plan's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredFiles {
    pub files: Vec<DeliveredFile>,
    pub layout: crate::js::delivery::DeliveredLayout,
}

struct Record {
    text: String,
    /// Every file of a delivery plan; empty for one file (`text`).
    files: Vec<ArtifactFile>,
    layout: Option<crate::js::delivery::DeliveredLayout>,
    execution: JavaScriptExecution,
    candidate: CandidateId,
    identity: SharedImplementationIdentity,
    provenance: ArtifactProvenance,
    output: OutputTactics,
    // Only derived codec scores are mutable; source bytes and provenance are
    // immutable.
    sizes: CachedSizes,
    /// The printed tree's structural digest (plan task M2.5), for a file
    /// printed whole; a delivery plan's files are parsed without one.
    structure: Option<StructureDigest>,
    /// The admission parse's verdict on these immutable bytes, once reached:
    /// qualification under each codec reuses it.
    parsed: std::sync::OnceLock<Result<(), Box<str>>>,
    charge: RetainedCharge<RevisionId>,
}
impl Record {
    /// Admission's independent parse (plan task M2.5; architecture A5): every
    /// delivered file parses as it executes, and a file printed whole parses
    /// to the printed tree's structure. Charged once per record, linear in its
    /// bytes.
    fn admit_parse(&self, budget: &mut AllocationBudget<'_>) -> Result<(), CandidateError> {
        let verdict = match self.parsed.get() {
            Some(verdict) => verdict,
            None => {
                let _timing = crate::timing::ADMISSION_PARSE.scope(0);
                let bytes =
                    self.text.len() + self.files.iter().map(|file| file.code.len()).sum::<usize>();
                budget.work(
                    WorkKind::Analysis,
                    crate::admission_parse::work_units(bytes),
                )?;
                let verdict = (|| {
                    match &self.layout {
                        None => {
                            let module = self.execution == JavaScriptExecution::Module;
                            match &self.structure {
                                Some(expected) => {
                                    crate::admission_parse::admit(expected, &self.text, module)?
                                }
                                None => {
                                    crate::admission_parse::parse_canonical(&self.text, module)?;
                                }
                            }
                        }
                        // A plan's files link as its format says: ES modules,
                        // or CommonJS scripts.
                        Some(layout) => {
                            let module = layout.format == crate::config::JavaScriptFormat::Esm;
                            for file in &self.files {
                                crate::admission_parse::parse_canonical(&file.code, module)
                                    .map_err(|refusal| {
                                        crate::admission_parse::Refusal(format!(
                                            "file {}: {}",
                                            file.name, refusal.0
                                        ))
                                    })?;
                            }
                        }
                    }
                    Ok(())
                })()
                .map_err(|refusal: crate::admission_parse::Refusal| refusal.0.into_boxed_str());
                self.parsed.get_or_init(|| verdict)
            }
        };
        verdict.clone().map_err(CandidateError::AdmissionParse)
    }
    fn view(&self) -> ArtifactView<'_> {
        ArtifactView {
            javascript: &self.text,
            files: &self.files,
            layout: self.layout.as_ref(),
            execution: self.execution,
            candidate: self.candidate,
            implementation: self.identity.description(),
            recipe_fingerprint: self.identity.fingerprint(),
            output: self.output.clone(),
            sizes: self.sizes.get(),
            // A plan's bytes live in its files: the search's byte floor
            // counts them as it counts one file's text.
            retained_capacity: self.files.iter().fold(self.text.capacity(), |total, file| {
                total
                    .saturating_add(file.code.capacity())
                    .saturating_add(file.name.capacity())
            }),
        }
    }
    /// Each file's size under `codec`, measuring each at most once.
    fn file_sizes(
        &self,
        codec: CompressionCostModel,
        settings: &CodecSettings,
        cache: &mut compression_cache::Measurements,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<usize>, CandidateError> {
        let mut sizes = budget.vector(AllocationClass::Retained, self.files.len())?;
        for file in &self.files {
            budget.work(WorkKind::Codec, 1)?;
            let size = match file.sizes.measured(codec) {
                Some(size) => size,
                None => {
                    let size = cache.measure(
                        owner,
                        file.code.as_bytes(),
                        codec,
                        settings,
                        crate::compression::Role::Exact,
                        budget,
                    )?;
                    file.sizes.publish(codec, size)?
                }
            };
            sizes.push(size);
        }
        Ok(sizes)
    }
    /// Each entry's row under `codec` (design §10); one row for one file.
    fn rows(
        &self,
        codec: CompressionCostModel,
        settings: &CodecSettings,
        cache: &mut compression_cache::Measurements,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<u64>, CandidateError> {
        match &self.layout {
            None => {
                let size = self.measure(codec, settings, cache, owner, budget)? as u64;
                Ok(budget.copy_slice(AllocationClass::Retained, &[size])?)
            }
            Some(layout) => budget.with_temporary(
                |budget| self.file_sizes(codec, settings, cache, owner, budget),
                |sizes, budget| Ok(layout.rows_in(sizes, AllocationClass::Retained, budget)?),
            ),
        }
    }
    /// The proxy judge's size under `codec`: gzip and raw are their own
    /// proxies (exact); Brotli is measured at the proxy quality, once, unless
    /// the objective's quality is no higher. A delivery plan's files are
    /// judged exactly.
    fn proxy(
        &self,
        codec: CompressionCostModel,
        settings: &CodecSettings,
        cache: &mut compression_cache::Measurements,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, CandidateError> {
        if proxy_is_exact(codec, settings, self.layout.is_some()) {
            return self.measure(codec, settings, cache, owner, budget);
        }
        budget.work(WorkKind::Codec, 1)?;
        let slot = &self.sizes.brotli_proxy;
        let cached = slot.load(std::sync::atomic::Ordering::Relaxed);
        if cached != 0 {
            return Ok(cached);
        }
        let size = cache.measure(
            owner,
            self.text.as_bytes(),
            codec,
            settings,
            crate::compression::Role::Proxy,
            budget,
        )?;
        if size == 0 {
            return Err(CandidateError::Codec("proxy compressed artifact is empty"));
        }
        slot.store(size, std::sync::atomic::Ordering::Relaxed);
        Ok(size)
    }
    fn measure(
        &self,
        codec: CompressionCostModel,
        settings: &CodecSettings,
        cache: &mut compression_cache::Measurements,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, CandidateError> {
        // A cache lookup is work, but it neither allocates nor invokes a codec.
        budget.work(WorkKind::Codec, 1)?;
        if let Some(size) = self.sizes.measured(codec) {
            return Ok(size);
        }
        let size = match &self.layout {
            None => cache.measure(
                owner,
                self.text.as_bytes(),
                codec,
                settings,
                crate::compression::Role::Exact,
                budget,
            )?,
            // The sum of the entries' rows: shared code weighs by how many
            // entries load it. A refusal in any file, a zero score, or an
            // overflowing sum never publishes a partial coordinate.
            Some(layout) => budget.with_temporary(
                |budget| self.file_sizes(codec, settings, cache, owner, budget),
                |sizes, budget| {
                    let mut phase = budget.scope();
                    let rows = layout.rows_in(sizes, AllocationClass::Scratch, &mut phase)?;
                    let total = rows.iter().try_fold(0u64, |sum, row| {
                        sum.checked_add(*row).ok_or(AllocationError::Capacity)
                    })?;
                    Ok::<_, CandidateError>(
                        usize::try_from(total).map_err(|_| AllocationError::Capacity)?,
                    )
                },
            )?,
        };
        // Other completed codecs retain their independent monotone entries.
        self.sizes.publish(codec, size)
    }
    fn take(
        self,
        owner: RevisionId,
        budget: &mut AllocationBudget<'_>,
    ) -> (
        String,
        Vec<DeliveredFile>,
        Option<crate::js::delivery::DeliveredLayout>,
    ) {
        let Self {
            text,
            files,
            layout,
            charge,
            provenance,
            identity,
            ..
        } = self;
        discard_provenance(provenance, owner, budget);
        discard_identity(identity, owner, budget);
        release(charge, owner, budget);
        let files = files
            .into_iter()
            .map(|file| DeliveredFile {
                sizes: file.sizes.get(),
                name: file.name,
                code: file.code,
            })
            .collect();
        (text, files, layout)
    }
    fn discard(self, owner: RevisionId, budget: &mut AllocationBudget<'_>) {
        let Self {
            text,
            files,
            charge,
            layout,
            provenance,
            identity,
            ..
        } = self;
        drop((text, files, layout));
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
    if left.files.len() != right.files.len() {
        return Ok(false);
    }
    for (left, right) in left.files.iter().zip(right.files) {
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

/// Whether the proxy judge's measurement is the exact one (architecture
/// §9.4): gzip and raw are their own proxies, a delivery plan's files are
/// judged exactly, and Brotli's proxy is its exact codec when the objective's
/// quality is no higher than the proxy's.
fn proxy_is_exact(codec: CompressionCostModel, settings: &CodecSettings, layout: bool) -> bool {
    codec != CompressionCostModel::Brotli
        || layout
        || settings.brotli.quality <= crate::compression::PROXY_BROTLI_QUALITY
}

pub(super) struct ArtifactArena {
    identity: RevisionId,
    owner: RevisionId,
    slots: Vec<Slot>,
    charge: Option<RetainedCharge<RevisionId>>,
    free: Option<u32>,
    live: usize,
    /// The codec settings every size this arena measures is judged with:
    /// the objective's, bound with the JavaScript contract (law B2).
    settings: CodecSettings,
    measurements: std::cell::RefCell<compression_cache::Measurements>,
    bound: bool,
}
impl ArtifactArena {
    pub(super) fn new(owner: RevisionId) -> Self {
        Self::with_settings(owner, CodecSettings::CANONICAL)
    }
    #[cfg(test)]
    pub(super) fn measurement_hits(&self) -> usize {
        self.measurements.borrow().hits
    }
    fn with_settings(owner: RevisionId, settings: CodecSettings) -> Self {
        Self {
            identity: RevisionId::fresh(),
            owner,
            slots: Vec::new(),
            charge: None,
            free: None,
            live: 0,
            settings,
            measurements: std::cell::RefCell::default(),
            bound: false,
        }
    }
    pub(super) fn configure_measurements(&mut self, config: &crate::config::CacheConfig) {
        self.measurements.get_mut().configure(config);
    }
    pub(super) fn measurement_stats(&self) -> compression_cache::MeasurementStats {
        self.measurements.borrow().stats
    }
    /// Bind the objective's codec settings, once per compilation: sizes are
    /// cached per artifact and codec, so one compilation measures with one
    /// setting.
    pub(super) fn bind_codec_settings(
        &mut self,
        settings: CodecSettings,
    ) -> Result<(), CandidateError> {
        if self.bound && self.settings != settings {
            return Err(CandidateError::Artifact(
                "a compilation measures with one objective's codec settings",
            ));
        }
        self.settings = settings;
        self.bound = true;
        Ok(())
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
    #[cfg(test)]
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
        self.live += 1;
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
        self.live -= 1;
        let slot = &mut self.slots[index];
        let record = slot.record.take().unwrap();
        slot.next_free = self.free;
        self.free = Some(index as u32);
        Ok(record)
    }
    fn clear(&mut self, budget: &mut AllocationBudget<'_>) {
        self.measurements.get_mut().clear(self.owner, budget);
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
        self.live = 0;
    }
    // Keep scores between trials while an incumbent or another artifact is
    // live. An empty output owner has no continuation that needs this memo.
    fn release_idle_measurements(&mut self, budget: &mut AllocationBudget<'_>) {
        if self.live == 0 {
            self.measurements.get_mut().clear(self.owner, budget);
        }
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
        let settings = self.settings;
        self.get(id.0)?.measure(
            codec,
            &settings,
            &mut self.measurements.borrow_mut(),
            self.owner,
            budget,
        )
    }
    /// The walk's proxy judgement of an artifact (architecture §9.4).
    pub(super) fn measure_proxy(
        &mut self,
        id: ArtifactId,
        codec: CompressionCostModel,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, CandidateError> {
        let settings = self.settings;
        self.get(id.0)?.proxy(
            codec,
            &settings,
            &mut self.measurements.borrow_mut(),
            self.owner,
            budget,
        )
    }
    /// Whether the proxy judge's size under `codec` is the exact one, so its
    /// measurement spent the exact codec (gzip, raw, a delivery plan's
    /// files, and Brotli at a quality no higher than the proxy's).
    pub(super) fn proxy_is_exact(
        &self,
        id: ArtifactId,
        codec: CompressionCostModel,
    ) -> Result<bool, CandidateError> {
        Ok(proxy_is_exact(
            codec,
            &self.settings,
            self.get(id.0)?.layout.is_some(),
        ))
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
    /// Tests only: put a delivered text in place that is not the printed
    /// tree's, as a printer bug would, so admission's parse can refuse it.
    #[cfg(test)]
    pub(super) fn replace_text(&mut self, id: ArtifactId, text: String) {
        let record = self.get_mut(id.0).unwrap();
        record.text = text;
        record.parsed = std::sync::OnceLock::new();
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
        self.qualify_with(
            id,
            formation_contract,
            policy,
            codec,
            runtime,
            baseline,
            true,
            budget,
        )
    }

    /// Admission without the codec (M3.5): the artifact is qualified on its
    /// raw bytes, which stand in for the codec's size. Only the level-0
    /// artifact of a build without a walk is admitted so; nothing compares
    /// it with another artifact.
    pub(super) fn qualify_unmeasured(
        &self,
        id: ArtifactId,
        formation_contract: &CompilationContract,
        policy: &ResolvedPolicy,
        codec: CompressionCostModel,
        runtime: ArtifactRuntimeEvidence,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<QualifiedArtifact, CandidateError> {
        self.qualify_with(
            id,
            formation_contract,
            policy,
            codec,
            runtime,
            None,
            false,
            budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn qualify_with(
        &self,
        id: ArtifactId,
        formation_contract: &CompilationContract,
        policy: &ResolvedPolicy,
        codec: CompressionCostModel,
        runtime: ArtifactRuntimeEvidence,
        baseline: Option<&QualifiedArtifact>,
        exact: bool,
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
        record.admit_parse(budget)?;
        let size = if exact {
            record
                .sizes
                .measured(codec)
                .ok_or(CandidateError::Artifact(
                    "requested artifact codec is unmeasured",
                ))?
        } else {
            record.sizes.raw
        };
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
            exact,
            policy_fingerprint: policy.fingerprint(),
        })
    }

    pub(super) fn check_qualified(
        &self,
        qualified: &QualifiedArtifact,
    ) -> Result<(), CandidateError> {
        let record = self.get(qualified.artifact.0)?;
        let size = if qualified.exact {
            record.sizes.measured(qualified.codec)
        } else {
            Some(record.sizes.raw)
        };
        if record.identity.snapshot() != qualified.snapshot
            || record.identity.meaning() != qualified.meaning
            || size.and_then(|size| u64::try_from(size).ok()) != Some(qualified.cost.transfer_bytes)
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
        // The files of equal streams are equal too: their sizes carry over,
        // so manifest v3 never encodes the delivered files again.
        if !target.files.is_empty() {
            for slot in &self.slots {
                let Some(StoredRecord::JavaScript(donor)) = &slot.record else {
                    continue;
                };
                if donor.files.len() != target.files.len()
                    || !same_streams(target.view(), donor.view(), budget)?
                {
                    continue;
                }
                for (file, source) in target.files.iter().zip(&donor.files) {
                    for codec in objectives.iter() {
                        if codec == CompressionCostModel::Raw
                            || file.sizes.measured(codec).is_some()
                        {
                            continue;
                        }
                        if let Some(size) = source.sizes.measured(codec) {
                            file.sizes.publish(codec, size)?;
                        }
                    }
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
        if self.get(id.0)?.layout.is_some() {
            return Err(CandidateError::Artifact(
                "a multi-file artifact is delivered with all of its files",
            ));
        }
        let text = self.remove(id.0)?.take(self.owner, budget).0;
        self.release_idle_measurements(budget);
        Ok(text)
    }
    /// Every file of one multi-file artifact, with its layout.
    pub(super) fn take_files(
        &mut self,
        id: ArtifactId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<DeliveredFiles, CandidateError> {
        if self.get(id.0)?.layout.is_none() {
            return Err(CandidateError::Artifact(
                "a one-file artifact has no layout",
            ));
        }
        let (_, files, layout) = self.remove(id.0)?.take(self.owner, budget);
        self.release_idle_measurements(budget);
        Ok(DeliveredFiles {
            files,
            layout: layout.expect("checked above"),
        })
    }
    /// Compare temporary entry rows without returning unowned vector backing.
    pub(super) fn compare_rows<R>(
        &self,
        left: ArtifactId,
        right: ArtifactId,
        codec: CompressionCostModel,
        budget: &mut AllocationBudget<'_>,
        inspect: impl FnOnce(&[u64], &[u64]) -> R,
    ) -> Result<R, CandidateError> {
        budget.with_temporary(
            |budget| {
                let mut measurements = self.measurements.borrow_mut();
                let left = self.get(left.0)?.rows(
                    codec,
                    &self.settings,
                    &mut measurements,
                    self.owner,
                    budget,
                )?;
                let right = self.get(right.0)?.rows(
                    codec,
                    &self.settings,
                    &mut measurements,
                    self.owner,
                    budget,
                )?;
                Ok::<_, CandidateError>((left, right))
            },
            |rows, _| Ok(inspect(&rows.0, &rows.1)),
        )
    }
    pub(super) fn discard(
        &mut self,
        id: ArtifactId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), CandidateError> {
        self.remove(id.0)?.discard(self.owner, budget);
        self.release_idle_measurements(budget);
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
}
impl<'scope, 'target> BudgetedJavaScriptOutput<'scope, 'target> {
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
        retained.configure_measurements(policy.cache());
        Self {
            output,
            staging: ArtifactArena::with_settings(retained.owner, retained.settings),
            retained,
            candidate,
            identity,
            structural,
            policy,
            execution,
            choices,
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
        let (text, charge, literals, files, layout) = if self.output.has_delivery_plan() {
            let (files, layout, charge, literals) =
                self.output.render_plan_with_literals_admitted(
                    plan,
                    literals,
                    byte_limit,
                    self.staging.owner,
                    |name, code| ArtifactFile {
                        sizes: CachedSizes::new(code.len()),
                        name,
                        code,
                    },
                )?;
            (String::new(), charge, literals, files, Some(layout))
        } else {
            let (text, charge, literals) = self.output.render_with_literals_admitted(
                plan,
                literals,
                byte_limit,
                self.staging.owner,
            )?;
            (text, charge, literals, Vec::new(), None)
        };
        // The artifact reservation is already detached. Failed row admission
        // must destroy the complete bundle before returning that reservation.
        let raw = self
            .output
            .with_allocation_budget(|budget| -> Result<usize, CandidateError> {
                let Some(layout) = &layout else {
                    return Ok(text.len());
                };
                let mut phase = budget.scope();
                let mut sizes = phase.vector(AllocationClass::Scratch, files.len())?;
                sizes.extend(files.iter().map(|file| file.code.len()));
                let rows = layout.rows_in(&sizes, AllocationClass::Scratch, &mut phase)?;
                let total = rows.iter().try_fold(0u64, |sum, row| {
                    sum.checked_add(*row).ok_or(AllocationError::Capacity)
                })?;
                Ok(usize::try_from(total).map_err(|_| AllocationError::Capacity)?)
            });
        let raw = match raw {
            Ok(raw) => raw,
            Err(error) => {
                drop((text, files, layout));
                self.output
                    .with_allocation_budget(|budget| release(charge, self.staging.owner, budget));
                return Err(error);
            }
        };
        let actual_output = OutputTactics {
            literals,
            ..self.choices.clone()
        };
        let retained = match self.output.with_allocation_budget(|budget| {
            let provenance = ArtifactProvenance::build(
                self.structural.iter().chain(self.identity.tactics()),
                actual_output.clone(),
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
                drop((text, files, layout));
                self.output
                    .with_allocation_budget(|budget| release(charge, self.staging.owner, budget));
                return Err(error);
            }
        };
        let (provenance, identity) = retained;
        let sizes = CachedSizes::new(raw);
        // The printed tree's structural digest, for a file printed whole
        // (plan task M2.5); a delivery plan's files are parsed without one.
        let structure = match layout {
            Some(_) => None,
            None => match self.output.structure_digest() {
                Ok(structure) => Some(structure),
                Err(error) => {
                    drop((text, files, layout));
                    self.output.with_allocation_budget(|budget| {
                        discard_provenance(provenance, self.staging.owner, budget);
                        discard_identity(identity, self.staging.owner, budget);
                        release(charge, self.staging.owner, budget);
                    });
                    return Err(error.into());
                }
            },
        };
        Ok(ScopedArtifactId(self.staging.insert(Record {
            text,
            files,
            layout,
            execution: self.execution,
            candidate: self.candidate,
            identity,
            provenance,
            output: actual_output,
            sizes,
            structure,
            parsed: std::sync::OnceLock::new(),
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
        self.output.with_allocation_budget(|budget| {
            let settings = self.staging.settings;
            // A callback-local measurement must not allocate persistent
            // compilation storage. Retained search artifacts use their own
            // arena's memo through ArtifactArena::measure.
            self.staging.get(id.0)?.measure(
                codec,
                &settings,
                &mut self.staging.measurements.borrow_mut(),
                self.staging.owner,
                budget,
            )
        })
    }
    pub fn retain_artifact(&mut self, id: ScopedArtifactId) -> Result<ArtifactId, CandidateError> {
        self.staging.index(id.0)?;
        self.output
            .with_allocation_budget(|budget| self.retained.prepare_insert(budget))?;
        Ok(ArtifactId(self.retained.insert(self.staging.remove(id.0)?)))
    }
    pub fn take_artifact(&mut self, id: ScopedArtifactId) -> Result<String, CandidateError> {
        if self.staging.get(id.0)?.layout.is_some() {
            return Err(CandidateError::Artifact(
                "a multi-file artifact is delivered with all of its files",
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
