//! Compilation-owned semantic checkpoints and admitted checked-source edits.
//!
//! Source checkpoints allocate no JavaScript choices. Physical candidates share
//! one immutable semantic snapshot and retain only their implementation map.
//! Source patches change meaning; only private checked rules preserve meaning.
//! Prepared frontend input transfers its existing graph reservations. Inspection
//! adoption accounts already constructed input without retroactive admission of
//! construction or externally retained source/type storage.

pub use super::artifact_provenance::{ArtifactProvenanceDescription, LiteralOutput, OutputTactics};
pub use super::artifacts::{DeliveredBundle, DeliveredChunk, EntryLinks};
use super::artifacts::ArtifactArena;
pub use super::artifacts::{
    ArtifactId, ArtifactRuntimeEvidence, ArtifactView,
    BudgetedJavaScriptOutput, NativeArtifactView, QualifiedArtifact, QualifiedNativeArtifact,
    ScopedArtifactId,
};
use super::facts::{
    CacheLimits, FactRequest, FactsError, FactsSession, FactsSessionWork, RetainedFactsCache,
    UnitFacts, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};
use super::implementation_identity::SharedImplementationIdentity;
pub use super::implementation_identity::ImplementationDescription;
use super::implementations::{ImplementationError, ImplementationMap};
pub use super::native::{BudgetedNativeOutput, NativeError, NativeHostBinding, NativeHostBindings};
use super::rewrite_lineage::RewriteLineage;
pub use super::rewrite_lineage::{
    CheckedRewriteStep, DeadValueDrop, LiteralIntFold, NeutralConstant, RewriteDescription,
    RewriteRule, DEAD_VALUE_DROP_VERSION, LITERAL_INT_FOLD_VERSION,
};
use super::uses::{UseError, UseIndex, UseIndexReceipt};
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisWorkReceipt, BudgetError, BudgetLedger, CompilationContract,
    ResolvedPolicy, RuntimeRisk, TacticId, WorkDomain, WorkKind,
};
use crate::output_budget::{AllocationBudget, AllocationError};
use std::mem::size_of;
use std::sync::Arc;

#[path = "publication_edits.rs"]
mod edits;
pub(super) use edits::FixedUnitEdits;
#[path = "publication_rewrites.rs"]
mod rewrites;
pub use rewrites::RewriteError;
#[path = "search.rs"]
mod search;
pub use search::{
    JavaScriptSearch, SearchCounters, SearchError, SearchLimit, SearchObservation, SearchRequest,
};

