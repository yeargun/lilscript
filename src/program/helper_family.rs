//! Complete private leaf-helper evidence, independent of representation choice.
//!
//! Preparation owns bounded storage and initialization/use evidence. It consumes
//! required record/product evidence through its existing owner, then a single
//! borrowed query from Compilation's local facts owner certifies body behavior.
//! No semantic operation is edited and no cache, target body or name is retained.

use super::activation::StructuredDominance;
use super::callable_inputs::{self, CallObservations, CallableInputs, InputOutcome, InputScope};
use super::facts::{EvaluationBehavior, MemoryAccess, UnitFacts};
use super::product_family::{self, ProductAccessRoot, ProductOperationKind, ReferenceAccess};
use super::raw_domains::{Admission, DomainInputs, DomainProof, Recipes, ResultRecipe, Subject};
use super::record_family::{self, OpRef};
use super::uses::{CellUse, CellUseSite, UseIndex};
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, AnalysisWorkReceipt, BudgetError, BudgetLedger, WorkDomain,
};
use crate::output_budget::{AllocationError, VectorLayout};
use std::mem::size_of;

pub(super) const HELPER_FAMILY_PLAN: u32 = 3;
pub(super) const HELPER_FAMILY_VERSION: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HelperRoot {
    pub cell: CellId,
    pub body: UnitId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HelperCall {
    pub caller: UnitId,
    pub load: OpId,
    pub prepare: OpId,
    pub call: OpId,
    pub target: CallId,
    pub environment: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HelperEnvironment {
    pub caller: UnitId,
    /// Unique lexical producer, or None for a proven environment root.
    /// Verified module initializers are distinct roots, never synthetic ancestors.
    pub creator: Option<OpRef>,
}
#[derive(Debug)]
pub(super) struct HelperDependencies {
    tables: RevisionId,
    use_sets: Vec<(CellId, RevisionId)>,
    creator_sets: Vec<(UnitId, RevisionId)>,
    units: Vec<(UnitId, RevisionId)>,
}
impl HelperDependencies {
    pub(super) fn units(&self) -> &[(UnitId, RevisionId)] {
        &self.units
    }
    pub(super) fn cells(&self) -> &[(CellId, RevisionId)] {
        &self.use_sets
    }
    pub(super) fn creators(&self) -> &[(UnitId, RevisionId)] {
        &self.creator_sets
    }
    pub(super) fn valid_for(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        uses.valid_for(program) && self.valid_for_published(program, uses)
    }
    pub(super) fn valid_for_published(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        self.tables == program.tables_revision
            && self.use_sets.iter().all(|(id, revision)| {
                uses.cell(*id)
                    .is_some_and(|cell| cell.revision() == *revision)
            })
            && self.units.iter().all(|(id, revision)| {
                program
                    .units
                    .get(id.index())
                    .is_some_and(|unit| unit.revision() == *revision)
            })
            && self.creator_sets.iter().all(|(id, revision)| {
                uses.creators(*id)
                    .is_some_and(|creators| creators.revision() == *revision)
            })
    }
    pub(super) fn validation_work(&self) -> Option<u64> {
        (self.use_sets.len() as u64)
            .checked_add(self.units.len() as u64)?
            .checked_add(self.creator_sets.len() as u64)?
            .checked_add(1)
    }
}
/// Runtime evidence for one unchanged body operation. Source/storage types do
/// not certify the raw value or the behavior of its conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HelperOperationFacts {
    pub(super) primitive_result: bool,
    pub(super) behavior: EvaluationBehavior,
    // Exact access evidence shares this existing immutable operation row.
    // No separately retained reference operation list or alias table exists.
    reference: Option<ReferenceAccess>,
}
#[must_use = "retain in Compilation or discard through the original ledger"]
#[derive(Debug)]
pub(super) struct HelperFamily {
    root: HelperRoot,
    closure: OpRef,
    initialize: OpRef,
    // None certifies normal fallthrough of an exactly Void callable; Some is
    // the sole explicit entry-tail return. Neither permits other abrupt exits.
    tail_return: Option<OpId>,
    handle_loads: Vec<OpRef>,
    calls: Vec<HelperCall>,
    environments: Vec<HelperEnvironment>,
    // These same original storage identities are visible in every environment.
    captures: Vec<CellId>,
    dependencies: HelperDependencies,
    body_facts: Vec<HelperOperationFacts>,
    return_primitive: bool,
    body_effects: EvaluationBehavior,
    execution: JavaScriptExecution,
    charge: (WorkDomain, u64),
}
impl HelperFamily {
    pub(super) fn root(&self) -> HelperRoot {
        self.root
    }
    pub(super) fn closure(&self) -> OpRef {
        self.closure
    }
    pub(super) fn initialize(&self) -> OpRef {
        self.initialize
    }
    pub(super) fn tail_return(&self) -> Option<OpId> {
        self.tail_return
    }
    pub(super) fn handle_loads(&self) -> &[OpRef] {
        &self.handle_loads
    }
    pub(super) fn calls(&self) -> &[HelperCall] {
        &self.calls
    }
    pub(super) fn environments(&self) -> &[HelperEnvironment] {
        &self.environments
    }
    pub(super) fn captures(&self) -> &[CellId] {
        &self.captures
    }
    pub(super) fn dependencies(&self) -> &HelperDependencies {
        &self.dependencies
    }
    pub(super) fn body_effects(&self) -> EvaluationBehavior {
        self.body_effects
    }
    pub(super) fn frame_elision_allowed(&self, execution: JavaScriptExecution) -> bool {
        self.execution == execution
            && (!self.body_effects.may_reenter || execution.guarantees_strict_execution())
    }
    pub(super) fn operation_facts(&self, operation: OpId) -> Option<HelperOperationFacts> {
        self.body_facts.get(operation.index()).copied()
    }
    pub(super) fn reference_access(&self, operation: OpId) -> Option<&ReferenceAccess> {
        self.body_facts.get(operation.index())?.reference.as_ref()
    }
    pub(super) fn return_primitive(&self) -> bool {
        self.return_primitive
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
    NotPrivateCallable,
    Initialization,
    Reassigned,
    CallableObservation,
    UnsupportedCall,
    Arity,
    Recursion,
    CompletionShape,
    BodyOperation,
    BodyEffects,
    ObservableFrame,
    EarlyCaptureOrRead,
    UnrootedCapture,
    Environment,
    RequiredRecordEvidence,
    RequiredProductEvidence,
    UnqualifiedFacts,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    Work,
    Scratch,
    Output,
    LocalFacts,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FamilyError {
    InvalidAttempt,
    InvalidProgram(&'static str),
    StaleUseIndex,
    Capacity,
    AllocationFailed,
    AlreadyChecked,
    Budget(BudgetError),
    Record(record_family::FamilyError),
    Product(product_family::FamilyError),
}
impl From<BudgetError> for FamilyError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
#[derive(Debug)]
pub(super) enum FamilyOutcome {
    Complete(HelperFamily),
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
}
#[derive(Debug)]
pub(super) enum PreparationOutcome {
    Ready(PreparedHelper),
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PrerequisiteWork {
    pub attempts: usize,
    pub logical_work: u64,
}
#[derive(Debug)]
pub(super) struct Preparation {
    pub outcome: PreparationOutcome,
    pub receipt: AnalysisWorkReceipt,
    pub prerequisites: PrerequisiteWork,
}
#[derive(Debug, Clone, Copy)]
enum BodyAccess {
    None,
    ScalarRead(CellId),
    ScalarWrite(CellId),
    RecordHandle,
    RecordRead,
    RecordWrite,
    Product(ProductOperationKind),
    Reference(ReferenceAccess),
}
#[derive(Debug, Clone, Copy)]
enum BodyStatus {
    Unchecked,
    Complete,
    Unknown(UnknownReason),
    Truncated,
}
#[must_use = "finish or discard after the borrowed facts query ends"]
#[derive(Debug)]
pub(super) struct PreparedHelper {
    family: HelperFamily,
    access: Vec<BodyAccess>,
    domains: Vec<bool>,
    status: BodyStatus,
    scratch: u64,
}
impl PreparedHelper {
    pub(super) fn root(&self) -> HelperRoot {
        self.family.root
    }
    /// Exactly one allocation-free scan, admitted and charged in prepare.
    /// Facts remain borrowed; their cache payload cannot escape this owner.
    pub(super) fn check_body(
        &mut self,
        program: &Program<'_>,
        facts: &UnitFacts,
        receipt: AnalysisWorkReceipt,
    ) -> Result<(), FamilyError> {
        if !matches!(self.status, BodyStatus::Unchecked) {
            return Err(FamilyError::AlreadyChecked);
        }
        self.status = BodyStatus::Unknown(UnknownReason::UnqualifiedFacts);
        let deps = facts.dependencies();
        if deps.unit != self.family.root.body
            || !deps.valid_for(program)
            || !self
                .family
                .dependencies
                .units
                .iter()
                .any(|&(id, revision)| id == deps.unit && revision == deps.unit_revision)
            || receipt.attempt.plan != facts::LOCAL_FACTS_PLAN
            || receipt.attempt.algorithm_version != facts::LOCAL_FACTS_VERSION
        {
            return Ok(());
        }
        if receipt.completion != AnalysisCompletion::Complete {
            self.status = BodyStatus::Truncated;
            return Ok(());
        }
        let unit = program
            .unit(self.family.root.body)
            .ok_or(FamilyError::InvalidProgram("helper body"))?;
        let mut good = true;
        let mut effects = EvaluationBehavior::TOTAL;
        for (index, op) in unit.operations.iter().enumerate() {
            let id = OpId::from_index(index).ok_or(FamilyError::Capacity)?;
            let operands = unit
                .operands(op.operands)
                .ok_or(FamilyError::InvalidProgram("helper operands"))?;
            let all_primitive = operands.iter().all(|value| self.domains[value.index()]);
            let mut domain = op.result.is_some_and(|value| self.domains[value.index()]);
            let behavior = match self.access[index] {
                BodyAccess::ScalarRead(cell) => {
                    // Initialization proves this read cannot observe TDZ or a
                    // getter. It says nothing about the stored raw value.
                    EvaluationBehavior {
                        reads: MemoryAccess::Cell(cell),
                        ..EvaluationBehavior::TOTAL
                    }
                }
                BodyAccess::ScalarWrite(cell) => {
                    domain |= operands
                        .first()
                        .is_some_and(|value| self.domains[value.index()]);
                    EvaluationBehavior {
                        writes: MemoryAccess::Cell(cell),
                        ..EvaluationBehavior::TOTAL
                    }
                }
                BodyAccess::RecordHandle => EvaluationBehavior {
                    reads: MemoryAccess::Unknown,
                    ..EvaluationBehavior::TOTAL
                },
                BodyAccess::RecordRead => EvaluationBehavior {
                    reads: MemoryAccess::Unknown,
                    ..EvaluationBehavior::TOTAL
                },
                BodyAccess::RecordWrite => {
                    domain |= operands
                        .first()
                        .is_some_and(|value| self.domains[value.index()]);
                    EvaluationBehavior {
                        writes: MemoryAccess::Unknown,
                        ..EvaluationBehavior::TOTAL
                    }
                }
                BodyAccess::Reference(access) => {
                    let place = match op.kind {
                        OperationKind::Load(place)
                        | OperationKind::Store(place)
                        | OperationKind::CheckPlace(place) => place,
                        _ => return Err(FamilyError::InvalidProgram("reference access operation")),
                    };
                    debug_assert_eq!(place, access.place());
                    debug_assert!(program.is_reference_parameter(access.parameter()));
                    match op.kind {
                        OperationKind::Load(_) => {
                            let result = op
                                .result
                                .ok_or(FamilyError::InvalidProgram("reference load result"))?;
                            let ty = &program.types[unit.values[result.index()].ty.index()];
                            // Whole int-cell actuals remain raw; projected int
                            // aliases normalize at this original Load. Until
                            // the actual location is known, retain possible
                            // coercion WITHOUT manufacturing primitive domain.
                            let normalized = super::javascript::JavaScriptRecipes.result(
                                program,
                                self.family.root.body,
                                id,
                            ) == ResultRecipe::NormalizedI32;
                            domain |= normalized;
                            if matches!(ty, Type::Int) {
                                EvaluationBehavior::COERCION
                            } else {
                                EvaluationBehavior {
                                    reads: MemoryAccess::Unknown,
                                    may_exhaust_resources: matches!(ty, Type::Struct(_)),
                                    ..EvaluationBehavior::TOTAL
                                }
                            }
                        }
                        OperationKind::Store(_) => EvaluationBehavior {
                            reads: MemoryAccess::Unknown,
                            writes: MemoryAccess::Unknown,
                            may_exhaust_resources: true,
                            ..EvaluationBehavior::TOTAL
                        },
                        OperationKind::CheckPlace(_) => EvaluationBehavior {
                            reads: MemoryAccess::Unknown,
                            ..EvaluationBehavior::TOTAL
                        },
                        _ => unreachable!(),
                    }
                }
                BodyAccess::Product(kind) => match kind {
                    ProductOperationKind::Access(access) => {
                        let memory = match access.root() {
                            ProductAccessRoot::Cell(cell) => MemoryAccess::Cell(cell),
                            ProductAccessRoot::Value(_) => MemoryAccess::None,
                        };
                        match op.kind {
                            OperationKind::Load(place) => {
                                debug_assert_eq!(place, access.place());
                                if super::javascript::JavaScriptRecipes.result(
                                    program,
                                    self.family.root.body,
                                    id,
                                ) == ResultRecipe::NormalizedI32
                                {
                                    // Presence removes property/TDZ uncertainty,
                                    // never the conversion of the raw field.
                                    domain = true;
                                    EvaluationBehavior::COERCION
                                } else {
                                    EvaluationBehavior {
                                        reads: memory,
                                        ..EvaluationBehavior::TOTAL
                                    }
                                }
                            }
                            OperationKind::Store(place) => {
                                debug_assert_eq!(place, access.place());
                                EvaluationBehavior {
                                    reads: memory,
                                    writes: memory,
                                    may_exhaust_resources: true,
                                    ..EvaluationBehavior::TOTAL
                                }
                            }
                            OperationKind::CheckPlace(place) => {
                                debug_assert_eq!(place, access.place());
                                EvaluationBehavior {
                                    reads: memory,
                                    ..EvaluationBehavior::TOTAL
                                }
                            }
                            _ => {
                                return Err(FamilyError::InvalidProgram("product access operation"))
                            }
                        }
                    }
                    ProductOperationKind::Snapshot { cell, .. } => EvaluationBehavior {
                        reads: MemoryAccess::Cell(cell),
                        ..EvaluationBehavior::TOTAL
                    },
                    ProductOperationKind::Copy { .. } => facts.effects(id),
                    ProductOperationKind::Construct { .. } => EvaluationBehavior {
                        may_exhaust_resources: true,
                        ..EvaluationBehavior::TOTAL
                    },
                    ProductOperationKind::Initialize { cell, .. }
                    | ProductOperationKind::Assign { cell, .. } => EvaluationBehavior {
                        writes: MemoryAccess::Cell(cell),
                        may_exhaust_resources: true,
                        ..EvaluationBehavior::TOTAL
                    },
                },
                BodyAccess::None => match op.kind {
                    OperationKind::Constant(_) => {
                        domain |= facts::primitive_result_domain(program, unit, op, &self.domains);
                        facts.effects(id)
                    }
                    OperationKind::CopyValue => {
                        domain |= all_primitive;
                        facts.effects(id)
                    }
                    // The shared semantic owner already states that a logical
                    // struct construction has no hook/reference identity.
                    // Its separately scheduled producers still execute; the
                    // reference certificate closes nominal writers' presence.
                    OperationKind::Allocate {
                        kind: AllocationKind::Struct(_),
                        ..
                    } => facts.effects(id),
                    OperationKind::Initialize(cell) => EvaluationBehavior {
                        writes: MemoryAccess::Cell(cell),
                        ..EvaluationBehavior::TOTAL
                    },
                    OperationKind::IntBinary(_)
                    | OperationKind::Unary { .. }
                    | OperationKind::Binary(_) => {
                        domain |= facts::primitive_result_domain(program, unit, op, &self.domains);
                        // Ordinary conversion hooks remain legal to inline at
                        // this unchanged site, with their complete behavior.
                        facts::primitive_evaluation_behavior(program, unit, op, all_primitive)
                            .unwrap_or(EvaluationBehavior::UNKNOWN)
                    }
                    OperationKind::ShortCircuit { right, .. } => {
                        domain |= all_primitive
                            && unit.regions[right.index()]
                                .result
                                .is_some_and(|value| self.domains[value.index()]);
                        EvaluationBehavior::TOTAL
                    }
                    OperationKind::Select { yes, no } => {
                        domain |= unit.regions[yes.index()]
                            .result
                            .is_some_and(|value| self.domains[value.index()])
                            && unit.regions[no.index()]
                                .result
                                .is_some_and(|value| self.domains[value.index()]);
                        EvaluationBehavior::TOTAL
                    }
                    OperationKind::Return if Some(id) == self.family.tail_return => {
                        self.family.return_primitive = operands
                            .first()
                            .map_or(true, |value| self.domains[value.index()]);
                        EvaluationBehavior {
                            transfers_control: true,
                            ..EvaluationBehavior::TOTAL
                        }
                    }
                    // Operations this representation does not specialize
                    // keep the one per-operation answer.
                    _ => facts.effects(id),
                },
            };
            if let Some(value) = op.result {
                self.domains[value.index()] = domain;
            }
            self.family.body_facts.push(HelperOperationFacts {
                primitive_result: domain,
                behavior,
                reference: match self.access[index] {
                    BodyAccess::Reference(access) => Some(access),
                    _ => None,
                },
            });
            good &= !behavior.may_suspend
                && !behavior.creates_identity
                && (!behavior.transfers_control || Some(id) == self.family.tail_return);
            // The one tail return is consumed by the selected call recipe;
            // it does not transfer control out of the caller's source region.
            effects = effects.join(EvaluationBehavior {
                transfers_control: behavior.transfers_control
                    && Some(id) != self.family.tail_return,
                ..behavior
            });
        }
        self.family.body_effects = effects;
        self.status = if good {
            BodyStatus::Complete
        } else {
            BodyStatus::Unknown(UnknownReason::BodyEffects)
        };
        Ok(())
    }
    pub(super) fn finish(self, ledger: &mut BudgetLedger) -> Result<FamilyOutcome, FamilyError> {
        let Self {
            family,
            access,
            domains,
            status,
            scratch,
        } = self;
        drop(access);
        drop(domains);
        ledger.release(family.charge.0, scratch)?;
        match status {
            BodyStatus::Complete => Ok(FamilyOutcome::Complete(family)),
            BodyStatus::Unchecked | BodyStatus::Unknown(UnknownReason::UnqualifiedFacts) => {
                family.discard(ledger)?;
                Ok(FamilyOutcome::Unknown(UnknownReason::UnqualifiedFacts))
            }
            BodyStatus::Unknown(reason) => {
                family.discard(ledger)?;
                Ok(FamilyOutcome::Unknown(reason))
            }
            BodyStatus::Truncated => {
                family.discard(ledger)?;
                Ok(FamilyOutcome::Truncated(ResourceLimit::LocalFacts))
            }
        }
    }
    pub(super) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let Self {
            family,
            access,
            domains,
            scratch,
            ..
        } = self;
        drop(access);
        drop(domains);
        ledger.release(family.charge.0, scratch)?;
        family.discard(ledger)
    }
}
fn supported_scalar_storage_type(ty: &Type<'_>) -> bool {
    matches!(
        ty,
        Type::Int | Type::Float | Type::Bool | Type::String | Type::Null | Type::Enum(_)
    )
}
enum Stop {
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
    Error(FamilyError),
}
impl From<FamilyError> for Stop {
    fn from(error: FamilyError) -> Self {
        Self::Error(error)
    }
}
type ResultIn<T> = Result<T, Stop>;
fn invalid(message: &'static str) -> Stop {
    Stop::Error(FamilyError::InvalidProgram(message))
}
fn unknown(reason: UnknownReason) -> Stop {
    Stop::Unknown(reason)
}
struct Attempt<'a> {
    ledger: &'a mut BudgetLedger,
    domain: WorkDomain,
    request: FamilyRequest,
    work: u64,
    scratch: u64,
    output: u64,
    reservation: u64,
    prerequisites: PrerequisiteWork,
}
impl Attempt<'_> {
    fn work(&mut self, amount: usize) -> ResultIn<()> {
        let next = self
            .work
            .checked_add(amount as u64)
            .ok_or(FamilyError::Capacity)?;
        if next
            .checked_add(self.prerequisites.logical_work)
            .ok_or(FamilyError::Capacity)?
            > self.request.attempt.work_quota
        {
            return Err(Stop::Truncated(ResourceLimit::Work));
        }
        self.work = next;
        Ok(())
    }
    /// Consume one completed prerequisite's already charged output as helper
    /// scratch. This Attempt is the only owner after the checked counter handoff;
    /// no allocation or second retain occurs. The ordinary merge callback may
    /// allocate only through this same Attempt. On unwind the payload drops
    /// before the enclosing Attempt releases its reservation.
    fn with_prerequisite_scratch<T, R>(
        &mut self,
        value: T,
        bytes: u64,
        inspect: impl FnOnce(&T, &mut Self) -> ResultIn<R>,
    ) -> ResultIn<R> {
        let next = self
            .scratch
            .checked_add(bytes)
            .filter(|value| *value <= self.request.scratch_bytes)
            .zip(self.reservation.checked_add(bytes));
        let Some((scratch, reservation)) = next else {
            drop(value);
            self.ledger
                .release(self.domain, bytes)
                .map_err(FamilyError::from)?;
            return Err(FamilyError::Capacity.into());
        };
        self.scratch = scratch;
        self.reservation = reservation;
        let result = inspect(&value, self);
        drop(value);
        self.ledger
            .release(self.domain, bytes)
            .map_err(FamilyError::from)?;
        self.scratch -= bytes;
        self.reservation -= bytes;
        result
    }
    fn prerequisite_work(&mut self, receipt: AnalysisWorkReceipt) -> ResultIn<()> {
        let attempts = self
            .prerequisites
            .attempts
            .checked_add(1)
            .ok_or(FamilyError::Capacity)?;
        let logical_work = self
            .prerequisites
            .logical_work
            .checked_add(receipt.logical_work)
            .ok_or(FamilyError::Capacity)?;
        self.prerequisites = PrerequisiteWork {
            attempts,
            logical_work,
        };
        Ok(())
    }
    fn reserve(&mut self, amount: u64, output: bool) -> ResultIn<()> {
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
            .checked_add(amount)
            .filter(|n| *n <= limit)
            .ok_or(Stop::Truncated(reason))?;
        self.ledger
            .retain(self.domain, amount)
            .map_err(FamilyError::from)?;
        *used = next;
        self.reservation += amount;
        Ok(())
    }
    fn vector<T>(&mut self, capacity: usize, output: bool) -> ResultIn<Vec<T>> {
        let layout = VectorLayout::new(capacity).map_err(vector_error)?;
        self.reserve(layout.bytes(), output)?;
        Ok(layout.allocate().map_err(vector_error)?)
    }
    fn release_vector<T>(&mut self, vector: Vec<T>, output: bool) -> ResultIn<()> {
        let bytes = (vector.capacity() as u64)
            .checked_mul(size_of::<T>() as u64)
            .ok_or(FamilyError::Capacity)?;
        drop(vector);
        self.ledger
            .release(self.domain, bytes)
            .map_err(FamilyError::from)?;
        let used = if output {
            &mut self.output
        } else {
            &mut self.scratch
        };
        *used = used
            .checked_sub(bytes)
            .ok_or_else(|| invalid("helper vector reservation"))?;
        self.reservation = self
            .reservation
            .checked_sub(bytes)
            .ok_or_else(|| invalid("helper reservation release"))?;
        Ok(())
    }
    fn push_into<T>(&mut self, vector: &mut Vec<T>, value: T, output: bool) -> ResultIn<()> {
        self.work(1)?;
        if vector.len() == vector.capacity() {
            self.work(vector.len())?;
            let capacity = vector
                .capacity()
                .max(4)
                .checked_mul(2)
                .ok_or(FamilyError::Capacity)?;
            // Reserve the complete replacement while its predecessor remains
            // admitted, then move elements and release the actual old buffer.
            let mut next = self.vector(capacity, output)?;
            next.append(vector);
            let old = std::mem::replace(vector, next);
            self.release_vector(old, output)?;
        }
        vector.push(value);
        Ok(())
    }
}
fn vector_error(error: AllocationError) -> FamilyError {
    match error {
        AllocationError::Capacity => FamilyError::Capacity,
        AllocationError::AllocationFailed => FamilyError::AllocationFailed,
        AllocationError::Budget(error) => FamilyError::Budget(error),
        AllocationError::Unaccounted | AllocationError::WrongOwner => {
            FamilyError::InvalidProgram("helper vector layout")
        }
    }
}
impl Admission for Attempt<'_> {
    type Error = Stop;
    fn work(&mut self, amount: usize) -> ResultIn<()> {
        Attempt::work(self, amount)
    }
    fn vector<T>(&mut self, capacity: usize) -> ResultIn<Vec<T>> {
        Attempt::vector(self, capacity, false)
    }
    fn push<T>(&mut self, vector: &mut Vec<T>, value: T) -> ResultIn<()> {
        self.push_into(vector, value, false)
    }
    fn release<T>(&mut self, vector: Vec<T>) -> ResultIn<()> {
        self.release_vector(vector, false)
    }
    fn invalid(&self, reason: &'static str) -> Stop {
        invalid(reason)
    }
}
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        self.ledger
            .release(self.domain, self.reservation)
            .expect("helper attempt owns reservation");
    }
}

