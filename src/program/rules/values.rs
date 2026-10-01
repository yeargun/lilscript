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
use super::storage::Map;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum Know {
    Bottom,
    Exact(StoredExact),
    Finite(Arc<[StoredExact]>),
    Top,
}

impl Know {
    fn join(&self, other: &Self, work: &mut Work) -> Self {
        match (self, other) {
            (Self::Bottom, other) | (other, Self::Bottom) => other.clone(),
            (Self::Top, _) | (_, Self::Top) => Self::Top,
            _ if self == other => self.clone(),
            _ => {
                let left = self.alternatives().unwrap();
                let right = other.alternatives().unwrap();
                if right.iter().all(|value| left.contains(value)) {
                    return self.clone();
                }
                if left.iter().all(|value| right.contains(value)) {
                    return other.clone();
                }
                Self::set(left.iter().chain(right).cloned(), work)
            }
        }
    }

    fn set(values: impl IntoIterator<Item = StoredExact>, work: &mut Work) -> Self {
        // Construct a bounded set on the stack. Shared immutable sets make
        // propagation/cloning allocation-free, keeping the common Exact small.
        let mut unique = std::array::from_fn::<_, FINITE_LIMIT, _>(|_| StoredExact::Undefined);
        let mut len = 0;
        for value in values {
            if !unique[..len].contains(&value) {
                if len == FINITE_LIMIT {
                    return Self::Top;
                }
                unique[len] = value;
                len += 1;
            }
        }
        match len {
            0 => Self::Bottom,
            1 => Self::Exact(std::mem::replace(&mut unique[0], StoredExact::Undefined)),
            _ => {
                let bytes =
                    len * std::mem::size_of::<StoredExact>() + 2 * std::mem::size_of::<usize>();
                if !work.reserve_evaluation(bytes as u64) {
                    return Self::Top;
                }
                Self::Finite(Arc::from(&unique[..len]))
            }
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

    pub(super) fn compute_in(
        program: &Program<'_>,
        effects: &ProgramEffects,
        seal: Seal,
        pristine: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut work = Work::admitted(WORK_QUOTA, RESULT_LIMIT, budget.scope());
        let result = Self::compute(program, effects, seal, pristine, &mut work);
        work.finish()?;
        result.ok_or(AllocationError::Capacity)
    }

    fn compute(
        program: &Program<'_>,
        effects: &ProgramEffects,
        seal: Seal,
        pristine: bool,
        work: &mut Work,
    ) -> Option<Self> {
        let graph = effects.graph();
        let count = program.units.len();

        // Who calls whom, by resolved call.
        let mut callers = work.collect(Scratch, (0..count).map(|_| Vec::<UnitId>::new()))?;
        let mut callees = work.collect(Scratch, (0..count).map(|_| Vec::<UnitId>::new()))?;
        for frozen in &program.units {
            let unit = frozen.id();
            for operation in &frozen.data().operations {
                visit(work, 1)?;
                if let OperationKind::Call(call) = operation.kind {
                    if let Callee::Unit(callee) = graph.callee(unit, call) {
                        work.push(Scratch, &mut callers[callee.index()], unit)?;
                        work.push(Scratch, &mut callees[unit.index()], callee)?;
                    }
                }
            }
        }
        for list in callers.iter_mut().chain(callees.iter_mut()) {
            visit(work, sort_work(list.len()))?;
            list.sort_unstable();
            list.dedup();
        }

        // Parameters wait on their callers when every call is known.
        let mut formals = work.collect(Scratch, (0..count).map(|_| Vec::new()))?;
        for frozen in &program.units {
            let start = if graph.complete_callers(frozen.id()).is_some() {
                Know::Bottom
            } else {
                Know::Top
            };
            formals[frozen.id().index()] = work.collect(
                Scratch,
                std::iter::repeat_n(start, frozen.data().parameters.len()),
            )?;
        }
        let mut results = work.collect(Scratch, std::iter::repeat_n(Know::Bottom, count))?;

        // Cells initialized once, never written after, that can hold primitives.
        // Writes are read from the current program, not from conversion's
        // `reassigned` flag, which goes stale as rules remove stores. An
        // explicitly open root contract permits other scripts to write them.
        let mut cells =
            work.collect(Scratch, std::iter::repeat_n(Know::Top, program.cells.len()))?;
        let mut settled = work.admit(|budget| {
            budget.filled(Scratch, program.cells.len(), None::<(UnitId, ValueId)>)
        })?;
        let mut initialized_in = work.collect(Scratch, (0..count).map(|_| Vec::<CellId>::new()))?;
        for (index, cell) in program.cells.iter().enumerate() {
            visit(work, 1)?;
            let id = CellId::from_index(index).unwrap();
            let root = program
                .unit(cell.owner)
                .is_some_and(|owner| owner.kind == UnitKind::ModuleInitialization);
            if cell.binding != CellBinding::Local
                || written(graph, id)
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
            work.push(Scratch, &mut initialized_in[unit.index()], id)?;
        }
        let mut readers = work.collect(
            Scratch,
            (0..program.cells.len()).map(|_| Vec::<UnitId>::new()),
        )?;
        for frozen in &program.units {
            let data = frozen.data();
            for operation in &data.operations {
                visit(work, 1)?;
                if let OperationKind::Load(place) = operation.kind {
                    if let Some(&Place::Cell(cell)) = data.places.get(place.index()) {
                        if settled[cell.index()].is_some() {
                            work.push(Scratch, &mut readers[cell.index()], frozen.id())?;
                        }
                    }
                }
            }
        }
        for list in &mut readers {
            visit(work, sort_work(list.len()))?;
            list.sort_unstable();
            list.dedup();
        }

        let mut values = unit_vectors(
            program,
            Retained,
            work,
            |data| data.values.len(),
            Know::Bottom,
        )?;
        let mut evaluated_calls =
            unit_vectors(program, Retained, work, |data| data.operations.len(), false)?;
        let mut origins = unit_vectors(program, Retained, work, |data| data.values.len(), 0u8)?;
        let mut evaluated = 0u32;
        let mut refused = [0u32; 6];
        // At most one pending entry per unit. A fixed ring keeps FIFO order
        // without retaining an ever-growing history or reallocating the queue.
        let mut queue = work.collect(Scratch, 0..count)?;
        let mut head = 0;
        let mut pending = count;
        let mut queued = work.admit(|budget| budget.filled(Scratch, count, true))?;
        // Each channel adds at most FINITE_LIMIT alternatives before Top;
        // units are
        // re-queued only when one of their inputs rose.
        let mut remaining = count
            .saturating_mul(16)
            .saturating_add(program.cells.len())
            .saturating_add(64);
        while pending != 0 {
            let index = queue[head];
            head = (head + 1) % count;
            pending -= 1;
            visit(work, 1)?;
            queued[index] = false;
            if remaining == 0 {
                for value in values.iter_mut().flatten() {
                    *value = Know::Top;
                }
                for call in evaluated_calls.iter_mut().flatten() {
                    *call = false;
                }
                for origin in origins.iter_mut().flatten() {
                    *origin = 0;
                }
                refused[super::evaluate::Refusal::Limit as usize] += 1;
                return Some(Self {
                    units: values,
                    evaluated_calls,
                    origins,
                    evaluated,
                    refused,
                });
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
                work,
                pristine,
                &mut evaluated_calls[index],
                &mut origins[index],
                &mut evaluated,
                &mut refused,
            )?;
            let mut enqueue = |unit: UnitId, queue: &mut Vec<usize>| {
                if !std::mem::replace(&mut queued[unit.index()], true) {
                    queue[(head + pending) % count] = unit.index();
                    pending += 1;
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
            let joined = results[index].join(&result, work);
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
                let mut next = work.collect(Scratch, std::iter::repeat_n(Know::Bottom, slots))?;
                for edge in edges {
                    visit(work, slots as u64 + 1)?;
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
                                    known.join(&Know::Exact(StoredExact::Undefined), work)
                                } else {
                                    known
                                }
                            }
                            None => Know::Exact(StoredExact::Undefined),
                            _ => Know::Top,
                        };
                        *slot = slot.join(&argument, work);
                    }
                }
                let mut rose = false;
                for (old, new) in formals[callee.index()].iter_mut().zip(&next) {
                    let joined = old.join(new, work);
                    if joined != *old {
                        *old = joined;
                        rose = true;
                    }
                }
                work.release(Scratch, next)?;
                if rose {
                    enqueue(callee, &mut queue);
                }
            }

            // The cells this body initializes.
            for &cell in &initialized_in[index] {
                let (_, value) = settled[cell.index()].expect("a settled cell");
                let joined = cells[cell.index()].join(&values[index][value.index()], work);
                if joined != cells[cell.index()] {
                    cells[cell.index()] = joined;
                    for &reader in &readers[cell.index()] {
                        enqueue(reader, &mut queue);
                    }
                }
            }
        }
        Some(Self {
            units: values,
            evaluated_calls,
            origins,
            evaluated,
            refused,
        })
    }
}

