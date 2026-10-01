//! Turn expression-only terminal branches into a single return value. Each
//! original branch remains lazy; argument effects and throws stay in its region.
use super::edit::{self, Editor};
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

pub(super) fn apply(
    editor: &mut Editor<'_>,
    dirty: &mut super::dirty::DirtyUnits,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    dirty.prepare(editor.program().units.len(), budget)?;
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            let mut stats = RuleReceipt::default();
            let mut edits = Vec::new();
            for unit in &editor.program().units {
                if let Some(plan) = dirty.plan(
                    editor.program(),
                    unit.id(),
                    super::dirty::LocalRule::Returns,
                    true,
                    &mut stats,
                    budget,
                    |budget| plan(unit.data(), budget),
                    Option::is_none,
                )? {
                    budget.push(Retained, &mut edits, (unit.id(), plan))?;
                }
            }
            Ok::<_, super::RuleError>((edits, stats.local_units_visited, stats.local_units_reused))
        },
        |(edits, visited, reused), editor, budget| {
            receipt.local_units_visited += visited;
            receipt.local_units_reused += reused;
            for &(
                unit,
                Change {
                    branch,
                    yes,
                    no,
                    left,
                    right,
                },
            ) in edits
            {
                let data = editor.unit_mut_in(unit, budget)?;
                let region = data.operations[branch.index()].region;
                let span = data.operations[branch.index()].span;
                let no = if let Some(no) = no {
                    no
                } else {
                    let no =
                        RegionId::from_index(data.regions.len()).ok_or("return region capacity")?;
                    let list = &mut data.regions[region.index()].operations;
                    let position = list
                        .iter()
                        .position(|op| *op == branch)
                        .ok_or("return branch position")?;
                    let operations = budget.copy_slice(Retained, &list[position + 1..])?;
                    list.truncate(position + 1);
                    for &op in &operations {
                        data.operations[op.index()].region = no;
                        for child in data.operations[op.index()].kind.child_regions() {
                            data.regions[child.index()].parent = Some(no);
                        }
                    }
                    let authored = data.regions[region.index()].authored;
                    budget.push(
                        Retained,
                        &mut data.regions,
                        Region {
                            authored,
                            parent: Some(region),
                            operations,
                            result: None,
                            span,
                        },
                    )?;
                    no
                };
                for (child, value) in [(yes, left), (no, right)] {
                    data.regions[child.index()].operations.pop();
                    data.regions[child.index()].result = Some(value);
                }
                let value =
                    ValueId::from_index(data.values.len()).ok_or("return value capacity")?;
                let ty = data.values[left.index()].ty;
                budget.push(
                    Retained,
                    &mut data.values,
                    Value {
                        ty,
                        definition: branch,
                    },
                )?;
                data.operations[branch.index()].kind = OperationKind::Select { yes, no };
                data.operations[branch.index()].result = Some(value);
                let (returned, _) = edit::push_operation_in(
                    data,
                    OperationKind::Return,
                    &[value],
                    None,
                    region,
                    span,
                    budget,
                )?;
                budget.push(
                    Retained,
                    &mut data.regions[region.index()].operations,
                    returned,
                )?;
                receipt.normalized_returns += 1;
            }
            Ok(!edits.is_empty())
        },
    )
}

#[derive(Clone, Copy)]
struct Change {
    branch: OpId,
    yes: RegionId,
    no: Option<RegionId>,
    left: ValueId,
    right: ValueId,
}

fn expression(kind: &OperationKind) -> bool {
    !matches!(
        kind,
        OperationKind::Initialize(_)
            | OperationKind::Declare(_)
            | OperationKind::If { .. }
            | OperationKind::Loop { .. }
            | OperationKind::ForIn { .. }
            | OperationKind::ForOf { .. }
            | OperationKind::Block(_)
            | OperationKind::Try { .. }
            | OperationKind::Return
            | OperationKind::Throw
            | OperationKind::Break
            | OperationKind::Continue
            | OperationKind::Yield { .. }
            | OperationKind::Await
    )
}