pub(super) fn prepare(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<Preparation, FamilyError> {
    if request.attempt.plan != HELPER_FAMILY_PLAN
        || request.attempt.algorithm_version != HELPER_FAMILY_VERSION
    {
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
        prerequisites: PrerequisiteWork::default(),
    };
    let result = discover(program, uses, cell, &mut budget);
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
        Ok(mut ready) => {
            ready.family.charge = (domain, budget.output);
            // Other discovery buffers have been dropped. Only these two arrays
            // and the prepared owner header survive while facts are borrowed.
            ready.scratch = (ready.access.capacity() * size_of::<BodyAccess>()
                + ready.domains.capacity() * size_of::<bool>()
                + size_of::<PreparedHelper>()
                - size_of::<HelperFamily>()) as u64;
            budget.reservation -= budget.output + ready.scratch;
            PreparationOutcome::Ready(ready)
        }
        Err(Stop::Unknown(reason)) => PreparationOutcome::Unknown(reason),
        Err(Stop::Truncated(reason)) => PreparationOutcome::Truncated(reason),
        Err(Stop::Error(error)) => return Err(error),
    };
    Ok(Preparation {
        outcome,
        receipt,
        prerequisites: budget.prerequisites,
    })
}

fn dominance(unit: &UnitData, budget: &mut Attempt<'_>) -> ResultIn<StructuredDominance> {
    budget.work(
        unit.operations
            .len()
            .checked_add(unit.regions.len())
            .ok_or(FamilyError::Capacity)?,
    )?;
    let mut parents = budget.vector(unit.regions.len(), false)?;
    parents.resize(unit.regions.len(), None);
    let mut positions = budget.vector(unit.operations.len(), false)?;
    positions.resize(unit.operations.len(), 0);
    StructuredDominance::build(unit, parents, positions, |n| budget.work(n))
}
fn add_unit(
    program: &Program<'_>,
    deps: &mut HelperDependencies,
    id: UnitId,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    budget.work(deps.units.len())?;
    if !deps.units.iter().any(|(other, _)| *other == id) {
        budget.push_into(
            &mut deps.units,
            (
                id,
                program
                    .units
                    .get(id.index())
                    .ok_or_else(|| invalid("helper dependency unit"))?
                    .revision(),
            ),
            true,
        )?;
    }
    Ok(())
}
fn add_cell(
    uses: &UseIndex,
    deps: &mut HelperDependencies,
    id: CellId,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    budget.work(deps.use_sets.len())?;
    if !deps.use_sets.iter().any(|(other, _)| *other == id) {
        budget.push_into(
            &mut deps.use_sets,
            (
                id,
                uses.cell(id)
                    .ok_or_else(|| invalid("helper dependency cell"))?
                    .revision(),
            ),
            true,
        )?;
    }
    Ok(())
}

