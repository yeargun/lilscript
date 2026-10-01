//! Clone lexical callable subtrees for one inline occurrence. Function objects
//! still form at their original evaluation points, capturing that call's bank.
use super::edit::Editor;
use super::storage::Map;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

pub(super) fn children(
    program: &Program<'_>,
    root: UnitId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<Vec<UnitId>>, AllocationError> {
    storage::optional(budget, |attempt| {
        let mut result = Vec::new();
        let mut pending = attempt.vector(Scratch, 1)?;
        pending.push(root);
        let mut work = 0usize;
        while let Some(unit) = pending.pop() {
            let data = program.unit(unit)?;
            work = work.checked_add(data.operations.len())?;
            if work > 65_536 || result.len() > 256 {
                return None;
            }
            attempt.work(data.operations.len() as u64)?;
            for operation in &data.operations {
                if let OperationKind::Closure(child) = operation.kind {
                    attempt.work(result.len() as u64 + 1)?;
                    if child == root || result.contains(&child) {
                        return None;
                    }
                    if program.unit(child)?.constructor_of.is_some() {
                        return None;
                    }
                    attempt.push(Retained, &mut result, child)?;
                    attempt.push(Scratch, &mut pending, child)?;
                }
            }
        }
        Some(result)
    })
}

pub(super) fn clone(
    editor: &mut Editor<'_>,
    children: &[UnitId],
    cells: &mut Map<CellId, CellId>,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<Map<UnitId, UnitId>, super::RuleError> {
    let mut units = Map::new(Scratch);
    let base = editor.program().units.len();
    for (index, &unit) in children.iter().enumerate() {
        units.insert(
            unit,
            UnitId::from_index(base + index).ok_or("inline unit capacity")?,
            budget,
        )?;
    }
    let mut added = Vec::new();
    for (index, cell) in editor.program().cells.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        let Some(&owner) = units.get(&cell.owner) else {
            continue;
        };
        let id = CellId::from_index(index).ok_or("inline cell capacity")?;
        let mut cell = storage::cell(cell, budget)?;
        cell.owner = owner;
        cell.synthetic = true;
        if let CellBinding::Function(body) = &mut cell.binding {
            *body = units.get(body).copied().unwrap_or(*body);
        }
        budget.push(Scratch, &mut added, (id, cell))?;
    }
    for (id, cell) in added.drain(..) {
        let new = editor.add_cell_in(cell, budget)?;
        cells.insert(id, new, budget)?;
    }
    storage::release_vec(added, Scratch, budget)?;
    for &old in children {
        let mut data = storage::unit(
            editor.program().unit(old).ok_or("inline child unit")?,
            budget,
        )?;
        budget.work(
            WorkKind::Analysis,
            data.parameters.len() as u64
                + data.captures.len() as u64
                + data.places.len() as u64
                + data.operations.len() as u64,
        )?;
        remap(&mut data, cells, &units);
        let new = editor.add_unit_in(data, budget)?;
        if units.get(&old) != Some(&new) {
            return Err("inline unit order".into());
        }
    }
    Ok(units)
}

fn remap(data: &mut UnitData, cells: &Map<CellId, CellId>, units: &Map<UnitId, UnitId>) {
    let cell = |id: &mut CellId| *id = cells.get(id).copied().unwrap_or(*id);
    for id in &mut data.parameters {
        cell(id);
    }
    for id in &mut data.captures {
        cell(id);
    }
    for place in &mut data.places {
        if let Place::Cell(id) = place {
            cell(id);
        }
    }
    for operation in &mut data.operations {
        match &mut operation.kind {
            OperationKind::Initialize(id) | OperationKind::Declare(id) => cell(id),
            OperationKind::ForIn { key, .. } => cell(key),
            OperationKind::ForOf { item, .. } => cell(item),
            OperationKind::Try {
                catch: Some((Some(id), _)),
                ..
            } => cell(id),
            OperationKind::Closure(body) => *body = units.get(body).copied().unwrap_or(*body),
            _ => {}
        }
    }
}
