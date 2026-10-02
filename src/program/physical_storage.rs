//! Physical ownership witnesses derived from complete semantic occurrences.
//! A unique product bank has no whole-value observer. A transfer consumes one
//! SSA owner in the same activation that produced it. Neither answer changes
//! source value semantics, permits host aliases, or crosses a loop lifetime.
use super::activation::StructuredDominance;
use super::call_graph::Seal;
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StorageDemand {
    Products,
    Transfers,
}

#[derive(Default)]
pub(super) struct StorageProofs {
    unique: Vec<CellId>,
    transfers: Vec<(UnitId, OpId, ValueId)>,
    borrowed: Vec<UnitId>,
    stack_cells: Vec<CellId>,
}
impl StorageProofs {
    pub fn borrowed(&self, body: UnitId) -> bool {
        self.borrowed.binary_search(&body).is_ok()
    }
    pub fn stack_cell(&self, cell: CellId) -> bool {
        self.stack_cells.binary_search(&cell).is_ok()
    }
    pub fn has_borrowed(&self) -> bool {
        !self.borrowed.is_empty()
    }
    /// An address-taken bit alone is insufficient: another closure can keep
    /// a callable alive while only invoking it directly. Follow every semantic
    /// occurrence, and require all aliases and calls to stay in the creation
    /// region of one synchronous activation. Unknown uses keep heap ownership.
    pub fn borrow_closures(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        for body in &program.units {
            budget.work(WorkKind::Analysis, 1)?;
            if body.data().suspension != Suspension::None { continue; }
            let sites = uses.creators(body.id()).unwrap().sites();
            if sites.is_empty() { continue; }
            let mut valid = true;
            for site in sites {
                let data = program.unit(site.unit).unwrap();
                let op = &data.operations[site.operation.index()];
                if data.suspension != Suspension::None || !borrowed_value(
                    program, uses, site.unit, op.region, op.result.unwrap(), 0, budget,
                )? { valid = false; break; }
            }
            if valid { budget.push(AllocationClass::Scratch, &mut self.borrowed, body.id())?; }
        }
        for (index, cell) in program.cells.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if program.unit(cell.owner).unwrap().suspension != Suspension::None { continue; }
            let id = CellId::from_index(index).unwrap();
            let mut captured = false;
            let mut valid = true;
            for site in uses.cell(id).unwrap().sites() {
                budget.work(WorkKind::Analysis, 1)?;
                match *site {
                    CellUseSite::Unit { unit, usage: CellUse::Capture } => {
                        captured = true;
                        valid &= self.borrowed(unit);
                    }
                    CellUseSite::Export { .. } => valid = false,
                    _ => {}
                }
            }
            if valid && captured { budget.push(AllocationClass::Scratch, &mut self.stack_cells, id)?; }
        }
        Ok(())
    }
    pub fn unique(&self, cell: CellId) -> bool {
        self.unique.binary_search(&cell).is_ok()
    }
    pub fn transfers(&self, unit: UnitId, op: OpId, value: ValueId) -> bool {
        self.transfers.binary_search(&(unit, op, value)).is_ok()
    }
    pub fn build(
        program: &Program<'_>,
        uses: &UseIndex,
        seal: Seal,
        demand: StorageDemand,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut result = Self::default();
        if demand == StorageDemand::Transfers {
            for unit in &program.units {
                let id = unit.id();
                let data = unit.data();
                let parents = budget.filled(AllocationClass::Scratch, data.regions.len(), None)?;
                let positions =
                    budget.filled(AllocationClass::Scratch, data.operations.len(), 0)?;
                let dominance = StructuredDominance::build(data, parents, positions, |n| {
                    budget.work(WorkKind::Analysis, n as u64)
                })?;
                for (index, op) in data.operations.iter().enumerate() {
                    budget.work(WorkKind::Analysis, 1)?;
                    if !matches!(
                        op.kind,
                        OperationKind::Initialize(_) | OperationKind::CopyValue
                    ) && !matches!(op.kind, OperationKind::Store(p) if matches!(data.places[p.index()], Place::Cell(_)))
                    {
                        continue;
                    }
                    let args = data.operands(op.operands).unwrap_or(&[]);
                    let [source] = args else { continue };
                    if matches!(
                        program.types[data.values[source.index()].ty.index()],
                        Type::Int
                            | Type::Float
                            | Type::String
                            | Type::Bool
                            | Type::Null
                            | Type::Void
                            | Type::Enum(_)
                            | Type::Struct(_)
                            | Type::StructInstance { .. }
                    ) {
                        continue;
                    }
                    let definition = data.values[source.index()].definition;
                    let at = OpId::from_index(index).unwrap();
                    if data.operations[definition.index()].region != op.region {
                        continue;
                    }
                    let occurrences = uses.unit(id).unwrap().value_uses(*source).unwrap_or(&[]);
                    if !matches!(occurrences, [ValueUse::Operand { operation, .. }] if *operation == at)
                    {
                        continue;
                    }
                    if dominance.after(data, definition, at, |n| {
                        budget.work(WorkKind::Analysis, n as u64)
                    })? {
                        budget.push(
                            AllocationClass::Scratch,
                            &mut result.transfers,
                            (id, at, *source),
                        )?;
                    }
                }
                // Dominance buffers are owned by this formation scope. Drop their
                // actual storage here; the encompassing scope releases its charge.
            }
            return Ok(result);
        }
        let free = super::defaults::arguments_free_all_in(program, budget)?;
        let blocked = free.iter().any(|free| !free);
        super::analysis_storage::release(free, AllocationClass::Retained, budget)?;
        if blocked {
            return Ok(result);
        }
        for (index, cell) in program.cells.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if cell.binding != CellBinding::Local
                || !matches!(program.types[cell.ty.index()], Type::Struct(_))
            {
                continue;
            }
            if seal != Seal::Module
                && program.unit(cell.owner).unwrap().kind == UnitKind::ModuleInitialization
            {
                continue;
            }
            let id = CellId::from_index(index).unwrap();
            let mut initialized = false;
            let mut valid = true;
            for usage in uses.cell(id).unwrap().sites() {
                budget.work(WorkKind::Analysis, 1)?;
                let CellUseSite::Unit { unit, usage } = *usage else {
                    valid = false;
                    break;
                };
                let data = program.unit(unit).unwrap();
                match usage {
                    CellUse::Capture => {}
                    CellUse::Initialize(at) => {
                        initialized = true;
                        valid &= fresh(
                            program,
                            uses,
                            unit,
                            data.operands(data.operations[at.index()].operands).unwrap()[0],
                            0,
                            budget,
                        )?;
                    }
                    CellUse::Read { operation, place } | CellUse::Write { operation, place } => {
                        let op = &data.operations[operation.index()];
                        if place == root(data, place) {
                            if matches!(op.kind, OperationKind::Store(_)) {
                                valid &= fresh(
                                    program,
                                    uses,
                                    unit,
                                    data.operands(op.operands).unwrap()[0],
                                    0,
                                    budget,
                                )?;
                            } else {
                                valid = false;
                            }
                        } else if matches!(op.kind, OperationKind::Load(_)) {
                            // Reading a product-valued leaf creates a snapshot;
                            // every mutable ancestor must then remain persistent.
                            valid &= op.result.is_none_or(|v| {
                                !matches!(
                                    program.types[data.values[v.index()].ty.index()],
                                    Type::Struct(_)
                                        | Type::StructInstance { .. }
                                        | Type::Nullable(_)
                                )
                            });
                        } else if matches!(op.kind, OperationKind::Store(_)) {
                            let value = data.operands(op.operands).unwrap()[0];
                            if matches!(
                                program.types[data.values[value.index()].ty.index()],
                                Type::Struct(_) | Type::StructInstance { .. }
                            ) {
                                valid &= fresh(program, uses, unit, value, 0, budget)?;
                            }
                        }
                    }
                    _ => valid = false,
                }
                if !valid {
                    break;
                }
            }
            if valid && initialized {
                budget.push(AllocationClass::Scratch, &mut result.unique, id)?;
            }
        }
        Ok(result)
    }
}

