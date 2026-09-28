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

use super::super::call_graph::{Callee, Seal};
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
    pub(super) fn exact(&self, unit: UnitId, value: ValueId) -> Option<&StoredExact> {
        match self.units.get(unit.index())?.get(value.index())? {
            Know::Exact(value) => Some(value),
            _ => None,
        }
    }

    fn unknown(program: &Program<'_>) -> Self {
        Self {
            units: program
                .units
                .iter()
                .map(|unit| vec![Know::Top; unit.data().values.len()])
                .collect(),
        }
    }

    pub(super) fn compute(program: &Program<'_>, effects: &ProgramEffects, seal: Seal) -> Self {
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

        // Cells initialized once, never reassigned, of a primitive type. A
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
                || cell.reassigned
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
        Self { units: values }
    }
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
                        CellBinding::Parameter(position) if !storage.reassigned => channels
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
            OperationKind::Call(call) => match graph.callee(unit, *call) {
                Callee::Unit(callee)
                    if program
                        .unit(callee)
                        .is_some_and(|body| body.suspension == Suspension::None) =>
                {
                    channels.results[callee.index()].clone()
                }
                _ => Know::Top,
            },
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