fn add_creators(
    uses: &UseIndex,
    deps: &mut HelperDependencies,
    body: UnitId,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    budget.work(deps.creator_sets.len())?;
    if !deps.creator_sets.iter().any(|(other, _)| *other == body) {
        budget.push_into(
            &mut deps.creator_sets,
            (
                body,
                uses.creators(body)
                    .ok_or_else(|| invalid("helper dependency creators"))?
                    .revision(),
            ),
            true,
        )?;
    }
    Ok(())
}

fn callable_unknown(reason: callable_inputs::UnknownReason) -> UnknownReason {
    use callable_inputs::UnknownReason as Input;
    match reason {
        Input::ExecutionBoundary => UnknownReason::ObservableFrame,
        Input::NoCreators | Input::NoCalls | Input::NotPrivateCallable => {
            UnknownReason::NotPrivateCallable
        }
        Input::NotClosure | Input::Initialization => UnknownReason::Initialization,
        Input::CallableObservation => UnknownReason::CallableObservation,
        Input::UnsupportedCall => UnknownReason::UnsupportedCall,
        Input::Signature => UnknownReason::Arity,
    }
}

fn discover(
    program: &Program<'_>,
    uses: &UseIndex,
    target: CellId,
    budget: &mut Attempt<'_>,
) -> ResultIn<PreparedHelper> {
    budget.work(
        program
            .units
            .len()
            .checked_add(1)
            .ok_or(FamilyError::Capacity)?,
    )?;
    if !uses.valid_for(program) {
        return Err(FamilyError::StaleUseIndex.into());
    }
    let inputs = match CallableInputs::for_cell_published(
        program,
        uses,
        target,
        CallObservations::from_execution(budget.request.execution),
        budget,
    )? {
        InputOutcome::Complete(inputs) => inputs,
        InputOutcome::Unknown(reason) => return Err(unknown(callable_unknown(reason))),
    };
    // The provider alone owns callable identity and complete explicit uses.
    // Its temporary witnesses remain admitted until the helper has copied the
    // exact physical call plan and dependency closure, including raw domains.
    let result = discover_qualified(program, uses, target, &inputs, budget);
    let released = inputs.discard(budget);
    let prepared = result?;
    released?;
    Ok(prepared)
}