fn visit(work: &mut Work, count: u64) -> Option<()> {
    work.admit(|budget| budget.work(WorkKind::Analysis, count))
}
fn sort_work(count: usize) -> u64 {
    (count as u64).saturating_mul(u64::from(usize::BITS - count.max(1).leading_zeros()) + 1)
}
fn unit_vectors<T: Clone>(
    program: &Program<'_>,
    class: crate::output_budget::AllocationClass,
    work: &mut Work,
    count: impl Fn(&UnitData) -> usize,
    initial: T,
) -> Option<Vec<Vec<T>>> {
    let mut rows = work.admit(|budget| budget.vector(class, program.units.len()))?;
    for unit in &program.units {
        rows.push(work.collect(
            class,
            std::iter::repeat_n(initial.clone(), count(unit.data())),
        )?);
    }
    Some(rows)
}

/// Whether a store or a reference argument writes the cell after its
/// initialization, anywhere in the program as it now stands.
fn written(graph: &CallGraph, cell: CellId) -> bool {
    let storage = graph.storage(cell);
    storage.stored || storage.referenced
}

/// Whether `value` reads a parameter that always holds a value of its type:
/// every caller is known, typed and passes an argument in its position, and
/// the supplied argument's type excludes every spelling accepted by this
/// guard. A nonnullable defaulted formal does not prove a present input: the
/// unified contract permits optional arguments and the callee runs defaults.
/// Formation's `Binding::defined` states the same fact on the target tree.
fn typed_argument(
    program: &Program<'_>,
    graph: &CallGraph,
    unit: UnitId,
    data: &UnitData,
    value: ValueId,
    nullish: bool,
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
                    && (!nullish || matches!(arguments.get(position as usize), Some(CallArgument::Value(value))
                        if !crate::check::absence::may_be_absent(&program.types[caller.values[value.index()].ty.index()])))
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
    work.temporary(|work| {
        let mut varied: Vec<(ValueId, &[StoredExact])> = Vec::new();
        let mut seen = Vec::new();
        let mut combinations = 1usize;
        for input in inputs {
            if seen.contains(&input) {
                continue;
            }
            visit(work, seen.len() as u64 + 1)?;
            work.push(Scratch, &mut seen, input)?;
            let alternatives = know.get(input.index())?.alternatives()?;
            if alternatives.len() > 1 {
                combinations = combinations.checked_mul(alternatives.len())?;
                if combinations > POINTWISE_LIMIT {
                    return None;
                }
                work.push(Scratch, &mut varied, (input, alternatives))?;
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
            result = result.join(&Know::Exact(value), work);
            if result == Know::Top {
                break;
            }
        }
        for &(value, _) in &varied {
            stored[value.index()] = know[value.index()].stored();
        }
        complete.then_some(result)
    })
}

fn operation_inputs<'a>(
    data: &'a UnitData,
    operation: &'a Operation,
) -> impl Iterator<Item = ValueId> + 'a {
    let (operands, call) = match operation.kind {
        OperationKind::Call(call) => (&[][..], Some(call)),
        _ => (data.operands(operation.operands).unwrap_or(&[]), None),
    };
    let loaded = match operation.kind {
        OperationKind::Load(place) => match data.places.get(place.index()) {
            Some(Place::Value(value)) => Some(*value),
            _ => None,
        },
        _ => None,
    };
    operands
        .iter()
        .copied()
        .chain(loaded)
        .chain(
            call.into_iter()
                .flat_map(move |call| call_inputs(data, call)),
        )
        .chain(
            operation
                .kind
                .child_regions()
                .filter_map(|region| data.regions[region.index()].result),
        )
}