fn borrowed_value(
    program: &Program<'_>, uses: &UseIndex, unit: UnitId, region: RegionId,
    value: ValueId, depth: usize, budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    budget.work(WorkKind::Analysis, 1)?;
    if depth >= 64 { return Ok(false); }
    let data = program.unit(unit).unwrap();
    for usage in uses.unit(unit).unwrap().value_uses(value).unwrap() {
        budget.work(WorkKind::Analysis, 1)?;
        let at = match *usage {
            ValueUse::CallCallee { prepare, call } => {
                let invoke = uses.unit(unit).unwrap().call_operation(call).unwrap();
                if data.operations[invoke.index()].region != region { return Ok(false); }
                prepare
            }
            ValueUse::Operand { operation, .. } => {
                let op = &data.operations[operation.index()];
                match op.kind {
                    OperationKind::CopyValue => {
                        if !borrowed_value(program, uses, unit, region, op.result.unwrap(), depth+1, budget)? { return Ok(false); }
                    }
                    OperationKind::Initialize(cell) => {
                        if !borrowed_cell(program, uses, unit, region, cell, operation, depth+1, budget)? { return Ok(false); }
                    }
                    _ => return Ok(false),
                }
                operation
            }
            _ => return Ok(false),
        };
        if data.operations[at.index()].region != region { return Ok(false); }
    }
    Ok(true)
}