fn discover_qualified(
    program: &Program<'_>,
    uses: &UseIndex,
    target: CellId,
    inputs: &CallableInputs,
    budget: &mut Attempt<'_>,
) -> ResultIn<PreparedHelper> {
    let &[producer] = inputs.producers() else {
        return Err(invalid("helper requires one callable producer"));
    };
    if producer.cell != target || inputs.scope() != InputScope::Producer(producer.creation) {
        return Err(invalid("helper callable producer qualifier"));
    }
    let cell = &program.cells[target.index()];
    let owner = program.unit(cell.owner).unwrap();
    let sites = uses.cell(target).unwrap();
    let body = inputs.body();
    let body_data = program.unit(body).unwrap();
    let initialize = producer.initialize.operation;
    budget.work(1)?;
    // A suspending body completes with a promise or a generator, not with the
    // value its `return` yields, so its call is not its body inlined.
    if body_data.suspension != Suspension::None {
        return Err(unknown(UnknownReason::CompletionShape));
    }
    let tail_return = body_data.regions[body_data.entry.index()]
        .operations
        .last()
        .copied()
        .filter(|id| matches!(body_data.operations[id.index()].kind, OperationKind::Return));
    if tail_return.is_none() {
        // Fallthrough is a real callable completion, not a fabricated Return
        // operation. The existing complete body walk below still rejects every
        // abrupt completion and unsupported nested control-flow shape.
        let signature = body_data
            .callable_type
            .and_then(|id| program.types.get(id.index()));
        let returns_void = match signature {
            Some(Type::Function(signature)) => matches!(*signature.return_type, Type::Void),
            Some(Type::GenericFunction(function)) => {
                matches!(*function.signature.return_type, Type::Void)
            }
            _ => false,
        };
        if !returns_void {
            return Err(unknown(UnknownReason::CompletionShape));
        }
    }
    let participants_bound = sites
        .sites()
        .len()
        .checked_add(2)
        .ok_or(FamilyError::Capacity)?;
    let mut dependency_bound = inputs.dependencies().units().len();
    for &capture in &body_data.captures {
        budget.work(1)?;
        let count = uses
            .cell(capture)
            .ok_or_else(|| invalid("helper capture uses"))?
            .sites()
            .len();
        dependency_bound = dependency_bound
            .checked_add(count)
            .and_then(|n| n.checked_add(2))
            .ok_or(FamilyError::Capacity)?;
    }
    dependency_bound = dependency_bound.min(program.units.len());
    budget.reserve(size_of::<HelperFamily>() as u64, true)?;
    budget.reserve(
        (size_of::<PreparedHelper>() - size_of::<HelperFamily>()) as u64,
        false,
    )?;
    let mut family = HelperFamily {
        root: HelperRoot { cell: target, body },
        closure: producer.creation,
        initialize: producer.initialize,
        tail_return,
        handle_loads: budget.vector(inputs.handle_loads().len(), true)?,
        calls: budget.vector(inputs.calls().len(), true)?,
        environments: budget.vector(participants_bound, true)?,
        captures: budget.vector(body_data.captures.len(), true)?,
        dependencies: HelperDependencies {
            tables: program.tables_revision,
            use_sets: budget.vector(
                body_data
                    .captures
                    .len()
                    .checked_add(inputs.dependencies().cells().len())
                    .ok_or(FamilyError::Capacity)?,
                true,
            )?,
            units: budget.vector(dependency_bound, true)?,
            creator_sets: budget.vector(inputs.dependencies().creators().len(), true)?,
        },
        body_facts: budget.vector(body_data.operations.len(), true)?,
        return_primitive: tail_return.is_none(),
        body_effects: EvaluationBehavior::UNKNOWN,
        execution: budget.request.execution,
        charge: (budget.domain, 0),
    };
    budget.work(body_data.captures.len())?;
    family.captures.extend_from_slice(&body_data.captures);
    for &(unit, _) in inputs.dependencies().units() {
        add_unit(program, &mut family.dependencies, unit, budget)?;
    }
    for &(cell, _) in inputs.dependencies().cells() {
        add_cell(uses, &mut family.dependencies, cell, budget)?;
    }
    for &(body, _) in inputs.dependencies().creators() {
        add_creators(uses, &mut family.dependencies, body, budget)?;
    }
    family.environments.push(HelperEnvironment {
        caller: cell.owner,
        creator: None,
    });
    // Capture topology and initialization order belong to helper substitution,
    // independently of whether the provider proves all callable inputs.
    for site in sites.sites() {
        budget.work(1)?;
        if let CellUseSite::Unit {
            unit,
            usage: CellUse::Capture,
        } = *site
        {
            if unit == body {
                return Err(unknown(UnknownReason::Recursion));
            }
            budget.work(family.environments.len())?;
            if !family.environments.iter().any(|env| env.caller == unit) {
                family.environments.push(HelperEnvironment {
                    caller: unit,
                    creator: None,
                });
            }
        }
    }
    // A verified named module callable is instantiated before every module's
    // ordinary evaluation. Its availability needs neither another prefix parser
    // nor a per-helper dominance table for the initializer's whole body.
    let instantiated = module_instantiated_helper(program, &family, budget)?;
    let owner_dominance = if instantiated {
        None
    } else {
        Some(dominance(owner, budget)?)
    };
    prove_environments(program, uses, &mut family, owner_dominance.as_ref(), budget)?;
    for &load in inputs.handle_loads() {
        budget.work(1)?;
        if load.unit == body {
            return Err(unknown(UnknownReason::Recursion));
        }
        if let Some(dominance) = owner_dominance.as_ref().filter(|_| load.unit == cell.owner) {
            if !dominance.after(owner, initialize, load.operation, |n| budget.work(n))? {
                return Err(unknown(UnknownReason::EarlyCaptureOrRead));
            }
        }
        budget.work(family.environments.len())?;
        if !family
            .environments
            .iter()
            .any(|env| env.caller == load.unit)
        {
            return Err(unknown(UnknownReason::Environment));
        }
        family.handle_loads.push(load);
    }
    for call in inputs.calls() {
        budget.work(
            family
                .environments
                .len()
                .checked_add(1)
                .ok_or(FamilyError::Capacity)?,
        )?;
        let environment = family
            .environments
            .iter()
            .position(|env| env.caller == call.caller)
            .ok_or_else(|| unknown(UnknownReason::Environment))?;
        family.calls.push(HelperCall {
            caller: call.caller,
            load: call.load,
            prepare: call.prepare,
            call: call.call,
            target: call.target,
            environment: u32::try_from(environment).map_err(|_| FamilyError::Capacity)?,
        });
    }
    finish_discovery(program, uses, family, inputs, budget)
}

