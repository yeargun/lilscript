//! Turn expression-only terminal branches into a single return value. Each
//! original branch remains lazy; argument effects and throws stay in its region.
use super::edit::{self, Editor};
use super::*;

pub(super) fn apply(
    editor: &mut Editor<'_>,
    dirty: &[bool],
    receipt: &mut RuleReceipt,
) -> Result<bool, &'static str> {
    let mut edits = Vec::new();
    for unit in &editor.program().units {
        if !dirty[unit.id().index()] { continue; }
        let data = unit.data();
        if data.kind == UnitKind::ModuleInitialization || data.suspension != Suspension::None {
            continue;
        }
        // Inner branches first. A later round can then absorb their one return.
        for region in (0..data.regions.len()).rev() {
            let list = &data.regions[region].operations;
            let Some(&last) = list.last() else { continue };
            let found = if let OperationKind::If { yes, no: Some(no) } =
                data.operations[last.index()].kind
            {
                terminal_value(data, yes)
                    .zip(terminal_value(data, no))
                    .map(|(left, right)| (last, yes, Some(no), left, right))
            } else {
                list.iter().enumerate().rev().find_map(|(index, &branch)| {
                    let OperationKind::If { yes, no: None } = data.operations[branch.index()].kind
                    else {
                        return None;
                    };
                    terminal_value(data, yes)
                        .zip(terminal_operations(data, &list[index + 1..]))
                        .map(|(left, right)| (branch, yes, None, left, right))
                })
            };
            let Some((branch, yes, no, left, right)) = found else {
                continue;
            };
            if data.operations[branch.index()].authored.get(crate::representation::ChoiceFamily::ConditionalReturns) == Some(crate::representation::AltId(0)) {
                continue;
            }
            if data.values[left.index()].ty != data.values[right.index()].ty {
                continue;
            }
            edits.push((unit.id(), branch, yes, no, left, right));
            break;
        }
    }
    for &(unit, branch, yes, no, left, right) in &edits {
        let data = editor.unit_mut(unit);
        let region = data.operations[branch.index()].region;
        let span = data.operations[branch.index()].span;
        let no = if let Some(no) = no {
            no
        } else {
            let no = RegionId::from_index(data.regions.len()).ok_or("return region capacity")?;
            let list = &mut data.regions[region.index()].operations;
            let position = list
                .iter()
                .position(|op| *op == branch)
                .ok_or("return branch position")?;
            let operations = list.split_off(position + 1);
            for &op in &operations {
                data.operations[op.index()].region = no;
                for child in data.operations[op.index()].kind.child_regions() {
                    data.regions[child.index()].parent = Some(no);
                }
            }
            data.regions.push(Region {
                authored: data.regions[region.index()].authored,
                parent: Some(region),
                operations,
                result: None,
                span,
            });
            no
        };
        for (child, value) in [(yes, left), (no, right)] {
            data.regions[child.index()].operations.pop();
            data.regions[child.index()].result = Some(value);
        }
        let value = ValueId::from_index(data.values.len()).ok_or("return value capacity")?;
        data.values.push(Value {
            ty: data.values[left.index()].ty,
            definition: branch,
        });
        data.operations[branch.index()].kind = OperationKind::Select { yes, no };
        data.operations[branch.index()].result = Some(value);
        let (returned, _) =
            edit::push_operation(data, OperationKind::Return, &[value], None, region, span)?;
        data.regions[region.index()].operations.push(returned);
        receipt.normalized_returns += 1;
    }
    Ok(!edits.is_empty())
}

fn terminal_value(data: &UnitData, region: RegionId) -> Option<ValueId> {
    let region = &data.regions[region.index()];
    if region.result.is_some() {
        return None;
    }
    terminal_operations(data, &region.operations)
}

fn terminal_operations(data: &UnitData, operations: &[OpId]) -> Option<ValueId> {
    let (&tail, prefix) = operations.split_last()?;
    let operation = &data.operations[tail.index()];
    if !matches!(operation.kind, OperationKind::Return) {
        return None;
    }
    let [value] = data.operands(operation.operands)? else {
        return None;
    };
    let mut pending = prefix.to_vec();
    while let Some(op) = pending.pop() {
        let operation = &data.operations[op.index()];
        if matches!(
            operation.kind,
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
        ) {
            return None;
        }
        for child in operation.kind.child_regions() {
            pending.extend(data.regions[child.index()].operations.iter().copied());
        }
    }
    Some(*value)
}