#[derive(Debug, Clone, Copy)]
pub struct CheckpointLimit {
    pub max_live: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticId {
    store: RevisionId,
    slot: u32,
    generation: RevisionId,
}

/// An immutable JavaScript implementation choice over a semantic checkpoint.
/// Construction is private; a source-only checkpoint cannot become a candidate
/// merely by changing the type of its identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CandidateId(SemanticId);
impl CandidateId {
    pub fn semantic_id(self) -> SemanticId {
        self.0
    }
}

pub use super::function_layout::{
    ResourceLimit as FunctionLimit, UnknownReason as FunctionUnknownReason,
};
pub use super::helper_family::{
    ResourceLimit as HelperLimit, UnknownReason as HelperUnknownReason,
};
pub type FunctionRequest = ScalarRequest;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionOutcome {
    Published(CandidateId),
    Unknown(FunctionUnknownReason),
    Truncated(FunctionLimit),
}
#[derive(Debug, Clone, Copy)]
pub struct FunctionPublication {
    pub outcome: FunctionOutcome,
    pub receipt: AnalysisWorkReceipt,
}
pub use super::product_family::{
    ResourceLimit as ProductLimit, UnknownReason as ProductUnknownReason,
};
pub use super::record_family::{
    ResourceLimit as ScalarLimit, UnknownReason as ScalarUnknownReason,
};
/// Product and record proofs use the same bounded scalar request contract.
pub type ProductRequest = ScalarRequest;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductOutcome {
    Published(CandidateId),
    Unknown(ProductUnknownReason),
    Truncated(ProductLimit),
}
#[derive(Debug, Clone, Copy)]
pub struct ProductPublication {
    pub outcome: ProductOutcome,
    pub receipt: AnalysisWorkReceipt,
}

pub use super::string_family::{
    ResourceLimit as StringLimit, StringChoice, UnknownReason as StringUnknownReason, ValueRef,
};

#[derive(Debug, Clone, Copy)]
pub struct ScalarRequest {
    pub max_work: u64,
    pub scratch_bytes: u64,
    pub output_bytes: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarOutcome {
    Published(CandidateId),
    Unknown(ScalarUnknownReason),
    Truncated(ScalarLimit),
}
#[derive(Debug, Clone, Copy)]
pub struct ScalarPublication {
    pub outcome: ScalarOutcome,
    pub receipt: AnalysisWorkReceipt,
}

#[derive(Debug, Clone, Copy)]
pub struct HelperRequest {
    pub max_work: u64,
    pub scratch_bytes: u64,
    pub output_bytes: u64,
    /// Independent qualification of the common local-facts prerequisite.
    /// Cache occupancy never chooses this attempt's strength or result cap.
    pub local_facts: LocalFactsRequest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperOutcome {
    Published(CandidateId),
    Unknown(HelperUnknownReason),
    Truncated(HelperLimit),
}
#[derive(Debug, Clone, Copy)]
pub struct HelperPublication {
    pub outcome: HelperOutcome,
    /// The helper proof's own qualified work, excluding prerequisite attempts.
    pub receipt: AnalysisWorkReceipt,
    pub prerequisite_attempts: usize,
    pub prerequisite_work: u64,
    pub local_facts_receipt: Option<AnalysisWorkReceipt>,
}

#[derive(Debug, Clone, Copy)]
pub struct StringRequest {
    pub max_work: u64,
    pub scratch_bytes: u64,
    pub output_bytes: u64,
    /// The same explicit prerequisite qualification is used cold and warm.
    pub local_facts: LocalFactsRequest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringOutcome {
    Published(CandidateId),
    Unknown(StringUnknownReason),
    Truncated(StringLimit),
}
#[derive(Debug, Clone, Copy)]
pub struct StringPublication {
    pub outcome: StringOutcome,
    /// Actual family work, excluding the separately qualified facts query.
    pub receipt: AnalysisWorkReceipt,
    pub local_facts_receipt: Option<AnalysisWorkReceipt>,
}
#[derive(Debug)]
pub enum CandidateError {
    Admission(crate::compilation_policy::AdmissionError),
    Publication(PublicationError),
    Budget(BudgetError),
    NotJavaScript,
    ContractMismatch,
    UnknownCandidate,
    ForbiddenTactic(TacticId),
    UnsupportedRisk,
    StaleEvidence,
    DuplicateRoot,
    ConflictingChoice,
    InvalidRequest,
    InvalidSemanticInput(&'static str),
    Capacity,
    AllocationFailed,
    Unsupported(Unsupported),
    Output(crate::js::extract::OutputError),
    Artifact(&'static str),
    Codec(&'static str),
    LocalFacts(CompilationFactsError),
}
impl From<AllocationError> for CandidateError {
    fn from(error: AllocationError) -> Self {
        match error {
            AllocationError::Budget(error) => Self::Budget(error),
            AllocationError::Capacity => Self::Capacity,
            AllocationError::AllocationFailed => Self::AllocationFailed,
            AllocationError::Unaccounted | AllocationError::WrongOwner => Self::InvalidRequest,
        }
    }
}
impl From<crate::js::extract::OutputError> for CandidateError {
    fn from(error: crate::js::extract::OutputError) -> Self {
        match error {
            crate::js::extract::OutputError::Admission(error) => error.into(),
            error => Self::Output(error),
        }
    }
}
impl From<crate::compression::CodecError> for CandidateError {
    fn from(error: crate::compression::CodecError) -> Self {
        match error {
            crate::compression::CodecError::Admission(error) => error.into(),
            crate::compression::CodecError::Capacity => Self::Capacity,
            crate::compression::CodecError::Version(reason)
            | crate::compression::CodecError::Encoder(reason) => Self::Codec(reason),
        }
    }
}
impl From<PublicationError> for CandidateError {
    fn from(error: PublicationError) -> Self {
        Self::Publication(error)
    }
}
impl From<BudgetError> for CandidateError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
impl From<CompilationFactsError> for CandidateError {
    fn from(error: CompilationFactsError) -> Self {
        Self::LocalFacts(error)
    }
}
impl From<ImplementationError> for CandidateError {
    fn from(error: ImplementationError) -> Self {
        match error {
            ImplementationError::Budget(error) => Self::Budget(error),
            ImplementationError::DuplicateRoot => Self::DuplicateRoot,
            ImplementationError::ConflictingChoice => Self::ConflictingChoice,
            ImplementationError::Capacity => Self::Capacity,
            ImplementationError::AllocationFailed => Self::AllocationFailed,
        }
    }
}
impl From<super::record_family::FamilyError> for CandidateError {
    fn from(error: super::record_family::FamilyError) -> Self {
        use super::record_family::FamilyError;
        match error {
            FamilyError::Budget(error) => Self::Budget(error),
            FamilyError::InvalidAttempt => Self::InvalidRequest,
            FamilyError::InvalidProgram(reason) => Self::InvalidSemanticInput(reason),
            FamilyError::StaleUseIndex => Self::StaleEvidence,
            FamilyError::Capacity => Self::Capacity,
            FamilyError::AllocationFailed => Self::AllocationFailed,
        }
    }
}
impl From<super::product_family::FamilyError> for CandidateError {
    fn from(error: super::product_family::FamilyError) -> Self {
        use super::product_family::FamilyError;
        match error {
            FamilyError::Budget(error) => Self::Budget(error),
            FamilyError::InvalidAttempt => Self::InvalidRequest,
            FamilyError::InvalidProgram(reason) => Self::InvalidSemanticInput(reason),
            FamilyError::StaleUseIndex => Self::StaleEvidence,
            FamilyError::Capacity => Self::Capacity,
            FamilyError::AllocationFailed => Self::AllocationFailed,
        }
    }
}
impl From<super::helper_family::FamilyError> for CandidateError {
    fn from(error: super::helper_family::FamilyError) -> Self {
        use super::helper_family::FamilyError;
        match error {
            FamilyError::InvalidAttempt | FamilyError::AlreadyChecked => Self::InvalidRequest,
            FamilyError::InvalidProgram(reason) => Self::InvalidSemanticInput(reason),
            FamilyError::StaleUseIndex => Self::StaleEvidence,
            FamilyError::Capacity => Self::Capacity,
            FamilyError::AllocationFailed => Self::AllocationFailed,
            FamilyError::Budget(error) => Self::Budget(error),
            FamilyError::Record(error) => error.into(),
            FamilyError::Product(error) => error.into(),
        }
    }
}
impl From<super::string_family::FamilyError> for CandidateError {
    fn from(error: super::string_family::FamilyError) -> Self {
        use super::string_family::FamilyError;
        match error {
            FamilyError::InvalidAttempt | FamilyError::AlreadyChecked => Self::InvalidRequest,
            FamilyError::InvalidProgram(reason) => Self::InvalidSemanticInput(reason),
            FamilyError::Capacity => Self::Capacity,
            FamilyError::AllocationFailed => Self::AllocationFailed,
            FamilyError::Budget(error) => Self::Budget(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalFactsRequest {
    pub work_quota: u64,
    pub result_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationFactsError {
    AlreadyEnabled,
    NotEnabled,
    Publication(PublicationError),
    Facts(FactsError),
}
impl From<PublicationError> for CompilationFactsError {
    fn from(error: PublicationError) -> Self {
        Self::Publication(error)
    }
}
impl From<FactsError> for CompilationFactsError {
    fn from(error: FactsError) -> Self {
        Self::Facts(error)
    }
}
impl From<BudgetError> for CompilationFactsError {
    fn from(error: BudgetError) -> Self {
        Self::Facts(FactsError::Budget(error))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalFactsCacheStatus {
    pub entries: usize,
    pub reserved_bytes: u64,
    pub retained_bytes: u64,
}

/// One scoped logical analysis group over this compilation's immutable
/// checkpoints. Cache, ledger, Program and snapshot owners remain private.
pub struct CompilationFacts<'scope, 'src> {
    store: RevisionId,
    slots: &'scope [Slot<'src>],
    session: FactsSession<'scope>,
}
impl<'src> CompilationFacts<'_, 'src> {
    pub fn query(
        &mut self,
        snapshot: SemanticId,
        unit: UnitId,
        request: LocalFactsRequest,
    ) -> Result<LocalFactsView<'_, 'src>, CompilationFactsError> {
        // Wrong-store/stale IDs fail before analysis, cache lookup or query
        // counters. Invalid caller loops are not hidden analysis computation.
        let slot = lookup_slot(self.store, self.slots, snapshot)?;
        let program = &self.slots[slot]
            .checkpoint
            .as_ref()
            .unwrap()
            .semantic
            .program;
        let result = self.session.query(
            program,
            unit,
            FactRequest {
                attempt: AnalysisAttempt {
                    plan: LOCAL_FACTS_PLAN,
                    algorithm_version: LOCAL_FACTS_VERSION,
                    work_quota: request.work_quota,
                },
                result_bytes: request.result_bytes,
            },
        )?;
        Ok(LocalFactsView {
            program,
            facts: result.facts,
            receipt: result.receipt,
            cache_hit: result.cache_hit,
        })
    }

    pub fn work(&self) -> FactsSessionWork {
        self.session.work()
    }
}

/// A result borrows its live query and the matched source snapshot. A second
/// mutable query cannot evict it while borrowed, and it cannot escape the
/// compilation callback. Deliberate copied data belongs to its consumer.
///
/// A query view cannot leave the callback that owns its cache session:
///
/// ```compile_fail
/// use lilscript::compilation_policy::WorkDomain;
/// use lilscript::program::{UnitId, publication::{
///     Compilation, LocalFactsRequest, SemanticId,
/// }};
///
/// fn cannot_escape<'src>(compilation: &mut Compilation<'src>,
///     checkpoint: SemanticId, unit: UnitId, request: LocalFactsRequest)
/// {
///     let escaped = compilation.with_local_facts(WorkDomain::Baseline, 1, |group| {
///         group.query(checkpoint, unit, request).unwrap()
///     }).unwrap();
///     let _ = escaped.receipt();
/// }
/// ```
///
/// Another query cannot evict a computed string while a consumer borrows it:
///
/// ```compile_fail,E0499
/// use lilscript::compilation_policy::WorkDomain;
/// use lilscript::program::{UnitId, ValueId, publication::{
///     Compilation, LocalFactsRequest, SemanticId,
/// }};
///
/// fn cannot_evict<'src>(compilation: &mut Compilation<'src>,
///     checkpoint: SemanticId, unit: UnitId, value: ValueId, request: LocalFactsRequest)
/// {
///     compilation.with_local_facts(WorkDomain::Baseline, 2, |group| {
///         let first = group.query(checkpoint, unit, request).unwrap();
///         let text = first.string(value).unwrap();
///         let _second = group.query(checkpoint, unit, request).unwrap();
///         let _ = std::mem::size_of_val(text);
///     }).unwrap();
/// }
/// ```
pub struct LocalFactsView<'query, 'src> {
    program: &'query Program<'src>,
    facts: &'query UnitFacts,
    receipt: AnalysisWorkReceipt,
    cache_hit: bool,
}
impl LocalFactsView<'_, '_> {
    pub fn facts(&self) -> &UnitFacts {
        self.facts
    }
    pub fn string(&self, value: ValueId) -> Option<&StringValue> {
        self.facts.string(self.program, value)
    }
    pub fn string_construction(
        &self,
        operation: OpId,
    ) -> super::facts::StringConstructionKnowledge {
        self.facts.string_construction(self.program, operation)
    }
    pub fn receipt(&self) -> AnalysisWorkReceipt {
        self.receipt
    }
    pub fn cache_hit(&self) -> bool {
        self.cache_hit
    }
}

pub struct OperationPatch<'a> {
    pub operation: OpId,
    pub kind: &'a OperationKind,
    pub operands: &'a [ValueId],
}
pub struct PlacePatch<'a> {
    pub place: PlaceId,
    pub replacement: &'a Place,
}
pub struct UnitPatch<'a> {
    pub unit: UnitId,
    pub expected_revision: RevisionId,
    /// Strict operation/place order makes admission and rebuilding linear.
    pub operations: &'a [OperationPatch<'a>],
    pub places: &'a [PlacePatch<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitChange {
    pub unit: UnitId,
    pub previous: RevisionId,
    pub current: RevisionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellUseChange {
    pub cell: CellId,
    pub previous: RevisionId,
    pub current: RevisionId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PublicationReceipt {
    pub logical_work: u64,
    /// New retained payload; shared unit/table buffers are not charged again.
    /// Nested shared callable signatures use a conservative table payload bound.
    pub allocated_bytes: u64,
    /// Changed unit allocations acquired by this transaction. Shared handles,
    /// index chunks and temporary undo buffers are reported separately.
    pub copied_units: usize,
    pub reused_units: usize,
    pub copied_payload_bytes: u64,
    pub verified_units: usize,
    pub verified_operations: usize,
    pub index: UseIndexReceipt,
    pub adopted_after_frontend: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationError {
    Budget(BudgetError),
    Uses(UseError),
    InvalidLimit,
    Capacity,
    AllocationFailed,
    StoreFull,
    UnknownCheckpoint,
    SharedInput,
    StaleRevision(UnitId),
    InvalidPatch,
    InvalidReplacement,
    InvalidParameterContract(&'static str),
    InvalidNominalContract(&'static str),
}
impl From<BudgetError> for PublicationError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
impl From<UseError> for PublicationError {
    fn from(error: UseError) -> Self {
        Self::Uses(error)
    }
}
impl From<AllocationError> for PublicationError {
    fn from(error: AllocationError) -> Self {
        match error {
            AllocationError::Budget(error) => Self::Budget(error),
            AllocationError::Capacity => Self::Capacity,
            AllocationError::AllocationFailed => Self::AllocationFailed,
            AllocationError::Unaccounted | AllocationError::WrongOwner => Self::InvalidPatch,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Charge {
    domain: WorkDomain,
    bytes: u64,
}
impl Charge {
    fn empty(domain: WorkDomain) -> Self {
        Self { domain, bytes: 0 }
    }
    fn reserve(
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
        bytes: u64,
    ) -> Result<Self, PublicationError> {
        ledger.retain(domain, bytes)?;
        Ok(Self { domain, bytes })
    }
    fn release(self, ledger: &mut BudgetLedger) -> Result<(), PublicationError> {
        ledger.release(self.domain, self.bytes)?;
        Ok(())
    }
}

struct SemanticCharges {
    shell: Charge,
    units: Vec<Charge>,
    tables: [Charge; 10],
    // Borrowed source names are not owned table payload, but a rejected typed
    // edit may format them into a transient diagnostic String.
    diagnostic_text_bytes: u64,
}

/// Private frontend-to-publication transfer on the same factory ledger. The
/// existing Pending owns the only graph; abandoning the factory drops it even
/// when no ledger remains available for an explicit release receipt.
pub(crate) struct PreparedProgram<'src> {
    pending: Pending<'src>,
}

impl<'src> PreparedProgram<'src> {
    pub(crate) fn new(
        program: Program<'src>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, PublicationError> {
        let domain = budget
            .with_ledger(|ledger| ledger.map(|(_, domain)| domain))
            .ok_or(AllocationError::Unaccounted)?;
        let mut units = budget.vector(
            crate::output_budget::AllocationClass::Retained,
            program.units.len(),
        )?;
        for unit in &program.units {
            budget.work(
                WorkKind::Analysis,
                u64::try_from(
                    unit.data()
                        .operations
                        .len()
                        .checked_add(unit.data().regions.len())
                        .and_then(|n| n.checked_add(unit.data().call_instantiations.len()))
                        .ok_or(AllocationError::Capacity)?,
                )
                .map_err(|_| AllocationError::Capacity)?,
            )?;
            let bytes = unit.allocation_bytes().ok_or(PublicationError::Capacity)?;
            budget.push(
                crate::output_budget::AllocationClass::Retained,
                &mut units,
                Charge { domain, bytes },
            )?;
        }
        let mut tables = [Charge::empty(domain); 10];
        let mut diagnostic_text_bytes = 0;
        for (index, charge) in tables.iter_mut().enumerate() {
            let (bytes, text) = budget.with_ledger(|ledger| {
                let (ledger, domain) = ledger.expect("prepared conversion owns a ledger");
                table_bytes(&program, index, ledger, domain)
            })?;
            charge.bytes = bytes;
            diagnostic_text_bytes = sum(&[diagnostic_text_bytes, text])?;
        }
        let shell = Charge {
            domain,
            bytes: sum(&[capacity(&program.units)?, capacity(&units)?])?,
        };
        let total = sum(&[shell.bytes, sum_charges(&units)?, sum_charges(&tables)?])?;
        // Construction admitted every allocation before it happened. Exact
        // owner partitioning cannot conceal missing or surplus reservations.
        if budget.retained_bytes(crate::output_budget::AllocationClass::Retained) != total {
            return Err(AllocationError::Unaccounted.into());
        }
        let charge = budget.detach_retained((), total)?;
        let (transferred_domain, transferred_bytes) = charge
            .into_parts(&())
            .unwrap_or_else(|_| unreachable!("private same-factory charge"));
        debug_assert_eq!(transferred_domain, domain);
        debug_assert_eq!(transferred_bytes, total);
        Ok(Self {
            pending: Pending {
                program,
                charges: SemanticCharges {
                    shell,
                    units,
                    tables,
                    diagnostic_text_bytes,
                },
                changes: Vec::new(),
                cell_changes: Vec::new(),
                uses: None,
            },
        })
    }

    pub(crate) fn program(&self) -> &Program<'src> {
        &self.pending.program
    }

    pub(crate) fn discard(self, ledger: &mut BudgetLedger) {
        self.pending
            .discard(ledger)
            .expect("prepared payload owns same-factory reservations");
    }
}
struct BoundJavaScript {
    contract: CompilationContract,
    /// Inline contract storage belongs to Compilation. Only the preserved-name
    /// vector and strings allocate when the first target is published.
    charge: Charge,
}
impl BoundJavaScript {
    fn language(&self) -> &crate::compilation_contract::JavaScriptCompilationContract {
        let CompilationContract::JavaScript { language, .. } = &self.contract else {
            unreachable!("only JavaScript contracts enter the target binding")
        };
        language
    }
    fn discard(self, ledger: &mut BudgetLedger) -> Result<(), PublicationError> {
        let Self { contract, charge } = self;
        drop(contract);
        charge.release(ledger)
    }
}
struct SemanticSnapshot<'src> {
    /// Immutable source identity; target siblings share it, committed edits do not.
    identity: RevisionId,
    meaning: RevisionId,
    lineage: RewriteLineage,
    program: Program<'src>,
    uses: UseIndex,
    charges: SemanticCharges,
}
struct SnapshotOrigin {
    identity: RevisionId,
    meaning: RevisionId,
    lineage: RewriteLineage,
}
impl SnapshotOrigin {
    fn source() -> Self {
        Self {
            identity: RevisionId::fresh(),
            meaning: RevisionId::fresh(),
            lineage: RewriteLineage::default(),
        }
    }
}
struct Checkpoint<'src> {
    semantic: Arc<SemanticSnapshot<'src>>,
    charge: Charge,
    implementations: Option<ImplementationMap>,
    identity: Option<SharedImplementationIdentity>,
    changes: Vec<UnitChange>,
    cell_changes: Vec<CellUseChange>,
    receipt: PublicationReceipt,
}

/// One transient formation, borrowed exclusively through its naming phases.
/// Target buffers drop before their fixed-domain reservation. Each output owns
/// a separate budget so Optional naming cannot spend the Baseline reserve.
struct JavaScriptTarget<'scope, 'src> {
    /// Source module stems for delivered chunk names.
    module_names: &'scope [String],
    chunk_extension: &'static str,
    hosts: Option<&'scope crate::host_modules::HostDelivery>,
    module: crate::js::Module,
    literals: Vec<crate::js::LiteralAlternative>,
    #[cfg(test)]
    _test_lifetime: super::search_target_reuse_tests::TargetLifetime,
    checkpoint: &'scope mut Checkpoint<'src>,
    artifacts: &'scope mut ArtifactArena,
    store: RevisionId,
    candidate: CandidateId,
    policy: &'scope ResolvedPolicy,
    choices: OutputTactics,
    budget: AllocationBudget<'scope>,
}

impl JavaScriptTarget<'_, '_> {
    fn with_output_in<R>(
        &mut self,
        domain: WorkDomain,
        inspect: impl FnOnce(&mut BudgetedJavaScriptOutput<'_, '_>) -> R,
    ) -> Result<R, CandidateError> {
        let Self {
            module,
            literals,
            checkpoint,
            artifacts,
            store,
            candidate,
            policy,
            choices,
            budget,
            module_names,
            chunk_extension,
            hosts,
            ..
        } = self;
        // Delivered host code runs inside this module's scope; its globals
        // must stay visible there. A script output runs it strict, as the
        // module it was written as.
        let hosts = hosts.filter(|hosts| {
            module.imports.iter().any(|import| {
                import
                    .source
                    .as_unicode()
                    .is_some_and(|source| hosts.position(source).is_some())
            })
        });
        if let Some(hosts) = hosts {
            if module.carried.is_empty() {
                module.reserved = hosts.reserved.clone();
                module.carried = hosts
                    .modules
                    .iter()
                    .map(|host| host.specifier.clone())
                    .collect();
            }
        }
        let strict = policy.javascript_contract().is_some_and(|contract| {
            contract.execution != crate::compilation_contract::JavaScriptExecution::Module
        });
        // Multi-file delivery: every source module but the entry may carry a
        // chunk of its self-contained functions; split mode then selects.
        let bundle = match policy.contract() {
            crate::compilation_policy::CompilationContract::JavaScript {
                language,
                bundle_mode,
                split,
                preload,
                ..
            } if *bundle_mode != crate::config::BundleMode::Single => {
                let program = &checkpoint.semantic.program;
                let modules = program.modules();
                let entry = program.entry_module().index();
                // Importing modules per module, counted once per importer.
                let mut importers = vec![0usize; modules.len()];
                for module in modules {
                    let mut unique = module.dependencies.clone();
                    unique.sort_unstable();
                    unique.dedup();
                    for dependency in unique {
                        importers[dependency.index()] += 1;
                    }
                }
                // Modules static imports reach from the entry.
                let mut eager = vec![false; modules.len()];
                let mut pending = vec![entry];
                while let Some(module) = pending.pop() {
                    if !std::mem::replace(&mut eager[module], true) {
                        pending.extend(modules[module].dependencies.iter().map(|id| id.index()));
                    }
                }
                Some(super::artifacts::BundleSpec {
                    hosted: module
                        .imports
                        .iter()
                        .map(|import| {
                            hosts.is_some_and(|hosts| {
                                import
                                    .source
                                    .as_unicode()
                                    .is_some_and(|source| hosts.position(source).is_some())
                            })
                        })
                        .collect(),
                    extension: chunk_extension,
                    eager,
                    dynamic_import: language
                        .ecmascript
                        .allows(crate::js_syntax_target::JsSyntaxFeature::DynamicImport),
                    preload: *preload,
                    allowed: (0..modules.len()).map(|index| index != entry).collect(),
                    stems: (0..modules.len())
                        .map(|index| {
                            module_names
                                .get(index)
                                .cloned()
                                .unwrap_or_else(|| format!("m{index}"))
                        })
                        .collect(),
                    importers,
                    split: *split,
                })
            }
            _ => None,
        };
        budget.with_ledger(|ledger| {
            let mut phase = AllocationBudget::new(ledger.map(|(ledger, _)| (ledger, domain)));
            let mut output =
                module.prepare_output_with_literals_admitted(policy, literals, &mut phase)?;
            output.set_hosts(hosts.map(|hosts| (hosts, strict)));
            #[cfg(test)]
            let output = super::search_target_reuse_tests::AdmittedOutputOwner::new(output);
            output.with_allocation_budget(|budget| budget.work(WorkKind::Render, 0))?;
            let map = checkpoint.implementations.as_ref().unwrap();
            // Failed preparation must not install a persistent descriptor cache.
            output.with_allocation_budget(|budget| {
                SharedImplementationIdentity::ensure(
                    &mut checkpoint.identity,
                    Some(map),
                    checkpoint.semantic.identity,
                    checkpoint.semantic.meaning,
                    &checkpoint.semantic.lineage,
                    *store,
                    budget,
                )
            })?;
            let mut facade = BudgetedJavaScriptOutput::new(
                &output,
                artifacts,
                *candidate,
                checkpoint.identity.as_ref().unwrap(),
                map.tactics(),
                policy,
                policy.javascript_contract().unwrap().execution,
                *choices,
            )
            .with_bundle(bundle.as_ref());
            Ok(inspect(&mut facade))
        })
    }
}

struct Slot<'src> {
    generation: Option<RevisionId>,
    checkpoint: Option<Box<Checkpoint<'src>>>,
    next_free: Option<u32>,
}

/// Borrowed semantic payloads cannot clone published Arc owners. In particular
/// this view does not expose Program, FrozenUnit or Arc-backed Type objects.
pub struct SemanticView<'a, 'src> {
    checkpoint: &'a Checkpoint<'src>,
}
impl SemanticView<'_, '_> {
    pub fn snapshot_identity(&self) -> RevisionId {
        self.checkpoint.semantic.identity
    }
    pub fn meaning_identity(&self) -> RevisionId {
        self.checkpoint.semantic.meaning
    }
    pub fn rewrites(&self) -> RewriteDescription<'_> {
        self.checkpoint.semantic.lineage.description()
    }
    pub fn unit(&self, id: UnitId) -> Option<&UnitData> {
        self.checkpoint.semantic.program.unit(id)
    }
    pub fn unit_revision(&self, id: UnitId) -> Option<RevisionId> {
        self.checkpoint
            .semantic
            .program
            .units
            .get(id.index())
            .map(FrozenUnit::revision)
    }
    pub fn unit_count(&self) -> usize {
        self.checkpoint.semantic.program.units.len()
    }
    pub fn cell_count(&self) -> usize {
        self.checkpoint.semantic.program.cells.len()
    }
    pub fn cell(&self, id: CellId) -> Option<&Cell> {
        self.checkpoint.semantic.program.cells.get(id.index())
    }
    pub fn unit_uses(&self, id: UnitId) -> Option<&super::uses::UnitUses> {
        self.checkpoint.semantic.uses.unit(id)
    }
    pub fn cell_users(&self, id: CellId) -> Option<&super::uses::CellUsers> {
        self.checkpoint.semantic.uses.cell(id)
    }
    pub fn tables_revision(&self) -> RevisionId {
        self.checkpoint.semantic.program.tables_revision
    }
    pub fn changes(&self) -> &[UnitChange] {
        &self.checkpoint.changes
    }
    pub fn cell_changes(&self) -> &[CellUseChange] {
        &self.checkpoint.cell_changes
    }
    pub fn receipt(&self) -> PublicationReceipt {
        self.checkpoint.receipt
    }
}

pub struct Compilation<'src> {
    ledger: BudgetLedger,
    store: RevisionId,
    slots: Vec<Slot<'src>>,
    slots_charge: Charge,
    free: Option<u32>,
    live: usize,
    /// One immutable delivery contract per compilation. Objective/effort/tactic
    /// policy is separate and can vary without cloning or rebinding this value.
    javascript: Option<BoundJavaScript>,
    /// No cache payload or analysis is allocated until explicitly enabled.
    /// Capacity persists between callbacks and belongs to the same ledger.
    local_facts: Option<RetainedFactsCache>,
    artifacts: ArtifactArena,
    /// Source module file stems for delivered chunk names: descriptive only,
    /// never part of a program's meaning.
    module_names: Option<(Vec<String>, Charge)>,
    chunk_extension: &'static str,
    /// Relative host modules delivered with every output (008-D3).
    host_modules: Option<(crate::host_modules::HostDelivery, Charge)>,
}

impl<'src> Compilation<'src> {
    /// Name each source module, by index, for multi-file delivery.
    /// Delivered chunk file names end in `.extension`.
    pub fn set_chunk_extension(&mut self, extension: &'static str) {
        self.chunk_extension = extension;
    }

    /// Host modules every output carries instead of importing them.
    pub(crate) fn set_host_modules(
        &mut self,
        delivery: crate::host_modules::HostDelivery,
    ) -> Result<(), PublicationError> {
        let bytes = delivery.retained_bytes();
        let charge = Charge::reserve(&mut self.ledger, WorkDomain::Baseline, bytes)?;
        if let Some((_, previous)) = self.host_modules.replace((delivery, charge)) {
            previous.release(&mut self.ledger)?;
        }
        Ok(())
    }

    pub fn set_module_names(&mut self, names: Vec<String>) -> Result<(), PublicationError> {
        let bytes = names
            .iter()
            .map(|name| name.capacity() as u64)
            .sum::<u64>()
            .saturating_add((names.capacity() * std::mem::size_of::<String>()) as u64);
        let charge = Charge::reserve(&mut self.ledger, WorkDomain::Baseline, bytes)?;
        if let Some((_, previous)) = self.module_names.replace((names, charge)) {
            previous.release(&mut self.ledger)?;
        }
        Ok(())
    }

    pub fn new(ledger: BudgetLedger, limit: CheckpointLimit) -> Result<Self, PublicationError> {
        Self::new_preserving_ledger(ledger, limit).map_err(|(_, error)| error)
    }

    /// Frontend clients may already own admitted input when the store refuses
    /// construction. Return its unchanged owner so they can release that input.
    pub(crate) fn new_preserving_ledger(
        mut ledger: BudgetLedger,
        limit: CheckpointLimit,
    ) -> Result<Self, (BudgetLedger, PublicationError)> {
        let initialize = (|| -> Result<_, PublicationError> {
            if limit.max_live == 0 || limit.max_live > u32::MAX as usize {
                return Err(PublicationError::InvalidLimit);
            }
            ledger.charge(WorkDomain::Baseline, WorkKind::Edit, limit.max_live as u64)?;
            let slots_charge = Charge::reserve(
                &mut ledger,
                WorkDomain::Baseline,
                sum(&[
                    size_of::<Self>() as u64,
                    bytes::<Slot<'src>>(limit.max_live)?,
                ])?,
            )?;
            let mut slots = Vec::with_capacity(limit.max_live);
            for index in 0..limit.max_live {
                slots.push(Slot {
                    generation: None,
                    checkpoint: None,
                    next_free: (index + 1 < limit.max_live).then_some((index + 1) as u32),
                });
            }
            Ok((slots, slots_charge))
        })();
        let (slots, slots_charge) = match initialize {
            Ok(parts) => parts,
            Err(error) => return Err((ledger, error)),
        };
        let store = RevisionId::fresh();
        Ok(Self {
            ledger,
            store,
            slots,
            slots_charge,
            free: Some(0),
            live: 0,
            javascript: None,
            local_facts: None,
            artifacts: ArtifactArena::new(store),
            module_names: None,
            chunk_extension: "js",
            host_modules: None,
        })
    }
    pub fn ledger(&self) -> &BudgetLedger {
        &self.ledger
    }
    pub fn checkpoint_count(&self) -> usize {
        self.live
    }
    pub fn view(&self, id: SemanticId) -> Result<SemanticView<'_, 'src>, PublicationError> {
        Ok(SemanticView {
            checkpoint: self.checkpoint(id)?,
        })
    }

    /// Admit persistent cache capacity once, before allocating it. This does
    /// not bind a JavaScript contract or run an analysis. Drivers derive finite
    /// limits from their compilation plan; resizing is an explicit discard and
    /// subsequent enable, never an implicit replacement of the current owner.
    pub fn enable_local_facts(
        &mut self,
        limits: CacheLimits,
        capacity_domain: WorkDomain,
    ) -> Result<(), CompilationFactsError> {
        if self.local_facts.is_some() {
            return Err(CompilationFactsError::AlreadyEnabled);
        }
        let cache = RetainedFactsCache::new(limits, &mut self.ledger, capacity_domain)?;
        self.local_facts = Some(cache);
        Ok(())
    }

    pub fn local_facts_status(&self) -> Option<LocalFactsCacheStatus> {
        self.local_facts
            .as_ref()
            .map(|cache| LocalFactsCacheStatus {
                entries: cache.retained_entries(),
                reserved_bytes: cache.reserved_bytes(),
                retained_bytes: cache.retained_bytes(),
            })
    }

    /// One callback owns one finite query group and its common session history.
    /// Identical qualified requests share logical billing within the group; a
    /// later group replays the receipt even when cached. Session Drop releases
    /// scratch on return, callback errors and unwinding; cache capacity remains
    /// reserved. The copied work counters distinguish physical computation from
    /// logical billing, without claiming complete dispatch or wall-time bounds.
    pub fn with_local_facts<R>(
        &mut self,
        domain: WorkDomain,
        max_queries: usize,
        inspect: impl FnOnce(&mut CompilationFacts<'_, 'src>) -> R,
    ) -> Result<R, CompilationFactsError> {
        let cache = self
            .local_facts
            .as_mut()
            .ok_or(CompilationFactsError::NotEnabled)?;
        let session = cache.session(&mut self.ledger, domain, max_queries)?;
        let mut group = CompilationFacts {
            store: self.store,
            slots: &self.slots,
            session,
        };
        Ok(inspect(&mut group))
    }

    /// Reclaim the original cache reservation, leaving checkpoints and target
    /// candidates intact. An absent cache returns NotEnabled; later enabling
    /// constructs a cold cache rather than reusing hidden retained entries.
    pub fn discard_local_facts(&mut self) -> Result<(), CompilationFactsError> {
        let cache = self
            .local_facts
            .take()
            .ok_or(CompilationFactsError::NotEnabled)?;
        cache.discard(&mut self.ledger)?;
        Ok(())
    }

    /// Transfer already admitted frontend payload. Only the new publication
    /// headers and reverse-use index acquire new retained memory here.
    pub(crate) fn adopt_prepared(
        &mut self,
        prepared: PreparedProgram<'src>,
    ) -> Result<SemanticId, PublicationError> {
        let mut pending = prepared.pending;
        let domain = pending.charges.shell.domain;
        let start = self.ledger.work_used(domain);
        let outcome = (|| {
            let slot = self.free.ok_or(PublicationError::StoreFull)?;
            work(
                &mut self.ledger,
                domain,
                pending
                    .program
                    .units
                    .len()
                    .checked_add(9)
                    .ok_or(PublicationError::Capacity)?,
            )?;
            if !unique_input(&pending.program) {
                return Err(PublicationError::SharedInput);
            }
            let headers = shell_bytes(0, 0, 0)?;
            let next_shell = sum(&[pending.charges.shell.bytes, headers])?;
            self.ledger.retain(domain, headers)?;
            pending.charges.shell.bytes = next_shell;
            pending.uses = Some(UseIndex::build(&pending.program, &mut self.ledger, domain)?);
            let receipt = PublicationReceipt {
                logical_work: self.ledger.work_used(domain) - start,
                allocated_bytes: sum(&[
                    headers,
                    pending.uses.as_ref().unwrap().receipt().allocated_bytes,
                ])?,
                index: pending.uses.as_ref().unwrap().receipt(),
                adopted_after_frontend: false,
                ..PublicationReceipt::default()
            };
            Ok((slot, receipt))
        })();
        match outcome {
            Ok((slot, receipt)) => {
                Ok(self.publish(slot, pending, receipt, SnapshotOrigin::source()))
            }
            Err(error) => {
                pending.discard(&mut self.ledger)?;
                Err(error)
            }
        }
    }

    /// A moved, previously checked input must have no externally retained unit
    /// or table Arc owners. Initial frontend allocations remain outside this
    /// operation's preconstruction guarantee; the receipt states that boundary.
    pub fn adopt_checked(
        &mut self,
        program: Program<'src>,
        domain: WorkDomain,
    ) -> Result<SemanticId, PublicationError> {
        let slot = self.free.ok_or(PublicationError::StoreFull)?;
        let start = self.ledger.work_used(domain);
        work(&mut self.ledger, domain, program.units.len() + 8)?;
        if !unique_input(&program) {
            return Err(PublicationError::SharedInput);
        }
        let shell = Charge::reserve(
            &mut self.ledger,
            domain,
            shell_bytes(program.units.capacity(), program.units.len(), 0)?,
        )?;
        let unit_count = program.units.len();
        let mut pending = Pending {
            program,
            charges: SemanticCharges {
                shell,
                units: Vec::with_capacity(unit_count),
                tables: [Charge::empty(domain); 10],
                diagnostic_text_bytes: 0,
            },
            changes: Vec::new(),
            cell_changes: Vec::new(),
            uses: None,
        };
        let outcome = (|| {
            for unit in &pending.program.units {
                work(
                    &mut self.ledger,
                    domain,
                    unit.data().operations.len()
                        + unit.data().regions.len()
                        + unit.data().call_instantiations.len(),
                )?;
                let bytes = unit.allocation_bytes().ok_or(PublicationError::Capacity)?;
                pending
                    .charges
                    .units
                    .push(Charge::reserve(&mut self.ledger, domain, bytes)?);
            }
            for index in 0..10 {
                let (bytes, diagnostics) =
                    table_bytes(&pending.program, index, &mut self.ledger, domain)?;
                pending.charges.tables[index] = Charge::reserve(&mut self.ledger, domain, bytes)?;
                pending.charges.diagnostic_text_bytes =
                    sum(&[pending.charges.diagnostic_text_bytes, diagnostics])?;
            }
            pending.uses = Some(UseIndex::build(&pending.program, &mut self.ledger, domain)?);
            Ok(())
        })();
        if let Err(error) = outcome {
            pending.discard(&mut self.ledger)?;
            return Err(error);
        }
        let receipt = PublicationReceipt {
            logical_work: self.ledger.work_used(domain) - start,
            allocated_bytes: sum(&[
                pending.charges.shell.bytes,
                sum_charges(&pending.charges.units)
                    .expect("unit payload was admitted by one ledger"),
                sum_charges(&pending.charges.tables)
                    .expect("table payload was admitted by one ledger"),
                pending.uses.as_ref().unwrap().receipt().allocated_bytes,
            ])
            .expect("new retained payload was admitted by one ledger"),
            index: pending.uses.as_ref().unwrap().receipt(),
            adopted_after_frontend: true,
            ..PublicationReceipt::default()
        };
        Ok(self.publish(slot, pending, receipt, SnapshotOrigin::source()))
    }

    /// Patches only existing operations/places. Unit kind, callable type/name,
    /// parameters, captures, tables, result IDs and schedules have no edit path.
    /// Changed operand arenas rebuild once, so edits retain no dead ranges.
    pub fn edit_source(
        &mut self,
        base: SemanticId,
        patches: &[UnitPatch<'_>],
        domain: WorkDomain,
    ) -> Result<SemanticId, PublicationError> {
        self.edit_transaction(base, patches, domain, false)
    }

    /// Replace a source checkpoint in its existing slot. Success invalidates
    /// the old handle; refusal or unwind preserves its exact checked state.
    /// Shared or differently charged allocations are copied once, while an
    /// exclusive allocation in this domain can be edited with bounded undo.
    /// Physical candidates keep their immutable snapshots and cannot be used
    /// as the source handle for this operation.
    pub fn advance_source(
        &mut self,
        base: SemanticId,
        patches: &[UnitPatch<'_>],
        domain: WorkDomain,
    ) -> Result<SemanticId, PublicationError> {
        self.edit_transaction(base, patches, domain, true)
    }

    /// Retain a direct implementation without rescanning or copying semantic
    /// arenas, table handles, or use-index headers. Permission here is not
    /// complete-artifact cost admission; scoring follows output construction.
    pub fn direct_javascript(
        &mut self,
        base: SemanticId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<CandidateId, CandidateError> {
        let base = self.lookup(base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        let provisional = self.prepare_javascript_contract(policy, domain)?;
        self.publish_candidate(
            base,
            ImplementationMap::direct(),
            policy,
            domain,
            start,
            provisional,
        )
    }

    /// Borrow the full canonical selected-recipe descriptor for diagnostics.
    /// Descriptor equality requires the same semantic snapshot. Construction
    /// uses this compilation's ledger once on first demand. The immutable
    /// backing then follows its candidate and completed artifacts.
    pub fn with_recipe_descriptor<R>(
        &mut self,
        candidate: CandidateId,
        domain: WorkDomain,
        inspect: impl FnOnce(&[u32]) -> R,
    ) -> Result<R, CandidateError> {
        let index = self.candidate_slot(candidate)?;
        if !self.slots[index]
            .checkpoint
            .as_ref()
            .unwrap()
            .semantic
            .lineage
            .description()
            .is_empty()
        {
            return Err(CandidateError::Artifact(
                "complete implementation description is required for rewrites",
            ));
        }
        self.ensure_implementation_identity(index, domain)?;
        let identity = self.slots[index]
            .checkpoint
            .as_ref()
            .unwrap()
            .identity
            .as_ref()
            .unwrap();
        Ok(inspect(identity.description().whole_words().unwrap()))
    }

    /// Borrow the complete implementation identity. Descriptor backing is
    /// admitted lazily and shared with completed artifacts, never rebuilt per view.
    pub fn with_implementation_description<R>(
        &mut self,
        candidate: CandidateId,
        domain: WorkDomain,
        inspect: impl FnOnce(ImplementationDescription<'_>) -> R,
    ) -> Result<R, CandidateError> {
        let index = self.candidate_slot(candidate)?;
        self.ensure_implementation_identity(index, domain)?;
        Ok(inspect(
            self.slots[index]
                .checkpoint
                .as_ref()
                .unwrap()
                .identity
                .as_ref()
                .unwrap()
                .description(),
        ))
    }

    /// Analyze and publish one complete producer/consumer family. Unknown or
    /// truncated analysis publishes nothing. Source meaning and every semantic
    /// revision stay unchanged; this is a target implementation choice.
    pub fn scalar_javascript(
        &mut self,
        base: CandidateId,
        state: CellId,
        request: ScalarRequest,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<ScalarPublication, CandidateError> {
        use super::record_family::{self, FamilyOutcome, FamilyRequest};
        let base = self.candidate_slot(base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        if !policy.tactic(TacticId::ScalarReplacement).enabled {
            return Err(CandidateError::ForbiddenTactic(TacticId::ScalarReplacement));
        }
        self.check_existing_javascript_contract(policy, domain)?;
        let checkpoint = self.slots[base].checkpoint.as_ref().unwrap();
        let implementations = checkpoint.implementations.as_ref().unwrap();
        check_candidate_policy(implementations, policy)?;
        // The snapshot constructor already established index coherence. The
        // family query owns the admitted analysis work; no caller fabricates
        // its algorithm identity or a complete-proof payload.
        let analysis = record_family::analyze(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            state,
            FamilyRequest {
                attempt: AnalysisAttempt {
                    plan: record_family::RECORD_FAMILY_PLAN,
                    algorithm_version: record_family::RECORD_FAMILY_VERSION,
                    work_quota: request.max_work,
                },
                scratch_bytes: request.scratch_bytes,
                output_bytes: request.output_bytes,
            },
            &mut self.ledger,
            domain,
        )?;
        let outcome = match analysis.outcome {
            FamilyOutcome::Unknown(reason) => ScalarOutcome::Unknown(reason),
            FamilyOutcome::Truncated(limit) => ScalarOutcome::Truncated(limit),
            FamilyOutcome::Complete(family) => {
                // with_scalar consumes and releases the new family on failure.
                let map = implementations.with_scalar(family, &mut self.ledger, domain)?;
                ScalarOutcome::Published(
                    self.publish_candidate(base, map, policy, domain, start, None)?,
                )
            }
        };
        Ok(ScalarPublication {
            outcome,
            receipt: analysis.receipt,
        })
    }

    /// Borrow the existing candidate choice owner; callers admit their own
    /// bounded inspection and cannot retain an uncharged proof clone.
    pub(super) fn with_implementations<R>(
        &self,
        candidate: CandidateId,
        inspect: impl FnOnce(&ImplementationMap) -> R,
    ) -> Result<R, CandidateError> {
        let slot = self.candidate_slot(candidate)?;
        Ok(inspect(
            self.slots[slot]
                .checkpoint
                .as_ref()
                .unwrap()
                .implementations
                .as_ref()
                .unwrap(),
        ))
    }

    pub fn scalar_product_javascript(
        &mut self,
        base: CandidateId,
        state: CellId,
        request: ProductRequest,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<ProductPublication, CandidateError> {
        use super::product_family::{self, FamilyOutcome, FamilyRequest};
        let base = self.candidate_slot(base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        if !policy.tactic(TacticId::ScalarReplacement).enabled {
            return Err(CandidateError::ForbiddenTactic(TacticId::ScalarReplacement));
        }
        self.check_existing_javascript_contract(policy, domain)?;
        let checkpoint = self.slots[base].checkpoint.as_ref().unwrap();
        let implementations = checkpoint.implementations.as_ref().unwrap();
        check_candidate_policy(implementations, policy)?;
        // The snapshot constructor already established index coherence. The
        // family query owns the admitted analysis work; no caller fabricates
        // its algorithm identity or a complete-proof payload.
        let analysis = product_family::analyze_published(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            state,
            FamilyRequest {
                execution: policy
                    .javascript_contract()
                    .ok_or(CandidateError::InvalidRequest)?
                    .execution,
                attempt: AnalysisAttempt {
                    plan: product_family::PRODUCT_FAMILY_PLAN,
                    algorithm_version: product_family::PRODUCT_FAMILY_VERSION,
                    work_quota: request.max_work,
                },
                scratch_bytes: request.scratch_bytes,
                output_bytes: request.output_bytes,
            },
            &mut self.ledger,
            domain,
        )?;
        let outcome = match analysis.outcome {
            FamilyOutcome::Unknown(reason) => ProductOutcome::Unknown(reason),
            FamilyOutcome::Truncated(limit) => ProductOutcome::Truncated(limit),
            FamilyOutcome::Complete(family) => {
                // with_scalar consumes and releases the new family on failure.
                let map = implementations.with_product(family, &mut self.ledger, domain)?;
                ProductOutcome::Published(
                    self.publish_candidate(base, map, policy, domain, start, None)?,
                )
            }
        };
        Ok(ProductPublication {
            outcome,
            receipt: analysis.receipt,
        })
    }

    /// Select full top-field transport for a complete private callable body.
    /// Local product storage and exact-call inlining remain independent choices.
    pub fn scalar_function_javascript(
        &mut self,
        base: CandidateId,
        body: UnitId,
        request: FunctionRequest,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<FunctionPublication, CandidateError> {
        use super::function_layout::{self, FamilyOutcome, FamilyRequest};
        let base = self.candidate_slot(base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        if !policy.tactic(TacticId::CallSpecialization).enabled {
            return Err(CandidateError::ForbiddenTactic(
                TacticId::CallSpecialization,
            ));
        }
        self.check_existing_javascript_contract(policy, domain)?;
        let checkpoint = self.slots[base].checkpoint.as_ref().unwrap();
        let implementations = checkpoint.implementations.as_ref().unwrap();
        check_candidate_policy(implementations, policy)?;
        // The snapshot constructor already established index coherence. The
        // family query owns the admitted analysis work; no caller fabricates
        // its algorithm identity or a complete-proof payload.
        let analysis = function_layout::analyze_published(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            body,
            FamilyRequest {
                execution: policy
                    .javascript_contract()
                    .ok_or(CandidateError::InvalidRequest)?
                    .execution,
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
        let outcome = match analysis.outcome {
            FamilyOutcome::Unknown(reason) => FunctionOutcome::Unknown(reason),
            FamilyOutcome::Truncated(limit) => FunctionOutcome::Truncated(limit),
            FamilyOutcome::Complete(family) => {
                // with_function consumes and releases the new family on failure.
                let map = implementations.with_function(family, &mut self.ledger, domain)?;
                FunctionOutcome::Published(
                    self.publish_candidate(base, map, policy, domain, start, None)?,
                )
            }
        };
        Ok(FunctionPublication {
            outcome,
            receipt: analysis.receipt,
        })
    }

    /// Select a complete private leaf-helper family without changing semantic
    /// units, call schedules or captures. The shared implementation remains a
    /// valid sibling. Local facts come from this compilation's enabled cache;
    /// record prerequisites are proved independently of selected scalar layouts.
    pub fn inline_helper_javascript(
        &mut self,
        base: CandidateId,
        cell: CellId,
        request: HelperRequest,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<HelperPublication, CandidateError> {
        use super::helper_family::{self, FamilyOutcome, FamilyRequest, PreparationOutcome};

        let semantic_id = base.semantic_id();
        let base = self.candidate_slot(base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        if !policy.tactic(TacticId::Inlining).enabled {
            return Err(CandidateError::ForbiddenTactic(TacticId::Inlining));
        }
        self.check_existing_javascript_contract(policy, domain)?;
        if self.local_facts.is_none() {
            return Err(CompilationFactsError::NotEnabled.into());
        }
        let checkpoint = self.slots[base].checkpoint.as_ref().unwrap();
        check_candidate_policy(checkpoint.implementations.as_ref().unwrap(), policy)?;
        let preparation = helper_family::prepare(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            cell,
            FamilyRequest {
                attempt: AnalysisAttempt {
                    plan: helper_family::HELPER_FAMILY_PLAN,
                    algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                    work_quota: request.max_work,
                },
                scratch_bytes: request.scratch_bytes,
                output_bytes: request.output_bytes,
                execution: policy.javascript_contract().unwrap().execution,
            },
            &mut self.ledger,
            domain,
        )?;
        let mut local_facts_receipt = None;
        let family = match preparation.outcome {
            PreparationOutcome::Unknown(reason) => FamilyOutcome::Unknown(reason),
            PreparationOutcome::Truncated(limit) => FamilyOutcome::Truncated(limit),
            PreparationOutcome::Ready(mut prepared) => {
                let body = prepared.root().body;
                // prepare has admitted the fixed body scan and its scratch.
                // During this callback the existing facts session exclusively
                // owns ledger access; the helper only inspects borrowed input.
                let checked = self.with_local_facts(domain, 1, |group| {
                    let facts = group.query(semantic_id, body, request.local_facts)?;
                    let receipt = facts.receipt();
                    prepared.check_body(facts.program, facts.facts(), receipt)?;
                    Ok::<_, CandidateError>(receipt)
                });
                match checked {
                    Ok(Ok(receipt)) => local_facts_receipt = Some(receipt),
                    Ok(Err(error)) => {
                        prepared.discard(&mut self.ledger)?;
                        return Err(error);
                    }
                    Err(error) => {
                        prepared.discard(&mut self.ledger)?;
                        return Err(error.into());
                    }
                }
                // Session scratch has been released. This same compilation
                // ledger now finishes or discards the pending proof payload.
                prepared.finish(&mut self.ledger)?
            }
        };
        let outcome = match family {
            FamilyOutcome::Unknown(reason) => HelperOutcome::Unknown(reason),
            FamilyOutcome::Truncated(limit) => HelperOutcome::Truncated(limit),
            FamilyOutcome::Complete(family)
                if !family
                    .frame_elision_allowed(policy.javascript_contract().unwrap().execution) =>
            {
                family.discard(&mut self.ledger)?;
                HelperOutcome::Unknown(helper_family::UnknownReason::ObservableFrame)
            }
            FamilyOutcome::Complete(family) => {
                let map = self.slots[base]
                    .checkpoint
                    .as_ref()
                    .unwrap()
                    .implementations
                    .as_ref()
                    .unwrap()
                    .with_inline_helper(family, &mut self.ledger, domain)?;
                HelperOutcome::Published(
                    self.publish_candidate(base, map, policy, domain, start, None)?,
                )
            }
        };
        Ok(HelperPublication {
            outcome,
            receipt: preparation.receipt,
            prerequisite_attempts: preparation.prerequisites.attempts,
            prerequisite_work: preparation.prerequisites.logical_work,
            local_facts_receipt,
        })
    }

    /// Select the representation of proven exact string definitions. The source
    /// computation remains an available sibling; no semantic operation or exact
    /// fact is rewritten. The first proof client supports one unit/activation.
    ///
    /// Payload admission precedes the single borrowed local-facts query. Its
    /// result may populate the prepared family's admitted storage, but no facts
    /// cache owner or borrowed source payload can escape into the new map.
    pub fn represent_string_javascript(
        &mut self,
        base: CandidateId,
        definitions: &[ValueRef],
        choice: StringChoice,
        request: StringRequest,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<StringPublication, CandidateError> {
        use super::string_family::{
            self, FamilyAnalysis, FamilyOutcome, FamilyRequest, PreparationOutcome,
        };

        let semantic_id = base.semantic_id();
        let base = self.candidate_slot(base)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        if !policy.tactic(TacticId::ConstantFolding).enabled {
            return Err(CandidateError::ForbiddenTactic(TacticId::ConstantFolding));
        }
        if matches!(choice, StringChoice::SharedLiteral { .. })
            && !policy.tactic(TacticId::StringPooling).enabled
        {
            return Err(CandidateError::ForbiddenTactic(TacticId::StringPooling));
        }
        self.check_existing_javascript_contract(policy, domain)?;
        if self.local_facts.is_none() {
            return Err(CompilationFactsError::NotEnabled.into());
        }
        let checkpoint = self.slots[base].checkpoint.as_ref().unwrap();
        check_candidate_policy(checkpoint.implementations.as_ref().unwrap(), policy)?;
        let preparation = string_family::prepare(
            &checkpoint.semantic.program,
            definitions,
            choice,
            FamilyRequest {
                attempt: AnalysisAttempt {
                    plan: string_family::STRING_FAMILY_PLAN,
                    algorithm_version: string_family::STRING_FAMILY_VERSION,
                    work_quota: request.max_work,
                },
                scratch_bytes: request.scratch_bytes,
                output_bytes: request.output_bytes,
                local_facts: FactRequest {
                    attempt: AnalysisAttempt {
                        plan: LOCAL_FACTS_PLAN,
                        algorithm_version: LOCAL_FACTS_VERSION,
                        work_quota: request.local_facts.work_quota,
                    },
                    result_bytes: request.local_facts.result_bytes,
                },
            },
            &mut self.ledger,
            domain,
        )?;
        let mut local_facts_receipt = None;
        let analysis = match preparation.outcome {
            PreparationOutcome::Unknown(reason) => FamilyAnalysis {
                outcome: FamilyOutcome::Unknown(reason),
                receipt: preparation.receipt,
            },
            PreparationOutcome::Truncated(limit) => FamilyAnalysis {
                outcome: FamilyOutcome::Truncated(limit),
                receipt: preparation.receipt,
            },
            PreparationOutcome::Ready(mut prepared) => {
                let unit = prepared.unit();
                // Preparation jointly admitted its own maximum and exactly one
                // planned facts attempt. No unrelated work may spend that
                // domain before finish/discard bills the actual family work.
                let checked = self.with_local_facts(domain, 1, |group| {
                    let facts = group.query(semantic_id, unit, request.local_facts)?;
                    let receipt = facts.receipt();
                    prepared.check_facts(facts.program, facts.facts(), receipt)?;
                    Ok::<_, CandidateError>(receipt)
                });
                match checked {
                    Ok(Ok(receipt)) => local_facts_receipt = Some(receipt),
                    Ok(Err(error)) => {
                        prepared.discard(&mut self.ledger)?;
                        return Err(error);
                    }
                    Err(error) => {
                        prepared.discard(&mut self.ledger)?;
                        return Err(error.into());
                    }
                }
                prepared.finish(&mut self.ledger)?
            }
        };
        let outcome = match analysis.outcome {
            FamilyOutcome::Unknown(reason) => StringOutcome::Unknown(reason),
            FamilyOutcome::Truncated(limit) => StringOutcome::Truncated(limit),
            FamilyOutcome::Complete(family) => {
                // Insertion consumes the proof even on overlap/budget failure;
                // atomic publication owns all subsequent map cleanup.
                let map = self.slots[base]
                    .checkpoint
                    .as_ref()
                    .unwrap()
                    .implementations
                    .as_ref()
                    .unwrap()
                    .with_string(family, &mut self.ledger, domain)?;
                StringOutcome::Published(
                    self.publish_candidate(base, map, policy, domain, start, None)?,
                )
            }
        };
        Ok(StringPublication {
            outcome,
            receipt: analysis.receipt,
            local_facts_receipt,
        })
    }

    /// Combine complete immutable recipes without repeating their proofs. Both
    /// candidates must refer to the same pinned semantic snapshot; equal table
    /// stamps alone do not establish that after a checked source edit.
    pub fn combine_javascript(
        &mut self,
        base: CandidateId,
        delta: CandidateId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<CandidateId, CandidateError> {
        let base = self.candidate_slot(base)?;
        let delta = self.candidate_slot(delta)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        self.check_existing_javascript_contract(policy, domain)?;
        self.ledger.charge(domain, WorkKind::Analysis, 1)?;
        let left = self.slots[base].checkpoint.as_ref().unwrap();
        let right = self.slots[delta].checkpoint.as_ref().unwrap();
        if !Arc::ptr_eq(&left.semantic, &right.semantic) {
            return Err(CandidateError::StaleEvidence);
        }
        let left = left.implementations.as_ref().unwrap();
        let right = right.implementations.as_ref().unwrap();
        check_candidate_policy(left, policy)?;
        check_candidate_policy(right, policy)?;
        let union = left.union(right, &mut self.ledger, domain)?;
        self.publish_candidate(base, union, policy, domain, start, None)
    }

    /// Sharing choices onto a checked source edit is explicit. Every complete
    /// family must still match the destination's tables, cell-use sets and unit
    /// revisions. A stale family rejects the whole request before publication.
    pub fn rebase_javascript(
        &mut self,
        base: CandidateId,
        destination: SemanticId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<CandidateId, CandidateError> {
        let base = self.candidate_slot(base)?;
        let destination = self.lookup(destination)?;
        self.free.ok_or(PublicationError::StoreFull)?;
        let start = (self.ledger.work_used(domain), self.ledger.retained_bytes());
        self.check_existing_javascript_contract(policy, domain)?;
        let map = self.slots[base]
            .checkpoint
            .as_ref()
            .unwrap()
            .implementations
            .as_ref()
            .unwrap();
        check_candidate_policy(map, policy)?;
        let target = self.slots[destination].checkpoint.as_ref().unwrap();
        validate_implementations(map, &target.semantic, &mut self.ledger, domain)?;
        let shared = map.share(&mut self.ledger, domain)?;
        self.publish_candidate(destination, shared, policy, domain, start, None)
    }

    /// Inspect a complete, unqualified C11 translation unit from checked
    /// meaning. Native formation creates no JavaScript implementation map,
    /// naming basis, prepared target or codecs. Caller-owned C compilation is
    /// outside this ledger; the qualified driver fixes the floating-point ABI.
    pub fn with_native_c<R>(
        &mut self,
        source: SemanticId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
        inspect: impl FnOnce(&mut BudgetedNativeOutput<'_, '_>) -> R,
    ) -> Result<R, NativeError> {
        self.with_native_c_and_hosts(source, policy, domain, &NativeHostBindings::EMPTY, inspect)
    }

    /// Form self-contained C and an optional separately compiled host header
    /// through the same admitted native target plan and output owner.
    pub fn with_native_c_and_hosts<R>(
        &mut self,
        source: SemanticId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
        hosts: &NativeHostBindings<'_>,
        inspect: impl FnOnce(&mut BudgetedNativeOutput<'_, '_>) -> R,
    ) -> Result<R, NativeError> {
        let index = self.lookup(source)?;
        match policy.contract() {
            CompilationContract::Native { abi_version }
                if *abi_version == crate::package::LILSCRIPT_ABI_VERSION => {}
            CompilationContract::Native { .. } => return Err(NativeError::UnsupportedAbi),
            _ => return Err(NativeError::WrongTarget),
        }
        let checkpoint = self.slots[index].checkpoint.as_ref().unwrap();
        work(
            &mut self.ledger,
            domain,
            checkpoint.semantic.lineage.tactics().len(),
        )?;
        checkpoint
            .semantic
            .lineage
            .check_policy(policy)
            .map_err(NativeError::Admission)?;
        let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
        budget.retain(
            crate::output_budget::AllocationClass::Scratch,
            size_of::<BudgetedNativeOutput<'_, '_>>() as u64,
        )?;
        let text = super::native::form(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            hosts,
            &mut budget,
        )?;
        budget.work(WorkKind::Render, 0)?;
        let mut output = BudgetedNativeOutput::new(text, &mut budget);
        Ok(inspect(&mut output))
    }

    /// Form and qualify immutable C/header files in the common artifact owner.
    /// Their buffers remain charged until terminal handoff or compilation end.
    pub fn retain_native_c(
        &mut self,
        source: SemanticId,
        policy: &ResolvedPolicy,
        runtime: ArtifactRuntimeEvidence,
        domain: WorkDomain,
    ) -> Result<QualifiedNativeArtifact, NativeError> {
        self.retain_native_c_and_hosts(source, policy, runtime, domain, &NativeHostBindings::EMPTY)
    }

    /// Host declarations and C are qualified and retained as one native output.
    pub fn retain_native_c_and_hosts(
        &mut self,
        source: SemanticId,
        policy: &ResolvedPolicy,
        runtime: ArtifactRuntimeEvidence,
        domain: WorkDomain,
        hosts: &NativeHostBindings<'_>,
    ) -> Result<QualifiedNativeArtifact, NativeError> {
        let index = self.lookup(source)?;
        match policy.contract() {
            CompilationContract::Native { abi_version }
                if *abi_version == crate::package::LILSCRIPT_ABI_VERSION => {}
            CompilationContract::Native { .. } => return Err(NativeError::UnsupportedAbi),
            _ => return Err(NativeError::WrongTarget),
        }
        let checkpoint = self.slots[index].checkpoint.as_ref().unwrap();
        work(
            &mut self.ledger,
            domain,
            checkpoint.semantic.lineage.tactics().len(),
        )?;
        checkpoint
            .semantic
            .lineage
            .check_policy(policy)
            .map_err(NativeError::Admission)?;
        let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
        let files = super::native::form(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            hosts,
            &mut budget,
        )?;
        self.artifacts.retain_native(
            files,
            checkpoint.semantic.identity,
            checkpoint.semantic.meaning,
            &checkpoint.semantic.lineage,
            hosts.callback_abi_version,
            policy,
            runtime,
            &mut budget,
        )
    }

    pub fn with_qualified_native_artifact<R>(
        &self,
        receipt: &QualifiedNativeArtifact,
        inspect: impl FnOnce(NativeArtifactView<'_>) -> R,
    ) -> Result<R, NativeError> {
        self.artifacts.with_qualified_native(receipt, inspect)
    }

    /// Transfer both existing buffers; no admission is released before this
    /// explicit terminal handoff. Reusing a consumed receipt is rejected.
    pub fn take_qualified_native_artifact(
        &mut self,
        receipt: QualifiedNativeArtifact,
    ) -> Result<(String, String), NativeError> {
        self.artifacts.take_qualified_native(
            receipt,
            &mut AllocationBudget::new(Some((&mut self.ledger, WorkDomain::Baseline))),
        )
    }

    /// Prepare mandatory or final output using the reserved baseline domain.
    /// Speculative candidate evaluation must use `with_javascript_output_in`
    /// with `WorkDomain::Optional`, even when inspecting a baseline candidate.
    pub fn with_javascript_output<R>(
        &mut self,
        candidate: CandidateId,
        policy: &ResolvedPolicy,
        inspect: impl FnOnce(&mut BudgetedJavaScriptOutput<'_, '_>) -> R,
    ) -> Result<R, CandidateError> {
        self.with_javascript_output_in(candidate, policy, WorkDomain::Baseline, inspect)
    }

    /// Construct and prepare a transient target under this compilation's one
    /// ledger. The facade keeps rendered bytes admitted through scoring and
    /// explicit retention. Its temporary artifacts and all target/preparation
    /// storage release on return or unwind; take_artifact is a terminal handoff.
    /// Caller-owned allocations are outside this compiler-storage contract.
    pub fn with_javascript_output_in<R>(
        &mut self,
        candidate: CandidateId,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
        inspect: impl FnOnce(&mut BudgetedJavaScriptOutput<'_, '_>) -> R,
    ) -> Result<R, CandidateError> {
        self.with_javascript_output_choices_in(
            candidate,
            policy,
            OutputTactics::from_policy(policy),
            domain,
            inspect,
        )
    }

    /// Select an allowed subset of the common output passes without changing
    /// language, delivery or naming policy. These flags record implementation
    /// choices; they do not supply runtime cost evidence for final admission.
    pub fn with_javascript_output_choices_in<R>(
        &mut self,
        candidate: CandidateId,
        policy: &ResolvedPolicy,
        choices: OutputTactics,
        domain: WorkDomain,
        inspect: impl FnOnce(&mut BudgetedJavaScriptOutput<'_, '_>) -> R,
    ) -> Result<R, CandidateError> {
        self.with_javascript_target_in(candidate, policy, choices, domain, |target| {
            target.with_output_in(domain, inspect)
        })?
    }

    fn with_javascript_target_in<R>(
        &mut self,
        candidate: CandidateId,
        policy: &ResolvedPolicy,
        choices: OutputTactics,
        domain: WorkDomain,
        inspect: impl FnOnce(&mut JavaScriptTarget<'_, '_>) -> R,
    ) -> Result<R, CandidateError> {
        let index = self.candidate_slot(candidate)?;
        self.check_existing_javascript_contract(policy, domain)?;
        self.ledger.charge(domain, WorkKind::Analysis, 2)?;
        choices.check_policy(policy).map_err(|error| match error {
            crate::compilation_policy::AdmissionError::ForbiddenTactic(tactic) => {
                CandidateError::ForbiddenTactic(tactic)
            }
            _ => unreachable!("output pass permission check does not evaluate costs"),
        })?;
        let checkpoint = self.slots[index].checkpoint.as_ref().unwrap();
        let map = checkpoint.implementations.as_ref().unwrap();
        work(
            &mut self.ledger,
            domain,
            checkpoint.semantic.lineage.tactics().len(),
        )?;
        check_semantic_policy(&checkpoint.semantic, policy)?;
        self.ledger
            .charge(domain, WorkKind::Analysis, map.tactics().len() as u64)?;
        check_candidate_policy(map, policy)?;
        let target = self
            .javascript
            .as_ref()
            .ok_or(CandidateError::UnknownCandidate)?;
        let checkpoint = self.slots[index].checkpoint.as_mut().unwrap();
        let map = checkpoint.implementations.as_ref().unwrap();
        // No source edits can mutate a retained snapshot. The map was already
        // validated atomically against this exact snapshot at publication.
        let demand = if choices.dead_code_elimination {
            super::demand::DemandMode::Prune
        } else {
            super::demand::DemandMode::Preserve
        };
        let mut budget = AllocationBudget::new(Some((&mut self.ledger, domain)));
        let (module, literals) = super::javascript::lower_output_admitted(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            map,
            target.language(),
            demand,
            choices.target_compaction,
            choices.raw_structure,
            self.host_modules.as_ref().map(|(delivery, _)| delivery),
            &mut budget,
        )
        .map_err(|error| match error {
            super::javascript::FormationError::Unsupported(error) => {
                CandidateError::Unsupported(error)
            }
            super::javascript::FormationError::Budget(error) => CandidateError::Budget(error),
            super::javascript::FormationError::Allocation(error) => error.into(),
        })?;
        let mut target = JavaScriptTarget {
            module_names: self
                .module_names
                .as_ref()
                .map_or(&[][..], |(names, _)| names.as_slice()),
            chunk_extension: self.chunk_extension,
            hosts: self.host_modules.as_ref().map(|(delivery, _)| delivery),
            module,
            literals,
            #[cfg(test)]
            _test_lifetime: super::search_target_reuse_tests::TargetLifetime::new(),
            checkpoint,
            artifacts: &mut self.artifacts,
            store: self.store,
            candidate,
            policy,
            choices,
            budget,
        };
        Ok(inspect(&mut target))
    }

    /// Borrow a retained complete artifact; the callback cannot return its borrow.
    pub fn with_artifact<R>(
        &self,
        artifact: ArtifactId,
        inspect: impl FnOnce(ArtifactView<'_>) -> R,
    ) -> Result<R, CandidateError> {
        self.artifacts.with_artifact(artifact, inspect)
    }

    pub fn measure_artifact(
        &mut self,
        artifact: ArtifactId,
        codec: crate::config::CompressionCostModel,
        domain: WorkDomain,
    ) -> Result<usize, CandidateError> {
        self.artifacts.measure(
            artifact,
            codec,
            &mut AllocationBudget::new(Some((&mut self.ledger, domain))),
        )
    }

    pub fn qualify_artifact(
        &mut self,
        artifact: ArtifactId,
        policy: &ResolvedPolicy,
        codec: crate::config::CompressionCostModel,
        runtime: ArtifactRuntimeEvidence,
        baseline: Option<&QualifiedArtifact>,
        domain: WorkDomain,
    ) -> Result<QualifiedArtifact, CandidateError> {
        self.check_existing_javascript_contract(policy, domain)?;
        self.artifacts.qualify(
            artifact,
            &self.javascript.as_ref().unwrap().contract,
            policy,
            codec,
            runtime,
            baseline,
            &mut AllocationBudget::new(Some((&mut self.ledger, domain))),
        )
    }

    pub fn with_qualified_artifact<R>(
        &self,
        artifact: &QualifiedArtifact,
        inspect: impl FnOnce(ArtifactView<'_>, ArtifactProvenanceDescription<'_>) -> R,
    ) -> Result<R, CandidateError> {
        self.artifacts.check_qualified(artifact)?;
        let provenance = self.artifacts.provenance(artifact.artifact())?;
        self.artifacts.with_artifact(artifact.artifact(), |view| {
            inspect(view, provenance.description())
        })
    }

    /// Deliver the exact multi-file artifact admitted by this receipt: its
    /// entry text and every chunk file scored with it.
    pub fn take_qualified_bundle(
        &mut self,
        artifact: QualifiedArtifact,
    ) -> Result<DeliveredBundle, CandidateError> {
        self.artifacts.check_qualified(&artifact)?;
        self.artifacts.take_bundle(
            artifact.artifact(),
            &mut AllocationBudget::new(Some((&mut self.ledger, WorkDomain::Baseline))),
        )
    }

    /// Deliver the exact single-file artifact admitted by this receipt.
    pub fn take_qualified_artifact(
        &mut self,
        artifact: QualifiedArtifact,
    ) -> Result<String, CandidateError> {
        self.artifacts.check_qualified(&artifact)?;
        self.take_artifact(artifact.artifact())
    }

    pub fn discard_artifact(&mut self, artifact: ArtifactId) -> Result<(), CandidateError> {
        self.artifacts.discard(
            artifact,
            &mut AllocationBudget::new(Some((&mut self.ledger, WorkDomain::Baseline))),
        )
    }

    /// Unqualified inspection transfer. Deployment uses take_qualified_artifact.
    pub fn take_artifact(&mut self, artifact: ArtifactId) -> Result<String, CandidateError> {
        self.artifacts.take(
            artifact,
            &mut AllocationBudget::new(Some((&mut self.ledger, WorkDomain::Baseline))),
        )
    }

    fn candidate_slot(&self, id: CandidateId) -> Result<usize, CandidateError> {
        let slot = self.lookup(id.0)?;
        if self.slots[slot]
            .checkpoint
            .as_ref()
            .unwrap()
            .implementations
            .is_none()
        {
            return Err(CandidateError::UnknownCandidate);
        }
        Ok(slot)
    }

    fn ensure_implementation_identity(
        &mut self,
        index: usize,
        domain: WorkDomain,
    ) -> Result<(), CandidateError> {
        let checkpoint = self.slots[index].checkpoint.as_mut().unwrap();
        SharedImplementationIdentity::ensure(
            &mut checkpoint.identity,
            checkpoint.implementations.as_ref(),
            checkpoint.semantic.identity,
            checkpoint.semantic.meaning,
            &checkpoint.semantic.lineage,
            self.store,
            &mut AllocationBudget::new(Some((&mut self.ledger, domain))),
        )?;
        Ok(())
    }

    fn share_implementation_identity(
        &mut self,
        candidate: CandidateId,
        domain: WorkDomain,
    ) -> Result<SharedImplementationIdentity, CandidateError> {
        let index = self.candidate_slot(candidate)?;
        self.ensure_implementation_identity(index, domain)?;
        Ok(self.slots[index]
            .checkpoint
            .as_ref()
            .unwrap()
            .identity
            .as_ref()
            .unwrap()
            .share(&mut AllocationBudget::new(Some((&mut self.ledger, domain))))?)
    }

    fn check_existing_javascript_contract(
        &mut self,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<(), CandidateError> {
        let target = self
            .javascript
            .as_ref()
            .ok_or(CandidateError::UnknownCandidate)?;
        admitted_contract_payload(policy.contract(), &mut self.ledger, domain)?;
        if target.contract != *policy.contract() {
            return Err(CandidateError::ContractMismatch);
        }
        Ok(())
    }

    fn prepare_javascript_contract(
        &mut self,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<Option<BoundJavaScript>, CandidateError> {
        let payload = admitted_contract_payload(policy.contract(), &mut self.ledger, domain)?;
        if let Some(target) = &self.javascript {
            if target.contract != *policy.contract() {
                return Err(CandidateError::ContractMismatch);
            }
            return Ok(None);
        }
        let charge = Charge::reserve(&mut self.ledger, domain, payload)?;
        match copy_javascript_contract(policy.contract()) {
            Ok(contract) => Ok(Some(BoundJavaScript { contract, charge })),
            Err(error) => {
                charge.release(&mut self.ledger)?;
                Err(error)
            }
        }
    }

    fn publish_candidate(
        &mut self,
        base: usize,
        map: ImplementationMap,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
        start: (u64, u64),
        provisional: Option<BoundJavaScript>,
    ) -> Result<CandidateId, CandidateError> {
        let prepared = self.prepare_candidate(base, &map, policy, domain);
        let (slot, charge) = match prepared {
            Ok(value) => value,
            Err(error) => {
                map.discard(&mut self.ledger)?;
                if let Some(provisional) = provisional {
                    provisional.discard(&mut self.ledger)?;
                }
                return Err(error);
            }
        };
        Ok(self.install_candidate(base, map, slot, charge, domain, start, provisional))
    }

    /// All publication admissions precede the first observable ownership move.
    fn prepare_candidate(
        &mut self,
        base: usize,
        map: &ImplementationMap,
        policy: &ResolvedPolicy,
        domain: WorkDomain,
    ) -> Result<(u32, Charge), CandidateError> {
        let slot = self.free.ok_or(PublicationError::StoreFull)?;
        check_candidate_policy(&map, policy)?;
        let semantic = &self.slots[base].checkpoint.as_ref().unwrap().semantic;
        work(&mut self.ledger, domain, semantic.lineage.tactics().len())?;
        check_semantic_policy(semantic, policy)?;
        validate_implementations(&map, semantic, &mut self.ledger, domain)?;
        self.ledger.charge(domain, WorkKind::Edit, 1)?;
        // The map already charges its inline header when nonempty. An
        // empty direct map allocates nothing, so its enclosing slot owns
        // that inline space. No unit/index vector is copied in this path.
        let charge = Charge::reserve(
            &mut self.ledger,
            domain,
            checkpoint_bytes(0, 0, map.owns_shell())?,
        )?;
        Ok::<_, CandidateError>((slot, charge))
    }

    fn install_candidate(
        &mut self,
        base: usize,
        map: ImplementationMap,
        slot: u32,
        charge: Charge,
        domain: WorkDomain,
        start: (u64, u64),
        provisional: Option<BoundJavaScript>,
    ) -> CandidateId {
        let receipt = PublicationReceipt {
            logical_work: self.ledger.work_used(domain) - start.0,
            allocated_bytes: self.ledger.retained_bytes() - start.1,
            ..PublicationReceipt::default()
        };
        let checkpoint = Checkpoint {
            semantic: self.slots[base]
                .checkpoint
                .as_ref()
                .unwrap()
                .semantic
                .clone(),
            charge,
            implementations: Some(map),
            identity: None,
            changes: Vec::new(),
            cell_changes: Vec::new(),
            receipt,
        };
        // All fallible preparation has finished. The first target and its
        // contract become observable together; rejected first attempts leave
        // the compilation unbound and can be retried under another contract.
        if let Some(provisional) = provisional {
            debug_assert!(self.javascript.is_none());
            self.javascript = Some(provisional);
        }
        CandidateId(self.publish_checkpoint(slot, checkpoint))
    }

    pub fn discard(&mut self, id: SemanticId) -> Result<(), PublicationError> {
        let index = self.lookup(id)?;
        let checkpoint = self.slots[index].checkpoint.take().unwrap();
        discard_checkpoint(*checkpoint, &mut self.ledger)?;
        self.slots[index].next_free = self.free;
        self.free = Some(index as u32);
        self.live -= 1;
        Ok(())
    }

    /// Internal read-only query access shares this owner's actual ledger. The
    /// callback cannot access a mutable unit or publish a detached candidate.
    pub(super) fn with_semantic<R>(
        &mut self,
        id: SemanticId,
        inspect: impl FnOnce(&Program<'src>, &UseIndex, &mut BudgetLedger) -> R,
    ) -> Result<R, PublicationError> {
        let index = self.lookup(id)?;
        let checkpoint = self.slots[index].checkpoint.as_ref().unwrap();
        Ok(inspect(
            &checkpoint.semantic.program,
            &checkpoint.semantic.uses,
            &mut self.ledger,
        ))
    }

    pub fn finish(self) -> BudgetLedger {
        let Self {
            mut ledger,
            slots,
            slots_charge,
            javascript,
            local_facts,
            artifacts,
            module_names,
            host_modules,
            ..
        } = self;
        if let Some((names, charge)) = module_names {
            drop(names);
            charge
                .release(&mut ledger)
                .expect("owned module name reservation");
        }
        if let Some((delivery, charge)) = host_modules {
            drop(delivery);
            charge
                .release(&mut ledger)
                .expect("owned host module reservation");
        }
        artifacts.finish(&mut ledger);
        for slot in slots {
            if let Some(checkpoint) = slot.checkpoint {
                discard_checkpoint(*checkpoint, &mut ledger)
                    .expect("owned semantic checkpoint reservations");
            }
        }
        if let Some(javascript) = javascript {
            javascript
                .discard(&mut ledger)
                .expect("owned JavaScript contract reservation");
        }
        if let Some(local_facts) = local_facts {
            local_facts
                .discard(&mut ledger)
                .expect("owned local facts cache reservation");
        }
        slots_charge
            .release(&mut ledger)
            .expect("owned checkpoint slots reservation");
        ledger
    }
    fn lookup(&self, id: SemanticId) -> Result<usize, PublicationError> {
        lookup_slot(self.store, &self.slots, id)
    }
    fn checkpoint(&self, id: SemanticId) -> Result<&Checkpoint<'src>, PublicationError> {
        Ok(self.slots[self.lookup(id)?].checkpoint.as_ref().unwrap())
    }
    fn publish(
        &mut self,
        slot: u32,
        pending: Pending<'src>,
        receipt: PublicationReceipt,
        origin: SnapshotOrigin,
    ) -> SemanticId {
        let Pending {
            program,
            uses,
            mut charges,
            changes,
            cell_changes,
        } = pending;
        let bytes = checkpoint_bytes(changes.capacity(), cell_changes.capacity(), false)
            .expect("admitted source checkpoint capacity");
        let charge = Charge {
            domain: charges.shell.domain,
            bytes,
        };
        charges.shell.bytes -= bytes;
        self.publish_checkpoint(
            slot,
            Checkpoint {
                semantic: Arc::new(SemanticSnapshot {
                    identity: origin.identity,
                    meaning: origin.meaning,
                    lineage: origin.lineage,
                    program,
                    uses: uses.unwrap(),
                    charges,
                }),
                charge,
                implementations: None,
                identity: None,
                changes,
                cell_changes,
                receipt,
            },
        )
    }
    fn publish_checkpoint(&mut self, slot: u32, checkpoint: Checkpoint<'src>) -> SemanticId {
        let generation = RevisionId::fresh();
        self.free = self.slots[slot as usize].next_free;
        self.slots[slot as usize].generation = Some(generation);
        self.slots[slot as usize].checkpoint = Some(Box::new(checkpoint));
        self.live += 1;
        SemanticId {
            store: self.store,
            slot,
            generation,
        }
    }
}

fn lookup_slot(
    store: RevisionId,
    slots: &[Slot<'_>],
    id: SemanticId,
) -> Result<usize, PublicationError> {
    if id.store != store {
        return Err(PublicationError::UnknownCheckpoint);
    }
    slots
        .get(id.slot as usize)
        .filter(|slot| slot.generation == Some(id.generation) && slot.checkpoint.is_some())
        .map(|_| id.slot as usize)
        .ok_or(PublicationError::UnknownCheckpoint)
}

struct Pending<'src> {
    program: Program<'src>,
    charges: SemanticCharges,
    changes: Vec<UnitChange>,
    cell_changes: Vec<CellUseChange>,
    uses: Option<UseIndex>,
}
/// Charge bounded contract inspection before walking names, then admit the
/// comparison/copy bytes. A warm comparison and first publication have the same
/// deterministic contract work; neither inspects semantic unit bodies.
fn admitted_contract_payload(
    contract: &CompilationContract,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<u64, CandidateError> {
    let CompilationContract::JavaScript {
        preserved_properties,
        ..
    } = contract
    else {
        return Err(CandidateError::NotJavaScript);
    };
    ledger.charge(
        domain,
        WorkKind::Edit,
        (preserved_properties.len() as u64)
            .checked_add(16)
            .ok_or(CandidateError::Capacity)?,
    )?;
    let strings = preserved_properties.iter().try_fold(0u64, |bytes, name| {
        bytes
            .checked_add(name.len() as u64)
            .ok_or(CandidateError::Capacity)
    })?;
    ledger.charge(domain, WorkKind::Edit, strings)?;
    Ok(bytes::<String>(preserved_properties.len())?
        .checked_add(strings)
        .ok_or(CandidateError::Capacity)?)
}

fn copy_javascript_contract(
    contract: &CompilationContract,
) -> Result<CompilationContract, CandidateError> {
    let CompilationContract::JavaScript {
        language,
        preserved_properties,
        bundle_mode,
        split,
        preload,
    } = contract
    else {
        return Err(CandidateError::NotJavaScript);
    };
    let mut names = Vec::new();
    names
        .try_reserve_exact(preserved_properties.len())
        .map_err(|_| CandidateError::AllocationFailed)?;
    for name in preserved_properties {
        let mut copied = String::new();
        copied
            .try_reserve_exact(name.len())
            .map_err(|_| CandidateError::AllocationFailed)?;
        copied.push_str(name);
        names.push(copied);
    }
    Ok(CompilationContract::JavaScript {
        language: *language,
        preserved_properties: names,
        bundle_mode: *bundle_mode,
        split: *split,
        preload: *preload,
    })
}
fn check_candidate_policy(
    map: &ImplementationMap,
    policy: &ResolvedPolicy,
) -> Result<(), CandidateError> {
    if policy.javascript_contract().is_none() {
        return Err(CandidateError::NotJavaScript);
    }
    for usage in map.tactics() {
        if !policy.tactic(usage.tactic).enabled {
            return Err(CandidateError::ForbiddenTactic(usage.tactic));
        }
        // These scalar-cell, private leaf and literal/storage recipes have
        // neutral semantic-work provenance. This is not final target/runtime
        // cost admission; added reconstruction needs an explicit cost client.
        if usage.risk != RuntimeRisk::Neutral {
            return Err(CandidateError::UnsupportedRisk);
        }
    }
    Ok(())
}
fn check_semantic_policy(
    semantic: &SemanticSnapshot<'_>,
    policy: &ResolvedPolicy,
) -> Result<(), CandidateError> {
    semantic
        .lineage
        .check_policy(policy)
        .map_err(|error| match error {
            crate::compilation_policy::AdmissionError::ForbiddenTactic(tactic) => {
                CandidateError::ForbiddenTactic(tactic)
            }
            _ => unreachable!("semantic lineage permission check"),
        })
}
fn validate_implementations(
    map: &ImplementationMap,
    semantic: &SemanticSnapshot<'_>,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<(), CandidateError> {
    ledger.charge(domain, WorkKind::Analysis, map.published_validation_work()?)?;
    if !map.valid_for_published(&semantic.program, &semantic.uses) {
        return Err(CandidateError::StaleEvidence);
    }
    Ok(())
}
impl Pending<'_> {
    fn discard(self, ledger: &mut BudgetLedger) -> Result<(), PublicationError> {
        let Self {
            program,
            charges,
            changes,
            cell_changes,
            uses,
        } = self;
        if let Some(uses) = uses {
            uses.discard(ledger)?;
        }
        drop(changes);
        drop(cell_changes);
        release_program(program, charges, ledger)
    }
}
fn discard_checkpoint(
    checkpoint: Checkpoint<'_>,
    ledger: &mut BudgetLedger,
) -> Result<(), PublicationError> {
    let Checkpoint {
        semantic,
        charge,
        implementations,
        identity,
        changes,
        cell_changes,
        ..
    } = checkpoint;
    if let Some(identity) = identity {
        let owner = identity.owner();
        identity.discard(owner, ledger).unwrap();
    }
    if let Some(implementations) = implementations {
        implementations.discard(ledger)?;
    }
    drop(changes);
    drop(cell_changes);
    if let Ok(SemanticSnapshot {
        identity: _,
        meaning: _,
        lineage,
        program,
        uses,
        charges,
    }) = Arc::try_unwrap(semantic)
    {
        lineage.discard(ledger);
        uses.discard(ledger)?;
        release_program(program, charges, ledger)?;
    }
    charge.release(ledger)
}
fn release_program(
    program: Program<'_>,
    charges: SemanticCharges,
    ledger: &mut BudgetLedger,
) -> Result<(), PublicationError> {
    let Program {
        units,
        cells,
        types,
        strings,
        structs,
        enums,
        fields,
        classes,
        exports,
        initialization,
        modules,
        ..
    } = program;
    let mut unit_charges = charges.units.into_iter();
    for unit in units {
        let charge = unit_charges.next();
        if unit.release_allocation() {
            if let Some(charge) = charge {
                charge.release(ledger)?;
            }
        }
    }
    drop(unit_charges);
    release_table(cells, charges.tables[0], ledger)?;
    release_table(types, charges.tables[1], ledger)?;
    release_table(strings, charges.tables[2], ledger)?;
    release_table(structs, charges.tables[3], ledger)?;
    release_table(enums, charges.tables[4], ledger)?;
    release_table(fields, charges.tables[5], ledger)?;
    release_table(exports, charges.tables[6], ledger)?;
    release_table(initialization, charges.tables[7], ledger)?;
    release_table(modules, charges.tables[8], ledger)?;
    release_table(classes, charges.tables[9], ledger)?;
    charges.shell.release(ledger)
}
fn release_table<T>(
    table: Arc<Vec<T>>,
    charge: Charge,
    ledger: &mut BudgetLedger,
) -> Result<(), PublicationError> {
    if let Ok(data) = Arc::try_unwrap(table) {
        drop(data);
        charge.release(ledger)?;
    }
    Ok(())
}
fn unique_input(program: &Program<'_>) -> bool {
    program.units.iter().all(FrozenUnit::allocation_is_unique)
        && Arc::strong_count(&program.cells) == 1
        && Arc::strong_count(&program.types) == 1
        && Arc::strong_count(&program.strings) == 1
        && Arc::strong_count(&program.structs) == 1
        && Arc::strong_count(&program.enums) == 1
        && Arc::strong_count(&program.fields) == 1
        && Arc::strong_count(&program.classes) == 1
        && Arc::strong_count(&program.exports) == 1
        && Arc::strong_count(&program.initialization) == 1
        && Arc::strong_count(&program.modules) == 1
}
fn share_program<'src>(program: &Program<'src>) -> Program<'src> {
    Program {
        tables_revision: program.tables_revision,
        units: program.units.clone(),
        cells: program.cells.clone(),
        types: program.types.clone(),
        strings: program.strings.clone(),
        structs: program.structs.clone(),
        enums: program.enums.clone(),
        fields: program.fields.clone(),
        classes: program.classes.clone(),
        exports: program.exports.clone(),
        initialization: program.initialization.clone(),
        modules: program.modules.clone(),
        entry: program.entry,
    }
}

fn shell_bytes(
    unit_capacity: usize,
    charges: usize,
    changes: usize,
) -> Result<u64, PublicationError> {
    // UseIndex owns its inline-header reservation. Source construction admits
    // the checkpoint Box and semantic Arc together, then splits their charges
    // at publication. Physical siblings retain only a new checkpoint Box.
    sum(&[
        checkpoint_bytes(changes, 0, false)?,
        (size_of::<SemanticSnapshot<'_>>() - size_of::<UseIndex>() + 2 * size_of::<usize>()) as u64,
        bytes::<FrozenUnit>(unit_capacity)?,
        bytes::<Charge>(charges)?,
    ])
}
fn checkpoint_bytes(
    changes: usize,
    cells: usize,
    map_charges_inline: bool,
) -> Result<u64, PublicationError> {
    sum(&[
        (size_of::<Checkpoint<'_>>()
            - if map_charges_inline {
                size_of::<ImplementationMap>()
            } else {
                0
            }) as u64,
        bytes::<UnitChange>(changes)?,
        bytes::<CellUseChange>(cells)?,
    ])
}
fn sum(values: &[u64]) -> Result<u64, PublicationError> {
    values.iter().try_fold(0u64, |sum, n| {
        sum.checked_add(*n).ok_or(PublicationError::Capacity)
    })
}
fn bytes<T>(count: usize) -> Result<u64, PublicationError> {
    (count as u64)
        .checked_mul(size_of::<T>() as u64)
        .ok_or(PublicationError::Capacity)
}
fn capacity<T>(values: &Vec<T>) -> Result<u64, PublicationError> {
    bytes::<T>(values.capacity())
}
fn sum_charges(charges: &[Charge]) -> Result<u64, PublicationError> {
    charges.iter().try_fold(0u64, |sum, charge| {
        sum.checked_add(charge.bytes)
            .ok_or(PublicationError::Capacity)
    })
}
fn work(
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
    amount: usize,
) -> Result<(), PublicationError> {
    ledger.charge(domain, WorkKind::Edit, amount as u64)?;
    Ok(())
}

struct Workspace<'a> {
    ledger: &'a mut BudgetLedger,
    domain: WorkDomain,
    bytes: u64,
}
impl<'a> Workspace<'a> {
    fn new(ledger: &'a mut BudgetLedger, domain: WorkDomain) -> Self {
        Self {
            ledger,
            domain,
            bytes: 0,
        }
    }
    fn reserve(&mut self, bytes: u64) -> Result<(), PublicationError> {
        let total = self
            .bytes
            .checked_add(bytes)
            .ok_or(PublicationError::Capacity)?;
        self.ledger.retain(self.domain, bytes)?;
        self.bytes = total;
        Ok(())
    }
    fn release(&mut self, bytes: u64) -> Result<(), PublicationError> {
        self.ledger.release(self.domain, bytes)?;
        self.bytes -= bytes;
        Ok(())
    }
    fn work(&mut self, amount: usize) -> Result<(), PublicationError> {
        work(self.ledger, self.domain, amount)
    }
}
impl Drop for Workspace<'_> {
    fn drop(&mut self) {
        self.ledger
            .release(self.domain, self.bytes)
            .expect("publication workspace reservation");
    }
}

struct PatchPlan {
    bytes: u64,
    operands: usize,
}
fn plan_patch(
    unit: &UnitData,
    patch: &UnitPatch<'_>,
    workspace: &mut Workspace<'_>,
) -> Result<PatchPlan, PublicationError> {
    if patch.operations.is_empty() && patch.places.is_empty() {
        return Err(PublicationError::InvalidPatch);
    }
    let mut previous = None;
    for edit in patch.operations {
        workspace.work(1)?;
        if edit.operation.index() >= unit.operations.len()
            || previous.is_some_and(|id| id >= edit.operation)
            || edit.operands.len() > u32::MAX as usize
        {
            return Err(PublicationError::InvalidPatch);
        }
        previous = Some(edit.operation);
        workspace.work(edit.operands.len())?;
        if edit
            .operands
            .iter()
            .any(|value| value.index() >= unit.values.len())
        {
            return Err(PublicationError::InvalidPatch);
        }
    }
    let mut previous = None;
    for edit in patch.places {
        workspace.work(1)?;
        if edit.place.index() >= unit.places.len() || previous.is_some_and(|id| id >= edit.place) {
            return Err(PublicationError::InvalidPatch);
        }
        previous = Some(edit.place);
    }
    workspace.work(unit.operations.len() + unit.regions.len() + unit.call_instantiations.len())?;
    let mut nested = 0;
    let mut operands = 0usize;
    let mut edits = patch.operations.iter().peekable();
    for (index, operation) in unit.operations.iter().enumerate() {
        let (kind, length) = if edits
            .peek()
            .is_some_and(|edit| edit.operation.index() == index)
        {
            let edit = edits.next().unwrap();
            (edit.kind, edit.operands.len())
        } else {
            (&operation.kind, operation.operands.len as usize)
        };
        operands = operands
            .checked_add(length)
            .ok_or(PublicationError::Capacity)?;
        if operands > u32::MAX as usize {
            return Err(PublicationError::Capacity);
        }
        if let OperationKind::Allocate {
            kind: AllocationKind::Record(keys) | AllocationKind::Object(keys),
            ..
        } = kind
        {
            nested = sum(&[nested, bytes::<StringId>(keys.len())?])?;
        }
        if let OperationKind::Allocate {
            kind: AllocationKind::SpreadArray(spread),
            ..
        } = kind
        {
            nested = sum(&[nested, bytes::<bool>(spread.len())?])?;
        }
    }
    for region in &unit.regions {
        nested = sum(&[nested, bytes::<OpId>(region.operations.len())?])?;
    }
    for instance in &unit.call_instantiations {
        nested = sum(&[nested, bytes::<TypeId>(instance.arguments.len())?])?;
    }
    let bytes = sum(&[
        (size_of::<UnitData>() + 2 * size_of::<usize>()) as u64,
        bytes::<CellId>(unit.parameters.len())?,
        bytes::<CellId>(unit.captures.len())?,
        bytes::<Operation>(unit.operations.len())?,
        bytes::<ValueId>(operands)?,
        bytes::<Value>(unit.values.len())?,
        bytes::<Region>(unit.regions.len())?,
        bytes::<Place>(unit.places.len())?,
        bytes::<CallSite>(unit.calls.len())?,
        bytes::<CallInstantiation>(unit.call_instantiations.len())?,
        bytes::<CallArgument>(unit.call_arguments.len())?,
        nested,
    ])?;
    Ok(PatchPlan { bytes, operands })
}
fn table_bytes(
    program: &Program<'_>,
    index: usize,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<(u64, u64), PublicationError> {
    let mut workspace = Workspace::new(ledger, domain);
    let mut total = (2 * size_of::<usize>() + size_of::<Vec<()>>()) as u64;
    let mut diagnostics = 0;
    match index {
        0 => {
            workspace.work(program.cells.len())?;
            total = sum(&[total, capacity(&program.cells)?])?;
            for cell in program.cells.iter() {
                total = sum(&[total, cell.name.capacity() as u64])?;
            }
        }
        1 => {
            total = sum(&[total, capacity(&program.types)?])?;
            let (payload, text) = type_payload(program, &mut workspace)?;
            total = sum(&[total, payload])?;
            diagnostics = text;
        }
        2 => {
            workspace.work(program.strings.len())?;
            total = sum(&[total, capacity(&program.strings)?])?;
            for string in program.strings.iter() {
                total = sum(&[total, string.capacity_bytes() as u64])?;
            }
        }
        3 => {
            workspace.work(program.structs.len())?;
            total = sum(&[total, capacity(&program.structs)?])?;
            for (index, definition) in program.structs.iter().enumerate() {
                if definition.identity.is_class()
                    || definition.identity.index() != index
                    || definition.module.index() >= program.modules.len()
                    || definition.span.start > definition.span.end
                {
                    return Err(PublicationError::InvalidNominalContract(
                        "invalid nominal declaration owner",
                    ));
                }
                workspace.work(definition.type_parameters.len())?;
                total = sum(&[
                    total,
                    definition.name.capacity() as u64,
                    capacity(&definition.type_parameters)?,
                ])?;
                for name in &definition.type_parameters {
                    total = sum(&[total, name.capacity() as u64])?;
                }
            }
        }
        4 => {
            workspace.work(program.enums.len())?;
            total = sum(&[total, capacity(&program.enums)?])?;
            for definition in program.enums.iter() {
                workspace.work(definition.variants.len())?;
                total = sum(&[
                    total,
                    definition.name.capacity() as u64,
                    capacity(&definition.variants)?,
                ])?;
                for variant in &definition.variants {
                    total = sum(&[total, variant.name.capacity() as u64])?;
                }
            }
        }
        5 => {
            workspace.work(program.fields.len())?;
            total = sum(&[total, capacity(&program.fields)?])?;
            let mut previous_member = None;
            for field in program.fields.iter() {
                if previous_member
                    .replace(field.identity.index())
                    .is_some_and(|previous| previous >= field.identity.index())
                {
                    return Err(PublicationError::InvalidNominalContract(
                        "nominal fields are not in canonical member order",
                    ));
                }
                total = sum(&[total, field.name.capacity() as u64])?;
            }
        }
        6 => {
            workspace.work(program.exports.len())?;
            total = sum(&[total, capacity(&program.exports)?])?;
            for export in program.exports.iter() {
                super::verify::validate_interface_target(program, export.target)
                    .map_err(PublicationError::InvalidNominalContract)?;
                total = sum(&[total, export.name.capacity() as u64])?;
            }
        }
        7 => {
            workspace.work(program.initialization.len())?;
            total = sum(&[total, capacity(&program.initialization)?])?;
        }
        8 => {
            workspace.work(program.modules.len())?;
            total = sum(&[total, capacity(&program.modules)?])?;
            for module in program.modules.iter() {
                workspace.work(module.dependencies.len() + module.imports.len())?;
                total = sum(&[
                    total,
                    capacity(&module.dependencies)?,
                    capacity(&module.imports)?,
                ])?;
                for import in &module.imports {
                    super::verify::validate_interface_target(program, import.target)
                        .map_err(PublicationError::InvalidNominalContract)?;
                    total = sum(&[total, import.name.capacity() as u64])?;
                }
                workspace.work(module.foreign_imports.len())?;
                total = sum(&[total, capacity(&module.foreign_imports)?])?;
                for import in &module.foreign_imports {
                    total = sum(&[
                        total,
                        import.source.capacity() as u64,
                        import.imported.capacity() as u64,
                    ])?;
                }
                workspace.work(module.dynamic_dependencies.len() + module.namespace.len())?;
                total = sum(&[
                    total,
                    capacity(&module.dynamic_dependencies)?,
                    capacity(&module.namespace)?,
                ])?;
                for (name, _) in &module.namespace {
                    total = sum(&[total, name.capacity() as u64])?;
                }
            }
        }
        9 => {
            workspace.work(program.classes.len())?;
            total = sum(&[total, capacity(&program.classes)?])?;
            for class in program.classes.iter() {
                workspace.work(class.fields.len() + class.type_params.len())?;
                total = sum(&[
                    total,
                    class.name.capacity() as u64,
                    class.base.as_ref().map_or(0, |base| base.capacity() as u64),
                    capacity(&class.fields)?,
                    capacity(&class.type_params)?,
                    capacity(&class.base_arguments)?,
                ])?;
                for parameter in &class.type_params {
                    total = sum(&[total, parameter.capacity() as u64])?;
                }
            }
        }
        _ => unreachable!(),
    }
    Ok((total, diagnostics))
}

fn type_payload(
    program: &Program<'_>,
    workspace: &mut Workspace<'_>,
) -> Result<(u64, u64), PublicationError> {
    use crate::check::type_payload::{measure_payload, Payload, PayloadError};
    let domain = workspace.domain;
    let mut budget = AllocationBudget::new(Some((&mut *workspace.ledger, domain)));
    let mut total = 0;
    let mut diagnostics = 0;
    for ty in program.types.iter() {
        let measured = measure_payload(Payload::Type(ty), &mut budget, |node| match node {
            Payload::Type(ty) => super::verify::validate_nominal_type(ty, &program.structs)
                .map_err(PublicationError::InvalidNominalContract),
            Payload::Signature(signature) => signature
                .validate_parameters()
                .map_err(PublicationError::InvalidParameterContract),
            Payload::Default(_) => Ok(()),
        })
        .map_err(|error| match error {
            PayloadError::Allocation(error) => PublicationError::from(error),
            PayloadError::Visitor(error) => error,
        })?;
        total = sum(&[total, measured.owned_bytes])?;
        diagnostics = sum(&[diagnostics, measured.text_bytes])?;
    }
    Ok((total, diagnostics))
}

#[cfg(test)]
#[path = "publication_fixed_tests.rs"]
mod fixed_tests;
