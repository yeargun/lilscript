//! Exact values (plan M6.4a; architecture §7, the exact tier of the value
//! lattice). Knowledge travels through cells that are initialized once and
//! never reassigned, through parameters joined over complete call sets, and
//! through call results. Each operation's evaluation is `facts::exact`, the
//! language's one owner of primitive evaluation; this module adds only the
//! channels between operations and units.
//!
//! Per value the lattice is Bottom (no evidence yet: code that has not run,
//! or waits on a caller), Exact, a bounded finite set, and Top. Knowledge only
//! rises; a worklist over units reaches the least fixed point, the optimistic
//! answer of sparse conditional constant propagation. A finite set keeps every
//! observed primitive value or becomes Top: it is never sampled or truncated.

use super::super::call_graph::{CallGraph, Callee, Seal};
use super::super::effects::ProgramEffects;
use super::super::facts::{exact, StoredExact, StoredKnowledge, UnknownReason, Work};
use super::super::*;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
enum Know {
    Bottom,
    Exact(StoredExact),
    Finite(Vec<StoredExact>),
    Top,
}

impl Know {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, other) | (other, Self::Bottom) => other.clone(),
            (Self::Exact(a), Self::Exact(b)) if a == b => Self::Exact(a.clone()),
            (Self::Exact(a), Self::Exact(b)) => Self::set([a.clone(), b.clone()]),
            (Self::Exact(value), Self::Finite(values))
            | (Self::Finite(values), Self::Exact(value)) => {
                let mut joined = values.clone();
                if !joined.contains(value) {
                    joined.push(value.clone());
                }
                Self::set(joined)
            }
            (Self::Finite(a), Self::Finite(b)) => {
                let mut joined = a.clone();
                for value in b {
                    if !joined.contains(value) {
                        joined.push(value.clone());
                    }
                }
                Self::set(joined)
            }
            _ => Self::Top,
        }
    }

    fn set(values: impl IntoIterator<Item = StoredExact>) -> Self {
        let mut unique = Vec::new();
        for value in values {
            if !unique.contains(&value) {
                unique.push(value);
                if unique.len() > FINITE_LIMIT {
                    return Self::Top;
                }
            }
        }
        match unique.len() {
            0 => Self::Bottom,
            1 => Self::Exact(unique.pop().unwrap()),
            _ => Self::Finite(unique),
        }
    }

    fn alternatives(&self) -> Option<&[StoredExact]> {
        match self {
            Self::Exact(value) => Some(std::slice::from_ref(value)),
            Self::Finite(values) => Some(values),
            Self::Bottom | Self::Top => None,
        }
    }

    fn stored(&self) -> StoredKnowledge {
        match self {
            Self::Exact(value) => StoredKnowledge::Exact(value.clone()),
            Self::Bottom | Self::Finite(_) | Self::Top => {
                StoredKnowledge::Unknown(UnknownReason::Unvisited)
            }
        }
    }
}

/// Four alternatives cover boolean/nullable switches and small enum-like call
/// sets without letting a public caller or a large dispatch table turn the
/// structural rule phase into combinatorial search.
const FINITE_LIMIT: usize = 4;
const POINTWISE_LIMIT: usize = 64;

/// Exact knowledge per value of every unit.
pub(super) struct ProgramValues {
    units: Vec<Vec<Know>>,
    evaluated_calls: Vec<Vec<bool>>,
    origins: Vec<Vec<u8>>,
    pub(super) evaluated: u32,
    pub(super) refused: [u32; 6],
}

