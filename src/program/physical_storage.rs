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
}
impl StorageProofs {
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
        if program
            .units
            .iter()
            .any(|u| !super::defaults::arguments_free(program, u.id()))
        {
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
