//! Exact values (plan M6.4a; architecture §7, the exact tier of the value
//! lattice). Knowledge travels through cells that are initialized once and
//! never reassigned, through parameters joined over complete call sets, and
//! through call results. Each operation's evaluation is `facts::exact`, the
//! language's one owner of primitive evaluation; this module adds only the
//! channels between operations and units.
//!
//! Per value the lattice is Bottom (no evidence yet: code that has not run,
//! or waits on a caller), Exact, and Top. Knowledge only rises; a worklist
//! over units reaches the least fixed point, the optimistic answer of sparse
//! conditional constant propagation, so a value is exact when every run that
//! computes it computes that one value.

use super::super::call_graph::{CallGraph, Callee, Seal};
use super::super::effects::ProgramEffects;
use super::super::facts::{exact, StoredExact, StoredKnowledge, UnknownReason, Work};
use super::super::*;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
enum Know {
    Bottom,
    Exact(StoredExact),
    Top,
}

impl Know {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, other) | (other, Self::Bottom) => other.clone(),
            (Self::Exact(a), Self::Exact(b)) if a == b => Self::Exact(a.clone()),
            _ => Self::Top,
        }
    }

    fn stored(&self) -> StoredKnowledge {
        match self {
            Self::Exact(value) => StoredKnowledge::Exact(value.clone()),
            _ => StoredKnowledge::Unknown(UnknownReason::Unvisited),
        }
    }
}

/// Exact knowledge per value of every unit.
pub(super) struct ProgramValues {
    units: Vec<Vec<Know>>,
    evaluated_calls: Vec<Vec<bool>>,
}

/// String evaluation is charged by `facts::exact`; these bound one analysis.
const WORK_QUOTA: u64 = 1 << 28;
const RESULT_LIMIT: u64 = 1 << 26;

struct Channels<'a> {
    formals: &'a [Vec<Know>],
    results: &'a [Know],
    cells: &'a [Know],
    settled: &'a [Option<(UnitId, ValueId)>],
}

impl ProgramValues {
    pub(super) fn evaluated_call(&self, unit: UnitId, op: OpId) -> bool {
        self.evaluated_calls[unit.index()][op.index()]
    }

    pub(super) fn exact(&self, unit: UnitId, value: ValueId) -> Option<&StoredExact> {
        match self.units.get(unit.index())?.get(value.index())? {
            Know::Exact(value) => Some(value),
            _ => None,
        }
    }

    fn unknown(program: &Program<'_>) -> Self {
        Self {
            evaluated_calls: program
                .units
                .iter()
                .map(|unit| vec![false; unit.data().operations.len()])
                .collect(),
            units: program
                .units
                .iter()
                .map(|unit| vec![Know::Top; unit.data().values.len()])
                .collect(),
        }
    }

