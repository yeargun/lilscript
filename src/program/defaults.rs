//! Checked default transport at the program-to-target boundary.
//!
//! The semantic call keeps all caller-built arguments. JavaScript can omit a
//! trailing literal only when the resolved body still applies that literal
//! on entry and cannot observe its argument count. Native keeps the original
//! operands. Proofs inspect the owned operations, never a formed target tree.

use super::ambient::{self, Ambient};
use super::analysis_storage as storage;
use super::call_graph::{CallGraph, Callee};
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

/// Literal defaults still implemented by the entry prefix, by parameter.
pub(super) fn entry_literals_in(
    data: &UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Option<Constant>>, AllocationError> {
    let mut defaults =
        storage::collect((0..data.parameters.len()).map(|_| None), Retained, budget)?;
    let operations = &data.regions[data.entry.index()].operations;
    let mut at = 0;
    while at + 2 < operations.len() {
        budget.work(WorkKind::Analysis, data.parameters.len() as u64 + 1)?;
        let load = &data.operations[operations[at].index()];
        let test = &data.operations[operations[at + 1].index()];
        let guard = &data.operations[operations[at + 2].index()];
        let OperationKind::Load(place) = load.kind else {
            break;
        };
        let Some(Place::Cell(cell)) = data.places.get(place.index()) else {
            break;
        };
        let Some(position) = data
            .parameters
            .iter()
            .position(|parameter| parameter == cell)
        else {
            break;
        };
        if !matches!(test.kind, OperationKind::IsUndefined { .. })
            || load.result.is_none()
            || data.operands(test.operands) != load.result.as_ref().map(std::slice::from_ref)
            || test.result.is_none()
            || data.operands(guard.operands) != test.result.as_ref().map(std::slice::from_ref)
        {
            break;
        }
        let OperationKind::If { yes, no: None } = guard.kind else {
            break;
        };
        let body = &data.regions[yes.index()].operations;
        let [literal, store] = body.as_slice() else {
            break;
        };
        let literal = &data.operations[literal.index()];
        let store = &data.operations[store.index()];
        let OperationKind::Constant(constant) = &literal.kind else {
            break;
        };
        let OperationKind::Store(target) = store.kind else {
            break;
        };
        if data.places.get(target.index()) != Some(&Place::Cell(*cell))
            || literal.result.is_none()
            || data.operands(store.operands) != literal.result.as_ref().map(std::slice::from_ref)
            || defaults[position].is_some()
        {
            break;
        }
        defaults[position] = Some(constant.clone());
        at += 3;
    }
    Ok(defaults)
}

/// Arity and mapped arguments remain observable in a body or any arrow
/// sharing its activation. Ordinary nested functions have their own frame.
pub(super) fn arguments_free(program: &Program<'_>, body: UnitId) -> bool {
    arguments_free_in(program, body, &mut AllocationBudget::new(None))
        .expect("inspection activation")
}

fn observes_arguments(program: &Program<'_>, data: &UnitData) -> bool {
    data.places.iter().any(|place| {
        matches!(place, Place::Cell(cell)
        if ambient::classify(&program.cells[cell.index()]) == Some(Ambient::Arguments))
    }) || data
        .captures
        .iter()
        .any(|cell| ambient::classify(&program.cells[cell.index()]) == Some(Ambient::Arguments))
        || data.calls.iter().any(|site| {
            matches!(
                site.target,
                CallTarget::Value {
                    invocation: Invocation::DirectEval,
                    ..
                }
            )
        })
}
fn body_work(data: &UnitData, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
    for count in [
        data.places.len(),
        data.captures.len(),
        data.calls.len(),
        data.operations.len(),
    ] {
        budget.work(WorkKind::Analysis, count as u64)?;
    }
    Ok(())
}

pub(super) fn arguments_free_in(
    program: &Program<'_>,
    body: UnitId,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    let mut budget = budget.scope();
    let mut pending = budget.vector(Scratch, program.units.len())?;
    let mut seen = budget.filled(Scratch, program.units.len(), false)?;
    seen[body.index()] = true;
    pending.push(body);
    while let Some(unit) = pending.pop() {
        let data = program.unit(unit).expect("a program unit");
        body_work(data, &mut budget)?;
        if observes_arguments(program, data) {
            return Ok(false);
        }
        for operation in &data.operations {
            if let OperationKind::Closure(child) = operation.kind {
                if program
                    .unit(child)
                    .is_some_and(|data| ambient::inherits(program, data))
                    && !std::mem::replace(&mut seen[child.index()], true)
                {
                    pending.push(child);
                }
            }
        }
    }
    Ok(true)
}

/// Propagate activation observations from lexical arrows to their creators.
/// Each body and closure edge is visited once, including shared/cyclic IR.
/// This is the same reachability answer as the single-body query above.
pub(super) fn arguments_free_all_in(
    program: &Program<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<bool>, AllocationError> {
    budget.retained_phase(|budget| {
        let count = program.units.len();
        let mut free = budget.filled(Retained, count, true)?;
        let mut parents = storage::collect((0..count).map(|_| Vec::new()), Scratch, budget)?;
        let mut pending = budget.vector(Scratch, count)?;
        for frozen in &program.units {
            let data = frozen.data();
            body_work(data, budget)?;
            if observes_arguments(program, data) {
                free[frozen.id().index()] = false;
                pending.push(frozen.id());
            }
            for operation in &data.operations {
                if let OperationKind::Closure(child) = operation.kind {
                    if program
                        .unit(child)
                        .is_some_and(|data| ambient::inherits(program, data))
                    {
                        budget.push(Scratch, &mut parents[child.index()], frozen.id())?;
                    }
                }
            }
        }
        let mut cursor = 0;
        while cursor < pending.len() {
            let unit = pending[cursor];
            cursor += 1;
            for &parent in &parents[unit.index()] {
                budget.work(WorkKind::Analysis, 1)?;
                if std::mem::replace(&mut free[parent.index()], false) {
                    pending.push(parent);
                }
            }
        }
        Ok(free)
    })
}

pub(super) struct Plan {
    pub calls: Vec<(UnitId, CallId, u32)>,
    pub lengths: Vec<(UnitId, Option<u32>)>,
}

pub(super) fn plan(program: &Program<'_>, graph: &CallGraph) -> Plan {
    plan_in(program, graph, &mut AllocationBudget::new(None)).expect("inspection default transport")
}

pub(super) fn plan_in(
    program: &Program<'_>,
    graph: &CallGraph,
    budget: &mut AllocationBudget<'_>,
) -> Result<Plan, AllocationError> {
    budget.retained_phase(|budget| {
        let mut literals = budget.vector(Scratch, program.units.len())?;
        for unit in &program.units {
            literals.push(entry_literals_in(unit.data(), budget)?);
        }
        let free = arguments_free_all_in(program, budget)?;
        let mut plan = Plan {
            calls: Vec::new(),
            lengths: Vec::new(),
        };
        for frozen in &program.units {
            let unit = frozen.id();
            let data = frozen.data();
            budget.work(WorkKind::Analysis, data.parameters.len() as u64 + 1)?;
            let length = graph
                .complete_callers(unit)
                .filter(|edges| !edges.is_empty() && free[unit.index()])
                .and_then(|_| {
                    let defaults = &literals[unit.index()];
                    let first = defaults.iter().position(Option::is_some)?;
                    defaults[first..]
                        .iter()
                        .all(Option::is_some)
                        .then_some(first as u32)
                });
            if length != data.native_default_length {
                budget.push(Retained, &mut plan.lengths, (unit, length))?;
            }
            for (index, site) in data.calls.iter().enumerate() {
                budget.work(
                    WorkKind::Analysis,
                    data.arguments(site.arguments)
                        .map_or(0, |arguments| arguments.len()) as u64
                        + 1,
                )?;
                let call = CallId::from_index(index).expect("a call index");
                let omitted = omission(program, graph, &literals, &free, unit, call).unwrap_or(0);
                if omitted != site.omit_trailing {
                    budget.push(Retained, &mut plan.calls, (unit, call, omitted))?;
                }
            }
        }
        for values in literals {
            storage::release(values, Retained, budget)?;
        }
        storage::release(free, Retained, budget)?;
        Ok(plan)
    })
}

fn omission(
    program: &Program<'_>,
    graph: &CallGraph,
    literals: &[Vec<Option<Constant>>],
    free: &[bool],
    unit: UnitId,
    call: CallId,
) -> Option<u32> {
    let data = program.unit(unit)?;
    let site = &data.calls[call.index()];
    if site.contract.defaults != DefaultConvention::ApplyAtCallee
        || site.contract.instantiation.is_some()
        || !matches!(
            site.target,
            CallTarget::Value {
                invocation: Invocation::Value,
                ..
            }
        )
    {
        return None;
    }
    let Callee::Unit(body) = graph.callee(unit, call) else {
        return None;
    };
    // An escaping callable can expose its activation through reflection even
    // when the body has no lexical `arguments` read.
    graph.complete_callers(body)?;
    if !free[body.index()] {
        return None;
    }
    let defaults = &literals[body.index()];
    let arguments = data.arguments(site.arguments)?;
    if arguments.len() > defaults.len() {
        return None;
    }
    let mut keep = arguments.len();
    while keep > 0 {
        let position = keep - 1;
        let CallArgument::Value(value) = arguments[position] else {
            break;
        };
        let definition = &data.operations[data.values[value.index()].definition.index()];
        // Only literal operations can disappear from the target argument.
        // An exact result alone does not authorize dropping its evaluation.
        let OperationKind::Constant(argument) = &definition.kind else {
            break;
        };
        if !matches!(argument, Constant::Undefined) && defaults[position].as_ref() != Some(argument)
        {
            break;
        }
        keep = position;
    }
    Some((arguments.len() - keep) as u32)
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    #[test]
    fn activation_observations_propagate_only_through_inheriting_closures() {
        for source in [
            "int add(int n,int m=3){return n+m;}print(add(2));",
            "extern JsValue arguments;void report(int n=3){auto middle=()=>{auto leaf=()=>arguments[\"length\"];return leaf();};print(middle());}report();",
            "extern JsValue arguments;void child(){print(arguments[\"length\"]);}void parent(int n=3){child();print(n);}parent();",
        ] {
            let arena = bumpalo::Bump::new();
            let syntax = crate::parse_source(&arena, source).unwrap();
            let checked = crate::analyze(&syntax).unwrap();
            let program = from_checked_source(&syntax, &checked).unwrap();
            let free = arguments_free_all_in(&program, &mut AllocationBudget::new(None)).unwrap();
            for unit in &program.units {
                assert_eq!(free[unit.id().index()], arguments_free(&program, unit.id()), "{source}: {:?}", unit.id());
            }
            if source.contains("void parent") {
                let parent = program.units.iter().find(|unit| unit.data().function_name.is_some_and(|name| program.strings[name.index()].as_unicode() == Some("parent"))).unwrap();
                assert!(free[parent.id().index()], "an ordinary callee has its own arguments");
            }
        }
    }
}
