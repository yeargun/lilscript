//! Complete closed value-product copies and raw access witnesses.
//!
//! The transient subject worklist follows the existing UseIndex and semantic
//! producers. Sealed seedless recursive input cycles are conditionally safe
//! because no invocation can enter them; this proves no reachability or new
//! allocation. Every opaque entry or writer rejects the closure.
//! It retains no second value/alias graph. A layout is a fixed cut at
//! the nominal's top-level fields; nested products remain packed values.
use super::activation::StructuredDominance;
use super::callable_inputs::{CallObservations, CallableInputs, InputOutcome, InputScope};
use super::raw_domains::Admission;
use super::record_family::OpRef;
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, AnalysisWorkReceipt, BudgetError, BudgetLedger, WorkDomain,
};
use crate::output_budget::{AllocationError, VectorLayout};
use std::mem::size_of;

pub(super) const PRODUCT_FAMILY_PLAN: u32 = 6;
pub(super) const PRODUCT_FAMILY_VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProductOrigin {
    Initialize(OpRef),
    Parameter,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProductCell {
    pub cell: CellId,
    pub origin: ProductOrigin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProductAccessRoot {
    Cell(CellId),
    Value(ValueId),
}
/// Construction is private: this is an exact raw-access/presence witness,
/// never a proof that a field payload or its integer conversion is harmless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProductAccess {
    place: PlaceId,
    root: ProductAccessRoot,
    slot: u32,
}
impl ProductAccess {
    pub(super) fn place(&self) -> PlaceId {
        self.place
    }
    pub(super) fn root(&self) -> ProductAccessRoot {
        self.root
    }
    pub(super) fn slot(&self) -> u32 {
        self.slot
    }
}
/// Body-wide evidence for an original reference-formal operation.  The private
/// constructor follows every incoming creator and writer; this is neither a
/// selected-bank recipe nor a primitive-domain/non-aliasing certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReferenceAccess {
    place: PlaceId,
    parameter: CellId,
}
impl ReferenceAccess {
    pub(super) fn place(&self) -> PlaceId {
        self.place
    }
    pub(super) fn parameter(&self) -> CellId {
        self.parameter
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProductOperationKind {
    Construct {
        value: ValueId,
    },
    Snapshot {
        cell: CellId,
        value: ValueId,
    },
    Copy {
        input: ValueId,
        value: ValueId,
    },
    Initialize {
        cell: CellId,
        input: ValueId,
    },
    /// Effect-only whole storage replacement; expression values reuse the RHS.
    Assign {
        cell: CellId,
        input: ValueId,
    },
    Access(ProductAccess),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProductOperation {
    pub operation: OpRef,
    pub kind: ProductOperationKind,
}
#[derive(Debug)]
pub(super) struct ProductDependencies {
    tables: RevisionId,
    units: Vec<(UnitId, RevisionId)>,
    cells: Vec<(CellId, RevisionId)>,
    creators: Vec<(UnitId, RevisionId)>,
}
impl ProductDependencies {
    pub(super) fn units(&self) -> &[(UnitId, RevisionId)] {
        &self.units
    }
    pub(super) fn cells(&self) -> &[(CellId, RevisionId)] {
        &self.cells
    }
    pub(super) fn creators(&self) -> &[(UnitId, RevisionId)] {
        &self.creators
    }
    pub(super) fn validation_work(&self) -> Option<u64> {
        (self.units.len() as u64)
            .checked_add(self.cells.len() as u64)?
            .checked_add(self.creators.len() as u64)?
            .checked_add(1)
    }
    pub(super) fn valid_for(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        uses.valid_for(program) && self.valid_for_published(program, uses)
    }
    pub(super) fn valid_for_published(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        self.tables == program.tables_revision
            && self.tables == uses.tables_revision()
            && self.units.iter().all(|(id, revision)| {
                program
                    .units
                    .get(id.index())
                    .is_some_and(|unit| unit.revision() == *revision)
            })
            && self.cells.iter().all(|(id, revision)| {
                uses.cell(*id)
                    .is_some_and(|cell| cell.revision() == *revision)
            })
            && self.creators.iter().all(|(id, revision)| {
                uses.creators(*id)
                    .is_some_and(|creators| creators.revision() == *revision)
            })
    }
}
#[must_use = "retain in Compilation or discard through the original ledger"]
#[derive(Debug)]
pub(super) struct ProductFamily {
    root: CellId,
    schema: NominalId,
    cells: Vec<ProductCell>,
    fields: Vec<NominalMemberId>,
    operations: Vec<ProductOperation>,
    dependencies: ProductDependencies,
    requires_module: bool,
    charge: (WorkDomain, u64),
}
impl ProductFamily {
    pub(super) fn root(&self) -> CellId {
        self.root
    }
    pub(super) fn schema(&self) -> NominalId {
        self.schema
    }
    pub(super) fn cells(&self) -> &[ProductCell] {
        &self.cells
    }
    pub(super) fn fields(&self) -> &[NominalMemberId] {
        &self.fields
    }
    pub(super) fn requires_module(&self) -> bool {
        self.requires_module
    }
    pub(super) fn operations(&self) -> &[ProductOperation] {
        &self.operations
    }
    /// Callers admit lookup work as part of their original operation scan.
    pub(super) fn access(&self, operation: OpRef) -> Option<&ProductAccess> {
        let index = self
            .operations
            .binary_search_by_key(
                &(operation.unit.index(), operation.operation.index()),
                |entry| {
                    (
                        entry.operation.unit.index(),
                        entry.operation.operation.index(),
                    )
                },
            )
            .ok()?;
        match &self.operations[index].kind {
            ProductOperationKind::Access(access) => Some(access),
            _ => None,
        }
    }
    pub(super) fn dependencies(&self) -> &ProductDependencies {
        &self.dependencies
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        self.charge.1
    }
    pub(super) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let (domain, bytes) = self.charge;
        drop(self);
        ledger.release(domain, bytes)
    }
}
#[derive(Debug, Clone, Copy)]
pub(super) struct FamilyRequest {
    pub execution: JavaScriptExecution,
    pub attempt: AnalysisAttempt,
    pub scratch_bytes: u64,
    pub output_bytes: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    NotLocalProduct,
    UnsupportedSchema,
    Initialization,
    CrossActivationCopy,
    WholeValueUse,
    ReferenceExposure,
    UnsupportedProducer,
    UnsupportedProjection,
    EarlyCaptureOrRead,
    UnrootedCapture,
    CallableInterface,
    ExecutionBoundary,
    ArgumentsObservation,
    NoProductParameters,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    Work,
    Scratch,
    Output,
}
#[derive(Debug)]
pub(super) enum Outcome<T> {
    Complete(T),
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
}
#[derive(Debug)]
pub(super) struct Analysis<T> {
    pub outcome: Outcome<T>,
    pub receipt: AnalysisWorkReceipt,
}
pub(super) type FamilyOutcome = Outcome<ProductFamily>;
pub(super) type FamilyAnalysis = Analysis<ProductFamily>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FamilyError {
    InvalidAttempt,
    InvalidProgram(&'static str),
    StaleUseIndex,
    Capacity,
    AllocationFailed,
    Budget(BudgetError),
}
impl From<BudgetError> for FamilyError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
pub(super) enum Stop {
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
    Error(FamilyError),
}
impl From<FamilyError> for Stop {
    fn from(error: FamilyError) -> Self {
        Self::Error(error)
    }
}
pub(super) type ResultIn<T> = Result<T, Stop>;
pub(super) fn invalid(message: &'static str) -> Stop {
    Stop::Error(FamilyError::InvalidProgram(message))
}
pub(super) fn unknown(reason: UnknownReason) -> Stop {
    Stop::Unknown(reason)
}
fn vector_error(error: AllocationError) -> Stop {
    Stop::Error(match error {
        AllocationError::Budget(error) => FamilyError::Budget(error),
        AllocationError::AllocationFailed => FamilyError::AllocationFailed,
        _ => FamilyError::Capacity,
    })
}
pub(super) struct Attempt<'a> {
    ledger: &'a mut BudgetLedger,
    domain: WorkDomain,
    pub(super) request: FamilyRequest,
    work: u64,
    scratch: u64,
    output: u64,
    reservation: u64,
}
impl Attempt<'_> {
    pub(super) fn work(&mut self, n: usize) -> ResultIn<()> {
        let next = self
            .work
            .checked_add(n as u64)
            .ok_or(FamilyError::Capacity)?;
        if next > self.request.attempt.work_quota {
            return Err(Stop::Truncated(ResourceLimit::Work));
        }
        self.work = next;
        Ok(())
    }
    pub(super) fn reserve(&mut self, bytes: u64, output: bool) -> ResultIn<()> {
        let (used, limit, reason) = if output {
            (
                &mut self.output,
                self.request.output_bytes,
                ResourceLimit::Output,
            )
        } else {
            (
                &mut self.scratch,
                self.request.scratch_bytes,
                ResourceLimit::Scratch,
            )
        };
        let next = used
            .checked_add(bytes)
            .filter(|next| *next <= limit)
            .ok_or(Stop::Truncated(reason))?;
        self.ledger
            .retain(self.domain, bytes)
            .map_err(FamilyError::from)?;
        *used = next;
        self.reservation = self
            .reservation
            .checked_add(bytes)
            .ok_or(FamilyError::Capacity)?;
        Ok(())
    }
    pub(super) fn vector<T>(&mut self, capacity: usize, output: bool) -> ResultIn<Vec<T>> {
        let layout = VectorLayout::new(capacity).map_err(vector_error)?;
        self.reserve(layout.bytes(), output)?;
        layout.allocate().map_err(vector_error)
    }
    pub(super) fn release_vec<T>(&mut self, values: Vec<T>, output: bool) -> ResultIn<()> {
        let bytes = (values.capacity() as u64)
            .checked_mul(size_of::<T>() as u64)
            .ok_or(FamilyError::Capacity)?;
        drop(values);
        self.ledger
            .release(self.domain, bytes)
            .map_err(FamilyError::from)?;
        if output {
            self.output -= bytes;
        } else {
            self.scratch -= bytes;
        }
        self.reservation -= bytes;
        Ok(())
    }
    pub(super) fn push<T>(&mut self, values: &mut Vec<T>, value: T, output: bool) -> ResultIn<()> {
        if values.len() == values.capacity() {
            let next = values
                .capacity()
                .checked_mul(2)
                .ok_or(FamilyError::Capacity)?
                .max(4);
            self.work(values.len())?;
            let mut replacement = self.vector(next, output)?;
            replacement.append(values);
            let previous = std::mem::replace(values, replacement);
            self.release_vec(previous, output)?;
        }
        values.push(value);
        Ok(())
    }
    pub(super) fn sort_work(&mut self, n: usize) -> ResultIn<()> {
        self.work(
            n.checked_mul((usize::BITS - n.leading_zeros()) as usize)
                .ok_or(FamilyError::Capacity)?,
        )
    }
}
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        self.ledger
            .release(self.domain, self.reservation)
            .expect("product proof owns reservation");
    }
}

