//! Bounded evidence for one private captured Record<int> representation family.
//!
//! This analysis leaves all semantic allocation, place and capture operations
//! intact. It proves one deliberately narrow implementation opportunity, not a
//! scalarization, tactic permission or compression win. Only the compilation
//! owner may retain its result, and must discard it through the original ledger.

use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, AnalysisWorkReceipt, BudgetError, BudgetLedger, WorkDomain,
};
use std::mem::size_of;

pub(super) const RECORD_FAMILY_PLAN: u32 = 2;
pub(super) const RECORD_FAMILY_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OpRef {
    pub unit: UnitId,
    pub operation: OpId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecordRoot {
    pub unit: UnitId,
    pub allocation: AllocationId,
    pub state: CellId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Activation {
    pub unit: UnitId,
    pub region: RegionId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecordSlot {
    pub key: StringId,
    /// Value in the allocation's unit; None denotes an absent key's null slot.
    pub initial_value: Option<ValueId>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadOrWrite {
    Read,
    Write,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Projection {
    pub operation: OpRef,
    pub slot: u32,
    pub kind: ReadOrWrite,
}

#[derive(Debug)]
pub(super) struct FamilyDependencies {
    tables: RevisionId,
    state: CellId,
    state_uses: RevisionId,
    units: Vec<(UnitId, RevisionId)>,
}
impl FamilyDependencies {
    /// The caller accounts for revision validation. Checking the complete index
    /// stamp is essential: an otherwise unrelated unit may add a new state use.
    pub(super) fn valid_for(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        uses.valid_for(program) && self.valid_for_published(program, uses)
    }
    /// Only the common publication owner may omit complete-index validation:
    /// it already guarantees that this index belongs to the checked snapshot.
    pub(super) fn valid_for_published(&self, program: &Program<'_>, uses: &UseIndex) -> bool {
        self.tables == program.tables_revision
            && uses
                .cell(self.state)
                .is_some_and(|cell| cell.revision() == self.state_uses)
            && self.units.iter().all(|(id, revision)| {
                program
                    .units
                    .get(id.index())
                    .is_some_and(|unit| unit.revision() == *revision)
            })
    }
    pub(super) fn units(&self) -> &[(UnitId, RevisionId)] {
        &self.units
    }
}

/// Constructors and fields stay private to this analysis. Getters expose only
/// semantic identities. There is no JS binding, emitted name or mutable proof.
#[must_use = "retain family evidence in its compilation owner or discard it through its original ledger"]
#[derive(Debug)]
pub(super) struct RecordFamily {
    root: RecordRoot,
    allocation: OpRef,
    initialize: OpRef,
    activation: Activation,
    slots: Vec<RecordSlot>,
    handle_loads: Vec<OpRef>,
    projections: Vec<Projection>,
    captures: Vec<UnitId>,
    dependencies: FamilyDependencies,
    charge: (WorkDomain, u64),
}
impl RecordFamily {
    pub(super) fn root(&self) -> RecordRoot {
        self.root
    }
    pub(super) fn allocation(&self) -> OpRef {
        self.allocation
    }
    pub(super) fn initialize(&self) -> OpRef {
        self.initialize
    }
    pub(super) fn activation(&self) -> Activation {
        self.activation
    }
    pub(super) fn slots(&self) -> &[RecordSlot] {
        &self.slots
    }
    pub(super) fn handle_loads(&self) -> &[OpRef] {
        &self.handle_loads
    }
    pub(super) fn projections(&self) -> &[Projection] {
        &self.projections
    }
    pub(super) fn captures(&self) -> &[UnitId] {
        &self.captures
    }
    pub(super) fn dependencies(&self) -> &FamilyDependencies {
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
    pub attempt: AnalysisAttempt,
    pub scratch_bytes: u64,
    pub output_bytes: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    NotLocalIntegerRecord,
    Initialization,
    Reassigned,
    WholeValueUse,
    AllocationUse,
    DynamicKey,
    UnsupportedProjection,
    DuplicateKey,
    EarlyCaptureOrRead,
    UnrootedCapture,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    Work,
    Scratch,
    Output,
}
#[derive(Debug)]
pub(super) enum FamilyOutcome {
    Complete(RecordFamily),
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
}
#[derive(Debug)]
pub(super) struct FamilyAnalysis {
    pub outcome: FamilyOutcome,
    pub receipt: AnalysisWorkReceipt,
}
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
}
impl Attempt<'_> {
    fn work(&mut self, amount: usize) -> ResultIn<()> {
        self.work = self
            .work
            .checked_add(amount as u64)
            .filter(|n| *n <= self.request.attempt.work_quota)
            .ok_or(Stop::Truncated(ResourceLimit::Work))?;
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
        *used = used
            .checked_add(amount)
            .filter(|n| *n <= limit)
            .ok_or(Stop::Truncated(reason))?;
        Ok(())
    }
    fn vector<T>(&mut self, capacity: usize, output: bool) -> ResultIn<Vec<T>> {
        let bytes = (capacity as u64)
            .checked_mul(size_of::<T>() as u64)
            .ok_or(FamilyError::Capacity)?;
        self.reserve(bytes, output)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| FamilyError::AllocationFailed)?;
        Ok(values)
    }
}
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        self.ledger
            .release(self.domain, self.reservation)
            .expect("record family attempt owns its remaining reservation");
    }
}

/// Every call is one deterministic attempt; no hidden cache or permission read.
/// Global budget admission precedes construction. Local work/scratch/output
/// exhaustion returns Truncated, distinct from an unsupported family (Unknown).
pub(super) fn analyze(
    program: &Program<'_>,
    uses: &UseIndex,
    state: CellId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<FamilyAnalysis, FamilyError> {
    analyze_impl(program, uses, state, request, ledger, domain, false)
}

/// The common snapshot owner (or an analysis that has already checked that
/// same immutable snapshot) supplies coherence; all family work remains the
/// same algorithm. Independent callers retain the checked `analyze` route.
pub(super) fn analyze_published(
    program: &Program<'_>,
    uses: &UseIndex,
    state: CellId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<FamilyAnalysis, FamilyError> {
    analyze_impl(program, uses, state, request, ledger, domain, true)
}

fn analyze_impl(
    program: &Program<'_>,
    uses: &UseIndex,
    state: CellId,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
    coherent: bool,
) -> Result<FamilyAnalysis, FamilyError> {
    if request.attempt.plan != RECORD_FAMILY_PLAN
        || request.attempt.algorithm_version != RECORD_FAMILY_VERSION
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
    let reservation = request
        .scratch_bytes
        .checked_add(request.output_bytes)
        .ok_or(FamilyError::Capacity)?;
    ledger.retain(domain, reservation)?;
    let mut attempt = Attempt {
        ledger,
        domain,
        request,
        work: 0,
        scratch: 0,
        output: 0,
        reservation,
    };
    let result = discover(program, uses, state, &mut attempt, coherent);
    let completion = if matches!(result, Err(Stop::Truncated(_))) {
        AnalysisCompletion::Truncated
    } else {
        AnalysisCompletion::Complete
    };
    let receipt = AnalysisWorkReceipt {
        attempt: request.attempt,
        completion,
        logical_work: attempt.work,
    };
    // No other spender has access to the ledger during this admitted attempt.
    attempt
        .ledger
        .charge_analysis(domain, request.attempt, receipt)?;
    let outcome = match result {
        Ok(mut family) => {
            family.charge = (domain, attempt.output);
            attempt.reservation -= attempt.output;
            FamilyOutcome::Complete(family)
        }
        Err(Stop::Unknown(reason)) => FamilyOutcome::Unknown(reason),
        Err(Stop::Truncated(reason)) => FamilyOutcome::Truncated(reason),
        Err(Stop::Error(error)) => return Err(error),
    };
    Ok(FamilyAnalysis { outcome, receipt })
}

fn discover(
    program: &Program<'_>,
    uses: &UseIndex,
    state: CellId,
    budget: &mut Attempt<'_>,
    coherent: bool,
) -> ResultIn<RecordFamily> {
    budget.work(1)?;
    if !coherent {
        budget.work(program.units.len())?;
        if !uses.valid_for(program) {
            return Err(FamilyError::StaleUseIndex.into());
        }
    }
    let cell = program
        .cells
        .get(state.index())
        .ok_or_else(|| invalid("record cell"))?;
    if cell.binding != CellBinding::Local
        || !matches!(program.types.get(cell.ty.index()), Some(Type::Record(element)) if **element == Type::Int)
    {
        return Err(unknown(UnknownReason::NotLocalIntegerRecord));
    }
    let owner = program
        .unit(cell.owner)
        .ok_or_else(|| invalid("record owner"))?;
    let sites = uses
        .cell(state)
        .ok_or_else(|| invalid("record cell uses"))?;
    let mut initializer = None;
    let mut projection_bound = 0usize;
    for site in sites.sites() {
        budget.work(1)?;
        match *site {
            CellUseSite::Export { .. } => return Err(unknown(UnknownReason::WholeValueUse)),
            CellUseSite::Unit {
                unit,
                usage: CellUse::Initialize(op),
            } => {
                if unit != cell.owner || initializer.replace(op).is_some() {
                    return Err(unknown(UnknownReason::Initialization));
                }
            }
            CellUseSite::Unit {
                usage: CellUse::Write { .. } | CellUse::Reference { .. },
                ..
            } => return Err(unknown(UnknownReason::Reassigned)),
            CellUseSite::Unit {
                usage: CellUse::Parameter(_) | CellUse::CatchBinding { .. },
                ..
            } => {
                return Err(unknown(UnknownReason::Initialization));
            }
            CellUseSite::Unit {
                unit,
                usage: CellUse::Read { operation, place },
            } => {
                let data = program
                    .unit(unit)
                    .ok_or_else(|| invalid("record read unit"))?;
                let op = data
                    .operations
                    .get(operation.index())
                    .ok_or_else(|| invalid("record read operation"))?;
                if !matches!(op.kind, OperationKind::Load(id) if id == place)
                    || data.places.get(place.index()) != Some(&Place::Cell(state))
                {
                    return Err(unknown(UnknownReason::UnsupportedProjection));
                }
                let value = op.result.ok_or_else(|| invalid("record read result"))?;
                let uses = uses
                    .unit(unit)
                    .and_then(|uses| uses.value_uses(value))
                    .ok_or_else(|| invalid("record value uses"))?;
                projection_bound = projection_bound
                    .checked_add(uses.len())
                    .ok_or(FamilyError::Capacity)?;
            }
            CellUseSite::Unit {
                usage: CellUse::Capture,
                ..
            } => {}
        }
    }
    let initialize = initializer.ok_or_else(|| unknown(UnknownReason::Initialization))?;
    let init = owner
        .operations
        .get(initialize.index())
        .ok_or_else(|| invalid("record initialize"))?;
    if init.region != cell.region {
        return Err(unknown(UnknownReason::Initialization));
    }
    let &[allocated] = owner
        .operands(init.operands)
        .ok_or_else(|| invalid("record initializer operand"))?
    else {
        return Err(invalid("record initializer arity"));
    };
    let allocation = owner
        .values
        .get(allocated.index())
        .ok_or_else(|| invalid("record allocation value"))?
        .definition;
    let alloc = owner
        .operations
        .get(allocation.index())
        .ok_or_else(|| invalid("record allocation operation"))?;
    let OperationKind::Allocate {
        identity,
        kind: AllocationKind::Record(keys),
    } = &alloc.kind
    else {
        return Err(unknown(UnknownReason::Initialization));
    };
    if alloc.region != cell.region {
        return Err(unknown(UnknownReason::Initialization));
    }
    let values = owner
        .operands(alloc.operands)
        .ok_or_else(|| invalid("record initial fields"))?;
    if keys.len() != values.len() {
        return Err(invalid("record field arity"));
    }
    let allocation_uses = uses
        .unit(cell.owner)
        .and_then(|uses| uses.value_uses(allocated))
        .ok_or_else(|| invalid("allocation uses"))?;
    budget.work(allocation_uses.len())?;
    if allocation_uses
        != [ValueUse::Operand {
            operation: initialize,
            position: 0,
        }]
    {
        return Err(unknown(UnknownReason::AllocationUse));
    }

    let participants_bound = sites
        .sites()
        .len()
        .checked_add(1)
        .ok_or(FamilyError::Capacity)?;
    let slot_bound = keys
        .len()
        .checked_add(projection_bound)
        .ok_or(FamilyError::Capacity)?;
    if slot_bound > u32::MAX as usize {
        return Err(FamilyError::Capacity.into());
    }
    budget.reserve(size_of::<RecordFamily>() as u64, true)?;
    let mut slots = budget.vector::<RecordSlot>(slot_bound, true)?;
    let mut handle_loads = budget.vector::<OpRef>(sites.sites().len(), true)?;
    let mut projections = budget.vector::<Projection>(projection_bound, true)?;
    let mut captures = budget.vector::<UnitId>(sites.sites().len(), true)?;
    let mut participants = budget.vector::<(UnitId, RevisionId)>(participants_bound, true)?;
    participants.push((cell.owner, program.units[cell.owner.index()].revision()));
    for (&key, &initial_value) in keys.iter().zip(values) {
        let slot = slot_for(program, &mut slots, key, budget)?;
        if slots[slot as usize]
            .initial_value
            .replace(initial_value)
            .is_some()
        {
            return Err(unknown(UnknownReason::DuplicateKey));
        }
    }
    let dominance = dominance(owner, budget)?;
    for site in sites.sites() {
        budget.work(1)?;
        let CellUseSite::Unit { unit, usage } = *site else {
            unreachable!()
        };
        add_participant(program, &mut participants, unit, budget)?;
        match usage {
            CellUse::Capture => captures.push(unit),
            CellUse::Read { operation, .. } => {
                if unit == cell.owner
                    && !dominance.after(owner, initialize, operation, |n| budget.work(n))?
                {
                    return Err(unknown(UnknownReason::EarlyCaptureOrRead));
                }
                let data = program
                    .unit(unit)
                    .ok_or_else(|| invalid("projection unit"))?;
                let handle = data.operations[operation.index()].result.unwrap();
                handle_loads.push(OpRef { unit, operation });
                for usage in uses.unit(unit).unwrap().value_uses(handle).unwrap() {
                    budget.work(1)?;
                    let ValueUse::PlaceReceiver { operation, place } = *usage else {
                        return Err(unknown(UnknownReason::WholeValueUse));
                    };
                    let projection = data
                        .operations
                        .get(operation.index())
                        .ok_or_else(|| invalid("projection operation"))?;
                    let kind = match projection.kind {
                        OperationKind::Load(id) if id == place => ReadOrWrite::Read,
                        OperationKind::Store(id) if id == place => ReadOrWrite::Write,
                        _ => return Err(unknown(UnknownReason::UnsupportedProjection)),
                    };
                    let key = match data
                        .places
                        .get(place.index())
                        .ok_or_else(|| invalid("projection place"))?
                    {
                        Place::Member { receiver, key } if *receiver == handle => *key,
                        Place::Index { receiver, key } if *receiver == handle => {
                            let definition = data
                                .values
                                .get(key.index())
                                .ok_or_else(|| invalid("projection key"))?
                                .definition;
                            match data.operations.get(definition.index()).map(|op| &op.kind) {
                                Some(OperationKind::Constant(Constant::String(key))) => *key,
                                _ => return Err(unknown(UnknownReason::DynamicKey)),
                            }
                        }
                        _ => return Err(unknown(UnknownReason::UnsupportedProjection)),
                    };
                    let slot = slot_for(program, &mut slots, key, budget)?;
                    projections.push(Projection {
                        operation: OpRef { unit, operation },
                        slot,
                        kind,
                    });
                }
            }
            CellUse::Initialize(_) => {}
            _ => unreachable!(),
        }
    }
    prove_captures(
        program,
        uses,
        cell.owner,
        initialize,
        &participants,
        &captures,
        &dominance,
        budget,
    )?;
    Ok(RecordFamily {
        root: RecordRoot {
            unit: cell.owner,
            allocation: *identity,
            state,
        },
        allocation: OpRef {
            unit: cell.owner,
            operation: allocation,
        },
        initialize: OpRef {
            unit: cell.owner,
            operation: initialize,
        },
        activation: Activation {
            unit: cell.owner,
            region: cell.region,
        },
        slots,
        handle_loads,
        projections,
        captures,
        dependencies: FamilyDependencies {
            tables: program.tables_revision,
            state,
            state_uses: sites.revision(),
            units: participants,
        },
        charge: (budget.domain, 0),
    })
}

fn add_participant(
    program: &Program<'_>,
    units: &mut Vec<(UnitId, RevisionId)>,
    id: UnitId,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    budget.work(units.len())?;
    if !units.iter().any(|(unit, _)| *unit == id) {
        let unit = program
            .units
            .get(id.index())
            .ok_or_else(|| invalid("participant unit"))?;
        units.push((id, unit.revision()));
    }
    Ok(())
}

fn slot_for(
    program: &Program<'_>,
    slots: &mut Vec<RecordSlot>,
    key: StringId,
    budget: &mut Attempt<'_>,
) -> ResultIn<u32> {
    let value = program
        .strings
        .get(key.index())
        .ok_or_else(|| invalid("record key string"))?;
    budget.work(1)?;
    for (index, slot) in slots.iter().enumerate() {
        let other = &program.strings[slot.key.index()];
        budget.work(
            value
                .storage_bytes()
                .checked_add(other.storage_bytes())
                .ok_or(FamilyError::Capacity)?,
        )?;
        // Compare contents rather than relying on interning remaining canonical
        // after a future source table edit, including UTF-16 surrogate contents.
        if value.code_units().eq(other.code_units()) {
            return Ok(index as u32);
        }
    }
    let index = u32::try_from(slots.len()).map_err(|_| FamilyError::Capacity)?;
    slots.push(RecordSlot {
        key,
        initial_value: None,
    });
    Ok(index)
}

type Dominance = super::activation::StructuredDominance;
fn dominance(unit: &UnitData, budget: &mut Attempt<'_>) -> ResultIn<Dominance> {
    budget.work(
        unit.regions
            .len()
            .checked_add(unit.operations.len())
            .ok_or(FamilyError::Capacity)?,
    )?;
    let mut parents = budget.vector(unit.regions.len(), false)?;
    parents.resize(unit.regions.len(), None);
    let mut positions = budget.vector(unit.operations.len(), false)?;
    positions.resize(unit.operations.len(), 0);
    Dominance::build(unit, parents, positions, |n| budget.work(n))
}

fn prove_captures(
    program: &Program<'_>,
    uses: &UseIndex,
    owner: UnitId,
    initialize: OpId,
    participants: &[(UnitId, RevisionId)],
    captures: &[UnitId],
    dominance: &Dominance,
    budget: &mut Attempt<'_>,
) -> ResultIn<()> {
    let mut edge_bound = 0usize;
    for (unit, _) in participants {
        budget.work(1)?;
        edge_bound = edge_bound
            .checked_add(uses.unit(*unit).unwrap().closures().len())
            .ok_or(FamilyError::Capacity)?;
    }
    let mut edges = budget.vector::<(usize, usize)>(edge_bound, false)?;
    let mut reached = budget.vector::<bool>(participants.len(), false)?;
    reached.resize(participants.len(), false);
    reached[0] = true;
    for (parent, (id, _)) in participants.iter().enumerate() {
        if *id != owner {
            budget.work(captures.len())?;
            if !captures.contains(id) {
                return Err(invalid("participant has no state capture"));
            }
        }
        for &(operation, child) in uses.unit(*id).unwrap().closures() {
            budget.work(captures.len().checked_add(1).ok_or(FamilyError::Capacity)?)?;
            if !captures.contains(&child) {
                continue;
            }
            if *id == owner
                && !dominance.after(program.unit(owner).unwrap(), initialize, operation, |n| {
                    budget.work(n)
                })?
            {
                return Err(unknown(UnknownReason::EarlyCaptureOrRead));
            }
            budget.work(participants.len())?;
            let child = participants
                .iter()
                .position(|(id, _)| *id == child)
                .ok_or_else(|| invalid("capture participant"))?;
            edges.push((parent, child));
        }
    }
    loop {
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
    if reached.iter().any(|value| !value) {
        return Err(unknown(UnknownReason::UnrootedCapture));
    }
    Ok(())
}