pub(super) const FROM_SET: u8 = 1;
pub(super) const FROM_PATH: u8 = 2;

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
    pub(super) fn origin(&self, unit: UnitId, value: ValueId) -> u8 {
        self.origins[unit.index()][value.index()]
    }
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
            evaluated: 0,
            refused: [0; 6],
            origins: program
                .units
                .iter()
                .map(|unit| vec![0; unit.data().values.len()])
                .collect(),
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

        // Cells initialized once, never written after, that can hold primitives.
        // Writes are read from the current program, not from conversion's
        // `reassigned` flag, which goes stale as rules remove stores. An
        // explicitly open root contract permits other scripts to write them.
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
        let mut origins: Vec<Vec<u8>> = program
            .units
            .iter()
            .map(|unit| vec![0; unit.data().values.len()])
            .collect();
        let mut evaluated = 0u32;
        let mut refused = [0u32; 6];
        let mut queue: VecDeque<usize> = (0..count).collect();
        let mut queued = vec![true; count];
        let mut work = Work::bounded(WORK_QUOTA, RESULT_LIMIT);
        // Each channel adds at most FINITE_LIMIT alternatives before Top;
        // units are
        // re-queued only when one of their inputs rose.
        let mut remaining = count
            .saturating_mul(16)
            .saturating_add(program.cells.len())
            .saturating_add(64);
        while let Some(index) = queue.pop_front() {
            queued[index] = false;
            if remaining == 0 {
                let mut unknown = Self::unknown(program);
                unknown.evaluated = evaluated;
                unknown.refused = refused;
                unknown.refused[super::evaluate::Refusal::Limit as usize] += 1;
                return unknown;
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
                &mut origins[index],
                &mut evaluated,
                &mut refused,
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
                            Some(CallArgument::Value(value)) => {
                                let known = values[edge.caller.index()]
                                    .get(value.index())
                                    .cloned()
                                    .unwrap_or(Know::Top);
                                if position
                                    >= arguments.len().saturating_sub(site.omit_trailing as usize)
                                {
                                    // Both target entry conventions are valid:
                                    // native supplies the materialized value;
                                    // JavaScript runs the retained default.
                                    known.join(&Know::Exact(StoredExact::Undefined))
                                } else {
                                    known
                                }
                            }
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
            origins,
            evaluated,
            refused,
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
            let site = &caller.calls[edge.call.index()];
            caller.arguments(site.arguments).is_some_and(|arguments| {
                arguments.len().saturating_sub(site.omit_trailing as usize) > position as usize
            })
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
    fn eligible(ty: &crate::check::Type<'_>) -> bool {
        use crate::check::Type;
        match ty {
            Type::Int
            | Type::Float
            | Type::Bool
            | Type::String
            | Type::Null
            | Type::Enum(_)
            | Type::Dynamic
            | Type::Unknown => true,
            // The primitive lattice can still prove null (or a primitive
            // arm) when another legal inhabitant is an aggregate. An actual
            // aggregate producer becomes Top; annotations never invent one.
            Type::Nullable(_) | Type::Union(_) => true,
            _ => false,
        }
    }
    program.ty(ty).is_some_and(eligible)
}

enum Step {
    Enter(RegionId),
    Prepare(OpId),
    Eval(OpId),
    Child {
        region: RegionId,
        refinements: Vec<(ValueId, Know)>,
    },
    Restore {
        values: Vec<(ValueId, Know)>,
        cells: Vec<(CellId, Option<Know>)>,
    },
}

/// Evaluate one transfer for every combination of its bounded finite inputs.
/// Any unknown input, unsupported combination or exhausted work makes the
/// transfer unknown. More than `FINITE_LIMIT` distinct results becomes Top;
/// no alternative is discarded to manufacture a fact.
fn pointwise(
    know: &[Know],
    stored: &mut [StoredKnowledge],
    inputs: impl IntoIterator<Item = ValueId>,
    work: &mut Work,
    mut evaluate: impl FnMut(&[StoredKnowledge], &mut Work) -> Option<StoredExact>,
) -> Option<Know> {
    let mut varied: Vec<(ValueId, &[StoredExact])> = Vec::new();
    let mut seen = Vec::new();
    let mut combinations = 1usize;
    for input in inputs {
        if seen.contains(&input) {
            continue;
        }
        seen.push(input);
        let alternatives = know.get(input.index())?.alternatives()?;
        if alternatives.len() > 1 {
            combinations = combinations.checked_mul(alternatives.len())?;
            if combinations > POINTWISE_LIMIT {
                return None;
            }
            varied.push((input, alternatives));
        }
    }

    let mut result = Know::Bottom;
    let mut complete = true;
    for ordinal in 0..combinations {
        let mut selection = ordinal;
        for &(value, alternatives) in &varied {
            let at = selection % alternatives.len();
            selection /= alternatives.len();
            stored[value.index()] = StoredKnowledge::Exact(alternatives[at].clone());
        }
        if !work.charge(1) {
            complete = false;
            break;
        }
        let Some(value) = evaluate(stored, work) else {
            complete = false;
            break;
        };
        result = result.join(&Know::Exact(value));
        if result == Know::Top {
            break;
        }
    }
    for &(value, _) in &varied {
        stored[value.index()] = know[value.index()].stored();
    }
    complete.then_some(result)
}

fn operation_inputs(data: &UnitData, operation: &Operation) -> Vec<ValueId> {
    let mut inputs = data.operands(operation.operands).unwrap_or(&[]).to_vec();
    for region in operation.kind.child_regions() {
        inputs.extend(data.regions[region.index()].result);
    }
    inputs
}

fn call_inputs(data: &UnitData, call: CallId) -> Vec<ValueId> {
    let site = &data.calls[call.index()];
    let mut inputs: Vec<ValueId> = data
        .arguments(site.arguments)
        .unwrap_or(&[])
        .iter()
        .filter_map(|argument| match argument {
            CallArgument::Value(value) | CallArgument::Spread(value) => Some(*value),
            CallArgument::Reference(_) => None,
        })
        .collect();
    if let CallTarget::Intrinsic {
        receiver: Some(receiver),
        ..
    } = site.target
    {
        inputs.push(receiver);
    }
    inputs
}

fn comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Less
            | BinaryOp::LessEq
            | BinaryOp::Greater
            | BinaryOp::GreaterEq
    )
}

