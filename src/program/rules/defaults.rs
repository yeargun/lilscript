//! Reify proved primitive declaration defaults at closed calls. Default values
//! come from the retained callee entry operations, never from its function type.
//! Each affected call gains its missing arguments once; later folding can remove
//! entry guards. Arguments/reflection must be unobservable for that activation.
use super::{edit, storage, RuleReceipt};
use crate::compilation_policy::WorkKind;
use crate::output_budget::AllocationBudget;
use crate::output_budget::AllocationClass::{Retained, Scratch};
use crate::program::call_graph::{CallGraph, Callee};
use crate::program::*;

pub(super) fn apply(
    editor: &mut edit::Editor<'_>,
    graph: &CallGraph,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| plan(editor.program(), graph, budget),
        |plans, editor, budget| apply_plans(editor, plans, receipt, budget),
    )
}

type Plan = (UnitId, OpId, CallId, Vec<(Constant, TypeId)>);

fn plan(
    program: &Program<'_>,
    graph: &CallGraph,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Plan>, super::RuleError> {
    budget.retained_phase(|budget| {
        let mut defaults = budget.vector(Scratch, program.units().len())?;
        for unit in program.units() {
            defaults.push(super::super::defaults::entry_literals_in(
                unit.data(),
                budget,
            )?);
        }
        let mut free = super::super::defaults::arguments_free_all_in(program, budget)?;
        for unit in program.units() {
            free[unit.id().index()] &= graph.complete_callers(unit.id()).is_some();
        }
        let mut plans = Vec::new();
        for unit in program.units() {
            let data = unit.data();
            for (index, op) in data.operations.iter().enumerate() {
                budget.work(WorkKind::Analysis, 1)?;
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
                let mut added = budget.vector(Retained, entries.len() - args.len())?;
                for position in args.len()..entries.len() {
                    budget.work(WorkKind::Analysis, 1)?;
                    let Some(value) = entries[position].clone() else {
                        added.clear();
                        break;
                    };
                    added.push((value, program.cells[target.parameters[position].index()].ty));
                }
                if added.is_empty() {
                    storage::release_vec(added, Retained, budget)?;
                } else {
                    budget.push(
                        Retained,
                        &mut plans,
                        (
                            unit.id(),
                            OpId::from_index(index).ok_or("operation capacity")?,
                            call,
                            added,
                        ),
                    )?;
                }
            }
        }
        for values in defaults {
            storage::release_vec(values, Retained, budget)?;
        }
        storage::release_vec(free, Retained, budget)?;
        Ok(plans)
    })
}

fn apply_plans(
    editor: &mut edit::Editor<'_>,
    plans: &[Plan],
    receipt: &mut RuleReceipt,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    let changed = !plans.is_empty();
    for &(unit, op, call, ref added) in plans {
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
                OperationKind::Constant(value.clone()),
                &[],
                Some(*ty),
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
