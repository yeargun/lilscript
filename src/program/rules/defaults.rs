//! Reify proved primitive declaration defaults at closed calls. Default values
//! come from the retained callee entry operations, never from its function type.
//! Each affected call gains its missing arguments once; later folding can remove
//! entry guards. Arguments/reflection must be unobservable for that activation.
use super::{edit, storage, RuleReceipt};
use crate::output_budget::AllocationClass::{Retained, Scratch};
use crate::program::call_graph::{CallGraph, Callee};
use crate::program::*;

pub(super) fn apply(
    editor: &mut edit::Editor<'_>,
    graph: &CallGraph,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    let program = editor.program();
    let defaults: Vec<_> = program
        .units()
        .iter()
        .map(|u| super::super::defaults::entry_literals(u.data()))
        .collect();
    let free: Vec<_> = program
        .units()
        .iter()
        .map(|u| {
            graph.complete_callers(u.id()).is_some()
                && super::super::defaults::arguments_free(program, u.id())
        })
        .collect();
    let mut plans = Vec::new();
    for unit in program.units() {
        let data = unit.data();
        for (index, op) in data.operations.iter().enumerate() {
            let OperationKind::Call(call) = op.kind else {
                continue;
            };
            let site = &data.calls[call.index()];
            if site.contract.defaults != DefaultConvention::ApplyAtCallee
                || site.contract.instantiation.is_some()
            {
                continue;
            }
            let Callee::Unit(body) = graph.callee(unit.id(), call) else {
                continue;
            };
            if !free[body.index()] {
                continue;
            }
            let args = data.arguments(site.arguments).ok_or("call arguments")?;
            let entries = &defaults[body.index()];
            if args.len() >= entries.len() {
                continue;
            }
            let target = program.unit(body).ok_or("default owner")?;
            let mut added = Vec::new();
            for position in args.len()..entries.len() {
                let Some(value) = entries[position].clone() else {
                    added.clear();
                    break;
                };
                // A scalar catalog/nominal type is preserved by the copied
                // constant; no allocation or symbol read moves across a call.
                added.push((value, program.cells[target.parameters[position].index()].ty));
            }
            if !added.is_empty() {
                plans.push((
                    unit.id(),
                    OpId::from_index(index).ok_or("operation capacity")?,
                    call,
                    added,
                ));
            }
        }
    }
    let changed = !plans.is_empty();
    for (unit, op, call, added) in plans {
        let data = editor.unit_mut_in(unit, budget)?;
        let region = data.operations[op.index()].region;
        let span = data.operations[op.index()].span;
        let mut args = budget.copy_slice(
            Scratch,
            data.arguments(data.calls[call.index()].arguments)
                .ok_or("call arguments")?,
        )?;
        let mut inserted = Vec::new();
        for (value, ty) in added {
            let (operation, value) = edit::push_operation_in(
                data,
                OperationKind::Constant(value),
                &[],
                Some(ty),
                region,
                span,
                budget,
            )?;
            budget.push(Scratch, &mut inserted, operation)?;
            budget.push(
                Scratch,
                &mut args,
                CallArgument::Value(value.ok_or("constant result")?),
            )?;
            receipt.materialized_default_arguments += 1;
        }
        let before = data.regions[region.index()]
            .operations
            .iter()
            .position(|id| *id == op)
            .ok_or("default call position")?;
        let list = &mut data.regions[region.index()].operations;
        budget.reserve_vec(Retained, list, inserted.len())?;
        list.splice(before..before, inserted.iter().copied());
        storage::release_vec(inserted, Scratch, budget)?;
        let start =
            u32::try_from(data.call_arguments.len()).map_err(|_| "call argument capacity")?;
        let len = u32::try_from(args.len()).map_err(|_| "call argument capacity")?;
        budget.extend_copy(Retained, &mut data.call_arguments, &args)?;
        storage::release_vec(args, Scratch, budget)?;
        let site = &mut data.calls[call.index()];
        site.arguments = ArgumentRange { start, len };
        site.contract.supplied = len;
        site.omit_trailing = 0;
    }
    Ok(changed)
}