    pub(super) fn compute(
        program: &Program<'_>,
        effects: &ProgramEffects,
        seal: Seal,
        pristine: bool,
    ) -> Self {
        let graph = effects.graph();
        let count = program.units.len();

        // Who calls whom, by resolved call.
        let mut callers: Vec<Vec<UnitId>> = vec![Vec::new(); count];
        let mut callees: Vec<Vec<UnitId>> = vec![Vec::new(); count];
        for frozen in &program.units {
            let unit = frozen.id();
            for operation in &frozen.data().operations {
                if let OperationKind::Call(call) = operation.kind {
                    if let Callee::Unit(callee) = graph.callee(unit, call) {
                        callers[callee.index()].push(unit);
                        callees[unit.index()].push(callee);
                    }
                }
            }
        }
        for list in callers.iter_mut().chain(callees.iter_mut()) {
            list.sort_unstable();
            list.dedup();
        }

        // Parameters wait on their callers when every call is known.
        let mut formals: Vec<Vec<Know>> = program
            .units
            .iter()
            .map(|frozen| {
                let complete = graph.complete_callers(frozen.id()).is_some();
                let start = if complete { Know::Bottom } else { Know::Top };
                vec![start; frozen.data().parameters.len()]
            })
            .collect();
        let mut results = vec![Know::Bottom; count];

        // Cells initialized once, never written after, of a primitive type.
        // Writes are read from the current program, not from conversion's
        // `reassigned` flag, which goes stale as rules remove stores. A
        // script's root bindings are globals: other scripts may write them.
        let mut cells = vec![Know::Top; program.cells.len()];
        let mut settled: Vec<Option<(UnitId, ValueId)>> = vec![None; program.cells.len()];
        let mut initialized_in: Vec<Vec<CellId>> = vec![Vec::new(); count];
        for (index, cell) in program.cells.iter().enumerate() {
            let id = CellId::from_index(index).unwrap();
            let root = program
                .unit(cell.owner)
                .is_some_and(|owner| owner.kind == UnitKind::ModuleInitialization);
            if cell.binding != CellBinding::Local
                || written(graph, id)
                || cell.synthetic
                || (root && seal != Seal::Module)
                || !primitive(program, cell.ty)
            {
                continue;
            }
            let Some((unit, op)) = graph.initializer(id) else {
                continue;
            };
            let data = program.unit(unit).expect("a program unit");
            let operation = &data.operations[op.index()];
            if !matches!(operation.kind, OperationKind::Initialize(target) if target == id) {
                continue;
            }
            let Some(&value) = data
                .operands(operation.operands)
                .and_then(|operands| operands.first())
            else {
                continue;
            };
            cells[index] = Know::Bottom;
            settled[index] = Some((unit, value));
            initialized_in[unit.index()].push(id);
        }
        let mut readers: Vec<Vec<UnitId>> = vec![Vec::new(); program.cells.len()];
        for frozen in &program.units {
            let data = frozen.data();
            for operation in &data.operations {
                if let OperationKind::Load(place) = operation.kind {
                    if let Some(&Place::Cell(cell)) = data.places.get(place.index()) {
                        if settled[cell.index()].is_some() {
                            readers[cell.index()].push(frozen.id());
                        }
                    }
                }
            }
        }
        for list in &mut readers {
            list.sort_unstable();
            list.dedup();
        }

        let mut values: Vec<Vec<Know>> = program
            .units
            .iter()
            .map(|frozen| vec![Know::Bottom; frozen.data().values.len()])
            .collect();
        let mut evaluated_calls: Vec<Vec<bool>> = program
            .units
            .iter()
            .map(|unit| vec![false; unit.data().operations.len()])
            .collect();
        let mut queue: VecDeque<usize> = (0..count).collect();
        let mut queued = vec![true; count];
        let mut work = Work::bounded(WORK_QUOTA, RESULT_LIMIT);
        // Every value, formal, result and cell rises at most twice; units are
        // re-queued only when one of their inputs rose.
        let mut remaining = count
            .saturating_mul(16)
            .saturating_add(program.cells.len())
            .saturating_add(64);
        while let Some(index) = queue.pop_front() {
            queued[index] = false;
            if remaining == 0 {
                return Self::unknown(program);
            }
            remaining -= 1;
            let unit = UnitId::from_index(index).unwrap();
            let data = program.units[index].data();
            let channels = Channels {
                formals: &formals,
                results: &results,
                cells: &cells,
                settled: &settled,
            };
            let returned = evaluate(
                program,
                effects,
                unit,
                data,
                &mut values[index],
                &channels,
                &mut work,
                pristine,
                &mut evaluated_calls[index],
            );
            let mut enqueue = |unit: UnitId, queue: &mut VecDeque<usize>| {
                if !std::mem::replace(&mut queued[unit.index()], true) {
                    queue.push_back(unit.index());
                }
            };

            // What this body returns.
            let result = if data.suspension != Suspension::None
                || data.kind == UnitKind::ModuleInitialization
            {
                Know::Top
            } else {
                returned
            };
            let joined = results[index].join(&result);
            if joined != results[index] {
                results[index] = joined;
                for &caller in &callers[index] {
                    enqueue(caller, &mut queue);
                }
            }

            // The parameters of the bodies this one calls.
            for &callee in &callees[index] {
                let Some(edges) = graph.complete_callers(callee) else {
                    continue;
                };
                let slots = formals[callee.index()].len();
                let mut next = vec![Know::Bottom; slots];
                for edge in edges {
                    let caller = program.unit(edge.caller).expect("a program unit");
                    let site = &caller.calls[edge.call.index()];
                    let arguments = caller.arguments(site.arguments).unwrap_or(&[]);
                    for (position, slot) in next.iter_mut().enumerate() {
                        let argument = match arguments.get(position) {
                            Some(CallArgument::Value(value)) => values[edge.caller.index()]
                                .get(value.index())
                                .cloned()
                                .unwrap_or(Know::Top),
                            _ => Know::Top,
                        };
                        *slot = slot.join(&argument);
                    }
                }
                let mut rose = false;
                for (old, new) in formals[callee.index()].iter_mut().zip(next) {
                    let joined = old.join(&new);
                    if joined != *old {
                        *old = joined;
                        rose = true;
                    }
                }
                if rose {
                    enqueue(callee, &mut queue);
                }
            }

            // The cells this body initializes.
            for &cell in &initialized_in[index] {
                let (_, value) = settled[cell.index()].expect("a settled cell");
                let joined = cells[cell.index()].join(&values[index][value.index()]);
                if joined != cells[cell.index()] {
                    cells[cell.index()] = joined;
                    for &reader in &readers[cell.index()] {
                        enqueue(reader, &mut queue);
                    }
                }
            }
        }
        Self {
            units: values,
            evaluated_calls,
        }
    }
}