fn borrowed_cell(
    program: &Program<'_>, uses: &UseIndex, unit: UnitId, region: RegionId,
    cell: CellId, initialize: OpId, depth: usize, budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    if depth >= 64 || program.cells[cell.index()].binding != CellBinding::Local { return Ok(false); }
    let data = program.unit(unit).unwrap();
    for usage in uses.cell(cell).unwrap().sites() {
        budget.work(WorkKind::Analysis, 1)?;
        let CellUseSite::Unit { unit: owner, usage } = *usage else { return Ok(false); };
        if owner != unit { return Ok(false); }
        match usage {
            CellUse::Initialize(at) if at == initialize => {}
            CellUse::Read { operation, place } => {
                let op = &data.operations[operation.index()];
                if op.region != region || !matches!(data.places[place.index()], Place::Cell(id) if id == cell) { return Ok(false); }
                match op.kind {
                    OperationKind::Load(_) => {
                        if !borrowed_value(program, uses, unit, region, op.result.unwrap(), depth+1, budget)? { return Ok(false); }
                    }
                    OperationKind::PrepareCall(call) => {
                        let invoke = uses.unit(unit).unwrap().call_operation(call).unwrap();
                        if data.operations[invoke.index()].region != region { return Ok(false); }
                    }
                    OperationKind::CheckPlace(_) => {}
                    _ => return Ok(false),
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}
fn root(data: &UnitData, mut place: PlaceId) -> PlaceId {
    while let Place::Field { base, .. } = data.places[place.index()] {
        place = base;
    }
    place
}
fn fresh(
    program: &Program<'_>,
    uses: &UseIndex,
    unit: UnitId,
    value: ValueId,
    depth: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    budget.work(WorkKind::Analysis, 1)?;
    if depth >= 64 {
        return Ok(false);
    }
    let data = program.unit(unit).unwrap();
    if uses
        .unit(unit)
        .unwrap()
        .value_uses(value)
        .unwrap_or(&[])
        .len()
        != 1
    {
        return Ok(false);
    }
    let op = &data.operations[data.values[value.index()].definition.index()];
    match op.kind {
        OperationKind::CopyValue => fresh(
            program,
            uses,
            unit,
            data.operands(op.operands).unwrap()[0],
            depth + 1,
            budget,
        ),
        OperationKind::Allocate {
            kind: AllocationKind::Struct(_),
            ..
        } => {
            for &input in data.operands(op.operands).unwrap_or(&[]) {
                if matches!(
                    program.types[data.values[input.index()].ty.index()],
                    Type::Struct(_) | Type::StructInstance { .. } | Type::Nullable(_)
                ) && !fresh(program, uses, unit, input, depth + 1, budget)?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        OperationKind::Select { yes, no } => {
            for region in [yes, no] {
                let Some(value) = data.regions[region.index()].result else {
                    return Ok(false);
                };
                if !fresh(program, uses, unit, value, depth + 1, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
