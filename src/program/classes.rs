//! Runtime primitive classes shared by program rules and target formation.
//!
//! Checked annotations do not prove a raw host value's class. These facts join
//! every producer of closed storage, complete-call-set arguments and results.
//! Starting at unknown and only narrowing makes every bounded transfer sound,
//! including recursive calls and loop-carried stores. Classes describe normal
//! results; initialization and evaluation effects require separate proofs.

use super::analysis_storage as storage;
use super::call_graph::{CallGraph, Callee, EdgeKind, Seal};
use super::views::Deps;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Class(u8);

impl Class {
    pub(crate) const NUMBER: Self = Self(1);
    pub(crate) const BOOLEAN: Self = Self(2);
    pub(crate) const STRING: Self = Self(4);
    const NULL: Self = Self(8);
    const UNDEFINED: Self = Self(16);
    const EMPTY: Self = Self(0);
    const UNKNOWN: Self = Self(63);

    fn join(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    fn meet(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
    pub(crate) fn primitive(self) -> bool {
        self.0 != 0 && self.0 & 32 == 0
    }
}

#[derive(Debug)]
pub(crate) struct ProgramClasses {
    deps: Deps,
    units: Vec<Vec<Class>>,
    cells: Vec<Class>,
}

impl ProgramClasses {
    pub(crate) fn deps(&self) -> &Deps {
        &self.deps
    }
    pub(crate) fn value(&self, unit: UnitId, value: ValueId) -> Class {
        self.units[unit.index()][value.index()]
    }
    pub(crate) fn cell(&self, cell: CellId) -> Class {
        self.cells[cell.index()]
    }

    pub(super) fn build(program: &Program<'_>, graph: &CallGraph) -> Self {
        Self::build_in(program, graph, &mut AllocationBudget::new(None))
            .expect("inspection classes")
    }
    pub(super) fn build_in(
        program: &Program<'_>,
        graph: &CallGraph,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.retained_phase(|budget| {
            let mut units = budget.vector(Retained, program.units.len())?;
            for unit in &program.units {
                units.push(budget.filled(Retained, unit.data().values.len(), Class::UNKNOWN)?);
                budget.work(WorkKind::Analysis, unit.data().calls.len() as u64)?;
            }
            let mut cells = budget.filled(Retained, program.cells.len(), Class::UNKNOWN)?;
            let mut results = budget.filled(Scratch, program.units.len(), Class::UNKNOWN)?;
            let direct_eval = program.units.iter().any(|unit| {
                unit.data().calls.iter().any(|call| {
                    matches!(
                        call.target,
                        CallTarget::Value {
                            invocation: Invocation::DirectEval,
                            ..
                        }
                    )
                })
            });
            let free = super::defaults::arguments_free_all_in(program, budget)?;
            let closed = storage::collect(
                program.cells.iter().enumerate().map(|(index, cell)| {
                    let id = CellId::from_index(index).expect("a cell index");
                    !direct_eval
                        && !graph.storage(id).referenced
                        && !program.is_reference_parameter(id)
                        && matches!(cell.binding, CellBinding::Local | CellBinding::Parameter(_))
                        && free[cell.owner.index()]
                        && (graph.seal() == Seal::Module
                            || program
                                .unit(cell.owner)
                                .is_none_or(|unit| unit.kind != UnitKind::ModuleInitialization))
                }),
                Scratch,
                budget,
            )?;
            storage::release(free, Retained, budget)?;
            propagate(
                program,
                graph,
                &closed,
                &mut units,
                &mut cells,
                &mut results,
                budget,
            )?;
            Ok(Self {
                deps: Deps::of_program_in(program, budget)?,
                units,
                cells,
            })
        })
    }
}

#[derive(Clone, Copy)]
enum Input {
    Value(UnitId, ValueId),
    Constant(Class),
}
impl Input {
    fn class(self, units: &[Vec<Class>]) -> Class {
        match self {
            Self::Value(unit, value) => units[unit.index()][value.index()],
            Self::Constant(class) => class,
        }
    }
}

/// Dense source identities become one bounded worklist: values, then cells,
/// then returned classes. Edges point from evidence to its consumers. All
/// producers of a cell are joined before it narrows, including every complete
/// caller and every write. A changed result also revisits its call sites.
struct Flow {
    offsets: Vec<usize>,
    owners: Vec<UnitId>,
    values: usize,
    cells: usize,
    inputs: Vec<Vec<Input>>,
}
impl Flow {
    fn value(&self, unit: UnitId, value: ValueId) -> usize {
        self.offsets[unit.index()] + value.index()
    }
    fn subject(&self, node: usize) -> (UnitId, ValueId) {
        let unit = self.owners[node];
        (
            unit,
            ValueId::from_index(node - self.offsets[unit.index()]).unwrap(),
        )
    }
    fn build(
        program: &Program<'_>,
        graph: &CallGraph,
        closed: &[bool],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut offsets = budget.vector(Scratch, program.units.len() + 1)?;
        offsets.push(0usize);
        for unit in &program.units {
            let next = offsets
                .last()
                .unwrap()
                .checked_add(unit.data().values.len())
                .ok_or(AllocationError::Capacity)?;
            offsets.push(next);
        }
        let values = *offsets.last().unwrap();
        let mut owners = budget.vector(Scratch, values)?;
        for unit in &program.units {
            budget.work(WorkKind::Analysis, unit.data().values.len() as u64)?;
            owners.resize(owners.len() + unit.data().values.len(), unit.id());
        }
        let cells = program.cells.len();
        let count = cells
            .checked_add(program.units.len())
            .ok_or(AllocationError::Capacity)?;
        let mut inputs = storage::collect((0..count).map(|_| Vec::new()), Scratch, budget)?;
        for frozen in &program.units {
            let unit = frozen.id();
            let data = frozen.data();
            for operation in &data.operations {
                budget.work(WorkKind::Analysis, 1)?;
                let first = data
                    .operands(operation.operands)
                    .and_then(|args| args.first())
                    .map_or(Input::Constant(Class::UNKNOWN), |&value| {
                        Input::Value(unit, value)
                    });
                let write = match operation.kind {
                    OperationKind::Initialize(cell) => Some((cell, first)),
                    OperationKind::Store(place) => match data.places[place.index()] {
                        Place::Cell(cell) => Some((cell, first)),
                        _ => None,
                    },
                    OperationKind::ForIn { key, .. } => Some((key, Input::Constant(Class::STRING))),
                    OperationKind::ForOf { item, .. } => {
                        Some((item, Input::Constant(Class::UNKNOWN)))
                    }
                    OperationKind::Try {
                        catch: Some((Some(cell), _)),
                        ..
                    } => Some((cell, Input::Constant(Class::UNKNOWN))),
                    OperationKind::Return => {
                        budget.push(Scratch, &mut inputs[cells + unit.index()], first)?;
                        None
                    }
                    _ => None,
                };
                if let Some((cell, value)) = write.filter(|(cell, _)| closed[cell.index()]) {
                    budget.push(Scratch, &mut inputs[cell.index()], value)?;
                }
            }
            for (position, &parameter) in data.parameters.iter().enumerate() {
                if !closed[parameter.index()] {
                    continue;
                }
                let into = &mut inputs[parameter.index()];
                match graph.complete_callers(unit) {
                    Some(edges) if !edges.is_empty() => {
                        for edge in edges {
                            budget.work(WorkKind::Analysis, 1)?;
                            let caller = program.unit(edge.caller).unwrap();
                            let site = &caller.calls[edge.call.index()];
                            let arguments = caller.arguments(site.arguments).unwrap_or(&[]);
                            let passed = if edge.kind != EdgeKind::Call
                                || site.contract.instantiation.is_some()
                            {
                                Input::Constant(Class::UNKNOWN)
                            } else {
                                match arguments.get(position) {
                                    Some(CallArgument::Value(value)) => {
                                        Input::Value(edge.caller, *value)
                                    }
                                    None => Input::Constant(Class::UNDEFINED),
                                    _ => Input::Constant(Class::UNKNOWN),
                                }
                            };
                            budget.push(Scratch, into, passed)?;
                            if position
                                >= arguments.len().saturating_sub(site.omit_trailing as usize)
                            {
                                budget.push(Scratch, into, Input::Constant(Class::UNDEFINED))?;
                            }
                        }
                    }
                    _ => budget.push(Scratch, into, Input::Constant(Class::UNKNOWN))?,
                }
            }
        }
        Ok(Self {
            offsets,
            owners,
            values,
            cells,
            inputs,
        })
    }
    fn result(&self, unit: UnitId) -> usize {
        self.values + self.cells + unit.index()
    }
    fn dependencies(
        &self,
        program: &Program<'_>,
        graph: &CallGraph,
        node: usize,
        mut visit: impl FnMut(usize) -> Result<(), AllocationError>,
    ) -> Result<(), AllocationError> {
        if node < self.values {
            let (unit, value) = self.subject(node);
            let data = program.unit(unit).unwrap();
            let operation = &data.operations[data.values[value.index()].definition.index()];
            for &value in data.operands(operation.operands).unwrap_or(&[]) {
                visit(self.value(unit, value))?;
            }
            match operation.kind {
                OperationKind::Load(place) => match data.places[place.index()] {
                    Place::Cell(cell) => visit(self.values + cell.index())?,
                    Place::Value(value) => visit(self.value(unit, value))?,
                    _ => {}
                },
                OperationKind::Call(call) => {
                    if let Callee::Unit(callee) = graph.callee(unit, call) {
                        visit(self.result(callee))?;
                    }
                }
                _ => {}
            }
            for region in operation.kind.child_regions() {
                if let Some(value) = data.regions[region.index()].result {
                    visit(self.value(unit, value))?;
                }
            }
        } else {
            for &input in &self.inputs[node - self.values] {
                if let Input::Value(unit, value) = input {
                    visit(self.value(unit, value))?;
                }
            }
        }
        Ok(())
    }
}

fn propagate(
    program: &Program<'_>,
    graph: &CallGraph,
    closed: &[bool],
    units: &mut [Vec<Class>],
    cells: &mut [Class],
    results: &mut [Class],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let mut budget = budget.scope();
    let flow = Flow::build(program, graph, closed, &mut budget)?;
    let count = flow
        .values
        .checked_add(flow.inputs.len())
        .ok_or(AllocationError::Capacity)?;
    // Compact reverse adjacency, admitted exactly after counting edges. No
    // per-node hash maps, reallocating queues, or dependency guesses.
    let mut starts = budget.filled(
        Scratch,
        count.checked_add(1).ok_or(AllocationError::Capacity)?,
        0usize,
    )?;
    for node in 0..count {
        budget.work(WorkKind::Analysis, 1)?;
        flow.dependencies(program, graph, node, |dependency| {
            budget.work(WorkKind::Analysis, 1)?;
            starts[dependency + 1] = starts[dependency + 1]
                .checked_add(1)
                .ok_or(AllocationError::Capacity)?;
            Ok(())
        })?;
    }
    for node in 1..starts.len() {
        starts[node] = starts[node]
            .checked_add(starts[node - 1])
            .ok_or(AllocationError::Capacity)?;
    }
    let mut edges = budget.filled(Scratch, starts[count], 0usize)?;
    let mut cursors = budget.copy_slice(Scratch, &starts[..count])?;
    for node in 0..count {
        flow.dependencies(program, graph, node, |dependency| {
            budget.work(WorkKind::Analysis, 1)?;
            edges[cursors[dependency]] = node;
            cursors[dependency] += 1;
            Ok(())
        })?;
    }
    storage::release(cursors, Scratch, &mut budget)?;
    let mut queue = storage::collect(0..count, Scratch, &mut budget)?;
    let mut queued = budget.filled(Scratch, count, true)?;
    let mut visits = budget.filled(Scratch, count, 0u8)?;
    let mut head = 0;
    let mut pending = count;
    while pending != 0 {
        let node = queue[head];
        head = (head + 1) % count;
        pending -= 1;
        queued[node] = false;
        visits[node] += 1;
        budget.work(WorkKind::Analysis, 1)?;
        let changed = if node < flow.values {
            let (unit, value) = flow.subject(node);
            let data = program.unit(unit).unwrap();
            let operation = &data.operations[data.values[value.index()].definition.index()];
            budget.work(
                WorkKind::Analysis,
                data.operands(operation.operands)
                    .map_or(0, |args| args.len()) as u64
                    + 1,
            )?;
            let class = transfer(
                data,
                operation,
                &units[unit.index()],
                cells,
                |call| match graph.callee(unit, call) {
                    Callee::Unit(callee)
                        if program.unit(callee).is_some_and(|body| {
                            body.suspension == Suspension::None && body.constructor_of.is_none()
                        }) =>
                    {
                        results[callee.index()]
                    }
                    Callee::Intrinsic(crate::primitive::ResolvedIntrinsic::Method(
                        crate::primitive::Intrinsic::FloatToInt,
                    )) => Class::NUMBER,
                    _ => Class::UNKNOWN,
                },
            );
            let old = &mut units[unit.index()][value.index()];
            let next = old.meet(class);
            let changed = next != *old;
            *old = next;
            changed
        } else {
            let index = node - flow.values;
            let input = &flow.inputs[index];
            budget.work(WorkKind::Analysis, input.len() as u64)?;
            let result = index >= flow.cells;
            let seed = if result {
                Class::UNDEFINED
            } else {
                Class::EMPTY
            };
            let joined = input
                .iter()
                .fold(seed, |class, input| class.join(input.class(units)));
            if !result && (!closed[index] || joined == Class::EMPTY) {
                false
            } else {
                let old = if result {
                    &mut results[index - flow.cells]
                } else {
                    &mut cells[index]
                };
                let next = old.meet(joined);
                let changed = next != *old;
                *old = next;
                changed
            }
        };
        if changed {
            for &consumer in &edges[starts[node]..starts[node + 1]] {
                budget.work(WorkKind::Analysis, 1)?;
                // As in the old sixteen rounds, exhausting this analysis cap
                // leaves a wider sound class. It cannot invent a primitive.
                if !queued[consumer] && visits[consumer] < 16 {
                    queue[(head + pending) % count] = consumer;
                    pending += 1;
                    queued[consumer] = true;
                }
            }
        }
    }
    Ok(())
}

fn transfer(
    data: &UnitData,
    operation: &Operation,
    values: &[Class],
    cells: &[Class],
    call: impl FnOnce(CallId) -> Class,
) -> Class {
    use crate::ast::{BinaryOp as B, UnaryOp as U};
    let operands = data.operands(operation.operands).unwrap_or(&[]);
    let first = || {
        operands
            .first()
            .map_or(Class::UNKNOWN, |value| values[value.index()])
    };
    let all = || {
        !operands.is_empty()
            && operands
                .iter()
                .all(|value| values[value.index()].primitive())
    };
    let join = || {
        operands.iter().fold(Class::EMPTY, |class, value| {
            class.join(values[value.index()])
        })
    };
    let region = |region: RegionId| {
        data.regions[region.index()]
            .result
            .map_or(Class::UNKNOWN, |value| values[value.index()])
    };
    match operation.kind {
        OperationKind::Constant(ref constant) => match constant {
            Constant::Integer(_) | Constant::Number(_) => Class::NUMBER,
            Constant::Boolean(_) => Class::BOOLEAN,
            Constant::String(_) => Class::STRING,
            Constant::Null => Class::NULL,
            Constant::Undefined => Class::UNDEFINED,
        },
        OperationKind::CopyValue => first(),
        OperationKind::Load(place) => match data.places[place.index()] {
            Place::Cell(cell) => cells[cell.index()],
            Place::Value(value) => values[value.index()],
            _ => Class::UNKNOWN,
        },
        OperationKind::IntBinary(_)
        | OperationKind::Unary {
            integer: true,
            op: U::Neg,
        } => Class::NUMBER,
        OperationKind::Unary { op: U::Not, .. }
        | OperationKind::IsUndefined { .. }
        | OperationKind::TypeTest(_)
        | OperationKind::ClosedClassTest(_) => Class::BOOLEAN,
        OperationKind::Unary { .. } if all() => Class::NUMBER,
        OperationKind::Binary(
            B::Eq | B::NotEq | B::Less | B::LessEq | B::Greater | B::GreaterEq,
        ) => Class::BOOLEAN,
        OperationKind::Binary(B::And | B::Or | B::Nullish) => join(),
        OperationKind::Binary(B::Add) if all() => Class::NUMBER.join(Class::STRING),
        OperationKind::Binary(_) if all() => Class::NUMBER,
        OperationKind::Template => Class::STRING,
        OperationKind::Intrinsic(crate::primitive::ResolvedIntrinsic::Property(
            crate::primitive::Intrinsic::StringLength,
        )) if first() == Class::STRING => Class::NUMBER,
        OperationKind::ShortCircuit { right, .. } => first().join(region(right)),
        OperationKind::Select { yes, no } => region(yes).join(region(no)),
        OperationKind::Call(id) => call(id),
        _ => Class::UNKNOWN,
    }
}

#[cfg(test)]
#[path = "classes_tests.rs"]
mod tests;