/// Whether a store or a reference argument writes the cell after its
/// initialization, anywhere in the program as it now stands.
fn written(graph: &CallGraph, cell: CellId) -> bool {
    let storage = graph.storage(cell);
    storage.stored || storage.referenced
}

/// Whether `value` reads a parameter that always holds a value of its type:
/// every caller is known, typed and passes an argument in its position, and
/// the type excludes `undefined`, so a default that only an omitted or
/// `undefined` argument triggers never runs. Typed callers evaluate every
/// default they can (`DefaultConvention::MaterializeAtCaller`) and omit only
/// trailing arrows, and every store into the parameter is typed too.
/// Formation's `Binding::defined` states the same fact on the target tree.
fn typed_argument(
    program: &Program<'_>,
    graph: &CallGraph,
    unit: UnitId,
    data: &UnitData,
    value: ValueId,
) -> bool {
    let definition = &data.operations[data.values[value.index()].definition.index()];
    let OperationKind::Load(place) = definition.kind else {
        return false;
    };
    let Some(&Place::Cell(cell)) = data.places.get(place.index()) else {
        return false;
    };
    let storage = &program.cells[cell.index()];
    let CellBinding::Parameter(position) = storage.binding else {
        return false;
    };
    // A caller omits a trailing default only the callee can build (an
    // arrow); then the argument is `undefined` and the default applies.
    let supplied = |edge: &super::super::call_graph::CallEdge| {
        program.unit(edge.caller).is_some_and(|caller| {
            caller
                .arguments(caller.calls[edge.call.index()].arguments)
                .is_some_and(|arguments| arguments.len() > position as usize)
        })
    };
    storage.owner == unit
        && !program.is_reference_parameter(cell)
        && graph
            .complete_callers(unit)
            .is_some_and(|edges| edges.iter().all(supplied))
        && matches!(
            program.ty(storage.ty),
            Some(
                crate::check::Type::Int
                    | crate::check::Type::Float
                    | crate::check::Type::Bool
                    | crate::check::Type::String
                    | crate::check::Type::Enum(_)
                    | crate::check::Type::Array(_)
                    | crate::check::Type::Record(_)
                    | crate::check::Type::Map(_, _)
                    | crate::check::Type::Set(_)
                    | crate::check::Type::Regex
                    | crate::check::Type::Struct(_)
                    | crate::check::Type::Class(_)
                    | crate::check::Type::StructInstance { .. }
                    | crate::check::Type::ClassInstance { .. }
                    | crate::check::Type::Function(_)
            )
        )
}

fn primitive(program: &Program<'_>, ty: TypeId) -> bool {
    matches!(
        program.ty(ty),
        Some(crate::check::Type::Int | crate::check::Type::Float)
            | Some(crate::check::Type::Bool | crate::check::Type::String)
    )
}

enum Step {
    Enter(RegionId),
    Eval(OpId),
}