/// Narrow the finite operands of a branch condition to alternatives that can
/// reach one outcome. Filtering each operand independently is an over-
/// approximation when both vary, so it never invents correlation.
#[allow(clippy::too_many_arguments)]
fn branch_refinements(
    program: &Program<'_>,
    data: &UnitData,
    know: &[Know],
    stored: &mut [StoredKnowledge],
    condition: ValueId,
    holds: bool,
    work: &mut Work,
    depth: usize,
) -> Option<Vec<(ValueId, Know)>> {
    if depth == 0 {
        return Some(Vec::new());
    }
    let mut refinements = Vec::new();
    if let Some(alternatives) = know.get(condition.index())?.alternatives() {
        let filtered =
            Know::set(alternatives.iter().filter_map(|value| {
                (facts::truthy(program, value) == holds).then_some(value.clone())
            }));
        if filtered == Know::Bottom {
            return None;
        }
        if &filtered != &know[condition.index()] {
            refinements.push((condition, filtered));
        }
    }

    let definition = &data.operations[data.values[condition.index()].definition.index()];
    let operands = data.operands(definition.operands).unwrap_or(&[]);
    match definition.kind {
        OperationKind::Unary {
            op: super::super::UnaryOp::Not,
            ..
        } => {
            let mut nested = branch_refinements(
                program,
                data,
                know,
                stored,
                *operands.first()?,
                !holds,
                work,
                depth - 1,
            )?;
            refinements.append(&mut nested);
        }
        OperationKind::IsUndefined => {
            let value = *operands.first()?;
            let Some(alternatives) = know.get(value.index())?.alternatives() else {
                return Some(refinements);
            };
            let filtered = Know::set(alternatives.iter().filter_map(|candidate| {
                (matches!(candidate, StoredExact::Undefined) == holds).then_some(candidate.clone())
            }));
            if filtered == Know::Bottom {
                return None;
            }
            if filtered != know[value.index()] {
                refinements.push((value, filtered));
            }
        }
        OperationKind::Binary(op) if comparison(op) && operands.len() == 2 => {
            let [left, right] = [operands[0], operands[1]];
            let Some(left_values) = know[left.index()].alternatives() else {
                return Some(refinements);
            };
            let Some(right_values) = know[right.index()].alternatives() else {
                return Some(refinements);
            };
            if left_values.len().saturating_mul(right_values.len()) > POINTWISE_LIMIT {
                return Some(refinements);
            }
            let mut left_kept = Vec::new();
            let mut right_kept = Vec::new();
            for left_value in left_values {
                for right_value in right_values {
                    stored[left.index()] = StoredKnowledge::Exact(left_value.clone());
                    stored[right.index()] = StoredKnowledge::Exact(right_value.clone());
                    if !work.charge(1) {
                        stored[left.index()] = know[left.index()].stored();
                        stored[right.index()] = know[right.index()].stored();
                        return Some(refinements);
                    }
                    let result = exact(program, data, definition, stored, work);
                    if !matches!(result, StoredKnowledge::Exact(StoredExact::Boolean(_))) {
                        stored[left.index()] = know[left.index()].stored();
                        stored[right.index()] = know[right.index()].stored();
                        return Some(refinements);
                    }
                    if matches!(result, StoredKnowledge::Exact(StoredExact::Boolean(value)) if value == holds)
                    {
                        if !left_kept.contains(left_value) {
                            left_kept.push(left_value.clone());
                        }
                        if !right_kept.contains(right_value) {
                            right_kept.push(right_value.clone());
                        }
                    }
                }
            }
            stored[left.index()] = know[left.index()].stored();
            stored[right.index()] = know[right.index()].stored();
            let left_next = Know::set(left_kept);
            let right_next = Know::set(right_kept);
            if left_next == Know::Bottom || right_next == Know::Bottom {
                return None;
            }
            if left_next != know[left.index()] {
                refinements.push((left, left_next));
            }
            if right_next != know[right.index()] {
                refinements.push((right, right_next));
            }
        }
        _ => {}
    }
    Some(refinements)
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
    origins: &mut [u8],
    evaluated: &mut u32,
    refused: &mut [u32; 6],
) -> Know {
    let graph = effects.graph();
    let initialization = effects.initialization();
    let mut stored: Vec<StoredKnowledge> = know.iter().map(Know::stored).collect();
    let mut path_cells: std::collections::HashMap<CellId, Know> = std::collections::HashMap::new();
    let mut returned: Option<Know> = None;
    let mut stack = vec![Step::Enter(data.entry)];
    while let Some(step) = stack.pop() {
        let op = match step {
            Step::Enter(region) => {
                for &op in data.regions[region.index()].operations.iter().rev() {
                    stack.push(Step::Prepare(op));
                }
                continue;
            }
            Step::Prepare(op) => {
                let operation = &data.operations[op.index()];
                let operands = data.operands(operation.operands).unwrap_or(&[]);
                let mut children: Vec<(RegionId, Vec<(ValueId, Know)>)> = Vec::new();
                match operation.kind {
                    OperationKind::If { yes, no } => {
                        if let Some(&condition) = operands.first() {
                            if let Some(refinements) = branch_refinements(
                                program,
                                data,
                                know,
                                &mut stored,
                                condition,
                                true,
                                work,
                                8,
                            ) {
                                children.push((yes, refinements));
                            }
                            if let Some(no) = no {
                                if let Some(refinements) = branch_refinements(
                                    program,
                                    data,
                                    know,
                                    &mut stored,
                                    condition,
                                    false,
                                    work,
                                    8,
                                ) {
                                    children.push((no, refinements));
                                }
                            }
                        }
                    }
                    OperationKind::Select { yes, no } => {
                        if let Some(&condition) = operands.first() {
                            if let Some(refinements) = branch_refinements(
                                program,
                                data,
                                know,
                                &mut stored,
                                condition,
                                true,
                                work,
                                8,
                            ) {
                                children.push((yes, refinements));
                            }
                            if let Some(refinements) = branch_refinements(
                                program,
                                data,
                                know,
                                &mut stored,
                                condition,
                                false,
                                work,
                                8,
                            ) {
                                children.push((no, refinements));
                            }
                        }
                    }
                    _ => children.extend(
                        operation
                            .kind
                            .child_regions()
                            .map(|region| (region, Vec::new())),
                    ),
                }
                stack.push(Step::Eval(op));
                for (region, refinements) in children.into_iter().rev() {
                    stack.push(Step::Child {
                        region,
                        refinements,
                    });
                }
                continue;
            }
            Step::Child {
                region,
                refinements,
            } => {
                let mut previous = Vec::new();
                let mut previous_cells = Vec::new();
                for (value, refinement) in refinements {
                    if previous.iter().any(|(seen, _)| *seen == value) {
                        continue;
                    }
                    previous.push((value, know[value.index()].clone()));
                    let definition =
                        &data.operations[data.values[value.index()].definition.index()];
                    if let OperationKind::Load(place) = definition.kind {
                        if let Some(&Place::Cell(cell)) = data.places.get(place.index()) {
                            if !written(graph, cell)
                                && !previous_cells.iter().any(|(seen, _)| *seen == cell)
                            {
                                let old = path_cells.insert(cell, refinement.clone());
                                previous_cells.push((cell, old));
                            }
                        }
                    }
                    know[value.index()] = refinement;
                    stored[value.index()] = know[value.index()].stored();
                }
                stack.push(Step::Restore {
                    values: previous,
                    cells: previous_cells,
                });
                stack.push(Step::Enter(region));
                continue;
            }
            Step::Restore { values, cells } => {
                for (value, knowledge) in values {
                    know[value.index()] = knowledge;
                    stored[value.index()] = know[value.index()].stored();
                }
                for (cell, previous) in cells {
                    match previous {
                        Some(known) => {
                            path_cells.insert(cell, known);
                        }
                        None => {
                            path_cells.remove(&cell);
                        }
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
        let inputs = match operation.kind {
            OperationKind::Call(call) => call_inputs(data, call),
            _ => operation_inputs(data, operation),
        };
        let mut origin = inputs.iter().fold(0, |bits, input| {
            bits | origins[input.index()]
                | if matches!(know[input.index()], Know::Finite(_)) {
                    FROM_SET
                } else {
                    0
                }
        });
        if matches!(operation.kind, OperationKind::Load(place)
            if matches!(data.places.get(place.index()), Some(Place::Cell(cell)) if path_cells.contains_key(cell)))
        {
            origin |= FROM_PATH;
        }
        let next = match &operation.kind {
            OperationKind::Load(place) => match data.places.get(place.index()) {
                Some(&Place::Cell(cell)) => {
                    let storage = &program.cells[cell.index()];
                    match storage.binding {
                        _ if path_cells.contains_key(&cell)
                            && initialization.initialized(program, unit, op, cell) =>
                        {
                            path_cells[&cell].clone()
                        }
                        CellBinding::Parameter(position) if !written(graph, cell) => channels
                            .formals
                            .get(storage.owner.index())
                            .and_then(|formals| formals.get(position as usize))
                            .cloned()
                            .unwrap_or(Know::Top),
                        CellBinding::Local if channels.settled[cell.index()].is_some() => {
                            match &channels.cells[cell.index()] {
                                known @ (Know::Exact(_) | Know::Finite(_))
                                    if initialization.initialized(program, unit, op, cell) =>
                                {
                                    known.clone()
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
                let result = pointwise(
                    know,
                    &mut stored,
                    call_inputs(data, *call),
                    work,
                    |stored, work| {
                        let attempt = super::evaluate::call(
                            program, effects, unit, *call, stored, pristine, work,
                        );
                        if let Some(reason) = attempt.refusal {
                            refused[reason as usize] = refused[reason as usize].saturating_add(1);
                        } else {
                            *evaluated = evaluated.saturating_add(1);
                        }
                        attempt.value
                    },
                );
                evaluated_calls[op.index()] = matches!(result, Some(Know::Exact(_)));
                if let Some(value) = result {
                    value
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
                    pointwise(
                        know,
                        &mut stored,
                        operation_inputs(data, operation),
                        work,
                        |stored, work| match exact(program, data, operation, stored, work) {
                            StoredKnowledge::Exact(value) => Some(value),
                            StoredKnowledge::Unknown(_) => None,
                        },
                    )
                    .unwrap_or(Know::Top)
                }
            }
        };
        let joined = know[result.index()].join(&next);
        if joined != know[result.index()] {
            stored[result.index()] = joined.stored();
            know[result.index()] = joined;
        }
        origins[result.index()] |= origin;
    }
    returned.unwrap_or(Know::Exact(StoredExact::Undefined))
}
