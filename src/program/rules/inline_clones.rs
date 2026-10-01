//! Clone lexical callable subtrees for one inline occurrence. Function objects
//! still form at their original evaluation points, capturing that call's bank.
use super::edit::Editor;
use super::*;
use std::collections::HashMap;

pub(super) fn children(program: &Program<'_>, root: UnitId) -> Option<Vec<UnitId>> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    let mut work = 0usize;
    while let Some(unit) = pending.pop() {
        let data = program.unit(unit)?;
        work = work.checked_add(data.operations.len())?;
        if work > 65_536 || result.len() > 256 {
            return None;
        }
        for operation in &data.operations {
            if let OperationKind::Closure(child) = operation.kind {
                if child == root || result.contains(&child) {
                    return None;
                }
                if program.unit(child)?.constructor_of.is_some() {
                    return None;
                }
                result.push(child);
                pending.push(child);
            }
        }
    }
    Some(result)
}

pub(super) fn clone(
    editor: &mut Editor<'_>,
    children: &[UnitId],
    cells: &mut HashMap<CellId, CellId>,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<HashMap<UnitId, UnitId>, super::RuleError> {
    let mut units = HashMap::new();
    let base = editor.program().units.len();
    for (index, &unit) in children.iter().enumerate() {
        units.insert(
            unit,
            UnitId::from_index(base + index).ok_or("inline unit capacity")?,
        );
    }
    let mut added = Vec::new();
    for (index, cell) in editor.program().cells.iter().enumerate() {
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
        added.push((id, cell));
    }
    for (id, cell) in added {
        cells.insert(id, editor.add_cell_in(cell, budget)?);
    }
    for &old in children {
        let mut data = storage::unit(
            editor.program().unit(old).ok_or("inline child unit")?,
            budget,
        )?;
        remap(&mut data, cells, &units);
        let new = editor.add_unit_in(data, budget)?;
        if units[&old] != new {
            return Err("inline unit order".into());
        }
    }
    Ok(units)
}

fn remap(data: &mut UnitData, cells: &HashMap<CellId, CellId>, units: &HashMap<UnitId, UnitId>) {
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