/// Evaluates one unit's owned operations, children before the operation that
/// owns them, and returns what its `return`s yield (undefined for a body with
/// none: only a body without a result can complete without one).
fn evaluate(
    program: &Program<'_>,
    effects: &ProgramEffects,
    unit: UnitId,
    data: &UnitData,
    know: &mut [Know],
    channels: &Channels<'_>,
    work: &mut Work,
    pristine: bool,
    evaluated_calls: &mut [bool],
) -> Know {
    let graph = effects.graph();
    let initialization = effects.initialization();
    let mut stored: Vec<StoredKnowledge> = know.iter().map(Know::stored).collect();
    let mut returned: Option<Know> = None;
    let mut stack = vec![Step::Enter(data.entry)];
    while let Some(step) = stack.pop() {
        let op = match step {
            Step::Enter(region) => {
                for &op in data.regions[region.index()].operations.iter().rev() {
                    stack.push(Step::Eval(op));
                    let children: Vec<RegionId> =
                        data.operations[op.index()].kind.child_regions().collect();
                    for child in children.into_iter().rev() {
                        stack.push(Step::Enter(child));
                    }
                }
                continue;
            }
            Step::Eval(op) => op,
        };
        let operation = &data.operations[op.index()];
        let operands = data.operands(operation.operands).unwrap_or(&[]);
        if matches!(operation.kind, OperationKind::Return) {
            let value = operands
                .first()
                .map_or(Know::Exact(StoredExact::Undefined), |value| {
                    know[value.index()].clone()
                });
            returned = Some(returned.map_or(value.clone(), |known| known.join(&value)));
            continue;
        }
        let Some(result) = operation.result else {
            continue;
        };
        let next = match &operation.kind {
            OperationKind::Load(place) => match data.places.get(place.index()) {
                Some(&Place::Cell(cell)) => {
                    let storage = &program.cells[cell.index()];
                    match storage.binding {
                        CellBinding::Parameter(position) if !written(graph, cell) => channels
                            .formals
                            .get(storage.owner.index())
                            .and_then(|formals| formals.get(position as usize))
                            .cloned()
                            .unwrap_or(Know::Top),
                        CellBinding::Local if channels.settled[cell.index()].is_some() => {
                            match &channels.cells[cell.index()] {
                                Know::Exact(value)
                                    if initialization.initialized(program, unit, op, cell) =>
                                {
                                    Know::Exact(value.clone())
                                }
                                Know::Bottom => Know::Bottom,
                                _ => Know::Top,
                            }
                        }
                        _ => Know::Top,
                    }
                }
                _ => Know::Top,
            },
            OperationKind::IsUndefined
                if operands
                    .first()
                    .is_some_and(|value| typed_argument(program, graph, unit, data, *value)) =>
            {
                Know::Exact(StoredExact::Boolean(false))
            }
            OperationKind::Call(call) => {
                let site = &data.calls[call.index()];
                let pending = data.arguments(site.arguments).unwrap_or(&[]).iter().any(|argument| {
                    matches!(argument, CallArgument::Value(value) if know[value.index()] == Know::Bottom)
                }) || matches!(site.target, CallTarget::Intrinsic { receiver: Some(value), .. }
                    if know[value.index()] == Know::Bottom);
                let result =
                    super::evaluate::call(program, effects, unit, *call, &stored, pristine, work);
                evaluated_calls[op.index()] = result.is_some();
                if let Some(value) = result {
                    Know::Exact(value)
                } else if pending {
                    // A later caller or initializer may supply the inputs.
                    // Unsupported after exact inputs is Top; unvisited is not.
                    Know::Bottom
                } else {
                    match graph.callee(unit, *call) {
                        Callee::Unit(callee)
                            if program
                                .unit(callee)
                                .is_some_and(|body| body.suspension == Suspension::None) =>
                        {
                            channels.results[callee.index()].clone()
                        }
                        _ => Know::Top,
                    }
                }
            }
            kind => {
                let mut pending = operands
                    .iter()
                    .any(|value| know[value.index()] == Know::Bottom);
                if matches!(
                    kind,
                    OperationKind::Select { .. } | OperationKind::ShortCircuit { .. }
                ) {
                    pending |= kind.child_regions().any(|region| {
                        data.regions[region.index()]
                            .result
                            .is_some_and(|value| know[value.index()] == Know::Bottom)
                    });
                }
                if pending {
                    Know::Bottom
                } else {
                    match exact(program, data, operation, &stored, work) {
                        StoredKnowledge::Exact(value) => Know::Exact(value),
                        StoredKnowledge::Unknown(_) => Know::Top,
                    }
                }
            }
        };
        let joined = know[result.index()].join(&next);
        if joined != know[result.index()] {
            stored[result.index()] = joined.stored();
            know[result.index()] = joined;
        }
    }
    returned.unwrap_or(Know::Exact(StoredExact::Undefined))
}