fn module_initializer(program: &Program<'_>, unit: UnitId) -> bool {
    program.unit(unit).is_some_and(|data| {
        data.kind == UnitKind::ModuleInitialization
            && program
                .modules
                .get(data.module.index())
                .is_some_and(|module| module.initializer == unit)
    })
}

fn module_instantiated_helper(
    program: &Program<'_>,
    family: &HelperFamily,
    budget: &mut Attempt<'_>,
) -> ResultIn<bool> {
    budget.work(1)?;
    let cell = &program.cells[family.root.cell.index()];
    // Program verification owns prefix coverage, exact creation/init pairing,
    // nonescaping prefix values, and the all-prefixes-before-evaluation plan.
    // CallableInputs has independently identified this exact unchanged producer.
    // The table stamp and participating unit stamps retain those qualifications.
    Ok(cell.binding == CellBinding::Function(family.root.body)
        && cell.owner == family.initialize.unit
        && module_initializer(program, cell.owner))
}

fn prove_environments(
    program: &Program<'_>,
    uses: &UseIndex,
    family: &mut HelperFamily,
    owner_dominance: Option<&StructuredDominance>,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    let owner_id = family.initialize.unit;
    let owner = program.unit(owner_id).unwrap();
    for parent in 0..family.environments.len() {
        let parent_id = family.environments[parent].caller;
        for &(creation, child) in uses.unit(parent_id).unwrap().closures() {
            budget.work(
                family
                    .environments
                    .len()
                    .checked_add(1)
                    .ok_or(FamilyError::Capacity)?,
            )?;
            let Some(index) = family
                .environments
                .iter()
                .position(|env| env.caller == child)
            else {
                continue;
            };
            if index == 0 {
                return Err(unknown(UnknownReason::UnrootedCapture));
            }
            let reference = OpRef {
                unit: parent_id,
                operation: creation,
            };
            if family.environments[index]
                .creator
                .replace(reference)
                .is_some()
            {
                return Err(unknown(UnknownReason::Environment));
            }
            if let Some(dominance) = owner_dominance.filter(|_| parent_id == owner_id) {
                if !dominance.after(owner, family.initialize.operation, creation, |n| {
                    budget.work(n)
                })? {
                    return Err(unknown(UnknownReason::EarlyCaptureOrRead));
                }
            }
        }
    }
    let mut reached = budget.vector::<bool>(family.environments.len(), false)?;
    reached.resize(family.environments.len(), false);
    reached[0] = true;
    if owner_dominance.is_none() {
        // These roots share the verified instantiation phase, not a fictitious
        // lexical parent. No ordinary module value receives an initialization
        // guarantee here; capture/record proofs retain their own TDZ obligations.
        for (index, env) in family.environments.iter().enumerate().skip(1) {
            budget.work(1)?;
            if module_initializer(program, env.caller) {
                reached[index] = true;
            }
        }
    }
    for _ in 0..family.environments.len() {
        let mut changed = false;
        for (index, env) in family.environments.iter().enumerate().skip(1) {
            budget.work(
                family
                    .environments
                    .len()
                    .checked_add(1)
                    .ok_or(FamilyError::Capacity)?,
            )?;
            let Some(creator) = env.creator else {
                if reached[index] {
                    continue;
                }
                return Err(unknown(UnknownReason::UnrootedCapture));
            };
            let parent = family
                .environments
                .iter()
                .position(|env| env.caller == creator.unit)
                .ok_or_else(|| unknown(UnknownReason::UnrootedCapture))?;
            if reached[parent] && !reached[index] {
                reached[index] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    budget.work(reached.len())?;
    if reached.iter().any(|value| !*value) {
        return Err(unknown(UnknownReason::UnrootedCapture));
    }
    Ok(())
}

fn finish_discovery(
    program: &Program<'_>,
    uses: &UseIndex,
    mut family: HelperFamily,
    inputs: &CallableInputs,
    budget: &mut Attempt<'_>,
) -> ResultIn<PreparedHelper> {
    let body = program.unit(family.root.body).unwrap();
    let mut access = budget.vector::<BodyAccess>(body.operations.len(), false)?;
    access.resize(body.operations.len(), BodyAccess::None);
    let mut domains = budget.vector::<bool>(body.values.len(), false)?;
    domains.resize(body.values.len(), false);
    budget.work(body.captures.len())?;
    let record_count = body.captures.iter().filter(|cell| {
        matches!(&program.types[program.cells[cell.index()].ty.index()], Type::Record(element) if **element == Type::Int)
    }).count();
    let mut records = budget.vector::<record_family::RecordFamily>(record_count, false)?;
    // Original parameter/capture lists and Initialize occurrences locate the
    // product prerequisites. Do not scan the program-wide cell arena per body.
    budget.work(
        body.captures
            .len()
            .checked_add(body.parameters.len())
            .and_then(|n| n.checked_add(body.operations.len()))
            .ok_or(FamilyError::Capacity)?,
    )?;
    let product_count = body
        .captures
        .iter()
        .copied()
        .chain(body.parameters.iter().copied())
        .chain(body.operations.iter().filter_map(|operation| {
            if let OperationKind::Initialize(cell) = operation.kind {
                Some(cell)
            } else {
                None
            }
        }))
        .filter(|cell| {
            !program.is_reference_parameter(*cell)
                && matches!(
                    program.types[program.cells[cell.index()].ty.index()],
                    Type::Struct(_)
                )
        })
        .count();
    let mut products = budget.vector::<product_family::ProductFamily>(product_count, false)?;
    let mut held_product_bytes = 0u64;
    let mut held_record_bytes = 0u64;
    let prepared = (|| {
        let body_dominance = dominance(body, budget)?;
        budget.work(body.parameters.len())?;
        if body
            .parameters
            .iter()
            .any(|&cell| program.is_reference_parameter(cell))
        {
            reference_prerequisite(program, uses, inputs, &mut family, &mut access, budget)?;
        }
        // Record prerequisites are facts about the semantic graph, irrespective of
        // whether this or another candidate selected scalar storage. No warm proof
        // reuse is asserted: each distinct captured root executes one qualified
        // common record attempt. Its proof stays live through the raw-domain
        // query, then is released before the local-facts query or publication.
        for &capture in &body.captures {
            budget.work(1)?;
            let cell = &program.cells[capture.index()];
            add_cell(uses, &mut family.dependencies, capture, budget)?;
            if matches!(&program.types[cell.ty.index()],Type::Record(element) if **element==Type::Int)
            {
                let record =
                    record_prerequisite(program, uses, capture, &mut family, &mut access, budget)?;
                // The record owner already charged this payload to the same ledger.
                // Count its continued lifetime against this attempt's scratch cap
                // without reserving it a second time. Each next record request is
                // bounded by the remaining scratch after these live prerequisites.
                let bytes = record.retained_bytes();
                let Some(next_scratch) = budget
                    .scratch
                    .checked_add(bytes)
                    .filter(|total| *total <= budget.request.scratch_bytes)
                else {
                    record.discard(budget.ledger).map_err(FamilyError::from)?;
                    return Err(Stop::Truncated(ResourceLimit::Scratch));
                };
                held_record_bytes = held_record_bytes
                    .checked_add(bytes)
                    .ok_or(FamilyError::Capacity)?;
                budget.scratch = next_scratch;
                records.push(record);
            } else if matches!(program.types[cell.ty.index()], Type::Struct(_)) {
                hold_product_prerequisite(
                    program,
                    uses,
                    capture,
                    &mut family,
                    &mut access,
                    &mut products,
                    &mut held_product_bytes,
                    budget,
                )?;
            } else {
                if !supported_scalar_storage_type(&program.types[cell.ty.index()])
                    || cell.binding == CellBinding::Foreign
                {
                    return Err(unknown(UnknownReason::BodyOperation));
                }
                prove_captured_scalar(program, uses, capture, &mut family, budget)?;
            }
        }
        // Parameters own fresh logical storage on entry. Their producer proof
        // follows every sealed actual; a declared struct type is insufficient.
        // Local copies use the same complete component and prerequisite owner.
        budget.work(
            body.parameters
                .len()
                .checked_add(body.operations.len())
                .ok_or(FamilyError::Capacity)?,
        )?;
        for cell in body
            .parameters
            .iter()
            .copied()
            .chain(body.operations.iter().filter_map(|operation| {
                if let OperationKind::Initialize(cell) = operation.kind {
                    Some(cell)
                } else {
                    None
                }
            }))
        {
            budget.work(1)?;
            if !program.is_reference_parameter(cell)
                && matches!(
                    program.types[program.cells[cell.index()].ty.index()],
                    Type::Struct(_)
                )
            {
                hold_product_prerequisite(
                    program,
                    uses,
                    cell,
                    &mut family,
                    &mut access,
                    &mut products,
                    &mut held_product_bytes,
                    budget,
                )?;
            }
        }
        release_products(&mut products, &mut held_product_bytes, budget)?;
        for (index, operation) in body.operations.iter().enumerate() {
            budget.work(1)?;
            let id = OpId::from_index(index).ok_or(FamilyError::Capacity)?;
            if matches!(
                access[index],
                BodyAccess::Product(_) | BodyAccess::Reference(_)
            ) {
                continue;
            }
            match operation.kind {
                OperationKind::Load(place) | OperationKind::Store(place) => {
                    if !matches!(access[index], BodyAccess::None) {
                        continue;
                    }
                    let Place::Cell(cell_id) = body.places[place.index()] else {
                        return Err(unknown(UnknownReason::BodyOperation));
                    };
                    let cell = &program.cells[cell_id.index()];
                    if cell.binding == CellBinding::Foreign
                        || program.is_reference_parameter(cell_id)
                        || uses
                            .cell(cell_id)
                            .is_some_and(|users| users.reference_exposed())
                        || !supported_scalar_storage_type(&program.types[cell.ty.index()])
                    {
                        return Err(unknown(UnknownReason::BodyOperation));
                    }
                    if cell.owner == family.root.body {
                        prove_local_initialized(
                            program,
                            uses,
                            cell_id,
                            family.root.body,
                            id,
                            &body_dominance,
                            budget,
                        )?;
                    } else if !body.captures.contains(&cell_id) {
                        return Err(unknown(UnknownReason::Environment));
                    }
                    access[index] = if matches!(operation.kind, OperationKind::Load(_)) {
                        BodyAccess::ScalarRead(cell_id)
                    } else {
                        BodyAccess::ScalarWrite(cell_id)
                    };
                }
                OperationKind::Initialize(cell_id) => {
                    let cell = &program.cells[cell_id.index()];
                    if cell.owner != family.root.body
                        || !supported_scalar_storage_type(&program.types[cell.ty.index()])
                    {
                        return Err(unknown(UnknownReason::BodyOperation));
                    }
                }
                OperationKind::Constant(_)
                | OperationKind::Allocate {
                    kind: AllocationKind::Struct(_),
                    ..
                }
                | OperationKind::CopyValue
                | OperationKind::IntBinary(_)
                | OperationKind::Binary(_)
                | OperationKind::Unary { .. }
                | OperationKind::ShortCircuit { .. }
                | OperationKind::Select { .. } => {}
                OperationKind::Return
                    if Some(id) == family.tail_return && operation.region == body.entry => {}
                OperationKind::Return
                | OperationKind::Throw
                | OperationKind::Break
                | OperationKind::Continue => return Err(unknown(UnknownReason::CompletionShape)),
                _ => return Err(unknown(UnknownReason::BodyOperation)),
            }
        }
        // The shared raw-value owner joins all writes and the family's complete
        // private call inputs. Source types and initialization never seed domains.
        // Its sparse dependency closure is retained with this publication; no
        // cache/query owner or borrowed proof survives the subsequent facts query.
        let mut roots = budget.vector(body.values.len(), false)?;
        budget.work(body.values.len())?;
        for index in 0..body.values.len() {
            roots.push(Subject::Value {
                unit: family.root.body,
                value: ValueId::from_index(index).ok_or(FamilyError::Capacity)?,
            });
        }
        let mut record_refs = budget.vector(records.len(), false)?;
        budget.work(records.len())?;
        record_refs.extend(records.iter());
        let proof = DomainProof::build(
            program,
            uses,
            &roots,
            DomainInputs::for_helper(&family)
                .with_records(&record_refs)
                .with_execution(budget.request.execution),
            &super::javascript::JavaScriptRecipes,
            budget,
        )?;
        budget.release_vector(record_refs, false)?;
        budget.release_vector(roots, false)?;
        let checked: ResultIn<()> = (|| {
            for (index, domain) in domains.iter_mut().enumerate() {
                *domain = proof.primitive(
                    Subject::Value {
                        unit: family.root.body,
                        value: ValueId::from_index(index).ok_or(FamilyError::Capacity)?,
                    },
                    budget,
                )?;
            }
            for &(unit, _) in proof.dependencies().units() {
                add_unit(program, &mut family.dependencies, unit, budget)?;
            }
            for &(cell, _) in proof.dependencies().cells() {
                add_cell(uses, &mut family.dependencies, cell, budget)?;
            }
            for &(body, _) in proof.dependencies().creators() {
                add_creators(uses, &mut family.dependencies, body, budget)?;
            }
            Ok(())
        })();
        let released = proof.discard(budget);
        checked?;
        released?;
        // Charge the subsequent complete borrowed-facts traversal before it can run.
        // All operations are visited even after one fails; operands/branch results
        // use these preallocated value-domain bits, with no new allocation or query.
        budget.work(
            body.operations
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(body.operands.len()))
                .and_then(|n| n.checked_add(family.dependencies.units.len()))
                .ok_or(FamilyError::Capacity)?,
        )?;
        Ok(PreparedHelper {
            family,
            access,
            domains,
            status: BodyStatus::Unchecked,
            scratch: 0,
        })
    })();
    // No prerequisite survives into publication or the borrowed facts query.
    // Execute every release even if an earlier accounting release failed.
    let mut released = Ok(());
    for record in records.drain(..) {
        let result = record
            .discard(budget.ledger)
            .map_err(FamilyError::from)
            .map_err(Stop::from);
        if released.is_ok() {
            released = result;
        }
    }
    budget.scratch -= held_record_bytes;
    let buffer_released = budget.release_vector(records, false);
    let products_released = release_products(&mut products, &mut held_product_bytes, budget);
    let products_buffer_released = budget.release_vector(products, false);
    released
        .and(buffer_released)
        .and(products_released)
        .and(products_buffer_released)?;
    prepared
}

/// Retain one prerequisite per complete local copy component while qualifying
/// this body. Captures, parameters and locals consume the same evidence path.
fn hold_product_prerequisite(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    family: &mut HelperFamily,
    access: &mut [BodyAccess],
    products: &mut Vec<product_family::ProductFamily>,
    held: &mut u64,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    for product in products.iter() {
        budget.work((usize::BITS - product.cells().len().leading_zeros()) as usize + 1)?;
        if product
            .cells()
            .binary_search_by_key(&cell, |entry| entry.cell)
            .is_ok()
        {
            return Ok(());
        }
    }
    let product = product_prerequisite(program, uses, cell, family, access, budget)?;
    let bytes = product.retained_bytes();
    let Some(next_scratch) = budget
        .scratch
        .checked_add(bytes)
        .filter(|total| *total <= budget.request.scratch_bytes)
    else {
        product.discard(budget.ledger).map_err(FamilyError::from)?;
        return Err(Stop::Truncated(ResourceLimit::Scratch));
    };
    let Some(next_held) = held.checked_add(bytes) else {
        product.discard(budget.ledger).map_err(FamilyError::from)?;
        return Err(FamilyError::Capacity.into());
    };
    // The fixed buffer was admitted from original root occurrences above.
    debug_assert!(products.len() < products.capacity());
    *held = next_held;
    budget.scratch = next_scratch;
    products.push(product);
    Ok(())
}

fn release_products(
    products: &mut Vec<product_family::ProductFamily>,
    held: &mut u64,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    let mut released = Ok(());
    for product in products.drain(..) {
        let result = product
            .discard(budget.ledger)
            .map_err(FamilyError::from)
            .map_err(Stop::from);
        if released.is_ok() {
            released = result;
        }
    }
    budget.scratch -= std::mem::take(held);
    released
}

fn local_initializer(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    budget: &mut Attempt<'_>,
) -> ResultIn<Option<OpId>> {
    let data = &program.cells[cell.index()];
    if program.is_reference_parameter(cell)
        || uses
            .cell(cell)
            .is_some_and(|users| users.reference_exposed())
    {
        return Err(unknown(UnknownReason::BodyOperation));
    }
    let mut initialize = None;
    let mut parameter = false;
    for site in uses
        .cell(cell)
        .ok_or_else(|| invalid("scalar cell uses"))?
        .sites()
    {
        budget.work(1)?;
        match *site {
            CellUseSite::Unit {
                unit,
                usage: CellUse::Initialize(operation),
            } => {
                if unit != data.owner || initialize.replace(operation).is_some() {
                    return Err(unknown(UnknownReason::Initialization));
                }
            }
            CellUseSite::Unit {
                unit,
                usage: CellUse::Parameter(_),
            } if unit == data.owner => parameter = true,
            CellUseSite::Unit {
                usage: CellUse::CatchBinding { .. },
                ..
            } => return Err(unknown(UnknownReason::Initialization)),
            _ => {}
        }
    }
    if parameter {
        if initialize.is_some() {
            return Err(unknown(UnknownReason::Initialization));
        }
        Ok(None)
    } else {
        initialize
            .map(Some)
            .ok_or_else(|| unknown(UnknownReason::Initialization))
    }
}
fn prove_local_initialized(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    unit: UnitId,
    operation: OpId,
    dominance: &StructuredDominance,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    if let Some(initialize) = local_initializer(program, uses, cell, budget)? {
        if !dominance.after(program.unit(unit).unwrap(), initialize, operation, |n| {
            budget.work(n)
        })? {
            return Err(unknown(UnknownReason::EarlyCaptureOrRead));
        }
    }
    Ok(())
}

fn prove_captured_scalar(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    family: &mut HelperFamily,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    let owner = program.cells[cell.index()].owner;
    let initialize = local_initializer(program, uses, cell, budget)?;
    add_unit(program, &mut family.dependencies, owner, budget)?;
    // A helper creation must physically see original storage. Its semantic
    // capture signature already proves lexical access; complete capture-use
    // creation edges below additionally prove initialization for that storage.
    let sites = uses.cell(cell).unwrap().sites();
    let mut participants = budget.vector::<UnitId>(
        sites.len().checked_add(1).ok_or(FamilyError::Capacity)?,
        false,
    )?;
    participants.push(owner);
    for site in sites {
        budget.work(1)?;
        if let CellUseSite::Unit {
            unit,
            usage: CellUse::Capture,
        } = *site
        {
            budget.work(participants.len())?;
            if !participants.contains(&unit) {
                participants.push(unit);
            }
            add_unit(program, &mut family.dependencies, unit, budget)?;
        }
    }
    let owner_data = program.unit(owner).unwrap();
    let owner_dominance = dominance(owner_data, budget)?;
    let mut reached = budget.vector::<bool>(participants.len(), false)?;
    reached.resize(participants.len(), false);
    reached[0] = true;
    budget.work(participants.len())?;
    let edge_bound = participants
        .iter()
        .try_fold(0usize, |sum, id| {
            sum.checked_add(uses.unit(*id).unwrap().closures().len())
        })
        .ok_or(FamilyError::Capacity)?;
    let mut edges = budget.vector::<(usize, usize)>(edge_bound, false)?;
    for (parent, &id) in participants.iter().enumerate() {
        for &(creation, child) in uses.unit(id).unwrap().closures() {
            budget.work(
                participants
                    .len()
                    .checked_add(1)
                    .ok_or(FamilyError::Capacity)?,
            )?;
            let Some(index) = participants.iter().position(|id| *id == child) else {
                continue;
            };
            if index == 0 {
                return Err(unknown(UnknownReason::UnrootedCapture));
            }
            if id == owner {
                if let Some(initialize) = initialize {
                    if !owner_dominance
                        .after(owner_data, initialize, creation, |n| budget.work(n))?
                    {
                        return Err(unknown(UnknownReason::EarlyCaptureOrRead));
                    }
                }
            }
            edges.push((parent, index));
        }
    }
    for _ in 0..participants.len() {
        let mut changed = false;
        for &(parent, child) in &edges {
            budget.work(1)?;
            if reached[parent] && !reached[child] {
                reached[child] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    budget.work(reached.len())?;
    if reached.iter().any(|value| !*value) {
        return Err(unknown(UnknownReason::UnrootedCapture));
    }
    Ok(())
}

fn record_prerequisite(
    program: &Program<'_>,
    uses: &UseIndex,
    state: CellId,
    family: &mut HelperFamily,
    access: &mut [BodyAccess],
    budget: &mut Attempt<'_>,
) -> ResultIn<record_family::RecordFamily> {
    let scratch = budget.request.scratch_bytes - budget.scratch;
    let remaining =
        budget.request.attempt.work_quota - budget.work - budget.prerequisites.logical_work;
    // Both temporary record result and its construction scratch count against
    // this attempt's remaining scratch, not the retained helper output cap.
    let request = record_family::FamilyRequest {
        attempt: AnalysisAttempt {
            plan: record_family::RECORD_FAMILY_PLAN,
            algorithm_version: record_family::RECORD_FAMILY_VERSION,
            work_quota: remaining,
        },
        scratch_bytes: scratch / 2,
        output_bytes: scratch - scratch / 2,
    };
    let analysis = record_family::analyze_published(
        program,
        uses,
        state,
        request,
        budget.ledger,
        budget.domain,
    )
    .map_err(FamilyError::Record)?;
    budget.prerequisite_work(analysis.receipt)?;
    match analysis.outcome {
        record_family::FamilyOutcome::Unknown(_) => {
            Err(unknown(UnknownReason::RequiredRecordEvidence))
        }
        record_family::FamilyOutcome::Truncated(limit) => Err(Stop::Truncated(match limit {
            record_family::ResourceLimit::Work => ResourceLimit::Work,
            record_family::ResourceLimit::Scratch | record_family::ResourceLimit::Output => {
                ResourceLimit::Scratch
            }
        })),
        record_family::FamilyOutcome::Complete(record) => {
            let result = (|| {
                for &(unit, _) in record.dependencies().units() {
                    add_unit(program, &mut family.dependencies, unit, budget)?;
                }
                for handle in record.handle_loads() {
                    budget.work(1)?;
                    if handle.unit == family.root.body {
                        access[handle.operation.index()] = BodyAccess::RecordHandle;
                    }
                }
                for projection in record.projections() {
                    budget.work(1)?;
                    if projection.operation.unit == family.root.body {
                        access[projection.operation.operation.index()] = match projection.kind {
                            record_family::ReadOrWrite::Read => BodyAccess::RecordRead,
                            record_family::ReadOrWrite::Write => BodyAccess::RecordWrite,
                        };
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => Ok(record),
                Err(error) => {
                    record.discard(budget.ledger).map_err(FamilyError::from)?;
                    Err(error)
                }
            }
        }
    }
}

/// The child uses the existing product Attempt. Its callback only writes the
/// helper's already-admitted operation slots. The existing dependency vectors
/// transfer their exact charge to THIS Attempt immediately on success; they
/// never require a retained proof holder or a second ledger-owning wrapper.
fn reference_prerequisite(
    program: &Program<'_>,
    uses: &UseIndex,
    inputs: &CallableInputs,
    family: &mut HelperFamily,
    access: &mut [BodyAccess],
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    let scratch = budget
        .request
        .scratch_bytes
        .checked_sub(budget.scratch)
        .ok_or_else(|| invalid("helper prerequisite scratch"))?;
    let remaining = budget
        .request
        .attempt
        .work_quota
        .checked_sub(budget.work)
        .and_then(|remaining| remaining.checked_sub(budget.prerequisites.logical_work))
        .ok_or(Stop::Truncated(ResourceLimit::Work))?;
    let request = product_family::FamilyRequest {
        execution: budget.request.execution,
        attempt: AnalysisAttempt {
            plan: product_family::PRODUCT_FAMILY_PLAN,
            algorithm_version: product_family::PRODUCT_FAMILY_VERSION,
            work_quota: remaining,
        },
        scratch_bytes: scratch / 2,
        output_bytes: scratch - scratch / 2,
    };
    let body = family.root.body;
    let mut inherited = 0;
    let analysis = product_family::run(
        request,
        product_family::PRODUCT_FAMILY_PLAN,
        product_family::PRODUCT_FAMILY_VERSION,
        budget.ledger,
        budget.domain,
        |child| {
            product_family::certify_reference_parameters(
                program,
                uses,
                inputs,
                child,
                |operation, witness| {
                    if operation.unit != body {
                        return Err(product_family::invalid("helper reference witness body"));
                    }
                    let slot = access.get_mut(operation.operation.index()).ok_or_else(|| {
                        product_family::invalid("helper reference witness operation")
                    })?;
                    if !matches!(slot, BodyAccess::None) {
                        return Err(product_family::invalid(
                            "helper reference witness collision",
                        ));
                    }
                    *slot = BodyAccess::Reference(witness);
                    Ok(())
                },
            )
        },
        |_, (_, bytes)| inherited = bytes,
    )
    .map_err(FamilyError::Product)?;
    match analysis.outcome {
        product_family::Outcome::Complete(dependencies) => {
            budget.with_prerequisite_scratch(dependencies, inherited, |dependencies, budget| {
                budget.prerequisite_work(analysis.receipt)?;
                for &(unit, _) in dependencies.units() {
                    add_unit(program, &mut family.dependencies, unit, budget)?;
                }
                for &(cell, _) in dependencies.cells() {
                    add_cell(uses, &mut family.dependencies, cell, budget)?;
                }
                for &(creator, _) in dependencies.creators() {
                    add_creators(uses, &mut family.dependencies, creator, budget)?;
                }
                Ok(())
            })
        }
        product_family::Outcome::Unknown(_) => {
            debug_assert_eq!(inherited, 0);
            budget.prerequisite_work(analysis.receipt)?;
            Err(unknown(UnknownReason::RequiredProductEvidence))
        }
        product_family::Outcome::Truncated(limit) => {
            debug_assert_eq!(inherited, 0);
            budget.prerequisite_work(analysis.receipt)?;
            Err(Stop::Truncated(match limit {
                product_family::ResourceLimit::Work => ResourceLimit::Work,
                product_family::ResourceLimit::Scratch | product_family::ResourceLimit::Output => {
                    ResourceLimit::Scratch
                }
            }))
        }
    }
}
fn product_prerequisite(
    program: &Program<'_>,
    uses: &UseIndex,
    state: CellId,
    family: &mut HelperFamily,
    access: &mut [BodyAccess],
    budget: &mut Attempt<'_>,
) -> ResultIn<product_family::ProductFamily> {
    let scratch = budget.request.scratch_bytes - budget.scratch;
    let remaining =
        budget.request.attempt.work_quota - budget.work - budget.prerequisites.logical_work;
    let analysis = product_family::analyze_published(
        program,
        uses,
        state,
        product_family::FamilyRequest {
            execution: budget.request.execution,
            attempt: AnalysisAttempt {
                plan: product_family::PRODUCT_FAMILY_PLAN,
                algorithm_version: product_family::PRODUCT_FAMILY_VERSION,
                work_quota: remaining,
            },
            scratch_bytes: scratch / 2,
            output_bytes: scratch - scratch / 2,
        },
        budget.ledger,
        budget.domain,
    )
    .map_err(FamilyError::Product)?;
    budget.prerequisite_work(analysis.receipt)?;
    match analysis.outcome {
        product_family::FamilyOutcome::Unknown(_) => {
            Err(unknown(UnknownReason::RequiredProductEvidence))
        }
        product_family::FamilyOutcome::Truncated(limit) => Err(Stop::Truncated(match limit {
            product_family::ResourceLimit::Work => ResourceLimit::Work,
            product_family::ResourceLimit::Scratch | product_family::ResourceLimit::Output => {
                ResourceLimit::Scratch
            }
        })),
        product_family::FamilyOutcome::Complete(product) => {
            let result = (|| {
                for &(unit, _) in product.dependencies().units() {
                    add_unit(program, &mut family.dependencies, unit, budget)?;
                }
                for &(cell, _) in product.dependencies().cells() {
                    add_cell(uses, &mut family.dependencies, cell, budget)?;
                }
                for &(creator, _) in product.dependencies().creators() {
                    add_creators(uses, &mut family.dependencies, creator, budget)?;
                }
                for operation in product.operations() {
                    budget.work(1)?;
                    if operation.operation.unit == family.root.body {
                        access[operation.operation.operation.index()] =
                            BodyAccess::Product(operation.kind);
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => Ok(product),
                Err(error) => {
                    product.discard(budget.ledger).map_err(FamilyError::from)?;
                    Err(error)
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "helper_reference_tests.rs"]
mod reference_tests;

#[cfg(test)]
#[path = "helper_completion_tests.rs"]
mod completion_tests;
