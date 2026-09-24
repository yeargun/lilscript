//! Bounded, read-only facts for one semantic unit at a time.
//!
//! This first query has no host assumptions or cross-unit summaries. Its coarse
//! dependencies deliberately include the whole unit and owned program tables.
//! Unknown and incomplete are different answers; discovering an exact value
//! never selects a literal, deletes its producer, or proves evaluation harmless.

#[path = "facts_return_origin.rs"]
mod return_origin;
pub(super) use return_origin::{returned_value_origin, ReturnedValueOrigin};

#[path = "facts_type_transport.rs"]
mod type_transport;
pub(super) use type_transport::contains_nominal_product;

use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, AnalysisWorkReceipt, BudgetError, BudgetLedger, WorkDomain,
};
use crate::primitive::Intrinsic;
use ahash::AHashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::mem::size_of;
use std::sync::Arc;

pub const LOCAL_FACTS_PLAN: u32 = 1;
// Version 5 distinguishes immutable struct values from reference identities.
// Version 4 no longer invents runtime domains from source cell annotations.
// Version 3 transfers existing exact primitive knowledge through CopyValue.
// Version 2 introduced shared transfer and separate resource-exhaustion effects.
// Older receipts cannot qualify this version's answers.
pub const LOCAL_FACTS_VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dependencies {
    pub unit: UnitId,
    pub unit_revision: RevisionId,
    pub tables_revision: RevisionId,
}
impl Dependencies {
    pub fn valid_for(self, program: &Program<'_>) -> bool {
        program.tables_revision == self.tables_revision
            && program
                .units
                .get(self.unit.index())
                .is_some_and(|unit| unit.id() == self.unit && unit.revision() == self.unit_revision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StoredString {
    Source(StringId),
    Computed(Arc<StringValue>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum StoredExact {
    Integer(i32),
    Number(u64),
    Boolean(bool),
    String(StoredString),
    Null,
    Undefined,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    Unvisited,
    MutableOrHostValue,
    UnsupportedOperation,
    UnknownOperand,
    WorkLimit,
    MemoryLimit,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum StoredKnowledge {
    Exact(StoredExact),
    Unknown(UnknownReason),
}

/// Borrowed answers cannot retain a cache-owned string allocation beyond the
/// query session. Explicit copies belong to the consuming candidate's budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactString<'a> {
    Source(StringId),
    Computed(&'a StringValue),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactValue<'a> {
    Integer(i32),
    Number(u64),
    Boolean(bool),
    String(ExactString<'a>),
    Null,
    Undefined,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKnowledge<'a> {
    Exact(ExactValue<'a>),
    Unknown(UnknownReason),
}

/// Bounded knowledge about one primitive-string concatenation, independent of
/// its chosen representation. This alone is not permission to drop, speculate
/// or materialize the operation: use the separate evaluation predicates and
/// preserve the original evaluation of both operands. Resource cost remains
/// relevant even when resource exhaustion is outside preserved observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedStringConstruction {
    dependencies: Dependencies,
    operation: OpId,
    operands: [ValueId; 2],
    utf16_units_upper_bound: u64,
}
impl BoundedStringConstruction {
    pub fn dependencies(self) -> Dependencies {
        self.dependencies
    }
    pub fn operation(self) -> OpId {
        self.operation
    }
    pub fn operands(self) -> [ValueId; 2] {
        self.operands
    }
    /// A conservative bound, not the exact UTF-16 length or a supported engine
    /// limit. Reading it never scans or allocates a string payload.
    pub fn utf16_units_upper_bound(self) -> u64 {
        self.utf16_units_upper_bound
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringConstructionUnknown {
    StaleDependencies,
    UnknownOperation,
    UnsupportedOperation,
    OperandNotString(ValueId),
    OperandUnknown {
        value: ValueId,
        reason: UnknownReason,
    },
    LengthOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringConstructionKnowledge {
    Bounded(BoundedStringConstruction),
    Unknown(StringConstructionUnknown),
}
impl StoredExact {
    fn view(&self) -> ExactValue<'_> {
        match self {
            Self::Integer(value) => ExactValue::Integer(*value),
            Self::Number(value) => ExactValue::Number(*value),
            Self::Boolean(value) => ExactValue::Boolean(*value),
            Self::Null => ExactValue::Null,
            Self::Undefined => ExactValue::Undefined,
            Self::String(StoredString::Source(id)) => ExactValue::String(ExactString::Source(*id)),
            Self::String(StoredString::Computed(value)) => {
                ExactValue::String(ExactString::Computed(value))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAccess {
    None,
    Cell(CellId),
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationBehavior {
    pub reads: MemoryAccess,
    pub writes: MemoryAccess,
    pub may_throw: bool,
    /// Implementation-dependent OOM, engine string-cap or stack exhaustion is
    /// excluded from preserved program observations by the language contract.
    /// This possibility remains visible to runtime-cost clients; it is not a
    /// byte bound and never covers ordinary host/argument/validation errors.
    pub may_exhaust_resources: bool,
    pub may_diverge: bool,
    pub may_reenter: bool,
    pub may_suspend: bool,
    pub creates_identity: bool,
    pub transfers_control: bool,
}
impl EvaluationBehavior {
    pub const TOTAL: Self = Self {
        reads: MemoryAccess::None,
        writes: MemoryAccess::None,
        may_throw: false,
        may_exhaust_resources: false,
        may_diverge: false,
        may_reenter: false,
        may_suspend: false,
        creates_identity: false,
        transfers_control: false,
    };
    pub const UNKNOWN: Self = Self {
        reads: MemoryAccess::Unknown,
        writes: MemoryAccess::Unknown,
        may_throw: true,
        may_exhaust_resources: true,
        may_diverge: true,
        may_reenter: true,
        may_suspend: true,
        creates_identity: true,
        transfers_control: true,
    };
    /// Synchronous scalar conversion may invoke arbitrary user hooks. The
    /// conversion itself returns a scalar and cannot suspend the enclosing
    /// frame or perform a source break/return; hook effects, exceptions and
    /// nontermination remain observable. This is not a primitive-domain proof.
    pub const COERCION: Self = Self {
        reads: MemoryAccess::Unknown,
        writes: MemoryAccess::Unknown,
        may_throw: true,
        may_exhaust_resources: true,
        may_diverge: true,
        may_reenter: true,
        ..Self::TOTAL
    };
    /// Whether evaluating an operation with an unobserved result is required.
    /// Passive reads and a fresh identity cannot be observed after their result
    /// is discarded. Reads with hooks, TDZ or other ordinary failures must carry
    /// the corresponding flags. Operand evaluation remains a separate schedule.
    pub fn requires_evaluation(self) -> bool {
        self.may_throw
            || self.may_diverge
            || self.may_reenter
            || self.may_suspend
            || self.transfers_control
            || self.writes != MemoryAccess::None
    }
    fn repeatable_without_observation(self) -> bool {
        !self.requires_evaluation() && self.reads == MemoryAccess::None && !self.creates_identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationDemand {
    Exact,
    Truthy,
    Nullish,
    Discarded,
}
impl ObservationDemand {
    /// The least common observation sufficient for both consumers. Neither
    /// truthiness nor nullishness determines the other.
    pub const fn join(self, other: Self) -> Self {
        use ObservationDemand::*;
        match (self, other) {
            (Discarded, right) => right,
            (left, Discarded) => left,
            (Truthy, Truthy) => Truthy,
            (Nullish, Nullish) => Nullish,
            _ => Exact,
        }
    }
    pub const fn is_observed(self) -> bool {
        !matches!(self, Self::Discarded)
    }
}
impl Default for ObservationDemand {
    fn default() -> Self {
        Self::Discarded
    }
}
/// Conditional assessment within this facts snapshot, never publication
/// authority. The edit owner must still validate dependencies, operation
/// identity and the actual placement/evaluation context in its checked batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legality {
    PermittedUnderContext,
    Unknown(&'static str),
}

/// Placement evidence is an input to the effect predicate, not a dominance
/// proof manufactured by it. A committing edit must establish availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeculationContext {
    pub operands_available: bool,
}

#[derive(Debug)]
pub struct UnitFacts {
    dependencies: Dependencies,
    values: Vec<StoredKnowledge>,
    effects: Vec<EvaluationBehavior>,
    primitive_domains: Vec<bool>,
    retained_bytes: u64,
}
impl UnitFacts {
    pub fn dependencies(&self) -> Dependencies {
        self.dependencies
    }
    pub fn value(&self, value: ValueId) -> ValueKnowledge<'_> {
        match self.values.get(value.index()) {
            Some(StoredKnowledge::Exact(value)) => ValueKnowledge::Exact(value.view()),
            Some(StoredKnowledge::Unknown(reason)) => ValueKnowledge::Unknown(*reason),
            None => ValueKnowledge::Unknown(UnknownReason::Unvisited),
        }
    }
    pub fn exact(&self, value: ValueId) -> Option<ExactValue<'_>> {
        match self.values.get(value.index())? {
            StoredKnowledge::Exact(value) => Some(value.view()),
            _ => None,
        }
    }
    pub fn string<'a>(
        &'a self,
        program: &'a Program<'_>,
        value: ValueId,
    ) -> Option<&'a StringValue> {
        if !self.dependencies.valid_for(program) {
            return None;
        }
        match self.exact(value)? {
            ExactValue::String(ExactString::Source(id)) => program.strings.get(id.index()),
            ExactValue::String(ExactString::Computed(value)) => Some(value),
            _ => None,
        }
    }
    /// Inspect existing exact operand knowledge in constant work, without
    /// evaluating the result, scanning payloads or adding a cache entry. Even
    /// a truncated result can retain this evidence when its inputs are known.
    /// The owning query receipt still qualifies how those inputs were found.
    pub fn string_construction(
        &self,
        program: &Program<'_>,
        operation: OpId,
    ) -> StringConstructionKnowledge {
        use StringConstructionKnowledge::{Bounded, Unknown};
        use StringConstructionUnknown as Why;
        if !self.dependencies.valid_for(program) {
            return Unknown(Why::StaleDependencies);
        }
        let unit = program.unit(self.dependencies.unit).unwrap();
        let Some(op) = unit.operations.get(operation.index()) else {
            return Unknown(Why::UnknownOperation);
        };
        if !matches!(op.kind, OperationKind::Binary(BinaryOp::Add)) {
            return Unknown(Why::UnsupportedOperation);
        }
        let Some(&[left, right]) = unit.operands(op.operands) else {
            return Unknown(Why::UnknownOperation);
        };
        let mut bound = 0u64;
        for value in [left, right] {
            let text = match self.value(value) {
                ValueKnowledge::Unknown(reason) => {
                    return Unknown(Why::OperandUnknown { value, reason });
                }
                ValueKnowledge::Exact(ExactValue::String(ExactString::Source(id))) => {
                    // The dependency check ties this source ID to its owner.
                    &program.strings[id.index()]
                }
                ValueKnowledge::Exact(ExactValue::String(ExactString::Computed(text))) => text,
                ValueKnowledge::Exact(_) => return Unknown(Why::OperandNotString(value)),
            };
            // UTF-16 units never exceed storage bytes in either StringValue
            // representation (UTF-8 or explicit u16 units). A tighter bound
            // would need shared length metadata or separately admitted work.
            let Some(next) = u64::try_from(text.storage_bytes())
                .ok()
                .and_then(|bytes| bound.checked_add(bytes))
            else {
                return Unknown(Why::LengthOverflow);
            };
            bound = next;
        }
        Bounded(BoundedStringConstruction {
            dependencies: self.dependencies,
            operation,
            operands: [left, right],
            utf16_units_upper_bound: bound,
        })
    }
    pub fn effects(&self, operation: OpId) -> EvaluationBehavior {
        self.effects
            .get(operation.index())
            .copied()
            .unwrap_or(EvaluationBehavior::UNKNOWN)
    }
    pub fn can_drop(&self, operation: OpId, demand: ObservationDemand) -> Legality {
        if demand != ObservationDemand::Discarded {
            return Legality::Unknown("result is observed");
        }
        if self.effects(operation).requires_evaluation() {
            Legality::Unknown("evaluation or initialization obligations remain")
        } else {
            Legality::PermittedUnderContext
        }
    }
    /// Conditional repeatability only: publication still checks dependencies
    /// and operand availability at each occurrence. Unlike dropping an unused
    /// allocation, duplicating a demanded identity or rereading mutable storage
    /// needs additional proof and is not authorized here.
    pub fn can_duplicate(&self, operation: OpId) -> Legality {
        if self.effects(operation).repeatable_without_observation() {
            Legality::PermittedUnderContext
        } else {
            Legality::Unknown("repeated evaluation may change observations or identity")
        }
    }
    pub fn can_speculate(&self, operation: OpId, context: SpeculationContext) -> Legality {
        if !context.operands_available {
            return Legality::Unknown("operand availability is unproved");
        }
        self.can_duplicate(operation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactRequest {
    pub attempt: AnalysisAttempt,
    /// Deterministic construction cap, independent of current cache occupancy.
    pub result_bytes: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct CacheLimits {
    pub entries: usize,
    pub bytes: u64,
    pub result_bytes: u64,
}

impl CacheLimits {
    /// Partition a target capacity between entry metadata and one fact result.
    /// If even one entry/result cannot fit, request the minimum representation;
    /// its owner must still admit it through the ledger before allocating it.
    pub fn within_budget(max_entries: usize, bytes: u64, max_result_bytes: u64) -> Self {
        let entry_bytes = size_of::<Entry>() as u64;
        let minimum_result = size_of::<UnitFacts>() as u64;
        let bytes = bytes.max(entry_bytes + minimum_result);
        let wanted_result = max_result_bytes
            .max(minimum_result)
            .min((bytes / 2).max(minimum_result))
            .min(bytes - entry_bytes);
        let entries = usize::try_from((bytes - wanted_result) / entry_bytes)
            .unwrap_or(usize::MAX)
            .min(max_entries.max(1));
        let metadata = entries as u64 * entry_bytes;
        Self {
            entries,
            bytes,
            result_bytes: max_result_bytes.max(minimum_result).min(bytes - metadata),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactsError {
    InvalidLimits,
    AllocationFailed,
    InvalidAttempt,
    UnknownUnit,
    QueryLimit,
    Budget(BudgetError),
}
impl From<BudgetError> for FactsError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    dependencies: Dependencies,
    request: FactRequest,
}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.dependencies.unit.hash(state);
        self.dependencies.unit_revision.hash(state);
        self.dependencies.tables_revision.hash(state);
        self.request.attempt.plan.hash(state);
        self.request.attempt.work_quota.hash(state);
        self.request.attempt.algorithm_version.hash(state);
        self.request.result_bytes.hash(state);
    }
}
#[derive(Debug)]
struct Entry {
    key: Key,
    facts: UnitFacts,
    receipt: AnalysisWorkReceipt,
}

/// Retains at most two attempts per unit, and also enforces global entry/byte
/// limits across arbitrary units and quotas. Entry order is deterministic FIFO.
/// This raw cache has no persistent ledger reservation. Publication must own a
/// RetainedFactsCache instead of retaining this raw cache between scoped queries.
#[derive(Debug)]
pub struct FactsCache {
    limits: CacheLimits,
    entries: Vec<Entry>,
    bytes: u64,
}
impl FactsCache {
    pub fn new(limits: CacheLimits) -> Result<Self, FactsError> {
        let metadata = Self::metadata_bytes(limits)?;
        Self::allocate(limits, metadata)
    }
    fn metadata_bytes(limits: CacheLimits) -> Result<u64, FactsError> {
        let metadata = (limits.entries as u64)
            .checked_mul(size_of::<Entry>() as u64)
            .ok_or(FactsError::InvalidLimits)?;
        if limits.entries == 0
            || limits.result_bytes < size_of::<UnitFacts>() as u64
            || metadata
                .checked_add(limits.result_bytes)
                .is_none_or(|bytes| bytes > limits.bytes)
        {
            return Err(FactsError::InvalidLimits);
        }
        Ok(metadata)
    }
    fn allocate(limits: CacheLimits, metadata: u64) -> Result<Self, FactsError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(limits.entries)
            .map_err(|_| FactsError::AllocationFailed)?;
        Ok(Self {
            limits,
            entries,
            bytes: metadata,
        })
    }
    pub fn retained_entries(&self) -> usize {
        self.entries.len()
    }
    pub fn retained_bytes(&self) -> u64 {
        self.bytes
    }
    fn remove(&mut self, index: usize) {
        let entry = self.entries.remove(index);
        self.bytes -= entry.facts.retained_bytes;
    }
    fn insert(&mut self, entry: Entry) -> Result<usize, FactsError> {
        while self
            .entries
            .iter()
            .filter(|old| old.key.dependencies.unit == entry.key.dependencies.unit)
            .count()
            >= 2
        {
            let index = self
                .entries
                .iter()
                .position(|old| old.key.dependencies.unit == entry.key.dependencies.unit)
                .unwrap();
            self.remove(index);
        }
        while !self.entries.is_empty()
            && (self.entries.len() >= self.limits.entries
                || self
                    .bytes
                    .checked_add(entry.facts.retained_bytes)
                    .is_none_or(|bytes| bytes > self.limits.bytes))
        {
            self.remove(0);
        }
        if self
            .bytes
            .checked_add(entry.facts.retained_bytes)
            .is_none_or(|bytes| bytes > self.limits.bytes)
        {
            return Err(FactsError::InvalidLimits);
        }
        self.bytes += entry.facts.retained_bytes;
        self.entries.push(entry);
        Ok(self.entries.len() - 1)
    }
}

/// Persistent cache capacity belongs to the compilation, including the time
/// between analysis sessions when other candidates may allocate memory.
///
/// The owner exposes no raw cache or clonable payload. Its sessions borrow the
/// same ledger and reserve only temporary result/history storage. Production
/// queries use publication::Compilation, which owns that ledger and prevents
/// sessions or discard from choosing another one. This standalone owner still
/// requires its original ledger. Ordinary Drop frees data but conservatively
/// leaves the reservation charged; explicit discard returns capacity.
#[must_use = "retain this cache in its compilation owner, or discard it through its original ledger"]
#[derive(Debug)]
pub struct RetainedFactsCache {
    cache: FactsCache,
    domain: WorkDomain,
    reservation: u64,
}

impl RetainedFactsCache {
    pub fn new(
        limits: CacheLimits,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, FactsError> {
        let metadata = FactsCache::metadata_bytes(limits)?;
        ledger.retain(domain, limits.bytes)?;
        let cache = match FactsCache::allocate(limits, metadata) {
            Ok(cache) => cache,
            Err(error) => {
                ledger.release(domain, limits.bytes)?;
                return Err(error);
            }
        };
        Ok(Self {
            cache,
            domain,
            reservation: limits.bytes,
        })
    }

    pub fn reserved_bytes(&self) -> u64 {
        self.reservation
    }
    pub fn retained_entries(&self) -> usize {
        self.cache.retained_entries()
    }
    pub fn retained_bytes(&self) -> u64 {
        self.cache.retained_bytes()
    }

    /// Work and scratch can belong to either domain independently of the
    /// original cache-capacity reservation. Query logic and attempt receipts
    /// are shared with the standalone scoped session.
    pub fn session<'a>(
        &'a mut self,
        ledger: &'a mut BudgetLedger,
        domain: WorkDomain,
        max_queries: usize,
    ) -> Result<FactsSession<'a>, FactsError> {
        let scratch = FactsSession::scratch_bytes(self.cache.limits, max_queries)?;
        FactsSession::admit(&mut self.cache, ledger, domain, max_queries, scratch)
    }

    pub fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let Self {
            cache,
            domain,
            reservation,
        } = self;
        drop(cache);
        ledger.release(domain, reservation)
    }
}

pub struct QueryResult<'a> {
    pub facts: &'a UnitFacts,
    pub receipt: AnalysisWorkReceipt,
    pub cache_hit: bool,
}

/// Physical fact-computation work, independent of logical attempt billing.
/// Cache dispatch/eviction scans are not included in executed_fact_steps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FactsSessionWork {
    pub queries: usize,
    pub cache_hits: usize,
    pub computations: usize,
    pub recomputations: usize,
    // One admitted logical attempt may be computed again after eviction. The
    // total of at most usize::MAX u64 receipts fits without saturation here.
    pub executed_fact_steps: u128,
}

/// Owns the ledger reservation, total query bound and unique-attempt billing
/// history for a group. Hits count against the same bound as cold queries, so
/// cache warmth cannot admit additional requests. Borrowed answers cannot
/// outlive or mutate this owner.
/// `RetainedFactsCache::session` retains cache accounting between sessions.
/// The standalone `new` route accounts for an external cache only while this
/// session lives; it does not certify that cache's later physical retention.
pub struct FactsSession<'a> {
    cache: &'a mut FactsCache,
    ledger: &'a mut BudgetLedger,
    domain: WorkDomain,
    reservation: u64,
    history: HashSet<Key>,
    max_queries: usize,
    work: FactsSessionWork,
}
impl<'a> FactsSession<'a> {
    pub fn new(
        cache: &'a mut FactsCache,
        ledger: &'a mut BudgetLedger,
        domain: WorkDomain,
        max_queries: usize,
    ) -> Result<Self, FactsError> {
        let reservation = cache
            .limits
            .bytes
            .checked_add(Self::scratch_bytes(cache.limits, max_queries)?)
            .ok_or(FactsError::InvalidLimits)?;
        Self::admit(cache, ledger, domain, max_queries, reservation)
    }

    fn scratch_bytes(limits: CacheLimits, max_queries: usize) -> Result<u64, FactsError> {
        // Reserve a conservative bound for table capacity rounding/control
        // bytes before allocation. Hash iteration never determines scheduling.
        // Lookup is per unit request, not per operation or operand.
        let history = (max_queries as u64)
            .checked_mul((size_of::<Key>() as u64 + 1) * 4)
            .and_then(|bytes| bytes.checked_add(size_of::<HashSet<Key>>() as u64))
            .ok_or(FactsError::InvalidLimits)?;
        limits
            .result_bytes
            .checked_add(history)
            .ok_or(FactsError::InvalidLimits)
    }

    fn admit(
        cache: &'a mut FactsCache,
        ledger: &'a mut BudgetLedger,
        domain: WorkDomain,
        max_queries: usize,
        reservation: u64,
    ) -> Result<Self, FactsError> {
        ledger.retain(domain, reservation)?;
        let mut history = HashSet::new();
        if history.try_reserve(max_queries).is_err() {
            ledger.release(domain, reservation)?;
            return Err(FactsError::AllocationFailed);
        }
        Ok(Self {
            cache,
            ledger,
            domain,
            reservation,
            history,
            max_queries,
            work: FactsSessionWork::default(),
        })
    }

    pub fn work(&self) -> FactsSessionWork {
        self.work
    }

    fn charge_attempt(
        &mut self,
        key: Key,
        receipt: AnalysisWorkReceipt,
        already_charged: bool,
    ) -> Result<(), FactsError> {
        if !already_charged {
            self.ledger
                .charge_analysis(self.domain, key.request.attempt, receipt)?;
            self.history.insert(key);
        }
        Ok(())
    }

    pub fn query(
        &mut self,
        program: &Program<'_>,
        unit: UnitId,
        request: FactRequest,
    ) -> Result<QueryResult<'_>, FactsError> {
        // Bounding only distinct keys would permit unbounded A,B,A,B...
        // recomputation in a one-entry cache after both keys had been billed.
        // Count hits and invalid requests too; a limit rejection does no work.
        if self.work.queries == self.max_queries {
            return Err(FactsError::QueryLimit);
        }
        self.work.queries += 1;
        if request.attempt.plan != LOCAL_FACTS_PLAN
            || request.attempt.algorithm_version != LOCAL_FACTS_VERSION
            || request.result_bytes < size_of::<UnitFacts>() as u64
            || request.result_bytes > self.cache.limits.result_bytes
        {
            return Err(FactsError::InvalidAttempt);
        }
        let frozen = program
            .units
            .get(unit.index())
            .filter(|frozen| frozen.id() == unit)
            .ok_or(FactsError::UnknownUnit)?;
        let key = Key {
            dependencies: Dependencies {
                unit,
                unit_revision: frozen.revision(),
                tables_revision: program.tables_revision,
            },
            request,
        };
        let already_charged = self.history.contains(&key);
        // Admit the deterministic maximum work before doing cold physical
        // computation. Warm attempts take the same admission path. The actual
        // receipt is charged once below; an exhausted ledger never triggers a
        // large uncharged analysis merely to discover that it cannot pay.
        if !already_charged {
            let mut admission = self.ledger.clone();
            admission.charge_analysis(
                self.domain,
                request.attempt,
                AnalysisWorkReceipt {
                    attempt: request.attempt,
                    completion: AnalysisCompletion::Truncated,
                    logical_work: request.attempt.work_quota,
                },
            )?;
        }
        let hit = self.cache.entries.iter().position(|entry| entry.key == key);
        let index = if let Some(index) = hit {
            self.work.cache_hits += 1;
            self.charge_attempt(key, self.cache.entries[index].receipt, already_charged)?;
            index
        } else {
            self.work.computations += 1;
            self.work.recomputations += usize::from(already_charged);
            let (facts, receipt) = compute(program, frozen.data(), key);
            self.work.executed_fact_steps += u128::from(receipt.logical_work);
            // Computation has happened even if retaining its answer fails.
            // Admission above makes its receipt payable before cache mutation.
            self.charge_attempt(key, receipt, already_charged)?;
            self.cache.insert(Entry {
                key,
                facts,
                receipt,
            })?
        };
        let entry = &self.cache.entries[index];
        Ok(QueryResult {
            facts: &entry.facts,
            receipt: entry.receipt,
            cache_hit: hit.is_some(),
        })
    }
}
impl Drop for FactsSession<'_> {
    fn drop(&mut self) {
        self.ledger
            .release(self.domain, self.reservation)
            .expect("facts session owns its memory reservation");
    }
}

struct Work {
    quota: u64,
    used: u64,
    result_limit: u64,
    result_used: u64,
    truncated: bool,
}
impl Work {
    fn charge(&mut self, amount: u64) -> bool {
        let Some(next) = self
            .used
            .checked_add(amount)
            .filter(|used| *used <= self.quota)
        else {
            self.truncated = true;
            return false;
        };
        self.used = next;
        true
    }
    fn allocation_fits(&mut self, bound: u64) -> bool {
        let Some(next) = self
            .result_used
            .checked_add(bound)
            .filter(|used| *used <= self.result_limit)
        else {
            self.truncated = true;
            return false;
        };
        let _ = next;
        true
    }
}

fn compute(program: &Program<'_>, unit: &UnitData, key: Key) -> (UnitFacts, AnalysisWorkReceipt) {
    let mut work = Work {
        quota: key.request.attempt.work_quota,
        used: 0,
        result_limit: key.request.result_bytes,
        result_used: size_of::<UnitFacts>() as u64,
        truncated: false,
    };
    let mut facts = UnitFacts {
        dependencies: key.dependencies,
        values: Vec::new(),
        effects: Vec::new(),
        primitive_domains: Vec::new(),
        retained_bytes: work.result_used,
    };
    let array_bytes = (unit.values.len() as u64)
        .checked_mul((size_of::<StoredKnowledge>() + size_of::<bool>()) as u64)
        .and_then(|bytes| {
            (unit.operations.len() as u64)
                .checked_mul(size_of::<EvaluationBehavior>() as u64)
                .and_then(|effects| bytes.checked_add(effects))
        });
    let array_work = (unit.values.len() as u64)
        .checked_mul(2)
        .and_then(|values| values.checked_add(unit.operations.len() as u64));
    if let (Some(bytes), Some(charge)) = (array_bytes, array_work) {
        if work.allocation_fits(bytes) && work.charge(charge) {
            work.result_used += bytes;
            facts.values =
                vec![StoredKnowledge::Unknown(UnknownReason::Unvisited); unit.values.len()];
            facts.effects = vec![EvaluationBehavior::UNKNOWN; unit.operations.len()];
            facts.primitive_domains = vec![false; unit.values.len()];
            // One linear pass, charged like the main one: which never
            // reassigned locals this unit initializes, and with what.
            let initializers = if work.charge(unit.operations.len() as u64) {
                unassigned_local_initializers(program, unit)
            } else {
                AHashMap::default()
            };
            let initialized_loads = if work.charge(unit.operations.len() as u64) {
                initialized_local_loads(program, unit)
            } else {
                vec![false; unit.operations.len()]
            };
            for (index, operation) in unit.operations.iter().enumerate() {
                if !work.charge(1) {
                    break;
                }
                facts.effects[index] = behavior(program, unit, operation, &facts);
                // A plain local read's only failure is the temporal dead zone,
                // which an earlier `Initialize` in the same region excludes.
                if initialized_loads[index] {
                    if let EvaluationBehavior {
                        reads: MemoryAccess::Cell(_),
                        may_throw: true,
                        ..
                    } = facts.effects[index]
                    {
                        let read = facts.effects[index].reads;
                        if facts.effects[index]
                            == (EvaluationBehavior {
                                reads: read,
                                may_throw: true,
                                ..EvaluationBehavior::TOTAL
                            })
                        {
                            facts.effects[index].may_throw = false;
                        }
                    }
                }
                if let Some(result) = operation.result {
                    facts.primitive_domains[result.index()] = primitive_result_domain_with_locals(
                        program,
                        unit,
                        operation,
                        &facts.primitive_domains,
                        &initializers,
                    );
                }
                if let Some(result) = operation.result {
                    let knowledge = exact(program, unit, operation, &facts.values, &mut work);
                    facts.values[result.index()] = knowledge;
                }
            }
        }
    } else {
        work.truncated = true;
    }
    facts.retained_bytes = work.result_used;
    let receipt = AnalysisWorkReceipt {
        attempt: key.request.attempt,
        completion: if work.truncated {
            AnalysisCompletion::Truncated
        } else {
            AnalysisCompletion::Complete
        },
        logical_work: work.used,
    };
    (facts, receipt)
}

fn behavior(
    program: &Program<'_>,
    unit: &UnitData,
    operation: &Operation,
    facts: &UnitFacts,
) -> EvaluationBehavior {
    if matches!(
        operation.kind,
        OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::StringLength))
    ) && unit
        .operands(operation.operands)
        .unwrap()
        .first()
        .is_some_and(|value| {
            matches!(
                facts.values.get(value.index()),
                Some(StoredKnowledge::Exact(StoredExact::String(_)))
            )
        })
    {
        return EvaluationBehavior::TOTAL;
    }
    operation_evaluation_behavior(program, unit, operation, &facts.primitive_domains)
}

/// Shared conservative effect transfer for a checked semantic operation. The
/// supplied unit-local slice proves primitive value domains at this context;
/// a missing entry means unknown. This reads only operation/operand metadata,
/// and neither evaluates exact values nor creates a cache/session. The caller
/// admits its domain storage and charges operation/operand traversal work.
/// Initialization, child-region completion and host assumptions remain the
/// caller's separate obligations, never inferred from a refined result type.
pub(super) fn operation_evaluation_behavior(
    program: &Program<'_>,
    unit: &UnitData,
    operation: &Operation,
    primitive_domains: &[bool],
) -> EvaluationBehavior {
    if matches!(
        operation.kind,
        OperationKind::IntBinary(_) | OperationKind::Unary { .. } | OperationKind::Binary(_)
    ) {
        return primitive_evaluation_behavior(
            program,
            unit,
            operation,
            primitive_inputs(unit, operation, primitive_domains),
        )
        .expect("primitive operation contract");
    }
    use OperationKind as Op;
    match &operation.kind {
        Op::Constant(_) => EvaluationBehavior::TOTAL,
        Op::IsUndefined => EvaluationBehavior::TOTAL,
        // `typeof` never throws; `Array.isArray` throws on a revoked proxy.
        Op::TypeTest(target) => {
            match crate::primitive::runtime_type_test(&program.types[target.index()]) {
                Some(crate::primitive::RuntimeTypeTest::TypeOf(_)) => EvaluationBehavior::TOTAL,
                _ => EvaluationBehavior {
                    may_throw: true,
                    ..EvaluationBehavior::TOTAL
                },
            }
        }
        // Building the string can exhaust memory; converting a non-primitive
        // operand can run user code or throw (a Symbol).
        Op::Template => {
            if primitive_inputs(unit, operation, primitive_domains) {
                EvaluationBehavior {
                    may_exhaust_resources: true,
                    ..EvaluationBehavior::TOTAL
                }
            } else {
                EvaluationBehavior::COERCION
            }
        }
        // Logical value transfer never invokes a conversion or reads host
        // fields. Struct value fields become independent; reference fields
        // retain their handles. A representation may copy immutable backing,
        // but that introduces no language reference identity. Wrapper/generic
        // cases conservatively retain possible resource cost without walking
        // nested type payloads in this constant-time transfer.
        Op::CopyValue => EvaluationBehavior {
            may_exhaust_resources: operation.result.is_none_or(|value| {
                matches!(
                    program.types[unit.values[value.index()].ty.index()],
                    Type::Struct(_)
                        | Type::StructInstance { .. }
                        | Type::Nullable(_)
                        | Type::Union(_)
                        | Type::TypeParameter(_)
                )
            }),
            ..EvaluationBehavior::TOTAL
        },
        // This validates an address, never loads/coerces the leaf value.
        // Presence/TDZ and product-domain evidence are separate obligations;
        // keep access effects conservative without an uncharged path walk.
        Op::CheckPlace(_) => EvaluationBehavior::UNKNOWN,
        Op::Load(place) => match unit.places[place.index()] {
            Place::Value(_) => EvaluationBehavior::TOTAL,
            Place::Cell(cell)
                if program.cells[cell.index()].binding != CellBinding::Foreign
                    && !program.is_reference_parameter(cell) =>
            {
                EvaluationBehavior {
                    reads: MemoryAccess::Cell(cell),
                    may_throw: true,
                    ..EvaluationBehavior::TOTAL
                }
            }
            _ => EvaluationBehavior::UNKNOWN,
        },
        Op::Store(place) => match unit.places[place.index()] {
            Place::Cell(cell)
                if program.cells[cell.index()].binding != CellBinding::Foreign
                    && !program.is_reference_parameter(cell) =>
            {
                EvaluationBehavior {
                    writes: MemoryAccess::Cell(cell),
                    may_throw: true,
                    ..EvaluationBehavior::TOTAL
                }
            }
            _ => EvaluationBehavior::UNKNOWN,
        },
        Op::Initialize(cell) => EvaluationBehavior {
            writes: MemoryAccess::Cell(*cell),
            ..EvaluationBehavior::TOTAL
        },
        Op::PrepareCall(call)
            if matches!(unit.calls[call.index()].target, CallTarget::Value { .. }) =>
        {
            EvaluationBehavior::TOTAL
        }
        // `JS.object(...)`, `JS.array(...)` and `JS.undefined()` print as
        // literals, their operands separately scheduled: defining data
        // properties on a fresh object neither throws nor runs user code.
        Op::PrepareCall(call) | Op::Call(call)
            if matches!(
                unit.calls[call.index()].target,
                CallTarget::Builtin(
                    BuiltinCall::JsObject | BuiltinCall::JsArray | BuiltinCall::JsUndefined
                )
            ) =>
        {
            if matches!(operation.kind, Op::Call(_))
                && !matches!(
                    unit.calls[call.index()].target,
                    CallTarget::Builtin(BuiltinCall::JsUndefined)
                )
            {
                EvaluationBehavior {
                    may_exhaust_resources: true,
                    creates_identity: true,
                    ..EvaluationBehavior::TOTAL
                }
            } else {
                EvaluationBehavior::TOTAL
            }
        }
        Op::Closure(_)
        | Op::Allocate {
            kind: AllocationKind::Array | AllocationKind::Record(_) | AllocationKind::Object(_),
            ..
        } => EvaluationBehavior {
            may_exhaust_resources: true,
            creates_identity: true,
            ..EvaluationBehavior::TOTAL
        },
        // Constructor operands remain separately scheduled. A logical struct
        // value has neither a constructor hook nor observable reference identity.
        Op::Allocate {
            kind: AllocationKind::Struct(_),
            ..
        } => EvaluationBehavior {
            may_exhaust_resources: true,
            ..EvaluationBehavior::TOTAL
        },
        Op::Return | Op::Break | Op::Continue => EvaluationBehavior {
            transfers_control: true,
            ..EvaluationBehavior::TOTAL
        },
        Op::Throw => EvaluationBehavior {
            may_throw: true,
            transfers_control: true,
            ..EvaluationBehavior::TOTAL
        },
        // Child regions and calls need completion/resource summaries before
        // this query can prove them removable. A loop may diverge even when no
        // individual operation writes, and an apparently simple getter reenters.
        _ => EvaluationBehavior::UNKNOWN,
    }
}

/// Shared primitive operation contract. The caller supplies a proved operand
/// domain; this assessment does not establish initialization, placement, or
/// publication authority. Contextual clients can refine domains without
/// duplicating arithmetic and string-construction effect rules.
pub(super) fn primitive_evaluation_behavior(
    program: &Program<'_>,
    unit: &UnitData,
    operation: &Operation,
    operands_primitive: bool,
) -> Option<EvaluationBehavior> {
    Some(match &operation.kind {
        OperationKind::Unary {
            op: UnaryOp::Not, ..
        } => EvaluationBehavior::TOTAL,
        OperationKind::IntBinary(_) | OperationKind::Unary { .. } => {
            if operands_primitive {
                EvaluationBehavior::TOTAL
            } else {
                EvaluationBehavior::COERCION
            }
        }
        OperationKind::Binary(op) => {
            if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Nullish) {
                return Some(EvaluationBehavior::TOTAL);
            }
            if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                // `==` on a `JsValue` is JavaScript's abstract equality: an
                // object compared with anything but `null`/`undefined` is
                // converted to a primitive, which can run user code. Typed
                // operands and proved primitives never convert.
                let ty = |value: ValueId| &program.types[unit.values[value.index()].ty.index()];
                let operands = unit.operands(operation.operands).unwrap_or(&[]);
                let dynamic = operands
                    .iter()
                    .any(|&value| matches!(ty(value), Type::TypeParameter("$js")));
                let against_null = operands
                    .iter()
                    .any(|&value| matches!(ty(value), Type::Null));
                return Some(if dynamic && !against_null && !operands_primitive {
                    EvaluationBehavior::COERCION
                } else {
                    EvaluationBehavior::TOTAL
                });
            }
            if !operands_primitive {
                return Some(EvaluationBehavior::COERCION);
            }
            let string_add = *op == BinaryOp::Add
                && operation.result.is_some_and(|value| {
                    matches!(
                        program.types[unit.values[value.index()].ty.index()],
                        Type::String
                    )
                });
            // Proved primitive inputs cannot invoke conversion hooks. The
            // language excludes implementation string-cap/OOM failures, while
            // construction cost remains explicit independently of exact value
            // knowledge or an implementation's supported string-size bound.
            EvaluationBehavior {
                may_exhaust_resources: string_add,
                ..EvaluationBehavior::TOTAL
            }
        }
        _ => return None,
    })
}

fn primitive_inputs(unit: &UnitData, operation: &Operation, domains: &[bool]) -> bool {
    unit.operands(operation.operands)
        .unwrap()
        .iter()
        .all(|value| domains.get(value.index()) == Some(&true))
}
/// Initializer operands of each local this unit initializes and never
/// reassigns. For such a cell every write is an `Initialize` here, so a load
/// can only observe one of these values. Empty when the unit passes storage
/// by reference anywhere: a callee could then write through it.
pub(super) fn unassigned_local_initializers(
    program: &Program<'_>,
    unit: &UnitData,
) -> AHashMap<CellId, Vec<ValueId>> {
    let mut initializers = AHashMap::<CellId, Vec<ValueId>>::default();
    if unit
        .operations
        .iter()
        .any(|operation| matches!(operation.kind, OperationKind::PrepareReference { .. }))
    {
        return initializers;
    }
    for operation in &unit.operations {
        let OperationKind::Initialize(cell) = operation.kind else {
            continue;
        };
        let Some(entry) = program.cells.get(cell.index()) else {
            continue;
        };
        if entry.binding != CellBinding::Local || entry.assigned {
            continue;
        }
        if let Some(&operand) = unit
            .operands(operation.operands)
            .and_then(|operands| operands.first())
        {
            initializers.entry(cell).or_default().push(operand);
        }
    }
    initializers
}

/// Loads of a local cell that the same region already initialized. Within one
/// region operations run in list order, and control only moves forward or
/// leaves the region, so such a load cannot run before its `Initialize` and
/// cannot observe the temporal dead zone. Loads in nested regions are not
/// marked; that needs a dominance proof this pass does not make.
pub(super) fn initialized_local_loads(program: &Program<'_>, unit: &UnitData) -> Vec<bool> {
    let mut marks = vec![false; unit.operations.len()];
    let mut initialized = Vec::<CellId>::new();
    for region in &unit.regions {
        initialized.clear();
        for &operation in &region.operations {
            let Some(entry) = unit.operations.get(operation.index()) else {
                continue;
            };
            match entry.kind {
                OperationKind::Initialize(cell) => initialized.push(cell),
                OperationKind::Load(place) => {
                    if let Some(Place::Cell(cell)) = unit.places.get(place.index()) {
                        if initialized.contains(cell)
                            && program
                                .cells
                                .get(cell.index())
                                .is_some_and(|entry| entry.binding == CellBinding::Local)
                        {
                            marks[operation.index()] = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    marks
}

/// `primitive_result_domain`, plus loads of never-reassigned locals whose
/// every initializer is already primitive in `domains`. A value not yet
/// visited counts as not primitive, so the refinement only ever upgrades a
/// load its initializers already justify.
pub(super) fn primitive_result_domain_with_locals(
    program: &Program<'_>,
    unit: &UnitData,
    operation: &Operation,
    domains: &[bool],
    initializers: &AHashMap<CellId, Vec<ValueId>>,
) -> bool {
    if let OperationKind::Load(place) = operation.kind {
        if let Some(Place::Cell(cell)) = unit.places.get(place.index()) {
            if let Some(values) = initializers.get(cell) {
                return !values.is_empty()
                    && values
                        .iter()
                        .all(|value| domains.get(value.index()) == Some(&true));
            }
        }
    }
    primitive_result_domain(program, unit, operation, domains)
}

/// Common result-domain transfer over a checked unit. A primitive domain
/// describes a value if evaluation completes; it does not make the producer
/// total or prove a mutable/refined load initialized. Work/storage admission
/// belongs to the client as for `operation_evaluation_behavior`.
pub(super) fn primitive_result_domain(
    _program: &Program<'_>,
    unit: &UnitData,
    operation: &Operation,
    domains: &[bool],
) -> bool {
    match operation.kind {
        OperationKind::Constant(_)
        | OperationKind::IntBinary(_)
        | OperationKind::Unary {
            op: UnaryOp::Not, ..
        }
        | OperationKind::Unary { integer: true, .. }
        | OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::StringLength)) => true,
        OperationKind::CopyValue | OperationKind::Unary { .. } | OperationKind::Binary(_) => {
            primitive_inputs(unit, operation, domains)
        }
        // Source annotations are language knowledge, not a proof of the raw
        // value in a JS cell. Parameters, host calls and replaceable methods
        // can introduce values whose conversion invokes user code. Proving
        // storage contents requires all producer edges and their revision
        // dependencies; this unit-local query owns neither that evidence nor
        // a runtime normalization at the load.
        OperationKind::Load(place) => match unit.places[place.index()] {
            Place::Value(value) => domains.get(value.index()) == Some(&true),
            _ => false,
        },
        _ => false,
    }
}

fn exact(
    program: &Program<'_>,
    unit: &UnitData,
    operation: &Operation,
    values: &[StoredKnowledge],
    work: &mut Work,
) -> StoredKnowledge {
    let known = |id: ValueId| match values.get(id.index()) {
        Some(StoredKnowledge::Exact(value)) => Some(value),
        _ => None,
    };
    let operands = unit.operands(operation.operands).unwrap();
    let unknown = || StoredKnowledge::Unknown(UnknownReason::UnknownOperand);
    let value = match &operation.kind {
        OperationKind::Constant(constant) => match constant {
            Constant::Integer(value) => StoredExact::Integer(*value),
            Constant::Number(value) => StoredExact::Number(*value),
            Constant::Boolean(value) => StoredExact::Boolean(*value),
            Constant::Null => StoredExact::Null,
            Constant::Undefined => StoredExact::Undefined,
            Constant::String(value) => StoredExact::String(StoredString::Source(*value)),
        },
        // Exact knowledge here contains only primitive values. Sharing a
        // computed string's immutable cache payload selects no target recipe
        // and allocates no second payload; aggregate/unknown copies stay unknown.
        OperationKind::CopyValue => match known(operands[0]) {
            Some(value) => value.clone(),
            None => return unknown(),
        },
        OperationKind::IntBinary(op) => {
            let (Some(StoredExact::Integer(left)), Some(StoredExact::Integer(right))) =
                (known(operands[0]), known(operands[1]))
            else {
                return unknown();
            };
            StoredExact::Integer(op.evaluate(*left, *right))
        }
        OperationKind::Unary { op, integer } => match (op, integer, known(operands[0])) {
            (UnaryOp::Neg, true, Some(StoredExact::Integer(value))) => {
                StoredExact::Integer(value.wrapping_neg())
            }
            (UnaryOp::Neg, false, Some(value)) if number(value).is_some() => {
                StoredExact::Number((-number(value).unwrap()).to_bits())
            }
            (UnaryOp::Not, _, Some(StoredExact::Boolean(value))) => StoredExact::Boolean(!value),
            _ => return unknown(),
        },
        OperationKind::Binary(op) => {
            let (Some(left), Some(right)) = (known(operands[0]), known(operands[1])) else {
                return unknown();
            };
            if *op == BinaryOp::Add {
                if let (Some(left), Some(right)) = (string(program, left), string(program, right)) {
                    // Count/copy/canonicalization can expand UTF-8/UTF-16. This
                    // checked conservative bound covers scratch AND retained
                    // payload before concat allocates or scans either string.
                    let bound = (left.storage_bytes() as u64)
                        .checked_add(right.storage_bytes() as u64)
                        .and_then(|bytes| bytes.checked_mul(8))
                        .and_then(|bytes| bytes.checked_add(256));
                    let Some(bound) = bound else {
                        work.truncated = true;
                        return StoredKnowledge::Unknown(UnknownReason::MemoryLimit);
                    };
                    if !work.allocation_fits(bound) {
                        return StoredKnowledge::Unknown(UnknownReason::MemoryLimit);
                    }
                    if !work.charge(bound) {
                        return StoredKnowledge::Unknown(UnknownReason::WorkLimit);
                    }
                    work.result_used += bound;
                    return StoredKnowledge::Exact(StoredExact::String(StoredString::Computed(
                        Arc::new(left.concat(right)),
                    )));
                }
            }
            let Some(value) = binary_exact(*op, left, right) else {
                return StoredKnowledge::Unknown(UnknownReason::UnsupportedOperation);
            };
            value
        }
        OperationKind::Select { yes, no } => {
            let branch = match known(operands[0]) {
                Some(StoredExact::Boolean(true)) => yes,
                Some(StoredExact::Boolean(false)) => no,
                _ => return unknown(),
            };
            let Some(value) = unit.regions[branch.index()].result.and_then(known) else {
                return unknown();
            };
            value.clone()
        }
        OperationKind::ShortCircuit { kind, right } => {
            let Some(left) = known(operands[0]) else {
                return unknown();
            };
            let take_right = match kind {
                ShortCircuit::Nullish => matches!(left, StoredExact::Null | StoredExact::Undefined),
                ShortCircuit::BooleanAnd | ShortCircuit::JavaScriptAnd => truthy(program, left),
                ShortCircuit::BooleanOr | ShortCircuit::JavaScriptOr => !truthy(program, left),
            };
            if take_right {
                let Some(value) = unit.regions[right.index()].result.and_then(known) else {
                    return unknown();
                };
                value.clone()
            } else {
                left.clone()
            }
        }
        OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::StringLength)) => {
            let Some(value) = known(operands[0]).and_then(|value| string(program, value)) else {
                return unknown();
            };
            if !work.charge(value.storage_bytes() as u64) {
                return StoredKnowledge::Unknown(UnknownReason::WorkLimit);
            }
            StoredExact::Integer(value.code_units().count() as i32)
        }
        OperationKind::Load(_) => {
            return StoredKnowledge::Unknown(UnknownReason::MutableOrHostValue);
        }
        _ => return StoredKnowledge::Unknown(UnknownReason::UnsupportedOperation),
    };
    StoredKnowledge::Exact(value)
}

fn string<'a>(program: &'a Program<'_>, value: &'a StoredExact) -> Option<&'a StringValue> {
    match value {
        StoredExact::String(StoredString::Source(id)) => program.strings.get(id.index()),
        StoredExact::String(StoredString::Computed(value)) => Some(value),
        _ => None,
    }
}
fn number(value: &StoredExact) -> Option<f64> {
    match value {
        StoredExact::Integer(value) => Some(f64::from(*value)),
        StoredExact::Number(bits) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}
fn truthy(program: &Program<'_>, value: &StoredExact) -> bool {
    match value {
        StoredExact::Integer(value) => *value != 0,
        StoredExact::Number(bits) => {
            let value = f64::from_bits(*bits);
            value != 0.0 && !value.is_nan()
        }
        StoredExact::Boolean(value) => *value,
        StoredExact::Null | StoredExact::Undefined => false,
        StoredExact::String(_) => !string(program, value).unwrap().is_empty(),
    }
}
fn binary_exact(op: BinaryOp, left: &StoredExact, right: &StoredExact) -> Option<StoredExact> {
    if let (StoredExact::Integer(left), StoredExact::Integer(right)) = (left, right) {
        let value = match op {
            BinaryOp::BitAnd => Some(*left & *right),
            BinaryOp::BitOr => Some(*left | *right),
            BinaryOp::Xor => Some(*left ^ *right),
            BinaryOp::ShiftLeft => Some(left.wrapping_shl(*right as u32 & 31)),
            BinaryOp::ShiftRight => Some(left.wrapping_shr(*right as u32 & 31)),
            _ => None,
        };
        if let Some(value) = value {
            return Some(StoredExact::Integer(value));
        }
    }
    if let (Some(left), Some(right)) = (number(left), number(right)) {
        return Some(match op {
            BinaryOp::Add => StoredExact::Number((left + right).to_bits()),
            BinaryOp::Sub => StoredExact::Number((left - right).to_bits()),
            BinaryOp::Mul => StoredExact::Number((left * right).to_bits()),
            BinaryOp::Div => StoredExact::Number((left / right).to_bits()),
            BinaryOp::Mod => StoredExact::Number((left % right).to_bits()),
            BinaryOp::Eq => StoredExact::Boolean(left == right),
            BinaryOp::NotEq => StoredExact::Boolean(left != right),
            BinaryOp::Less => StoredExact::Boolean(left < right),
            BinaryOp::LessEq => StoredExact::Boolean(left <= right),
            BinaryOp::Greater => StoredExact::Boolean(left > right),
            BinaryOp::GreaterEq => StoredExact::Boolean(left >= right),
            _ => return None,
        });
    }
    if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
        let equal = match (left, right) {
            (StoredExact::Boolean(a), StoredExact::Boolean(b)) => a == b,
            (StoredExact::Null, StoredExact::Null)
            | (StoredExact::Undefined, StoredExact::Undefined) => true,
            // Other string equality requires a separately charged scan.
            (
                StoredExact::String(StoredString::Source(a)),
                StoredExact::String(StoredString::Source(b)),
            ) if a == b => true,
            _ => return None,
        };
        return Some(StoredExact::Boolean(if op == BinaryOp::Eq {
            equal
        } else {
            !equal
        }));
    }
    None
}