fn call_inputs(data: &UnitData, call: CallId) -> impl Iterator<Item = ValueId> + '_ {
    let site = &data.calls[call.index()];
    let receiver = match site.target {
        CallTarget::Intrinsic { receiver, .. } => receiver,
        _ => None,
    };
    data.arguments(site.arguments)
        .unwrap_or(&[])
        .iter()
        .filter_map(|argument| match argument {
            CallArgument::Value(value) | CallArgument::Spread(value) => Some(*value),
            CallArgument::Reference(_) => None,
        })
        .chain(receiver)
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
    work.temporary(|work| {
    if depth == 0 {
        return Some(Vec::new());
    }
    let mut refinements = Vec::new();
    if let Some(alternatives) = know.get(condition.index())?.alternatives() {
        let filtered =
            Know::set(alternatives.iter().filter_map(|value| {
                (facts::truthy(program, value) == holds).then_some(value.clone())
            }), work);
        if filtered == Know::Bottom {
            return None;
        }
        if &filtered != &know[condition.index()] {
            work.push(Retained, &mut refinements, (condition, filtered))?;
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
            work.admit(|budget| budget.reserve_vec(Retained, &mut refinements, nested.len()))?;
            refinements.append(&mut nested);
            work.release(Retained, nested)?;
        }
        OperationKind::IsUndefined { nullish, .. } => {
            let value = *operands.first()?;
            let Some(alternatives) = know.get(value.index())?.alternatives() else {
                return Some(refinements);
            };
            let filtered = Know::set(alternatives.iter().filter_map(|candidate| {
                ((matches!(candidate, StoredExact::Undefined) || nullish && matches!(candidate, StoredExact::Null)) == holds).then_some(candidate.clone())
            }), work);
            if filtered == Know::Bottom {
                return None;
            }
            if filtered != know[value.index()] {
                work.push(Retained, &mut refinements, (value, filtered))?;
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
            let mut left_kept = Know::Bottom;
            let mut right_kept = Know::Bottom;
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
                        left_kept = left_kept.join(&Know::Exact(left_value.clone()), work);
                        right_kept = right_kept.join(&Know::Exact(right_value.clone()), work);
                    }
                }
            }
            stored[left.index()] = know[left.index()].stored();
            stored[right.index()] = know[right.index()].stored();
            let left_next = left_kept;
            let right_next = right_kept;
            if left_next == Know::Bottom || right_next == Know::Bottom {
                return None;
            }
            if left_next != know[left.index()] {
                work.push(Retained, &mut refinements, (left, left_next))?;
            }
            if right_next != know[right.index()] {
                work.push(Retained, &mut refinements, (right, right_next))?;
            }
        }
        _ => {}
    }
    Some(refinements)
    })
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
) -> Option<Know> {
    work.temporary(|work| {
    let graph = effects.graph();
    let initialization = effects.initialization();
    let mut stored = work.collect(Scratch, know.iter().map(Know::stored))?;
    let mut path_cells: Map<CellId, Know> = Map::new(Scratch);
    let mut returned: Option<Know> = None;
    let mut stack = work.collect(Scratch, std::iter::once(Step::Enter(data.entry)))?;
    while let Some(step) = stack.pop() {
        visit(work, 1)?;
        let op = match step {
            Step::Enter(region) => {
                for &op in data.regions[region.index()].operations.iter().rev() {
                    work.push(Scratch, &mut stack, Step::Prepare(op))?;
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
                                work.push(Scratch, &mut children, (yes, refinements))?;
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
                                    work.push(Scratch, &mut children, (no, refinements))?;
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
                                work.push(Scratch, &mut children, (yes, refinements))?;
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
                                work.push(Scratch, &mut children, (no, refinements))?;
                            }
                        }
                    }
                    _ => for region in operation.kind.child_regions() {
                        work.push(Scratch, &mut children, (region, Vec::new()))?;
                    },
                }
                work.push(Scratch, &mut stack, Step::Eval(op))?;
                for (region, refinements) in children.drain(..).rev() {
                    work.push(Scratch, &mut stack, Step::Child { region, refinements })?;
                }
                work.release(Scratch, children)?;
                continue;
            }
            Step::Child { region, mut refinements } => {
                let mut previous = work.admit(|budget| budget.vector(Scratch, refinements.len()))?;
                let mut previous_cells = work.admit(|budget| budget.vector(Scratch, refinements.len()))?;
                for (value, refinement) in refinements.drain(..) {
                    visit(work, (previous.len() + previous_cells.len()) as u64 + 1)?;
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
                                let old = work.admit(|budget| path_cells.insert(cell, refinement.clone(), budget))?;
                                previous_cells.push((cell, old));
                            }
                        }
                    }
                    know[value.index()] = refinement;
                    stored[value.index()] = know[value.index()].stored();
                }
                work.release(Retained, refinements)?;
                work.push(Scratch, &mut stack, Step::Restore { values: previous, cells: previous_cells })?;
                work.push(Scratch, &mut stack, Step::Enter(region))?;
                continue;
            }
            Step::Restore { mut values, mut cells } => {
                for (value, knowledge) in values.drain(..) {
                    know[value.index()] = knowledge;
                    stored[value.index()] = know[value.index()].stored();
                }
                for (cell, previous) in cells.drain(..) {
                    match previous {
                        Some(known) => {
                            work.admit(|budget| path_cells.insert(cell, known, budget))?;
                        }
                        None => {
                            work.admit(|budget| path_cells.remove(&cell, budget))?;
                        }
                    }
                }
                work.release(Scratch, values)?;
                work.release(Scratch, cells)?;
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
            returned = Some(returned.map_or(value.clone(), |known| known.join(&value, work)));
            continue;
        }
        let Some(result) = operation.result else {
            continue;
        };
        let mut origin = operation_inputs(data, operation).fold(0, |bits, input| {
            bits | origins[input.index()]
                | if matches!(know[input.index()], Know::Finite(_)) {
                    FROM_SET
                } else {
                    0
                }
        });
        if matches!(operation.kind, OperationKind::Load(place)
            if matches!(data.places.get(place.index()), Some(Place::Cell(cell)) if path_cells.get(cell).is_some()))
        {
            origin |= FROM_PATH;
        }
        let next = match &operation.kind {
            OperationKind::Load(place) => match data.places.get(place.index()) {
                Some(&Place::Value(value)) => know[value.index()].clone(),
                Some(&Place::Cell(cell)) => {
                    let storage = &program.cells[cell.index()];
                    match storage.binding {
                        _ if path_cells.get(&cell).is_some()
                            && initialization.initialized(program, unit, op, cell) =>
                        {
                            path_cells.get(&cell).unwrap().clone()
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
            OperationKind::IsUndefined { nullish, .. }
                if operands
                    .first()
                    .is_some_and(|value| typed_argument(program, graph, unit, data, *value, *nullish)) =>
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
        let joined = know[result.index()].join(&next, work);
        if joined != know[result.index()] {
            stored[result.index()] = joined.stored();
            know[result.index()] = joined;
        }
        origins[result.index()] |= origin;
    }
    Some(returned.unwrap_or(Know::Exact(StoredExact::Undefined)))
    })
}
