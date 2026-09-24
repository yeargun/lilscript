//! Complete value/cell occurrences owned by semantic publication.
//!
//! These are structural uses, not execution order, liveness or escape proofs.
//! Local values use contiguous lists; only unit and nonempty cell lists share
//! storage. Updating a checked program scans revision metadata, changed bodies
//! and affected cell lists. It does not rescan unrelated bodies.
//!
//! Each allocation is charged once to its original ledger domain. `discard`
//! releases shared storage only at its last owner; ordinary Drop conservatively
//! leaves the reservation charged. Owned shared chunks never leave this module.

use super::*;
use crate::compilation_policy::{BudgetError, BudgetLedger, WorkDomain, WorkKind};
use std::mem::size_of;
use std::sync::Arc;

#[path = "uses_update.rs"]
mod update;
pub(super) use update::PreparedUseUpdate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueUse {
    Operand {
        operation: OpId,
        position: u32,
    },
    CallArgument {
        operation: OpId,
        call: CallId,
        position: u32,
    },
    PlaceReceiver {
        operation: OpId,
        place: PlaceId,
    },
    PlaceKey {
        operation: OpId,
        place: PlaceId,
    },
    CallCallee {
        prepare: OpId,
        call: CallId,
    },
    CallReceiver {
        prepare: OpId,
        call: CallId,
    },
    RegionResult(RegionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CellUse {
    Initialize(OpId),
    Read {
        operation: OpId,
        place: PlaceId,
    },
    Write {
        operation: OpId,
        place: PlaceId,
    },
    /// Exposes this storage root to the callee, including fixed field aliases.
    Reference {
        operation: OpId,
        place: PlaceId,
        call: CallId,
        position: u32,
    },
    Parameter(u32),
    Capture,
    CatchBinding {
        operation: OpId,
        region: RegionId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellReference {
    pub cell: CellId,
    pub usage: CellUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CellUseSite {
    Unit { unit: UnitId, usage: CellUse },
    Export { index: u32 },
}

/// A complete structural occurrence that can instantiate a callable body.
/// The creator unit revision separately qualifies its operands and schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClosureCreation {
    pub unit: UnitId,
    pub operation: OpId,
}

#[derive(Debug)]
pub struct UnitUses {
    revision: RevisionId,
    offsets: Vec<usize>,
    values: Vec<ValueUse>,
    cells: Vec<CellReference>,
    closures: Vec<(OpId, UnitId)>,
    /// Indexed by this unit's CallId. Option uses the compact OpId niche and
    /// lets construction reject missing/duplicate invocation sites safely.
    call_operations: Vec<Option<OpId>>,
}

impl UnitUses {
    pub fn revision(&self) -> RevisionId {
        self.revision
    }
    pub fn value_uses(&self, value: ValueId) -> Option<&[ValueUse]> {
        let start = *self.offsets.get(value.index())?;
        let end = *self.offsets.get(value.index() + 1)?;
        self.values.get(start..end)
    }
    pub fn cell_uses(&self) -> &[CellReference] {
        &self.cells
    }
    pub fn closures(&self) -> &[(OpId, UnitId)] {
        &self.closures
    }
    /// The unique invocation consuming a prepared call in this unit revision.
    /// Call IDs follow preparation order, which can differ from invocation
    /// order when argument evaluation contains nested calls.
    pub fn call_operation(&self, call: CallId) -> Option<OpId> {
        self.call_operations.get(call.index()).copied().flatten()
    }
}

#[derive(Debug, Clone, Copy)]
struct Charge {
    domain: WorkDomain,
    bytes: u64,
}

// Only the two concrete index payloads use this private ownership helper.
#[derive(Debug)]
struct Shared<T> {
    data: T,
    charge: Charge,
}

#[derive(Debug)]
pub struct CellUsers {
    revision: RevisionId,
    reference_exposed: bool,
    sites: Option<Arc<Shared<Vec<CellUseSite>>>>,
}

impl CellUsers {
    pub fn reference_exposed(&self) -> bool {
        self.reference_exposed
    }
    pub fn revision(&self) -> RevisionId {
        self.revision
    }
    pub fn sites(&self) -> &[CellUseSite] {
        self.sites
            .as_ref()
            .map_or(&[], |sites| sites.data.as_slice())
    }
    fn empty() -> Self {
        Self {
            revision: RevisionId::fresh(),
            reference_exposed: false,
            sites: None,
        }
    }
    fn share(&self) -> Self {
        Self {
            revision: self.revision,
            reference_exposed: self.reference_exposed,
            sites: self.sites.clone(),
        }
    }
}

#[derive(Debug)]
pub struct CreatorUsers {
    revision: RevisionId,
    sites: Option<Arc<Shared<Vec<ClosureCreation>>>>,
}
impl CreatorUsers {
    pub fn revision(&self) -> RevisionId {
        self.revision
    }
    pub fn sites(&self) -> &[ClosureCreation] {
        self.sites
            .as_ref()
            .map_or(&[], |sites| sites.data.as_slice())
    }
    fn empty() -> Self {
        Self {
            revision: RevisionId::fresh(),
            sites: None,
        }
    }
    fn share(&self) -> Self {
        Self {
            revision: self.revision,
            sites: self.sites.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UseIndexReceipt {
    pub logical_work: u64,
    /// New retained payload, including this snapshot's vectors; shared chunks
    /// are charged only to the build that originally allocated them.
    pub allocated_bytes: u64,
    pub rebuilt_units: usize,
    pub rebuilt_cell_sets: usize,
    pub rebuilt_creator_sets: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseError {
    Budget(BudgetError),
    Capacity,
    InvalidProgram(&'static str),
    InvalidChangeSet,
    UndeclaredUnitChange(UnitId),
    UnstampedTables,
    /// Arena identities cannot be shifted/removed by an incremental update.
    /// A checked source replacement can build a new index for that shape.
    ShrinkingTables,
}

impl From<BudgetError> for UseError {
    fn from(value: BudgetError) -> Self {
        Self::Budget(value)
    }
}

#[must_use = "retain the index in its publication owner, or discard it through its ledger"]
#[derive(Debug)]
pub struct UseIndex {
    // Private destination/version guard for prepared installation only.
    update_owner: RevisionId,
    tables_revision: RevisionId,
    units: Vec<Arc<Shared<UnitUses>>>,
    cells: Vec<CellUsers>,
    creators: Vec<CreatorUsers>,
    exports: Vec<(u32, CellId)>,
    charge: Charge,
    receipt: UseIndexReceipt,
}

impl UseIndex {
    pub fn unit(&self, unit: UnitId) -> Option<&UnitUses> {
        self.units.get(unit.index()).map(|unit| &unit.data)
    }
    pub fn cell(&self, cell: CellId) -> Option<&CellUsers> {
        self.cells.get(cell.index())
    }
    pub fn creators(&self, body: UnitId) -> Option<&CreatorUsers> {
        self.creators.get(body.index())
    }
    pub fn tables_revision(&self) -> RevisionId {
        self.tables_revision
    }
    pub fn receipt(&self) -> UseIndexReceipt {
        self.receipt
    }
    /// Reachable payload, not a second reservation for shared chunks. Allocator
    /// bookkeeping and process RSS are measured separately from payload bytes.
    pub fn retained_bytes(&self) -> u64 {
        self.charge.bytes
            + self.units.iter().map(|unit| unit.charge.bytes).sum::<u64>()
            + self
                .cells
                .iter()
                .filter_map(|cell| cell.sites.as_ref())
                .map(|sites| sites.charge.bytes)
                .sum::<u64>()
            + self
                .creators
                .iter()
                .filter_map(|creators| creators.sites.as_ref())
                .map(|sites| sites.charge.bytes)
                .sum::<u64>()
    }
    pub fn valid_for(&self, program: &Program<'_>) -> bool {
        self.tables_revision == program.tables_revision
            && self.units.len() == program.units.len()
            && self.cells.len() == program.cells.len()
            && self.creators.len() == program.units.len()
            && self
                .units
                .iter()
                .zip(&program.units)
                .all(|(uses, unit)| uses.data.revision == unit.revision())
    }

    pub fn build(
        program: &Program<'_>,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, UseError> {
        let start = ledger.work_used(domain);
        let mut result = Self::shell(program, ledger, domain)?;
        let outcome = (|| {
            for (index, frozen) in program.units.iter().enumerate() {
                if frozen.id().index() != index {
                    return Err(UseError::InvalidProgram("unit identity"));
                }
                let unit = build_unit(frozen, program.cells.len(), ledger, domain)?;
                result.receipt.allocated_bytes += unit.charge.bytes;
                result.units.push(unit);
                result.receipt.rebuilt_units += 1;
            }
            let mut scratch = Scratch::new(ledger, domain);
            scratch.reserve(bytes::<usize>(program.cells.len())?)?;
            scratch.work(program.cells.len())?;
            let mut counts = vec![0usize; program.cells.len()];
            for unit in &result.units {
                scratch.work(unit.data.cells.len())?;
                for reference in &unit.data.cells {
                    counts[reference.cell.index()] += 1;
                }
            }
            scratch.work(result.exports.len())?;
            for (_, cell) in &result.exports {
                counts[cell.index()] += 1;
            }
            scratch.work(program.cells.len())?;
            for count in counts {
                result.cells.push(if count == 0 {
                    CellUsers::empty()
                } else {
                    let bytes = cell_bytes(count)?;
                    scratch.ledger.retain(domain, bytes)?;
                    result.receipt.allocated_bytes += bytes;
                    result.receipt.rebuilt_cell_sets += 1;
                    CellUsers {
                        revision: RevisionId::fresh(),
                        reference_exposed: false,
                        sites: Some(Arc::new(Shared {
                            data: Vec::with_capacity(count),
                            charge: Charge { domain, bytes },
                        })),
                    }
                });
            }
            for (index, unit) in result.units.iter().enumerate() {
                let id = UnitId::from_index(index).ok_or(UseError::Capacity)?;
                scratch.work(unit.data.cells.len())?;
                for reference in &unit.data.cells {
                    let cell = &mut result.cells[reference.cell.index()];
                    cell.reference_exposed |= matches!(reference.usage, CellUse::Reference { .. });
                    let sites = cell.sites.as_mut().unwrap();
                    Arc::get_mut(sites).unwrap().data.push(CellUseSite::Unit {
                        unit: id,
                        usage: reference.usage,
                    });
                }
            }
            scratch.work(result.exports.len())?;
            for &(index, cell) in &result.exports {
                let sites = result.cells[cell.index()].sites.as_mut().unwrap();
                Arc::get_mut(sites)
                    .unwrap()
                    .data
                    .push(CellUseSite::Export { index });
            }
            for cell in &mut result.cells {
                if let Some(sites) = &mut cell.sites {
                    let sites = &mut Arc::get_mut(sites).unwrap().data;
                    scratch.work(sort_work(sites.len())?)?;
                    sites.sort_unstable();
                }
            }
            update::build_creators(&mut result, scratch.ledger, domain)?;
            Ok(())
        })();
        if let Err(error) = outcome {
            result.discard(ledger)?;
            return Err(error);
        }
        result.receipt.logical_work = ledger.work_used(domain) - start;
        Ok(result)
    }

    /// Publish an index for verified replacements/appended units. Every changed
    /// revision must appear in `changed`; repeated, missing or dangling IDs are
    /// rejected before scanning bodies. Unchanged cell-use sets keep their stamp.
    pub fn updated(
        &self,
        program: &Program<'_>,
        changed: &[UnitId],
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, UseError> {
        let prepared = self.prepare_update(program, changed, ledger, domain)?;
        self.fork_replacements(program, prepared, ledger, domain)
    }

    fn shell(
        program: &Program<'_>,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, UseError> {
        if program.exports().len() > u32::MAX as usize {
            return Err(UseError::Capacity);
        }
        ledger.charge(domain, WorkKind::Analysis, program.exports().len() as u64)?;
        if program
            .exports()
            .iter()
            .any(|export| matches!(export.target, InterfaceTarget::Value(cell) if cell.index() >= program.cells.len()))
        {
            return Err(UseError::InvalidProgram("export cell"));
        }
        let bytes = (size_of::<Self>() as u64)
            .checked_add(bytes::<Arc<Shared<UnitUses>>>(program.units.len())?)
            .and_then(|n| n.checked_add(bytes::<CellUsers>(program.cells.len()).ok()?))
            .and_then(|n| n.checked_add(bytes::<CreatorUsers>(program.units.len()).ok()?))
            .and_then(|n| n.checked_add(bytes::<(u32, CellId)>(program.exports().len()).ok()?))
            .ok_or(UseError::Capacity)?;
        ledger.retain(domain, bytes)?;
        Ok(Self {
            update_owner: RevisionId::fresh(),
            tables_revision: program.tables_revision,
            units: Vec::with_capacity(program.units.len()),
            cells: Vec::with_capacity(program.cells.len()),
            creators: Vec::with_capacity(program.units.len()),
            exports: {
                // Admit the full interface bound before allocation; retain original
                // entry-table positions even when intervening type exports erase.
                let mut exports = Vec::with_capacity(program.exports().len());
                exports.extend(program.exports().iter().enumerate().filter_map(
                    |(index, export)| match export.target {
                        InterfaceTarget::Value(cell) => Some((index as u32, cell)),
                        InterfaceTarget::Struct(_) => None,
                    },
                ));
                exports
            },
            charge: Charge { domain, bytes },
            receipt: UseIndexReceipt {
                allocated_bytes: bytes,
                ..UseIndexReceipt::default()
            },
        })
    }

    pub fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let Self {
            units,
            cells,
            creators,
            exports,
            charge,
            ..
        } = self;
        let mut result = Ok(());
        for unit in units {
            result = result.and(release_shared(unit, ledger));
        }
        for cell in cells {
            if let Some(sites) = cell.sites {
                result = result.and(release_shared(sites, ledger));
            }
        }
        for creators in creators {
            if let Some(sites) = creators.sites {
                result = result.and(release_shared(sites, ledger));
            }
        }
        drop(exports);
        result.and(ledger.release(charge.domain, charge.bytes))
    }
}

fn release_shared<T>(shared: Arc<Shared<T>>, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
    if let Ok(Shared { data, charge }) = Arc::try_unwrap(shared) {
        drop(data);
        ledger.release(charge.domain, charge.bytes)?;
    }
    Ok(())
}

struct Scratch<'a> {
    ledger: &'a mut BudgetLedger,
    domain: WorkDomain,
    bytes: u64,
}
impl<'a> Scratch<'a> {
    fn new(ledger: &'a mut BudgetLedger, domain: WorkDomain) -> Self {
        Self {
            ledger,
            domain,
            bytes: 0,
        }
    }
    fn reserve(&mut self, bytes: u64) -> Result<(), UseError> {
        let total = self.bytes.checked_add(bytes).ok_or(UseError::Capacity)?;
        self.ledger.retain(self.domain, bytes)?;
        self.bytes = total;
        Ok(())
    }
    fn work(&mut self, count: usize) -> Result<(), UseError> {
        self.ledger
            .charge(self.domain, WorkKind::Analysis, count as u64)?;
        Ok(())
    }
}
impl Drop for Scratch<'_> {
    fn drop(&mut self) {
        self.ledger
            .release(self.domain, self.bytes)
            .expect("owned use-index scratch reservation");
    }
}

fn bytes<T>(count: usize) -> Result<u64, UseError> {
    (count as u64)
        .checked_mul(size_of::<T>() as u64)
        .ok_or(UseError::Capacity)
}
fn shared_bytes<T>() -> u64 {
    (size_of::<Shared<T>>() + 2 * size_of::<usize>()) as u64
}
fn cell_bytes(count: usize) -> Result<u64, UseError> {
    shared_bytes::<Vec<CellUseSite>>()
        .checked_add(bytes::<CellUseSite>(count)?)
        .ok_or(UseError::Capacity)
}
fn creator_bytes(count: usize) -> Result<u64, UseError> {
    shared_bytes::<Vec<ClosureCreation>>()
        .checked_add(bytes::<ClosureCreation>(count)?)
        .ok_or(UseError::Capacity)
}
fn sort_work(count: usize) -> Result<usize, UseError> {
    count
        .checked_mul((usize::BITS - count.leading_zeros()) as usize)
        .ok_or(UseError::Capacity)
}

enum Event {
    Tick,
    Value(ValueId, ValueUse),
    Cell(CellId, CellUse),
    Closure(OpId, UnitId),
    Call(CallId, OpId),
}

fn walk(
    unit: &UnitData,
    mut emit: impl FnMut(Event) -> Result<(), UseError>,
) -> Result<(), UseError> {
    // The canonical place table is a DAG even when a place is unused. This
    // also bounds every allocation-free projection walk below.
    for (index, place) in unit.places.iter().enumerate() {
        emit(Event::Tick)?;
        if matches!(place, Place::Field { base, .. } if base.index() >= index) {
            return Err(UseError::InvalidProgram("noncanonical place projection"));
        }
    }
    for (index, &cell) in unit.parameters.iter().enumerate() {
        emit(Event::Cell(
            cell,
            CellUse::Parameter(u32::try_from(index).map_err(|_| UseError::Capacity)?),
        ))?;
    }
    for &cell in &unit.captures {
        emit(Event::Cell(cell, CellUse::Capture))?;
    }
    for (index, region) in unit.regions.iter().enumerate() {
        emit(Event::Tick)?;
        let region_id = RegionId::from_index(index).ok_or(UseError::Capacity)?;
        for &id in &region.operations {
            emit(Event::Tick)?;
            let operation = unit
                .operations
                .get(id.index())
                .ok_or(UseError::InvalidProgram("scheduled operation"))?;
            if operation.region != region_id {
                return Err(UseError::InvalidProgram("operation region"));
            }
            let operands = unit
                .operands(operation.operands)
                .ok_or(UseError::InvalidProgram("operand range"))?;
            for (position, &value) in operands.iter().enumerate() {
                emit(Event::Value(
                    value,
                    ValueUse::Operand {
                        operation: id,
                        position: position as u32,
                    },
                ))?;
            }
            match operation.kind {
                OperationKind::Initialize(cell) => {
                    emit(Event::Cell(cell, CellUse::Initialize(id)))?
                }
                OperationKind::Load(place) | OperationKind::CheckPlace(place) => {
                    walk_place(unit, id, place, false, &mut emit)?
                }
                OperationKind::Store(place) => walk_place(unit, id, place, true, &mut emit)?,
                OperationKind::PrepareCall(call) => {
                    match &unit
                        .calls
                        .get(call.index())
                        .ok_or(UseError::InvalidProgram("prepared call"))?
                        .target
                    {
                        CallTarget::Value { callee, .. } => emit(Event::Value(
                            *callee,
                            ValueUse::CallCallee { prepare: id, call },
                        ))?,
                        CallTarget::Reference { place } => {
                            walk_place(unit, id, *place, false, &mut emit)?
                        }
                        CallTarget::Intrinsic {
                            receiver: Some(receiver),
                            ..
                        } => emit(Event::Value(
                            *receiver,
                            ValueUse::CallReceiver { prepare: id, call },
                        ))?,
                        _ => {}
                    }
                }
                OperationKind::Closure(child) => emit(Event::Closure(id, child))?,
                OperationKind::Call(call) => {
                    emit(Event::Call(call, id))?;
                    let site = unit
                        .calls
                        .get(call.index())
                        .ok_or(UseError::InvalidProgram("call"))?;
                    let arguments = unit
                        .arguments(site.arguments)
                        .ok_or(UseError::InvalidProgram("call argument range"))?;
                    for (position, argument) in arguments.iter().enumerate() {
                        emit(Event::Tick)?;
                        if let CallArgument::Value(value) = *argument {
                            emit(Event::Value(
                                value,
                                ValueUse::CallArgument {
                                    operation: id,
                                    call,
                                    position: position as u32,
                                },
                            ))?;
                        }
                    }
                }
                OperationKind::PrepareReference { call, position } => {
                    let site = unit
                        .calls
                        .get(call.index())
                        .ok_or(UseError::InvalidProgram("reference call"))?;
                    let argument = unit
                        .arguments(site.arguments)
                        .and_then(|arguments| arguments.get(position as usize))
                        .ok_or(UseError::InvalidProgram("reference argument"))?;
                    let CallArgument::Reference(place) = *argument else {
                        return Err(UseError::InvalidProgram("reference preparation of value"));
                    };
                    walk_place(unit, id, place, false, &mut emit)?;
                    let mut root = place;
                    loop {
                        emit(Event::Tick)?;
                        match unit
                            .places
                            .get(root.index())
                            .ok_or(UseError::InvalidProgram("reference place"))?
                        {
                            Place::Field { base, .. } => root = *base,
                            Place::Cell(cell) => {
                                emit(Event::Cell(
                                    *cell,
                                    CellUse::Reference {
                                        operation: id,
                                        place,
                                        call,
                                        position,
                                    },
                                ))?;
                                break;
                            }
                            _ => {
                                return Err(UseError::InvalidProgram(
                                    "reference requires lexical root",
                                ))
                            }
                        }
                    }
                }
                // A for-in key or for-of item is initialized by its loop at
                // body entry, exactly as a catch binding is by its try.
                OperationKind::Try {
                    catch: Some((Some(cell), region)),
                    ..
                }
                | OperationKind::ForIn {
                    key: cell,
                    body: region,
                }
                | OperationKind::ForOf {
                    item: cell,
                    body: region,
                } => emit(Event::Cell(
                    cell,
                    CellUse::CatchBinding {
                        operation: id,
                        region,
                    },
                ))?,
                _ => {}
            }
        }
        if let Some(value) = region.result {
            emit(Event::Value(value, ValueUse::RegionResult(region_id)))?;
        }
    }
    Ok(())
}

fn walk_place(
    unit: &UnitData,
    operation: OpId,
    place: PlaceId,
    write: bool,
    emit: &mut impl FnMut(Event) -> Result<(), UseError>,
) -> Result<(), UseError> {
    let mut root = place;
    let selected = loop {
        let selected = unit
            .places
            .get(root.index())
            .ok_or(UseError::InvalidProgram("place"))?;
        let Place::Field { base, .. } = selected else {
            break selected;
        };
        emit(Event::Tick)?;
        if base.index() >= root.index() {
            return Err(UseError::InvalidProgram("noncanonical place projection"));
        }
        root = *base;
    };
    match *selected {
        Place::Cell(cell) => {
            // A projected value update reads the current product to preserve
            // untouched fields, then writes that same mutable storage root.
            if !write || root != place {
                emit(Event::Cell(cell, CellUse::Read { operation, place }))?;
            }
            if write {
                emit(Event::Cell(cell, CellUse::Write { operation, place }))?;
            }
        }
        Place::Value(receiver) | Place::Member { receiver, .. } => {
            if write && matches!(selected, Place::Value(_)) {
                return Err(UseError::InvalidProgram("write through value place"));
            }
            emit(Event::Value(
                receiver,
                ValueUse::PlaceReceiver { operation, place },
            ))?;
        }
        Place::Index { receiver, key } => {
            emit(Event::Value(
                receiver,
                ValueUse::PlaceReceiver { operation, place },
            ))?;
            emit(Event::Value(key, ValueUse::PlaceKey { operation, place }))?;
        }
        Place::Field { .. } => unreachable!("projection loop reached a root"),
    }
    Ok(())
}

fn build_unit(
    frozen: &FrozenUnit,
    cell_count: usize,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<Arc<Shared<UnitUses>>, UseError> {
    let unit = frozen.data();
    let count = unit.values.len().checked_add(1).ok_or(UseError::Capacity)?;
    let mut scratch = Scratch::new(ledger, domain);
    scratch.reserve(bytes::<usize>(count)?)?;
    let mut counts = vec![0usize; count];
    let (mut values, mut cells, mut closures) = (0usize, 0usize, 0usize);
    walk(unit, |event| {
        scratch.work(1)?;
        match event {
            Event::Value(value, _) => {
                if value.index() >= unit.values.len() {
                    return Err(UseError::InvalidProgram("value use"));
                }
                counts[value.index()] = counts[value.index()]
                    .checked_add(1)
                    .ok_or(UseError::Capacity)?;
                values = values.checked_add(1).ok_or(UseError::Capacity)?;
            }
            Event::Cell(cell, _) => {
                if cell.index() >= cell_count {
                    return Err(UseError::InvalidProgram("cell use"));
                }
                cells = cells.checked_add(1).ok_or(UseError::Capacity)?;
            }
            Event::Closure(..) => closures = closures.checked_add(1).ok_or(UseError::Capacity)?,
            Event::Call(call, _) => {
                if call.index() >= unit.calls.len() {
                    return Err(UseError::InvalidProgram("call operation target"));
                }
            }
            Event::Tick => {}
        }
        Ok(())
    })?;
    let bytes = shared_bytes::<UnitUses>()
        .checked_add(bytes::<usize>(count)?)
        .and_then(|n| n.checked_add(bytes::<ValueUse>(values).ok()?))
        .and_then(|n| n.checked_add(bytes::<CellReference>(cells).ok()?))
        .and_then(|n| n.checked_add(bytes::<(OpId, UnitId)>(closures).ok()?))
        .and_then(|n| n.checked_add(bytes::<Option<OpId>>(unit.calls.len()).ok()?))
        .ok_or(UseError::Capacity)?;
    scratch.ledger.retain(domain, bytes)?;
    let outcome = (|| {
        let mut offsets = Vec::with_capacity(count);
        let mut next = 0;
        scratch.work(count)?;
        for count in &mut counts {
            offsets.push(next);
            let length = *count;
            *count = next;
            next += length;
        }
        scratch.work(unit.calls.len())?;
        let mut result = UnitUses {
            revision: frozen.revision(),
            offsets,
            values: vec![ValueUse::RegionResult(unit.entry); values],
            cells: Vec::with_capacity(cells),
            closures: Vec::with_capacity(closures),
            call_operations: vec![None; unit.calls.len()],
        };
        walk(unit, |event| {
            scratch.work(1)?;
            match event {
                Event::Value(value, usage) => {
                    result.values[counts[value.index()]] = usage;
                    counts[value.index()] += 1;
                }
                Event::Cell(cell, usage) => result.cells.push(CellReference { cell, usage }),
                Event::Closure(operation, child) => result.closures.push((operation, child)),
                Event::Call(call, operation) => {
                    if result.call_operations[call.index()]
                        .replace(operation)
                        .is_some()
                    {
                        return Err(UseError::InvalidProgram("duplicate call operation"));
                    }
                }
                Event::Tick => {}
            }
            Ok(())
        })?;
        scratch.work(result.call_operations.len())?;
        if result.call_operations.iter().any(Option::is_none) {
            return Err(UseError::InvalidProgram("missing call operation"));
        }
        scratch.work(sort_work(result.cells.len())?)?;
        result.cells.sort_unstable();
        Ok::<_, UseError>(result)
    })();
    match outcome {
        Ok(data) => Ok(Arc::new(Shared {
            data,
            charge: Charge { domain, bytes },
        })),
        Err(error) => {
            scratch.ledger.release(domain, bytes)?;
            Err(error)
        }
    }
}