fn plan(
    data: &UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<Change>, AllocationError> {
    if data.kind == UnitKind::ModuleInitialization || data.suspension != Suspension::None {
        return Ok(None);
    }
    budget.work(WorkKind::Analysis, data.operations.len() as u64)?;
    if !data
        .operations
        .iter()
        .any(|op| matches!(op.kind, OperationKind::If { .. }))
    {
        return Ok(None);
    }
    let mut budget = budget.scope();
    // Compute child-region safety once. Prefix probes then borrow those facts,
    // rather than copying and recursively walking each candidate's operations.
    let mut regions = budget.filled(Scratch, data.regions.len(), None)?;
    let mut pending = Vec::new();
    for index in 0..data.regions.len() {
        if regions[index].is_some() {
            continue;
        }
        budget.push(
            Scratch,
            &mut pending,
            (RegionId::from_index(index).unwrap(), false),
        )?;
        while let Some((region, finish)) = pending.pop() {
            budget.work(WorkKind::Analysis, 1)?;
            if regions[region.index()].is_some() {
                continue;
            }
            if finish {
                let mut safe = true;
                for &op in &data.regions[region.index()].operations {
                    let kind = &data.operations[op.index()].kind;
                    budget.work(WorkKind::Analysis, 1)?;
                    safe &= expression(kind)
                        && kind
                            .child_regions()
                            .all(|child| regions[child.index()] == Some(true));
                }
                regions[region.index()] = Some(safe);
            } else {
                budget.push(Scratch, &mut pending, (region, true))?;
                for &op in &data.regions[region.index()].operations {
                    budget.work(WorkKind::Analysis, 1)?;
                    let kind = &data.operations[op.index()].kind;
                    for child in kind.child_regions() {
                        budget.push(Scratch, &mut pending, (child, false))?;
                    }
                }
            }
        }
    }
    for region in (0..data.regions.len()).rev() {
        budget.work(WorkKind::Analysis, 1)?;
        let list = &data.regions[region].operations;
        let Some(&last) = list.last() else { continue };
        let found =
            if let OperationKind::If { yes, no: Some(no) } = data.operations[last.index()].kind {
                terminal_value(data, yes, &regions, &mut budget)?
                    .zip(terminal_value(data, no, &regions, &mut budget)?)
                    .map(|(left, right)| Change {
                        branch: last,
                        yes,
                        no: Some(no),
                        left,
                        right,
                    })
            } else {
                let mut found = None;
                if let Some(right) = returned(data, last) {
                    for &branch in list[..list.len() - 1].iter().rev() {
                        budget.work(WorkKind::Analysis, 1)?;
                        let kind = &data.operations[branch.index()].kind;
                        if let OperationKind::If { yes, no: None } = *kind {
                            if let Some(left) = terminal_value(data, yes, &regions, &mut budget)? {
                                found = Some(Change {
                                    branch,
                                    yes,
                                    no: None,
                                    left,
                                    right,
                                });
                                break;
                            }
                        }
                        if !expression(kind)
                            || !kind
                                .child_regions()
                                .all(|child| regions[child.index()] == Some(true))
                        {
                            break;
                        }
                    }
                }
                found
            };
        let Some(change) = found else { continue };
        if data.operations[change.branch.index()]
            .authored
            .get(crate::representation::ChoiceFamily::ConditionalReturns)
            == Some(crate::representation::AltId(0))
        {
            continue;
        }
        if data.values[change.left.index()].ty == data.values[change.right.index()].ty {
            return Ok(Some(change));
        }
    }
    Ok(None)
}
fn returned(data: &UnitData, operation: OpId) -> Option<ValueId> {
    let operation = &data.operations[operation.index()];
    if !matches!(operation.kind, OperationKind::Return) {
        return None;
    }
    let [value] = data.operands(operation.operands)? else {
        return None;
    };
    Some(*value)
}
fn terminal_value(
    data: &UnitData,
    region: RegionId,
    regions: &[Option<bool>],
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<ValueId>, AllocationError> {
    let region = &data.regions[region.index()];
    if region.result.is_some() {
        return Ok(None);
    }
    let Some((&tail, prefix)) = region.operations.split_last() else {
        return Ok(None);
    };
    let Some(value) = returned(data, tail) else {
        return Ok(None);
    };
    for &op in prefix {
        budget.work(WorkKind::Analysis, 1)?;
        let kind = &data.operations[op.index()].kind;
        if !expression(kind)
            || !kind
                .child_regions()
                .all(|child| regions[child.index()] == Some(true))
        {
            return Ok(None);
        }
    }
    Ok(Some(value))
}