pub(super) fn analyze(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<FamilyAnalysis, FamilyError> {
    analyze_impl(program, uses, cell, request, ledger, domain, false)
}
pub(super) fn analyze_published(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<FamilyAnalysis, FamilyError> {
    analyze_impl(program, uses, cell, request, ledger, domain, true)
}
fn analyze_impl(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
    coherent: bool,
) -> Result<FamilyAnalysis, FamilyError> {
    run(
        request,
        PRODUCT_FAMILY_PLAN,
        PRODUCT_FAMILY_VERSION,
        ledger,
        domain,
        |budget| discover(program, uses, cell, budget, coherent),
        |family, charge| family.charge = charge,
    )
}
pub(super) fn run<T>(
    request: FamilyRequest,
    plan: u32,
    version: u32,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
    discover: impl FnOnce(&mut Attempt<'_>) -> ResultIn<T>,
    retained: impl FnOnce(&mut T, (WorkDomain, u64)),
) -> Result<Analysis<T>, FamilyError> {
    if request.attempt.plan != plan || request.attempt.algorithm_version != version {
        return Err(FamilyError::InvalidAttempt);
    }
    let mut admission = ledger.clone();
    admission.charge_analysis(
        domain,
        request.attempt,
        AnalysisWorkReceipt {
            attempt: request.attempt,
            completion: AnalysisCompletion::Truncated,
            logical_work: request.attempt.work_quota,
        },
    )?;
    let mut budget = Attempt {
        ledger,
        domain,
        request,
        work: 0,
        scratch: 0,
        output: 0,
        reservation: 0,
    };
    let result = discover(&mut budget);
    let receipt = AnalysisWorkReceipt {
        attempt: request.attempt,
        completion: if matches!(result, Err(Stop::Truncated(_))) {
            AnalysisCompletion::Truncated
        } else {
            AnalysisCompletion::Complete
        },
        logical_work: budget.work,
    };
    budget
        .ledger
        .charge_analysis(domain, request.attempt, receipt)?;
    let outcome = match result {
        Ok(mut family) => {
            retained(&mut family, (domain, budget.output));
            budget.reservation -= budget.output;
            Outcome::Complete(family)
        }
        Err(Stop::Unknown(reason)) => Outcome::Unknown(reason),
        Err(Stop::Truncated(reason)) => Outcome::Truncated(reason),
        Err(Stop::Error(error)) => return Err(error),
    };
    Ok(Analysis { outcome, receipt })
}
impl Admission for Attempt<'_> {
    type Error = Stop;
    fn work(&mut self, amount: usize) -> ResultIn<()> {
        self.work(amount)
    }
    fn vector<T>(&mut self, capacity: usize) -> ResultIn<Vec<T>> {
        self.vector(capacity, false)
    }
    fn push<T>(&mut self, target: &mut Vec<T>, value: T) -> ResultIn<()> {
        self.push(target, value, false)
    }
    fn release<T>(&mut self, value: Vec<T>) -> ResultIn<()> {
        self.release_vec(value, false)
    }
    fn invalid(&self, reason: &'static str) -> Stop {
        invalid(reason)
    }
}
/// The same admission protocol, with evidence vectors retained instead of scratch.
pub(super) struct OutputAdmission<'a, 'b>(pub(super) &'a mut Attempt<'b>);
impl Admission for OutputAdmission<'_, '_> {
    type Error = Stop;
    fn work(&mut self, amount: usize) -> ResultIn<()> {
        self.0.work(amount)
    }
    fn vector<T>(&mut self, capacity: usize) -> ResultIn<Vec<T>> {
        self.0.vector(capacity, true)
    }
    fn push<T>(&mut self, target: &mut Vec<T>, value: T) -> ResultIn<()> {
        self.0.push(target, value, true)
    }
    fn release<T>(&mut self, value: Vec<T>) -> ResultIn<()> {
        self.0.release_vec(value, true)
    }
    fn invalid(&self, reason: &'static str) -> Stop {
        invalid(reason)
    }
}

// The sparse table is only a membership/queue locator for original semantic
// subjects. No operand, use, path or creator edge is copied into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Cell(CellId),
    Value(UnitId, ValueId),
    Present(UnitId, ValueId),
    Operation(UnitId, OpId),
    Unit(UnitId),
    Creator(UnitId),
    Capture(CellId, UnitId),
    PackedCell(CellId),
    Incoming(UnitId),
    Call(UnitId, CallId),
    ReturnOrigin(UnitId),
    Ambient(UnitId),
    LocationCell(CellId),
    ReferenceParameter(CellId),
    ReferenceActual(UnitId, CallId, u32),
    ReferenceOperation(UnitId, OpId),
}
impl Key {
    fn hash(self) -> usize {
        let (tag, a, b) = match self {
            Self::Cell(a) => (1, a.index(), 0),
            Self::Value(a, b) => (2, a.index(), b.index()),
            Self::Present(a, b) => (3, a.index(), b.index()),
            Self::Operation(a, b) => (4, a.index(), b.index()),
            Self::Unit(a) => (5, a.index(), 0),
            Self::Creator(a) => (6, a.index(), 0),
            Self::Capture(a, b) => (7, a.index(), b.index()),
            Self::PackedCell(a) => (8, a.index(), 0),
            Self::Incoming(a) => (9, a.index(), 0),
            Self::Call(a, b) => (10, a.index(), b.index()),
            Self::Ambient(a) => (11, a.index(), 0),
            Self::LocationCell(a) => (12, a.index(), 0),
            Self::ReferenceParameter(a) => (13, a.index(), 0),
            Self::ReferenceActual(a, b, position) => (
                14u64 ^ (position as u64).rotate_left(17),
                a.index(),
                b.index(),
            ),
            Self::ReferenceOperation(a, b) => (15, a.index(), b.index()),
            Self::ReturnOrigin(a) => (16, a.index(), 0),
        };
        let mut x = (a as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ (b as u64).rotate_left(29) ^ tag;
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58476d1ce4e5b9);
        x ^= x >> 27;
        x as usize
    }
}
#[derive(Clone, Copy)]
struct Subject {
    key: Key,
    origin: Option<ProductOrigin>,
    capture: u8,
    locator: Option<usize>,
}
struct Subjects {
    nodes: Vec<Subject>,
    table: Vec<Option<usize>>,
}
impl Subjects {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            table: Vec::new(),
        }
    }
    fn find(&self, key: Key, budget: &mut Attempt<'_>) -> ResultIn<Option<usize>> {
        if self.table.is_empty() {
            return Ok(None);
        }
        let mut slot = key.hash() & (self.table.len() - 1);
        loop {
            budget.work(1)?;
            match self.table[slot] {
                None => return Ok(None),
                Some(index) if self.nodes[index].key == key => return Ok(Some(index)),
                _ => slot = (slot + 1) & (self.table.len() - 1),
            }
        }
    }
    fn add(&mut self, key: Key, budget: &mut Attempt<'_>) -> ResultIn<(usize, bool)> {
        if let Some(index) = self.find(key, budget)? {
            return Ok((index, false));
        }
        if self
            .nodes
            .len()
            .checked_add(1)
            .ok_or(FamilyError::Capacity)?
            > self.table.len() / 2
        {
            let capacity = self
                .table
                .len()
                .checked_mul(2)
                .ok_or(FamilyError::Capacity)?
                .max(8);
            let mut next = budget.vector(capacity, false)?;
            next.resize(capacity, None);
            for (index, node) in self.nodes.iter().enumerate() {
                let mut slot = node.key.hash() & (capacity - 1);
                loop {
                    budget.work(1)?;
                    if next[slot].is_none() {
                        next[slot] = Some(index);
                        break;
                    }
                    slot = (slot + 1) & (capacity - 1);
                }
            }
            let old = std::mem::replace(&mut self.table, next);
            budget.release_vec(old, false)?;
        }
        let index = self.nodes.len();
        budget.push(
            &mut self.nodes,
            Subject {
                key,
                origin: None,
                capture: 0,
                locator: None,
            },
            false,
        )?;
        let mut slot = key.hash() & (self.table.len() - 1);
        loop {
            budget.work(1)?;
            if self.table[slot].is_none() {
                self.table[slot] = Some(index);
                break;
            }
            slot = (slot + 1) & (self.table.len() - 1);
        }
        Ok((index, true))
    }
}
struct Proof<'p, 'src> {
    program: &'p Program<'src>,
    uses: &'p UseIndex,
    owner: UnitId,
    schema: Option<NominalId>,
    subjects: Subjects,
    cells: Vec<ProductCell>,
    operations: Vec<ProductOperation>,
    dependencies: ProductDependencies,
    dominance: Vec<(UnitId, StructuredDominance)>,
    incoming: Vec<CallableInputs>,
    supplied: Option<&'p CallableInputs>,
    select: bool,
}
impl<'p, 'src> Proof<'p, 'src> {
    fn data(&self, unit: UnitId) -> ResultIn<&'p UnitData> {
        self.program
            .unit(unit)
            .ok_or_else(|| invalid("product unit"))
    }
    fn ty(&self, id: TypeId) -> ResultIn<&'p Type<'src>> {
        self.program
            .types
            .get(id.index())
            .ok_or_else(|| invalid("product type"))
    }
    fn schema_of(&self, ty: TypeId) -> ResultIn<NominalId> {
        let Type::Struct(declaration) = self.ty(ty)? else {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        };
        self.schema_data(declaration.identity)?;
        Ok(declaration.identity)
    }
    fn schema_data(&self, id: NominalId) -> ResultIn<&'p StructDefinition> {
        let schema = self
            .program
            .structs
            .get(id.index())
            .filter(|schema| schema.identity == id)
            .ok_or_else(|| invalid("product schema"))?;
        if id.is_class() || !schema.type_parameters.is_empty() {
            return Err(unknown(UnknownReason::UnsupportedSchema));
        }
        Ok(schema)
    }
    fn add(&mut self, key: Key, budget: &mut Attempt<'_>) -> ResultIn<usize> {
        Ok(self.subjects.add(key, budget)?.0)
    }
    fn unit(&mut self, id: UnitId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        if self.subjects.add(Key::Unit(id), budget)?.1 {
            let revision = self
                .program
                .units
                .get(id.index())
                .ok_or_else(|| invalid("product dependency unit"))?
                .revision();
            budget.push(&mut self.dependencies.units, (id, revision), true)?;
        }
        Ok(())
    }
    fn value(&mut self, unit: UnitId, value: ValueId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let ty = self
            .data(unit)?
            .values
            .get(value.index())
            .ok_or_else(|| invalid("product value"))?
            .ty;
        if Some(self.schema_of(ty)?) != self.schema {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        }
        self.add(Key::Value(unit, value), budget)?;
        Ok(())
    }
    fn present(&mut self, unit: UnitId, value: ValueId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let ty = self
            .data(unit)?
            .values
            .get(value.index())
            .ok_or_else(|| invalid("packed product value"))?
            .ty;
        self.schema_of(ty)?;
        self.add(Key::Present(unit, value), budget)?;
        Ok(())
    }
    fn operation(
        &mut self,
        unit: UnitId,
        operation: OpId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        self.add(Key::Operation(unit, operation), budget)?;
        Ok(())
    }
    fn inputs(&self, index: usize) -> &CallableInputs {
        if index == usize::MAX {
            self.supplied.unwrap()
        } else {
            &self.incoming[index]
        }
    }
    fn supplied_for_body(&mut self, body: UnitId, budget: &mut Attempt<'_>) -> ResultIn<bool> {
        let Some(inputs) = self.supplied.filter(|inputs| inputs.body() == body) else {
            return Ok(false);
        };
        if !inputs.runtime_inputs_sealed() {
            return Err(unknown(UnknownReason::ExecutionBoundary));
        }
        if !inputs
            .dependencies()
            .valid_for_published(self.program, self.uses, budget)?
        {
            return Err(FamilyError::StaleUseIndex.into());
        }
        match inputs.scope() {
            InputScope::Body(known) => Ok(known == body),
            InputScope::Producer(creation) => {
                budget.work(1)?;
                let creators = self
                    .uses
                    .creators(body)
                    .ok_or_else(|| invalid("reference creators"))?;
                if !matches!(creators.sites(), [only] if only.unit == creation.unit && only.operation == creation.operation)
                {
                    return Ok(false);
                }
                if self.subjects.add(Key::Creator(body), budget)?.1 {
                    budget.push(
                        &mut self.dependencies.creators,
                        (body, creators.revision()),
                        true,
                    )?;
                }
                Ok(true)
            }
        }
    }
    fn incoming(&mut self, body: UnitId, budget: &mut Attempt<'_>) -> ResultIn<usize> {
        let node = self.add(Key::Incoming(body), budget)?;
        if let Some(index) = self.subjects.nodes[node].locator {
            return Ok(index);
        }
        if budget.request.execution != JavaScriptExecution::Module {
            return Err(unknown(UnknownReason::ExecutionBoundary));
        }
        let index = if self.supplied_for_body(body, budget)? {
            usize::MAX
        } else {
            let InputOutcome::Complete(proof) = CallableInputs::for_body_published(
                self.program,
                self.uses,
                body,
                CallObservations::from_execution(budget.request.execution),
                budget,
            )?
            else {
                return Err(unknown(UnknownReason::CallableInterface));
            };
            let index = self.incoming.len();
            budget.push(&mut self.incoming, proof, false)?;
            index
        };
        self.subjects.nodes[node].locator = Some(index);
        for n in 0..self.inputs(index).dependencies().units().len() {
            budget.work(1)?;
            let (unit, _) = self.inputs(index).dependencies().units()[n];
            self.unit(unit, budget)?;
        }
        for n in 0..self.inputs(index).dependencies().cells().len() {
            budget.work(1)?;
            let stamp = self.inputs(index).dependencies().cells()[n];
            budget.push(&mut self.dependencies.cells, stamp, true)?;
        }
        for n in 0..self.inputs(index).dependencies().creators().len() {
            budget.work(1)?;
            let stamp = self.inputs(index).dependencies().creators()[n];
            budget.push(&mut self.dependencies.creators, stamp, true)?;
        }
        // Only an ordinary function owns the view changed by its formals.
        // Closure bodies inherit an enclosing activation's arguments instead.
        if !super::ambient::inherits(self.data(body)?.kind) {
            self.add(Key::Ambient(body), budget)?;
        }
        Ok(index)
    }
    /// Locate and seal a private call once. A missing local body is reported
    /// separately so producer and consumer clients keep their own refusal.
    fn call(
        &mut self,
        unit: UnitId,
        call: CallId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<Option<usize>> {
        let node = self.add(Key::Call(unit, call), budget)?;
        if let Some(index) = self.subjects.nodes[node].locator {
            return Ok(Some(index));
        }
        let Some(body) =
            super::callable_inputs::body_for_call(self.program, self.uses, unit, call, budget)?
        else {
            return Ok(None);
        };
        let index = self.incoming(body, budget)?;
        // The common locator plus complete Body evidence entails this call:
        // every creator has one Initialize, whose cell's complete reads contain
        // this Load and all its callee uses. A direct Closure callee would have
        // violated the creator's sole-Initialize rule. Do not rescan/copy calls.
        self.subjects.nodes[node].locator = Some(index);
        Ok(Some(index))
    }
    /// Cache only the common query's small answer in the existing subject
    /// locator. Original copies/return edges remain in Program and UseIndex.
    fn return_origin(&mut self, body: UnitId, budget: &mut Attempt<'_>) -> ResultIn<u32> {
        let node = self.add(Key::ReturnOrigin(body), budget)?;
        if let Some(position) = self.subjects.nodes[node].locator {
            return u32::try_from(position).map_err(|_| FamilyError::Capacity.into());
        }
        self.unit(body, budget)?;
        let cells = &mut self.dependencies.cells;
        let origin = super::facts::returned_value_origin(
            self.program,
            self.uses,
            body,
            budget,
            |cell, revision, budget| budget.push(cells, (cell, revision), true),
        )?;
        let super::facts::ReturnedValueOrigin::Parameter { position } = origin else {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        };
        self.subjects.nodes[node].locator = Some(position as usize);
        Ok(position)
    }
    fn forwarded_call_value(
        &mut self,
        unit: UnitId,
        call: CallId,
        schema: NominalId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<ValueId> {
        let incoming = self
            .call(unit, call, budget)?
            .ok_or_else(|| unknown(UnknownReason::UnsupportedProducer))?;
        let body = self.inputs(incoming).body();
        let position = self.return_origin(body, budget)?;
        let data = self.data(unit)?;
        let site = &data.calls[call.index()];
        budget.work(1)?;
        let Some(Type::Function(signature)) = data
            .call_signature(site)
            .and_then(|ty| self.program.types.get(ty.index()))
        else {
            return Err(unknown(UnknownReason::CallableInterface));
        };
        let Some(parameter) = signature.params.get(position as usize) else {
            return Err(invalid("forwarded product parameter position"));
        };
        if parameter.passing != crate::primitive::ParameterPassing::Value
            || !matches!(&parameter.ty, Type::Struct(declaration) if declaration.identity == schema)
            || !matches!(signature.return_type.as_ref(), Type::Struct(declaration) if declaration.identity == schema)
        {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        }
        let Some(CallArgument::Value(value)) = data
            .arguments(site.arguments)
            .and_then(|args| args.get(position as usize))
            .copied()
        else {
            return Err(unknown(UnknownReason::CallableInterface));
        };
        if self.schema_of(data.values[value.index()].ty)? != schema {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        }
        Ok(value)
    }
    fn parameter_inputs(&mut self, cell: CellId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let storage = &self.program.cells[cell.index()];
        let CellBinding::Parameter(position) = storage.binding else {
            return Err(invalid("product parameter origin"));
        };
        if self.program.is_reference_parameter(cell) {
            return Err(unknown(UnknownReason::ReferenceExposure));
        }
        let schema = self.schema_of(storage.ty)?;
        let incoming = self.incoming(storage.owner, budget)?;
        for n in 0..self.inputs(incoming).calls().len() {
            budget.work(1)?;
            let call = self.inputs(incoming).calls()[n];
            let data = self.data(call.caller)?;
            let site = &data.calls[call.target.index()];
            let Some(CallArgument::Value(value)) = data
                .arguments(site.arguments)
                .and_then(|a| a.get(position as usize))
                .copied()
            else {
                return Err(unknown(UnknownReason::CallableInterface));
            };
            if self.schema_of(data.values[value.index()].ty)? != schema {
                return Err(unknown(UnknownReason::UnsupportedProducer));
            }
            // Presence only: caller storage remains an independent component.
            self.present(call.caller, value, budget)?;
        }
        Ok(())
    }
    fn origin(&mut self, cell: CellId, budget: &mut Attempt<'_>) -> ResultIn<ProductOrigin> {
        let index = self.cell_subject(cell, budget)?;
        if let Some(origin) = self.subjects.nodes[index].origin {
            return Ok(origin);
        }
        let storage = &self.program.cells[cell.index()];
        let users = self
            .uses
            .cell(cell)
            .ok_or_else(|| invalid("product origin uses"))?;
        if self.program.is_reference_parameter(cell) {
            return Err(unknown(UnknownReason::ReferenceExposure));
        }
        let mut initialize = None;
        let mut parameter = false;
        for site in users.sites() {
            budget.work(1)?;
            match *site {
                CellUseSite::Unit {
                    unit,
                    usage: CellUse::Initialize(op),
                } if unit == storage.owner => {
                    if initialize.replace(op).is_some() {
                        return Err(unknown(UnknownReason::Initialization));
                    }
                }
                CellUseSite::Unit {
                    unit,
                    usage: CellUse::Parameter(position),
                } if unit == storage.owner
                    && storage.binding == CellBinding::Parameter(position)
                    && !parameter =>
                {
                    parameter = true
                }
                CellUseSite::Export { .. } => return Err(unknown(UnknownReason::WholeValueUse)),
                CellUseSite::Unit {
                    usage:
                        CellUse::Initialize(_) | CellUse::Parameter(_) | CellUse::CatchBinding { .. },
                    ..
                } => return Err(unknown(UnknownReason::Initialization)),
                _ => {}
            }
        }
        let origin = match (storage.binding, initialize, parameter) {
            (CellBinding::Local, Some(operation), false) => ProductOrigin::Initialize(OpRef {
                unit: storage.owner,
                operation,
            }),
            (CellBinding::Parameter(_), None, true) => ProductOrigin::Parameter,
            _ => return Err(unknown(UnknownReason::Initialization)),
        };
        self.subjects.nodes[index].origin = Some(origin);
        Ok(origin)
    }
    fn dominance(&mut self, unit: UnitId, budget: &mut Attempt<'_>) -> ResultIn<usize> {
        for (index, (known, _)) in self.dominance.iter().enumerate() {
            budget.work(1)?;
            if *known == unit {
                return Ok(index);
            }
        }
        let data = self.data(unit)?;
        let mut parents = budget.vector(data.regions.len(), false)?;
        parents.resize(data.regions.len(), None);
        let mut positions = budget.vector(data.operations.len(), false)?;
        positions.resize(data.operations.len(), 0);
        budget.work(
            data.regions
                .len()
                .checked_add(data.operations.len())
                .ok_or(FamilyError::Capacity)?,
        )?;
        let dominance = StructuredDominance::build(data, parents, positions, |n| budget.work(n))?;
        let index = self.dominance.len();
        budget.push(&mut self.dominance, (unit, dominance), false)?;
        Ok(index)
    }
    fn after(&mut self, cell: CellId, operation: OpRef, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let owner = self.program.cells[cell.index()].owner;
        if operation.unit != owner {
            self.add(Key::Capture(cell, operation.unit), budget)?;
            return Ok(());
        }
        if let ProductOrigin::Initialize(initialize) = self.origin(cell, budget)? {
            let index = self.dominance(owner, budget)?;
            if !self.dominance[index].1.after(
                self.data(owner)?,
                initialize.operation,
                operation.operation,
                |n| budget.work(n),
            )? {
                return Err(unknown(UnknownReason::EarlyCaptureOrRead));
            }
        }
        Ok(())
    }
    fn expand_cell(&mut self, cell: CellId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let storage = self
            .program
            .cells
            .get(cell.index())
            .ok_or_else(|| invalid("product cell"))?;
        if !matches!(
            storage.binding,
            CellBinding::Local | CellBinding::Parameter(_)
        ) || Some(self.schema_of(storage.ty)?) != self.schema
        {
            return Err(unknown(UnknownReason::NotLocalProduct));
        }
        if storage.owner != self.owner {
            return Err(unknown(UnknownReason::CrossActivationCopy));
        }
        self.unit(storage.owner, budget)?;
        let origin = self.origin(cell, budget)?;
        if origin == ProductOrigin::Parameter {
            self.parameter_inputs(cell, budget)?;
        }
        budget.push(&mut self.cells, ProductCell { cell, origin }, true)?;
        let users = self
            .uses
            .cell(cell)
            .ok_or_else(|| invalid("product cell uses"))?;
        budget.push(&mut self.dependencies.cells, (cell, users.revision()), true)?;
        for site in users.sites() {
            budget.work(1)?;
            let CellUseSite::Unit { unit, usage } = *site else {
                return Err(unknown(UnknownReason::WholeValueUse));
            };
            self.unit(unit, budget)?;
            match usage {
                CellUse::Parameter(_) => {}
                CellUse::Initialize(op) => self.operation(unit, op, budget)?,
                CellUse::Read { operation, .. } | CellUse::Write { operation, .. } => {
                    self.after(cell, OpRef { unit, operation }, budget)?;
                    if !matches!(
                        self.data(unit)?.operations[operation.index()].kind,
                        OperationKind::PrepareReference { .. }
                    ) {
                        self.operation(unit, operation, budget)?;
                    }
                }
                CellUse::Capture => {
                    self.add(Key::Capture(cell, unit), budget)?;
                }
                CellUse::Reference { call, position, .. } => {
                    self.add(Key::ReferenceActual(unit, call, position), budget)?;
                }
                _ => return Err(unknown(UnknownReason::Initialization)),
            }
        }
        Ok(())
    }
    fn expand_value(
        &mut self,
        unit: UnitId,
        value: ValueId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        self.unit(unit, budget)?;
        let data = self.data(unit)?;
        let value_data = data
            .values
            .get(value.index())
            .ok_or_else(|| invalid("product snapshot value"))?;
        self.operation(unit, value_data.definition, budget)?;
        for usage in self
            .uses
            .unit(unit)
            .and_then(|unit| unit.value_uses(value))
            .ok_or_else(|| invalid("product value uses"))?
        {
            budget.work(1)?;
            match *usage {
                ValueUse::Operand {
                    operation,
                    position: 0,
                } => {
                    let op = data
                        .operations
                        .get(operation.index())
                        .ok_or_else(|| invalid("product value consumer"))?;
                    match op.kind {
                        OperationKind::CopyValue => {
                            let result = op.result.ok_or_else(|| invalid("product copy result"))?;
                            self.value(unit, result, budget)?;
                        }
                        OperationKind::Initialize(cell) => {
                            self.add(Key::Cell(cell), budget)?;
                            self.operation(unit, operation, budget)?;
                        }
                        OperationKind::Store(place)
                            if matches!(data.places.get(place.index()), Some(Place::Cell(_))) =>
                        {
                            let Place::Cell(cell) = data.places[place.index()] else {
                                unreachable!()
                            };
                            if self.program.is_reference_parameter(cell) {
                                self.add(Key::ReferenceParameter(cell), budget)?;
                                self.reference_operation(unit, operation, cell, budget)?;
                            } else {
                                self.add(Key::Cell(cell), budget)?;
                                self.operation(unit, operation, budget)?;
                            }
                        }
                        OperationKind::Return => {
                            let Some(Type::Function(signature)) = data
                                .callable_type
                                .and_then(|ty| self.program.types.get(ty.index()))
                            else {
                                return Err(unknown(UnknownReason::WholeValueUse));
                            };
                            if !matches!(signature.return_type.as_ref(), Type::Struct(declaration) if Some(declaration.identity) == self.schema)
                            {
                                return Err(unknown(UnknownReason::WholeValueUse));
                            }
                            self.incoming(unit, budget)?;
                        }
                        _ => return Err(unknown(UnknownReason::WholeValueUse)),
                    }
                }
                ValueUse::CallArgument {
                    operation: _,
                    call,
                    position,
                } => {
                    let incoming = self
                        .call(unit, call, budget)?
                        .ok_or_else(|| unknown(UnknownReason::WholeValueUse))?;
                    let body = self.inputs(incoming).body();
                    let formal = *self
                        .data(body)?
                        .parameters
                        .get(position as usize)
                        .ok_or_else(|| invalid("product argument position"))?;
                    if self.program.is_reference_parameter(formal) {
                        return Err(unknown(UnknownReason::WholeValueUse));
                    }
                    if matches!(
                        self.ty(self.program.cells[formal.index()].ty)?,
                        Type::TypeParameter(_)
                    ) {
                        let schema = self
                            .schema
                            .ok_or_else(|| invalid("selected product schema"))?;
                        // The generic body owns no product bank. It can receive
                        // this packed value only through the same closed raw
                        // forwarding contract used by return presence.
                        if self.return_origin(body, budget)? != position
                            || self.forwarded_call_value(unit, call, schema, budget)? != value
                        {
                            return Err(unknown(UnknownReason::WholeValueUse));
                        }
                    } else if Some(self.schema_of(self.program.cells[formal.index()].ty)?)
                        != self.schema
                    {
                        return Err(unknown(UnknownReason::WholeValueUse));
                    }
                }
                ValueUse::PlaceReceiver { operation, place } => {
                    let (root, _, _, _) = self.path(unit, place, budget)?;
                    if root != ProductAccessRoot::Value(value) {
                        return Err(unknown(UnknownReason::WholeValueUse));
                    }
                    self.operation(unit, operation, budget)?;
                }
                _ => return Err(unknown(UnknownReason::WholeValueUse)),
            }
        }
        Ok(())
    }
    /// The canonical field owner permits a backward place cursor: retain only
    /// the leaf type and each next expected packed ancestor, never a path copy.
    fn path(
        &mut self,
        unit: UnitId,
        place: PlaceId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<(ProductAccessRoot, NominalId, u32, TypeId)> {
        let (root, schema, slot, leaf) = self.walk_path(unit, place, budget)?;
        let (Some(schema), Some(slot)) = (schema, slot) else {
            return Err(unknown(UnknownReason::UnsupportedProjection));
        };
        Ok((root, schema, slot, leaf))
    }
    fn walk_path(
        &mut self,
        unit: UnitId,
        place: PlaceId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<(ProductAccessRoot, Option<NominalId>, Option<u32>, TypeId)> {
        let data = self.data(unit)?;
        let mut current = place;
        let mut expected = None;
        let mut leaf = None;
        let mut slot = None;
        loop {
            budget.work(1)?;
            match *data
                .places
                .get(current.index())
                .ok_or_else(|| invalid("product place"))?
            {
                Place::Field { base, field } if base.index() < current.index() => {
                    budget.work(
                        (usize::BITS - self.program.fields.len().leading_zeros()) as usize + 1,
                    )?;
                    let field = self
                        .program
                        .field(field)
                        .ok_or_else(|| invalid("product field"))?;
                    self.schema_data(field.owner)?;
                    if let Some(expected) = expected {
                        if self.schema_of(field.ty)? != expected {
                            return Err(invalid("product packed ancestor"));
                        }
                    }
                    leaf.get_or_insert(field.ty);
                    expected = Some(field.owner);
                    slot = Some(u32::try_from(field.index).map_err(|_| FamilyError::Capacity)?);
                    current = base;
                }
                Place::Cell(cell) => {
                    let ty = self
                        .program
                        .cells
                        .get(cell.index())
                        .ok_or_else(|| invalid("product place root"))?
                        .ty;
                    if let Some(expected) = expected {
                        if self.schema_of(ty)? != expected {
                            return Err(unknown(UnknownReason::UnsupportedProjection));
                        }
                    }
                    return Ok((
                        ProductAccessRoot::Cell(cell),
                        expected,
                        slot,
                        leaf.unwrap_or(ty),
                    ));
                }
                Place::Value(value) => {
                    let ty = data
                        .values
                        .get(value.index())
                        .ok_or_else(|| invalid("product place value"))?
                        .ty;
                    if let Some(expected) = expected {
                        if self.schema_of(ty)? != expected {
                            return Err(unknown(UnknownReason::UnsupportedProjection));
                        }
                    }
                    return Ok((
                        ProductAccessRoot::Value(value),
                        expected,
                        slot,
                        leaf.unwrap_or(ty),
                    ));
                }
                _ => return Err(unknown(UnknownReason::UnsupportedProjection)),
            }
        }
    }
    fn qualify_root(
        &mut self,
        unit: UnitId,
        operation: OpId,
        root: ProductAccessRoot,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        match root {
            ProductAccessRoot::Cell(cell) => {
                let storage = self
                    .program
                    .cells
                    .get(cell.index())
                    .ok_or_else(|| invalid("product root"))?;
                let schema = self.schema_of(storage.ty)?;
                self.add(
                    if self.select && Some(schema) == self.schema && storage.owner == self.owner {
                        Key::Cell(cell)
                    } else {
                        Key::PackedCell(cell)
                    },
                    budget,
                )?;
                // Cell expansion performs every access's dominance check after
                // finding its initializer, irrespective of queue order.
                let _ = operation;
            }
            ProductAccessRoot::Value(value) => self.present(unit, value, budget)?,
        }
        Ok(())
    }
    fn allocation_presence(
        &mut self,
        unit: UnitId,
        operation: &Operation,
        schema: NominalId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        let fields = self
            .program
            .fields
            .get(self.schema_data(schema)?.fields.clone())
            .ok_or_else(|| invalid("product allocation fields"))?;
        let inputs = self
            .data(unit)?
            .operands(operation.operands)
            .ok_or_else(|| invalid("product allocation inputs"))?;
        if fields.len() != inputs.len() {
            return Err(invalid("product allocation arity"));
        }
        for (field, &input) in fields.iter().zip(inputs) {
            budget.work(1)?;
            match self.ty(field.ty)? {
                Type::Struct(_) => self.present(unit, input, budget)?,
                Type::StructInstance { .. } => {
                    return Err(unknown(UnknownReason::UnsupportedSchema));
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn expand_operation(
        &mut self,
        unit: UnitId,
        id: OpId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        self.unit(unit, budget)?;
        let data = self.data(unit)?;
        let operation = data
            .operations
            .get(id.index())
            .ok_or_else(|| invalid("product operation"))?;
        let input = || {
            data.operands(operation.operands)
                .and_then(|values| values.first())
                .copied()
                .ok_or_else(|| invalid("product input"))
        };
        let result = || operation.result.ok_or_else(|| invalid("product result"));
        let kind = match operation.kind {
            OperationKind::Allocate {
                kind: AllocationKind::Struct(schema),
                ..
            } if Some(schema) == self.schema => {
                self.allocation_presence(unit, operation, schema, budget)?;
                let value = result()?;
                self.value(unit, value, budget)?;
                ProductOperationKind::Construct { value }
            }
            OperationKind::Load(place)
                if matches!(data.places.get(place.index()), Some(Place::Cell(_))) =>
            {
                let Place::Cell(cell) = data.places[place.index()] else {
                    unreachable!()
                };
                self.add(Key::Cell(cell), budget)?;
                let value = result()?;
                self.value(unit, value, budget)?;
                ProductOperationKind::Snapshot { cell, value }
            }
            OperationKind::CopyValue => {
                let input = input()?;
                let value = result()?;
                self.value(unit, input, budget)?;
                self.value(unit, value, budget)?;
                ProductOperationKind::Copy { input, value }
            }
            OperationKind::Initialize(cell) => {
                self.add(Key::Cell(cell), budget)?;
                let input = input()?;
                self.value(unit, input, budget)?;
                ProductOperationKind::Initialize { cell, input }
            }
            OperationKind::Store(place)
                if matches!(data.places.get(place.index()), Some(Place::Cell(_))) =>
            {
                let Place::Cell(cell) = data.places[place.index()] else {
                    unreachable!()
                };
                self.add(Key::Cell(cell), budget)?;
                let input = input()?;
                self.value(unit, input, budget)?;
                ProductOperationKind::Assign { cell, input }
            }
            OperationKind::Load(place)
            | OperationKind::Store(place)
            | OperationKind::CheckPlace(place) => {
                let (root, schema, slot, leaf) = self.path(unit, place, budget)?;
                if Some(schema) != self.schema {
                    return Err(unknown(UnknownReason::UnsupportedProjection));
                }
                self.qualify_root(unit, id, root, budget)?;
                if matches!(operation.kind, OperationKind::Store(_))
                    && matches!(
                        self.ty(leaf)?,
                        Type::Struct(_) | Type::StructInstance { .. }
                    )
                {
                    self.present(unit, input()?, budget)?;
                }
                if matches!(operation.kind, OperationKind::Load(_))
                    && matches!(
                        self.ty(leaf)?,
                        Type::Struct(_) | Type::StructInstance { .. }
                    )
                {
                    self.present(unit, result()?, budget)?;
                }
                ProductOperationKind::Access(ProductAccess { place, root, slot })
            }
            _ => return Err(unknown(UnknownReason::UnsupportedProducer)),
        };
        budget.push(
            &mut self.operations,
            ProductOperation {
                operation: OpRef {
                    unit,
                    operation: id,
                },
                kind,
            },
            true,
        )
    }
    fn expand_present(
        &mut self,
        unit: UnitId,
        value: ValueId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        self.unit(unit, budget)?;
        let data = self.data(unit)?;
        let value = data
            .values
            .get(value.index())
            .ok_or_else(|| invalid("packed product"))?;
        let schema = self.schema_of(value.ty)?;
        let operation = data
            .operations
            .get(value.definition.index())
            .ok_or_else(|| invalid("packed producer"))?;
        match operation.kind {
            OperationKind::Allocate {
                kind: AllocationKind::Struct(identity),
                ..
            } if schema == identity => self.allocation_presence(unit, operation, schema, budget),
            OperationKind::Call(call) => {
                let actual = self.forwarded_call_value(unit, call, schema, budget)?;
                self.present(unit, actual, budget)
            }
            OperationKind::Select { yes, no } => {
                for region in [yes, no] {
                    let value = data.regions[region.index()]
                        .result
                        .ok_or_else(|| invalid("product conditional result"))?;
                    if self.schema_of(data.values[value.index()].ty)? != schema {
                        return Err(unknown(UnknownReason::UnsupportedProducer));
                    }
                    self.present(unit, value, budget)?;
                }
                Ok(())
            }
            OperationKind::CopyValue => {
                let input = data
                    .operands(operation.operands)
                    .and_then(|values| values.first())
                    .copied()
                    .ok_or_else(|| invalid("packed copy input"))?;
                self.present(unit, input, budget)
            }
            OperationKind::Load(place)
                if matches!(data.places.get(place.index()), Some(Place::Cell(_))) =>
            {
                let Place::Cell(cell) = data.places[place.index()] else {
                    unreachable!()
                };
                self.add(Key::PackedCell(cell), budget)?;
                Ok(())
            }
            OperationKind::Load(place) => {
                let (root, _, _, leaf) = self.path(unit, place, budget)?;
                // A checked Load may narrow a JsValue/nullable slot to its
                // result annotation. Presence follows the actual stored leaf,
                // never that annotation or only its enclosing product shape.
                if self.schema_of(leaf)? != schema {
                    return Err(unknown(UnknownReason::UnsupportedProducer));
                }
                match root {
                    ProductAccessRoot::Cell(cell) => {
                        self.add(Key::PackedCell(cell), budget)?;
                    }
                    ProductAccessRoot::Value(value) => self.present(unit, value, budget)?,
                }
                Ok(())
            }
            _ => Err(unknown(UnknownReason::UnsupportedProducer)),
        }
    }
    fn cell_subject(&self, cell: CellId, budget: &mut Attempt<'_>) -> ResultIn<usize> {
        if let Some(index) = self.subjects.find(Key::Cell(cell), budget)? {
            return Ok(index);
        }
        if let Some(index) = self.subjects.find(Key::PackedCell(cell), budget)? {
            return Ok(index);
        }
        self.subjects
            .find(Key::LocationCell(cell), budget)?
            .ok_or_else(|| invalid("product storage subject"))
    }
    /// Different-schema local cells can supply immutable packed leaves. They
    /// retain their direct representation and only contribute producer/presence
    /// evidence. No use/copy closure or scalar bank is invented for these cells.
    fn expand_packed_cell(&mut self, cell: CellId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        if self.program.is_reference_parameter(cell) {
            self.add(Key::ReferenceParameter(cell), budget)?;
            return Ok(());
        }
        let storage = self
            .program
            .cells
            .get(cell.index())
            .ok_or_else(|| invalid("packed cell"))?;
        if !matches!(
            storage.binding,
            CellBinding::Local | CellBinding::Parameter(_)
        ) {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        }
        self.schema_of(storage.ty)?;
        self.unit(storage.owner, budget)?;
        let origin = self.origin(cell, budget)?;
        if origin == ProductOrigin::Parameter {
            self.parameter_inputs(cell, budget)?;
        }
        let users = self
            .uses
            .cell(cell)
            .ok_or_else(|| invalid("packed cell uses"))?;
        budget.push(&mut self.dependencies.cells, (cell, users.revision()), true)?;
        for site in users.sites() {
            budget.work(1)?;
            let CellUseSite::Unit { unit, usage } = *site else {
                return Err(unknown(UnknownReason::WholeValueUse));
            };
            self.unit(unit, budget)?;
            match usage {
                CellUse::Parameter(_) => {}
                CellUse::Initialize(operation) => {
                    let data = self.data(unit)?;
                    let input = data
                        .operands(data.operations[operation.index()].operands)
                        .and_then(|v| v.first())
                        .copied()
                        .ok_or_else(|| invalid("packed initializer"))?;
                    self.present(unit, input, budget)?;
                }
                CellUse::Read { operation, .. } => {
                    self.after(cell, OpRef { unit, operation }, budget)?
                }
                CellUse::Write { operation, place } => {
                    self.after(cell, OpRef { unit, operation }, budget)?;
                    let data = self.data(unit)?;
                    let input = data
                        .operands(data.operations[operation.index()].operands)
                        .and_then(|v| v.first())
                        .copied()
                        .ok_or_else(|| invalid("packed writer"))?;
                    if matches!(data.places[place.index()], Place::Cell(_)) {
                        self.present(unit, input, budget)?;
                    } else {
                        let (_, _, _, leaf) = self.path(unit, place, budget)?;
                        if matches!(
                            self.ty(leaf)?,
                            Type::Struct(_) | Type::StructInstance { .. }
                        ) {
                            self.present(unit, input, budget)?;
                        }
                    }
                }
                CellUse::Capture => {
                    self.add(Key::Capture(cell, unit), budget)?;
                }
                CellUse::Reference { call, position, .. } => {
                    self.add(Key::ReferenceActual(unit, call, position), budget)?;
                }
                _ => return Err(unknown(UnknownReason::UnsupportedProducer)),
            }
        }
        Ok(())
    }
    // Shared location protocol proof. These subjects never append ProductCells.
    fn reference_operation(
        &mut self,
        unit: UnitId,
        operation: OpId,
        parameter: CellId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        let index = self.add(Key::ReferenceOperation(unit, operation), budget)?;
        match self.subjects.nodes[index].locator {
            Some(known) if known != parameter.index() => Err(invalid("reference operation root")),
            Some(_) => Ok(()),
            None => {
                self.subjects.nodes[index].locator = Some(parameter.index());
                Ok(())
            }
        }
    }
    fn reference_root(&mut self, cell: CellId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let storage = self
            .program
            .cells
            .get(cell.index())
            .ok_or_else(|| invalid("reference root"))?;
        let key = if self.program.is_reference_parameter(cell) {
            Key::ReferenceParameter(cell)
        } else {
            match self.ty(storage.ty)? {
                Type::Struct(_) => Key::PackedCell(cell),
                Type::Int | Type::Float | Type::Bool | Type::String => Key::LocationCell(cell),
                _ => return Err(unknown(UnknownReason::UnsupportedProducer)),
            }
        };
        self.add(key, budget)?;
        Ok(())
    }
    fn expand_reference_actual(
        &mut self,
        unit: UnitId,
        call: CallId,
        position: u32,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        self.unit(unit, budget)?;
        let incoming = self
            .call(unit, call, budget)?
            .ok_or_else(|| unknown(UnknownReason::WholeValueUse))?;
        let body = self.inputs(incoming).body();
        let data = self.data(unit)?;
        let site = data
            .calls
            .get(call.index())
            .ok_or_else(|| invalid("reference actual call"))?;
        let Some(CallArgument::Reference(place)) = data
            .arguments(site.arguments)
            .and_then(|arguments| arguments.get(position as usize))
            .copied()
        else {
            return Err(invalid("reference actual tag"));
        };
        let parameter = *self
            .data(body)?
            .parameters
            .get(position as usize)
            .ok_or_else(|| invalid("reference formal position"))?;
        if !self.program.is_reference_parameter(parameter) {
            return Err(invalid("reference formal mode"));
        }
        let (root, _, _, _) = self.walk_path(unit, place, budget)?;
        let ProductAccessRoot::Cell(cell) = root else {
            return Err(unknown(UnknownReason::UnsupportedProjection));
        };
        // The verifier owns invariant parameter/storage type matching. The
        // common path walk here closes presence, not a second type relation.
        self.reference_root(cell, budget)?;
        self.add(Key::ReferenceParameter(parameter), budget)?;
        Ok(())
    }
    fn expand_reference_parameter(
        &mut self,
        cell: CellId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        let storage = self
            .program
            .cells
            .get(cell.index())
            .ok_or_else(|| invalid("reference formal"))?;
        let CellBinding::Parameter(position) = storage.binding else {
            return Err(invalid("reference formal binding"));
        };
        if !self.program.is_reference_parameter(cell)
            || self.data(storage.owner)?.parameters.get(position as usize) != Some(&cell)
        {
            return Err(invalid("reference formal declaration"));
        }
        match self.ty(storage.ty)? {
            Type::Struct(_) => {
                self.schema_of(storage.ty)?;
            }
            Type::Int | Type::Float | Type::Bool | Type::String => {}
            _ => return Err(unknown(UnknownReason::UnsupportedProducer)),
        }
        self.unit(storage.owner, budget)?;
        let incoming = self.incoming(storage.owner, budget)?;
        for index in 0..self.inputs(incoming).calls().len() {
            budget.work(1)?;
            let call = self.inputs(incoming).calls()[index];
            self.add(
                Key::ReferenceActual(call.caller, call.target, position),
                budget,
            )?;
        }
        let users = self
            .uses
            .cell(cell)
            .ok_or_else(|| invalid("reference formal uses"))?;
        budget.push(&mut self.dependencies.cells, (cell, users.revision()), true)?;
        for site in users.sites() {
            budget.work(1)?;
            let CellUseSite::Unit { unit, usage } = *site else {
                return Err(unknown(UnknownReason::ReferenceExposure));
            };
            if unit != storage.owner {
                return Err(unknown(UnknownReason::ReferenceExposure));
            }
            self.unit(unit, budget)?;
            match usage {
                CellUse::Parameter(known) if known == position => {}
                CellUse::Read { operation, .. } | CellUse::Write { operation, .. } => {
                    // Preparation's ordinary read preserves the original
                    // access schedule; its Reference use closes the callee.
                    if !matches!(
                        self.data(unit)?.operations[operation.index()].kind,
                        OperationKind::PrepareReference { .. }
                    ) {
                        self.reference_operation(unit, operation, cell, budget)?;
                    }
                }
                CellUse::Reference { call, position, .. } => {
                    self.add(Key::ReferenceActual(unit, call, position), budget)?;
                }
                _ => return Err(unknown(UnknownReason::ReferenceExposure)),
            }
        }
        Ok(())
    }
    fn expand_reference_operation(
        &mut self,
        unit: UnitId,
        id: OpId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        self.unit(unit, budget)?;
        let index = self
            .subjects
            .find(Key::ReferenceOperation(unit, id), budget)?
            .unwrap();
        let parameter = CellId::from_index(
            self.subjects.nodes[index]
                .locator
                .ok_or_else(|| invalid("reference operation locator"))?,
        )
        .ok_or(FamilyError::Capacity)?;
        let data = self.data(unit)?;
        let operation = data
            .operations
            .get(id.index())
            .ok_or_else(|| invalid("reference operation"))?;
        let (OperationKind::Load(place)
        | OperationKind::Store(place)
        | OperationKind::CheckPlace(place)) = operation.kind
        else {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        };
        let (root, _, _, leaf) = self.walk_path(unit, place, budget)?;
        if root != ProductAccessRoot::Cell(parameter)
            || !self.program.is_reference_parameter(parameter)
        {
            return Err(invalid("reference operation place"));
        }
        self.add(Key::ReferenceParameter(parameter), budget)?;
        if matches!(
            self.ty(leaf)?,
            Type::Struct(_) | Type::StructInstance { .. }
        ) {
            match operation.kind {
                OperationKind::Store(_) => {
                    let input = data
                        .operands(operation.operands)
                        .and_then(|v| v.first())
                        .copied()
                        .ok_or_else(|| invalid("reference writer"))?;
                    self.present(unit, input, budget)?;
                }
                OperationKind::Load(_) => {
                    self.present(
                        unit,
                        operation.result.ok_or_else(|| invalid("reference read"))?,
                        budget,
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn expand_location_cell(&mut self, cell: CellId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let storage = self
            .program
            .cells
            .get(cell.index())
            .ok_or_else(|| invalid("location cell"))?;
        if self.program.is_reference_parameter(cell) {
            return Err(invalid("reference formal cannot own location storage"));
        }
        if !matches!(
            self.ty(storage.ty)?,
            Type::Int | Type::Float | Type::Bool | Type::String
        ) {
            return Err(unknown(UnknownReason::UnsupportedProducer));
        }
        self.unit(storage.owner, budget)?;
        self.origin(cell, budget)?;
        let users = self
            .uses
            .cell(cell)
            .ok_or_else(|| invalid("location uses"))?;
        budget.push(&mut self.dependencies.cells, (cell, users.revision()), true)?;
        for site in users.sites() {
            budget.work(1)?;
            let CellUseSite::Unit { unit, usage } = *site else {
                return Err(unknown(UnknownReason::ReferenceExposure));
            };
            self.unit(unit, budget)?;
            match usage {
                CellUse::Parameter(_) | CellUse::Initialize(_) => {}
                CellUse::Read { operation, .. } | CellUse::Write { operation, .. } => {
                    self.after(cell, OpRef { unit, operation }, budget)?;
                }
                CellUse::Reference { call, position, .. } => {
                    self.add(Key::ReferenceActual(unit, call, position), budget)?;
                }
                CellUse::Capture => {
                    self.add(Key::Capture(cell, unit), budget)?;
                }
                _ => return Err(unknown(UnknownReason::ReferenceExposure)),
            }
        }
        Ok(())
    }
    fn expand_ambient(&mut self, body: UnitId, budget: &mut Attempt<'_>) -> ResultIn<()> {
        self.unit(body, budget)?;
        let uses = self
            .uses
            .unit(body)
            .ok_or_else(|| invalid("product activation uses"))?;
        for reference in uses.cell_uses() {
            budget.work(1)?;
            if matches!(reference.usage, CellUse::Read { .. })
                && super::ambient::classify(&self.program.cells[reference.cell.index()])
                    == Some(super::ambient::Ambient::Arguments)
            {
                return Err(unknown(UnknownReason::ArgumentsObservation));
            }
        }
        for &(_, child) in uses.closures() {
            budget.work(1)?;
            if super::ambient::inherits(self.data(child)?.kind) {
                self.add(Key::Ambient(child), budget)?;
            }
        }
        Ok(())
    }
    fn expand_capture(
        &mut self,
        cell: CellId,
        unit: UnitId,
        budget: &mut Attempt<'_>,
    ) -> ResultIn<()> {
        let initial = self
            .subjects
            .find(Key::Capture(cell, unit), budget)?
            .unwrap();
        if self.subjects.nodes[initial].capture == 2 {
            return Ok(());
        }
        let origin = self.origin(cell, budget)?;
        let owner = self.program.cells[cell.index()].owner;
        #[derive(Clone, Copy)]
        struct Frame {
            subject: usize,
            next: usize,
        }
        let mut frames = budget.vector::<Frame>(0, false)?;
        self.subjects.nodes[initial].capture = 1;
        budget.push(
            &mut frames,
            Frame {
                subject: initial,
                next: 0,
            },
            false,
        )?;
        let result = (|| {
            while let Some(frame) = frames.last().copied() {
                let Key::Capture(_, body) = self.subjects.nodes[frame.subject].key else {
                    unreachable!()
                };
                self.unit(body, budget)?;
                let data = self.data(body)?;
                if frame.next == 0 {
                    budget.work(data.captures.len())?;
                    if !data.captures.contains(&cell) {
                        return Err(unknown(UnknownReason::UnrootedCapture));
                    }
                    if self.subjects.add(Key::Creator(body), budget)?.1 {
                        let revision = self
                            .uses
                            .creators(body)
                            .ok_or_else(|| invalid("product creators"))?
                            .revision();
                        budget.push(&mut self.dependencies.creators, (body, revision), true)?;
                    }
                }
                let creators = self
                    .uses
                    .creators(body)
                    .ok_or_else(|| invalid("product creators"))?
                    .sites();
                if creators.is_empty() {
                    return Err(unknown(UnknownReason::UnrootedCapture));
                }
                let Some(creator) = creators.get(frame.next).copied() else {
                    self.subjects.nodes[frame.subject].capture = 2;
                    frames.pop();
                    continue;
                };
                budget.work(1)?;
                frames.last_mut().unwrap().next += 1;
                self.unit(creator.unit, budget)?;
                if creator.unit == owner {
                    if let ProductOrigin::Initialize(initialize) = origin {
                        let index = self.dominance(owner, budget)?;
                        if !self.dominance[index].1.after(
                            self.data(owner)?,
                            initialize.operation,
                            creator.operation,
                            |n| budget.work(n),
                        )? {
                            return Err(unknown(UnknownReason::EarlyCaptureOrRead));
                        }
                    }
                } else {
                    let parent = self.add(Key::Capture(cell, creator.unit), budget)?;
                    match self.subjects.nodes[parent].capture {
                        2 => {}
                        1 => return Err(unknown(UnknownReason::UnrootedCapture)),
                        _ => {
                            self.subjects.nodes[parent].capture = 1;
                            budget.push(
                                &mut frames,
                                Frame {
                                    subject: parent,
                                    next: 0,
                                },
                                false,
                            )?;
                        }
                    }
                }
            }
            Ok(())
        })();
        budget.release_vec(frames, false)?;
        result
    }
}
pub(super) fn check_index(
    program: &Program<'_>,
    uses: &UseIndex,
    coherent: bool,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    budget.work(1)?;
    if uses.tables_revision() != program.tables_revision {
        return Err(FamilyError::StaleUseIndex.into());
    }
    if !coherent {
        budget.work(program.units.len())?;
        if !uses.valid_for(program) {
            return Err(FamilyError::StaleUseIndex.into());
        }
    }
    Ok(())
}
impl<'p, 'src> Proof<'p, 'src> {
    fn new(
        program: &'p Program<'src>,
        uses: &'p UseIndex,
        owner: UnitId,
        schema: Option<NominalId>,
        select: bool,
        supplied: Option<&'p CallableInputs>,
    ) -> Self {
        Self {
            program,
            uses,
            owner,
            schema,
            select,
            supplied,
            subjects: Subjects::new(),
            cells: Vec::new(),
            operations: Vec::new(),
            dominance: Vec::new(),
            incoming: Vec::new(),
            dependencies: ProductDependencies {
                tables: program.tables_revision,
                units: Vec::new(),
                cells: Vec::new(),
                creators: Vec::new(),
            },
        }
    }
    fn complete(&mut self, budget: &mut Attempt<'_>) -> ResultIn<()> {
        let mut cursor = 0;
        while cursor < self.subjects.nodes.len() {
            budget.work(1)?;
            let key = self.subjects.nodes[cursor].key;
            cursor += 1;
            match key {
                Key::Cell(cell) => self.expand_cell(cell, budget)?,
                Key::PackedCell(cell) => self.expand_packed_cell(cell, budget)?,
                Key::Value(unit, value) => self.expand_value(unit, value, budget)?,
                Key::Present(unit, value) => self.expand_present(unit, value, budget)?,
                Key::Operation(unit, operation) => {
                    self.expand_operation(unit, operation, budget)?
                }
                Key::Capture(cell, unit) => self.expand_capture(cell, unit, budget)?,
                Key::Ambient(body) => self.expand_ambient(body, budget)?,
                Key::LocationCell(cell) => self.expand_location_cell(cell, budget)?,
                Key::ReferenceParameter(cell) => self.expand_reference_parameter(cell, budget)?,
                Key::ReferenceActual(unit, call, position) => {
                    self.expand_reference_actual(unit, call, position, budget)?
                }
                Key::ReferenceOperation(unit, operation) => {
                    self.expand_reference_operation(unit, operation, budget)?
                }
                Key::Unit(_)
                | Key::Creator(_)
                | Key::Incoming(_)
                | Key::Call(_, _)
                | Key::ReturnOrigin(_) => {}
            }
        }
        budget.sort_work(self.cells.len())?;
        self.cells.sort_unstable_by_key(|entry| entry.cell.index());
        budget.sort_work(self.operations.len())?;
        self.operations.sort_unstable_by_key(|entry| {
            (
                entry.operation.unit.index(),
                entry.operation.operation.index(),
            )
        });
        budget.sort_work(self.dependencies.units.len())?;
        self.dependencies
            .units
            .sort_unstable_by_key(|entry| entry.0.index());
        budget.work(self.dependencies.units.len())?;
        self.dependencies.units.dedup();
        budget.sort_work(self.dependencies.cells.len())?;
        self.dependencies
            .cells
            .sort_unstable_by_key(|entry| entry.0.index());
        budget.work(self.dependencies.cells.len())?;
        self.dependencies.cells.dedup();
        budget.sort_work(self.dependencies.creators.len())?;
        self.dependencies
            .creators
            .sort_unstable_by_key(|entry| entry.0.index());
        budget.work(self.dependencies.creators.len())?;
        self.dependencies.creators.dedup();
        Ok(())
    }
}
/// Layout-neutral product parameter proof through the same source worklist.
/// Incoming edges only establish presence; they never join caller banks.
pub(super) fn certify_parameters(
    program: &Program<'_>,
    uses: &UseIndex,
    inputs: &CallableInputs,
    parameters: &[super::function_layout::ParameterLayout],
    budget: &mut Attempt<'_>,
) -> ResultIn<ProductDependencies> {
    let first = parameters
        .first()
        .ok_or_else(|| unknown(UnknownReason::NoProductParameters))?;
    let mut proof = Proof::new(
        program,
        uses,
        inputs.body(),
        Some(first.schema),
        false,
        Some(inputs),
    );
    proof.incoming(inputs.body(), budget)?;
    for parameter in parameters {
        budget.work(1)?;
        let cell = *proof
            .data(inputs.body())?
            .parameters
            .get(parameter.position as usize)
            .ok_or_else(|| invalid("product layout parameter"))?;
        proof.add(Key::PackedCell(cell), budget)?;
    }
    proof.complete(budget)?;
    Ok(proof.dependencies)
}
/// Layout-neutral, body-wide mutable-location evidence. The supplied callable
/// witness keeps its exact scope: a producer is promoted only by a stamped
/// sole-creator proof, otherwise Incoming obtains the existing Body witness.
///
/// Only the completion callback can observe private access witnesses. It runs
/// after the SAME worklist has closed every reference actual, forwarding site
/// and nominal writer. The caller owns any pre-admitted destination slots and
/// keeps the existing Attempt alive until these dependencies are copied/dropped.
/// No reference-formal operation becomes a selected ProductFamily recipe.
pub(super) fn certify_reference_parameters(
    program: &Program<'_>,
    uses: &UseIndex,
    inputs: &CallableInputs,
    budget: &mut Attempt<'_>,
    mut inspect: impl FnMut(OpRef, ReferenceAccess) -> ResultIn<()>,
) -> ResultIn<ProductDependencies> {
    check_index(program, uses, true, budget)?;
    let body = inputs.body();
    let mut proof = Proof::new(program, uses, body, None, false, Some(inputs));
    proof.incoming(body, budget)?;
    let data = proof.data(body)?;
    for &cell in &data.parameters {
        budget.work(1)?;
        if program.is_reference_parameter(cell) {
            proof.add(Key::ReferenceParameter(cell), budget)?;
        }
    }
    proof.complete(budget)?;
    // Prepay the finite existing-subject enumeration; no path is reconstructed
    // here and no unadmitted list of reference accesses is materialized.
    budget.work(proof.subjects.nodes.len())?;
    for subject in &proof.subjects.nodes {
        let Key::ReferenceOperation(unit, operation) = subject.key else {
            continue;
        };
        if unit != body {
            continue;
        }
        let parameter =
            CellId::from_index(subject.locator.unwrap()).ok_or(FamilyError::Capacity)?;
        let (OperationKind::Load(place)
        | OperationKind::Store(place)
        | OperationKind::CheckPlace(place)) = data.operations[operation.index()].kind
        else {
            unreachable!("completed reference operation")
        };
        inspect(
            OpRef { unit, operation },
            ReferenceAccess { place, parameter },
        )?;
    }
    Ok(proof.dependencies)
}
fn discover(
    program: &Program<'_>,
    uses: &UseIndex,
    root: CellId,
    budget: &mut Attempt<'_>,
    coherent: bool,
) -> ResultIn<ProductFamily> {
    check_index(program, uses, coherent, budget)?;
    let cell = program
        .cells
        .get(root.index())
        .ok_or_else(|| invalid("product root"))?;
    if !matches!(cell.binding, CellBinding::Local | CellBinding::Parameter(_)) {
        return Err(unknown(UnknownReason::NotLocalProduct));
    }
    let Some(Type::Struct(declaration)) = program.types.get(cell.ty.index()) else {
        return Err(unknown(UnknownReason::NotLocalProduct));
    };
    let schema = program
        .structs
        .get(declaration.identity.index())
        .filter(|schema| schema.identity == declaration.identity)
        .ok_or_else(|| invalid("product root schema"))?;
    if !schema.type_parameters.is_empty() {
        return Err(unknown(UnknownReason::UnsupportedSchema));
    }
    let fields = program
        .fields
        .get(schema.fields.clone())
        .ok_or_else(|| invalid("product root fields"))?;
    budget.reserve(size_of::<ProductFamily>() as u64, true)?;
    let mut cut = budget.vector(fields.len(), true)?;
    for field in fields {
        budget.work(1)?;
        cut.push(field.identity);
    }
    let mut proof = Proof::new(
        program,
        uses,
        cell.owner,
        Some(declaration.identity),
        true,
        None,
    );
    proof.add(Key::Cell(root), budget)?;
    proof.complete(budget)?;
    Ok(ProductFamily {
        root: proof.cells[0].cell,
        schema: proof.schema.unwrap(),
        cells: proof.cells,
        fields: cut,
        operations: proof.operations,
        requires_module: !proof.incoming.is_empty(),
        dependencies: proof.dependencies,
        charge: (budget.domain, 0),
    })
}

#[cfg(test)]
#[path = "product_reference_tests.rs"]
mod reference_tests;
