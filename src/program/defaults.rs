//! Checked default transport at the program-to-target boundary.
//!
//! The semantic call keeps all caller-built arguments. JavaScript can omit a
//! trailing literal only when the resolved body still applies that literal
//! on entry and cannot observe its argument count. Native keeps the original
//! operands. Proofs inspect the owned operations, never a formed target tree.

use super::ambient::{self, Ambient};
use super::call_graph::{CallGraph, Callee};
use super::*;

/// Literal defaults still implemented by the entry prefix, by parameter.
pub(super) fn entry_literals(data: &UnitData) -> Vec<Option<Constant>> {
    let mut defaults = vec![None; data.parameters.len()];
    let operations = &data.regions[data.entry.index()].operations;
    let mut at = 0;
    while at + 2 < operations.len() {
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
        if !matches!(test.kind, OperationKind::IsUndefined)
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
    defaults
}

/// Arity and mapped arguments remain observable in a body or any arrow
/// sharing its activation. Ordinary nested functions have their own frame.
pub(super) fn arguments_free(program: &Program<'_>, body: UnitId) -> bool {
    let mut pending = vec![body];
    let mut seen = std::collections::HashSet::new();
    while let Some(unit) = pending.pop() {
        if !seen.insert(unit) {
            continue;
        }
        let data = program.unit(unit).expect("a program unit");
        if data.places.iter().any(|place| {
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
        {
            return false;
        }
        for operation in &data.operations {
            if let OperationKind::Closure(child) = operation.kind {
                if program
                    .unit(child)
                    .is_some_and(|data| ambient::inherits(data.kind))
                {
                    pending.push(child);
                }
            }
        }
    }
    true
}

pub(super) struct Plan {
    pub calls: Vec<(UnitId, CallId, u32)>,
    pub lengths: Vec<(UnitId, Option<u32>)>,
}

pub(super) fn plan(program: &Program<'_>, graph: &CallGraph) -> Plan {
    let literals: Vec<_> = program
        .units
        .iter()
        .map(|unit| entry_literals(unit.data()))
        .collect();
    let free: Vec<_> = program
        .units
        .iter()
        .map(|unit| arguments_free(program, unit.id()))
        .collect();
    let mut plan = Plan {
        calls: Vec::new(),
        lengths: Vec::new(),
    };
    for frozen in &program.units {
        let unit = frozen.id();
        let data = frozen.data();
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
            plan.lengths.push((unit, length));
        }
        for (index, site) in data.calls.iter().enumerate() {
            let call = CallId::from_index(index).expect("a call index");
            let omitted = omission(program, graph, &literals, &free, unit, call).unwrap_or(0);
            if omitted != site.omit_trailing {
                plan.calls.push((unit, call, omitted));
            }
        }
    }
    plan
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
    if site.contract.defaults != DefaultConvention::MaterializeAtCaller
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
