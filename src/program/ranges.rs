//! Value ranges (plan M6.4b; architecture §7, the int32-range tier of the
//! value lattice): what each Number value of the program can be, as the
//! `NumberFacts` interval domain (`scalar_transfer.rs`) formation already
//! reasons in, computed once per program and read by every formation.
//!
//! **Flow.** Each unit is solved by the region-structured solver (M5.5,
//! `dataflow.rs`) over a state that holds, per cell only its owner reads and
//! writes (no other unit captures it, no call takes it by reference), the
//! facts of its current value:
//! * an operation's result follows the language's Number arithmetic, as
//!   formation's transfers do (an `int` operation's result after its ToInt32;
//!   a string's or array's length at most 2^30);
//! * a load of an owned cell reads the state, and an initialization or a
//!   store writes it;
//! * a comparison of an owned cell's load with a bound narrows the cell on
//!   each side of the branch it decides (`i < n` entering a loop body bounds
//!   `i` by `n`'s maximum less one), which is how a counting loop's counter
//!   stays within its bound;
//! * a loop head that grows after its first iteration widens to the value
//!   type's full range, so every loop settles.
//!
//! Any other value is what its type guarantees (R1): an `int` is an int32
//! Number, a `float` a Number. **Calls.** A call of a known body reads that
//! body's result range, the join of its returns; bodies are solved callees
//! first, and a recursive body's result is its type's. Parameters are their
//! type's (joining arguments over complete call sets is the next step).
//!
//! The facts describe normal results only; they never authorize removing,
//! moving or duplicating an evaluation. Formation meets them with its own
//! (each is sound, so their intersection is), and an `int` operation whose
//! result provably stays in int32 prints without its `|0`.
//!
//! Prior art: value-range propagation (Patterson, PLDI 1995); LLVM's
//! `LazyValueInfo` and `CorrelatedValuePropagation`, which narrow a value by
//! the branch that tests it; V8's Turbofan typer, whose range types widen at
//! loop phis and decide the same `|0` question at run time. Closure and
//! Terser keep no ranges: a `|0` stays wherever the source wrote one.

use super::call_graph::{EdgeKind, Seal};
use super::dataflow::{self, Forward};
use super::effects::ProgramEffects;
use super::views::Deps;
use super::*;
use crate::primitive::ResolvedIntrinsic;
use crate::scalar_transfer::NumberFacts;
use std::cell::RefCell;

/// The facts of every value of every unit.
#[derive(Debug)]
pub struct ProgramRanges {
    deps: Deps,
    units: Vec<Vec<NumberFacts>>,
}

impl ProgramRanges {
    /// What the program proves about `value` of `unit`.
    pub(crate) fn number(&self, unit: UnitId, value: ValueId) -> NumberFacts {
        self.units
            .get(unit.index())
            .and_then(|values| values.get(value.index()))
            .copied()
            .unwrap_or(NumberFacts::UNKNOWN)
    }

    pub fn deps(&self) -> &Deps {
        &self.deps
    }

    pub(super) fn build(program: &Program<'_>, effects: &ProgramEffects, _seal: Seal) -> Self {
        let graph = effects.graph();
        let mut units: Vec<Vec<NumberFacts>> = program
            .units
            .iter()
            .map(|unit| by_type_all(program, unit.data()))
            .collect();
        let mut results: Vec<Option<NumberFacts>> = vec![None; program.units.len()];
        // The one callee of each call operation the graph resolves.
        let mut callees: Vec<Vec<Option<UnitId>>> = program
            .units
            .iter()
            .map(|unit| vec![None; unit.data().operations.len()])
            .collect();
        for frozen in &program.units {
            let unit = frozen.id();
            let mut seen = vec![0u8; frozen.data().operations.len()];
            for edge in graph.calls_from(unit) {
                if edge.kind != EdgeKind::Call {
                    continue;
                }
                let slot = &mut seen[edge.operation.index()];
                *slot = slot.saturating_add(1);
                callees[unit.index()][edge.operation.index()] = Some(edge.callee);
            }
            for (index, count) in seen.iter().enumerate() {
                if *count > 1 {
                    callees[unit.index()][index] = None;
                }
            }
        }
        // Rounds: each solves every unit callees first. A parameter of a
        // body whose complete call set is known reads the join of what its
        // callers passed in the round before (the first round's parameters
        // are their type's). Every round is sound, and each is at least as
        // narrow as the one before, so stopping at any round is.
        let mut formals: Vec<Option<Vec<NumberFacts>>> = vec![None; program.units.len()];
        for _ in 0..ROUNDS {
            solve_round(program, graph, &callees, &formals, &mut units, &mut results);
            let next = passed(program, graph, &units);
            if next == formals {
                break;
            }
            formals = next;
        }
        Self {
            deps: Deps::of_program(program),
            units,
        }
    }
}

/// Rounds of parameter joining (M6.4b): a call chain's ranges reach two
/// calls deeper per round beyond the first.
const ROUNDS: usize = 3;

/// Per unit: the join, per parameter, of what every call of its complete
/// call set passes, when every call passes each parameter as a value.
fn passed(
    program: &Program<'_>,
    graph: &super::call_graph::CallGraph,
    units: &[Vec<NumberFacts>],
) -> Vec<Option<Vec<NumberFacts>>> {
    let mut formals = vec![None; program.units.len()];
    for frozen in &program.units {
        let unit = frozen.id();
        let Some(edges) = graph.complete_callers(unit) else {
            continue;
        };
        if edges.is_empty() {
            continue;
        }
        let parameters = frozen.data().parameters.len();
        let mut joined: Vec<Option<NumberFacts>> = vec![None; parameters];
        let usable = edges.iter().all(|edge| {
            let caller = program.units[edge.caller.index()].data();
            let Some(arguments) = caller
                .calls
                .get(edge.call.index())
                .and_then(|site| caller.arguments(site.arguments))
            else {
                return false;
            };
            if arguments.len() != parameters {
                return false;
            }
            arguments.iter().zip(&mut joined).all(|(argument, slot)| {
                let CallArgument::Value(value) = *argument else {
                    return false;
                };
                let facts = units[edge.caller.index()]
                    .get(value.index())
                    .copied()
                    .unwrap_or(NumberFacts::UNKNOWN);
                *slot = Some(match *slot {
                    Some(joined) => joined.join(facts),
                    None => facts,
                });
                true
            })
        });
        if usable {
            formals[unit.index()] = Some(
                joined
                    .into_iter()
                    .map(|facts| facts.unwrap_or(NumberFacts::UNKNOWN))
                    .collect(),
            );
        }
    }
    formals
}

/// One round: every unit solved callees first, with `formals` for the
/// parameters where known.
fn solve_round(
    program: &Program<'_>,
    graph: &super::call_graph::CallGraph,
    callees: &[Vec<Option<UnitId>>],
    formals: &[Option<Vec<NumberFacts>>],
    units: &mut [Vec<NumberFacts>],
    results: &mut [Option<NumberFacts>],
) {
    for component in graph.components() {
        for &unit in component {
            let data = program.units[unit.index()].data();
            let recursive = graph.recursive(unit);
            let (owned, integral) = owned_cells(program, graph, unit);
            let analysis = Ranges {
                program,
                data,
                owned,
                integral,
                values: RefCell::new(by_type_all(program, data)),
                callees: &callees[unit.index()],
                results: &*results,
            };
            let mut entry = analysis.unreachable();
            for (position, &cell) in data.parameters.iter().enumerate() {
                if let Some(ordinal) = analysis.ordinal(cell) {
                    let typed = by_type(program, program.cells[cell.index()].ty);
                    let passed = formals[unit.index()]
                        .as_ref()
                        .and_then(|formals| formals.get(position))
                        .map_or(typed, |facts| facts.meet(typed));
                    entry[ordinal] = Some(passed);
                }
            }
            if dataflow::solve(data, &analysis, entry, |_| Ok::<(), ()>(())).is_err() {
                continue;
            }
            let values = analysis.values.into_inner();
            // A body's result: the join of the values it returns (F4
            // removed the returns no path reaches). A recursive body's
            // is its type's.
            if !recursive {
                let mut result: Option<NumberFacts> = None;
                for operation in &data.operations {
                    if !matches!(operation.kind, OperationKind::Return) {
                        continue;
                    }
                    let returned = data
                        .operands(operation.operands)
                        .and_then(|operands| operands.first().copied())
                        .map_or(NumberFacts::UNKNOWN, |value| values[value.index()]);
                    result = Some(match result {
                        Some(result) => result.join(returned),
                        None => returned,
                    });
                }
                results[unit.index()] = result;
            }
            units[unit.index()] = values;
        }
    }
}

/// A value's facts from its type alone (R1).
fn by_type(program: &Program<'_>, ty: TypeId) -> NumberFacts {
    match program.ty(ty) {
        Some(Type::Int) => NumberFacts::I32,
        Some(Type::Float) => NumberFacts::NUMBER,
        _ => NumberFacts::UNKNOWN,
    }
}

fn by_type_all(program: &Program<'_>, data: &UnitData) -> Vec<NumberFacts> {
    data.values
        .iter()
        .map(|value| by_type(program, value.ty))
        .collect()
}

/// Per cell of the program: its ordinal in `unit`'s state, when it holds a
/// Number and only `unit` reads and writes it; and per ordinal, whether the
/// cell is an `int` (its widening threshold is int32's range).
fn owned_cells(
    program: &Program<'_>,
    graph: &super::call_graph::CallGraph,
    unit: UnitId,
) -> (Vec<Option<usize>>, Vec<bool>) {
    let mut ordinals = vec![None; program.cells.len()];
    let mut integral = Vec::new();
    for (index, cell) in program.cells.iter().enumerate() {
        if cell.owner != unit {
            continue;
        }
        let id = CellId::from_index(index).expect("a cell index");
        let storage = graph.storage(id);
        if storage.referenced || storage.shared || program.is_reference_parameter(id) {
            continue;
        }
        let integer = match program.ty(cell.ty) {
            Some(Type::Int) => true,
            Some(Type::Float) => false,
            _ => continue,
        };
        ordinals[index] = Some(integral.len());
        integral.push(integer);
    }
    (ordinals, integral)
}

/// The unit's analysis: per owned cell, the facts of its current value
/// (`None` where no path has written it yet).
struct Ranges<'a, 'p> {
    program: &'a Program<'p>,
    data: &'a UnitData,
    owned: Vec<Option<usize>>,
    /// Per ordinal: the cell is an `int`.
    integral: Vec<bool>,
    /// Every value's facts, as the solver last computed them.
    values: RefCell<Vec<NumberFacts>>,
    callees: &'a [Option<UnitId>],
    results: &'a [Option<NumberFacts>],
}

impl Ranges<'_, '_> {
    fn ordinal(&self, cell: CellId) -> Option<usize> {
        self.owned.get(cell.index()).copied().flatten()
    }

    fn value(&self, value: ValueId) -> NumberFacts {
        self.values.borrow()[value.index()]
    }

    /// The owned cell a value was just loaded from, when nothing between the
    /// load and `until` (in the load's region) can have written it.
    fn loaded_cell(&self, value: ValueId, until: Option<OpId>) -> Option<CellId> {
        let data = self.data;
        let definition = data.values[value.index()].definition;
        let OperationKind::Load(place) = data.operations[definition.index()].kind else {
            return None;
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return None;
        };
        self.ordinal(cell)?;
        let region = data.operations[definition.index()].region;
        let operations = &data.regions[region.index()].operations;
        let start = operations.iter().position(|op| *op == definition)?;
        let end = match until {
            Some(until) => operations.iter().position(|op| *op == until)?,
            None => operations.len(),
        };
        let writes = operations[start + 1..end]
            .iter()
            .any(|&op| self.writes(op, cell));
        (!writes).then_some(cell)
    }

    /// Whether `operation`, or anything in the regions it holds, writes
    /// `cell`. A call cannot: nothing outside the owner reaches an owned
    /// cell.
    fn writes(&self, operation: OpId, cell: CellId) -> bool {
        let data = self.data;
        let kind = &data.operations[operation.index()].kind;
        let direct = match *kind {
            OperationKind::Store(place) => {
                matches!(data.places[place.index()], Place::Cell(written) if written == cell)
            }
            OperationKind::Initialize(written) | OperationKind::Declare(written) => written == cell,
            OperationKind::ForIn { key: written, .. }
            | OperationKind::ForOf { item: written, .. } => written == cell,
            _ => false,
        };
        direct
            || kind.child_regions().any(|region| {
                data.regions[region.index()]
                    .operations
                    .iter()
                    .any(|&op| self.writes(op, cell))
            })
    }

    /// Narrow the state by the outcome of the comparison `condition`. When
    /// `a && b` holds both hold, and when `a || b` fails both fail: each
    /// side narrows in turn (the right side's comparison is read up to its
    /// own region's end).
    fn narrow(
        &self,
        condition: ValueId,
        until: Option<OpId>,
        holds: bool,
        state: &mut [Option<NumberFacts>],
    ) {
        let data = self.data;
        let definition = &data.operations[data.values[condition.index()].definition.index()];
        if let OperationKind::ShortCircuit { kind, right } = definition.kind {
            let both = match kind {
                ShortCircuit::BooleanAnd | ShortCircuit::JavaScriptAnd => holds,
                ShortCircuit::BooleanOr | ShortCircuit::JavaScriptOr => !holds,
                ShortCircuit::Nullish => false,
            };
            if both {
                if let Some(&left) = data.operands(definition.operands).and_then(|o| o.first()) {
                    self.narrow(left, until, holds, state);
                }
                if let Some(result) = data.regions[right.index()].result {
                    self.narrow(result, None, holds, state);
                }
            }
            return;
        }
        let OperationKind::Binary(op) = definition.kind else {
            return;
        };
        let Some(&[left, right]) = data.operands(definition.operands) else {
            return;
        };
        // `left op right` holds, or its negation does.
        let op = match (op, holds) {
            (BinaryOp::Less, true) | (BinaryOp::GreaterEq, false) => BinaryOp::Less,
            (BinaryOp::LessEq, true) | (BinaryOp::Greater, false) => BinaryOp::LessEq,
            (BinaryOp::Greater, true) | (BinaryOp::LessEq, false) => BinaryOp::Greater,
            (BinaryOp::GreaterEq, true) | (BinaryOp::Less, false) => BinaryOp::GreaterEq,
            _ => return,
        };
        let bounds = |value: ValueId| self.value(value).integer_bounds();
        let mut apply = |cell: CellId, facts: &dyn Fn(NumberFacts) -> NumberFacts| {
            if let Some(ordinal) = self.ordinal(cell) {
                if let Some(current) = state[ordinal] {
                    state[ordinal] = Some(facts(current));
                }
            }
        };
        if let (Some(cell), Some((minimum, maximum))) =
            (self.loaded_cell(left, until), bounds(right))
        {
            match op {
                BinaryOp::Less => apply(cell, &|facts| facts.at_most(maximum.saturating_sub(1))),
                BinaryOp::LessEq => apply(cell, &|facts| facts.at_most(maximum)),
                BinaryOp::Greater => {
                    apply(cell, &|facts| facts.at_least(minimum.saturating_add(1)))
                }
                BinaryOp::GreaterEq => apply(cell, &|facts| facts.at_least(minimum)),
                _ => {}
            }
        }
        if let (Some(cell), Some((minimum, maximum))) =
            (self.loaded_cell(right, until), bounds(left))
        {
            // `left op cell`: the mirrored bound on the cell.
            match op {
                BinaryOp::Less => apply(cell, &|facts| facts.at_least(minimum.saturating_add(1))),
                BinaryOp::LessEq => apply(cell, &|facts| facts.at_least(minimum)),
                BinaryOp::Greater => apply(cell, &|facts| facts.at_most(maximum.saturating_sub(1))),
                BinaryOp::GreaterEq => apply(cell, &|facts| facts.at_most(maximum)),
                _ => {}
            }
        }
    }

    /// The facts of `operation`'s result, from its operands'.
    fn result(&self, operation: OpId, state: &[Option<NumberFacts>]) -> Option<NumberFacts> {
        let program = self.program;
        let data = self.data;
        let op = &data.operations[operation.index()];
        let result = op.result?;
        let operands = data.operands(op.operands).unwrap_or(&[]);
        let operand = |index: usize| {
            operands
                .get(index)
                .map_or(NumberFacts::UNKNOWN, |&value| self.value(value))
        };
        let by_type = by_type(program, data.values[result.index()].ty);
        Some(
            match op.kind {
                OperationKind::Constant(Constant::Integer(value)) => {
                    NumberFacts::literal(f64::from(value))
                }
                OperationKind::Constant(Constant::Number(bits)) => {
                    NumberFacts::literal(f64::from_bits(bits))
                }
                OperationKind::CopyValue => operand(0),
                OperationKind::IntBinary(int) => {
                    operand(0).binary(int.javascript(), operand(1)).to_int32()
                }
                OperationKind::Binary(binary) => match javascript_binary(binary) {
                    Some(binary) => operand(0).binary(binary, operand(1)),
                    None => by_type,
                },
                OperationKind::Unary { op, integer } => {
                    let raw = operand(0).unary(match op {
                        UnaryOp::Neg => crate::js::Unary::Negate,
                        UnaryOp::Not => crate::js::Unary::Not,
                    });
                    if integer && op == UnaryOp::Neg {
                        raw.to_int32()
                    } else {
                        raw
                    }
                }
                OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                    crate::primitive::Intrinsic::StringLength
                    | crate::primitive::Intrinsic::ArrayLength,
                )) => NumberFacts::integer_range(0, 1 << 30, false).unwrap_or(NumberFacts::UNKNOWN),
                OperationKind::Select { yes, no } => {
                    let side = |region: RegionId| {
                        data.regions[region.index()]
                            .result
                            .map_or(NumberFacts::UNKNOWN, |value| self.value(value))
                    };
                    side(yes).join(side(no))
                }
                OperationKind::Load(place) => match data.places[place.index()] {
                    Place::Cell(cell) => match self.ordinal(cell) {
                        Some(ordinal) => state[ordinal].unwrap_or(by_type),
                        None => by_type,
                    },
                    _ => by_type,
                },
                OperationKind::Call(_) => match self.callees[operation.index()] {
                    Some(callee) => match self.results.get(callee.index()).copied().flatten() {
                        Some(result) => result.meet(by_type),
                        None => by_type,
                    },
                    None => by_type,
                },
                _ => by_type,
            }
            // Whatever the transfer says, the type holds too.
            .meet(by_type),
        )
    }
}

/// The JavaScript operator of a Number `BinaryOp`, when it has one.
fn javascript_binary(op: BinaryOp) -> Option<crate::js::Binary> {
    use crate::js::Binary;
    Some(match op {
        BinaryOp::Add => Binary::Add,
        BinaryOp::Sub => Binary::Subtract,
        BinaryOp::Mul => Binary::Multiply,
        BinaryOp::Div => Binary::Divide,
        BinaryOp::Mod => Binary::Remainder,
        BinaryOp::BitAnd => Binary::BitAnd,
        BinaryOp::BitOr => Binary::BitOr,
        BinaryOp::Xor => Binary::BitXor,
        BinaryOp::ShiftLeft => Binary::ShiftLeft,
        BinaryOp::ShiftRight => Binary::ShiftRight,
        BinaryOp::UnsignedShiftRight => Binary::UnsignedShiftRight,
        _ => return None,
    })
}

impl Forward for Ranges<'_, '_> {
    type State = Vec<Option<NumberFacts>>;

    fn unreachable(&self) -> Self::State {
        vec![None; self.integral.len()]
    }

    fn join(&self, into: &mut Self::State, from: &Self::State) {
        for (into, from) in into.iter_mut().zip(from) {
            *into = match (*into, *from) {
                (Some(a), Some(b)) => Some(a.join(b)),
                (a, None) => a,
                (None, b) => b,
            };
        }
    }

    fn transfer(&self, unit: &UnitData, operation: OpId, state: &mut Self::State) {
        let op = &unit.operations[operation.index()];
        if let (Some(result), Some(facts)) = (op.result, self.result(operation, state)) {
            self.values.borrow_mut()[result.index()] = facts;
        }
        let operands = unit.operands(op.operands).unwrap_or(&[]);
        match op.kind {
            OperationKind::Initialize(cell) => {
                if let (Some(ordinal), Some(&value)) = (self.ordinal(cell), operands.first()) {
                    state[ordinal] = Some(self.value(value));
                }
            }
            OperationKind::Store(place) => {
                if let Place::Cell(cell) = unit.places[place.index()] {
                    if let (Some(ordinal), Some(&value)) = (self.ordinal(cell), operands.last()) {
                        state[ordinal] = Some(self.value(value));
                    }
                }
            }
            // A per-iteration binding or a caught value is its type's.
            OperationKind::ForIn { key: cell, .. } | OperationKind::ForOf { item: cell, .. } => {
                if let Some(ordinal) = self.ordinal(cell) {
                    state[ordinal] =
                        Some(by_type(self.program, self.program.cells[cell.index()].ty));
                }
            }
            OperationKind::Try {
                catch: Some((Some(cell), _)),
                ..
            } => {
                if let Some(ordinal) = self.ordinal(cell) {
                    state[ordinal] =
                        Some(by_type(self.program, self.program.cells[cell.index()].ty));
                }
            }
            _ => {}
        }
    }

    fn branch(&self, unit: &UnitData, operation: OpId, taken: bool, state: &mut Self::State) {
        match unit.operations[operation.index()].kind {
            OperationKind::If { .. } => {
                let op = &unit.operations[operation.index()];
                if let Some(&condition) = unit.operands(op.operands).and_then(|o| o.first()) {
                    self.narrow(condition, Some(operation), taken, state);
                }
            }
            OperationKind::Loop { test, .. } => {
                if let Some(condition) = unit.regions[test.index()].result {
                    self.narrow(condition, None, taken, state);
                }
            }
            _ => {}
        }
    }

    fn widen(&self, previous: &Self::State, next: &mut Self::State) {
        for ((next, previous), &integer) in next.iter_mut().zip(previous).zip(&self.integral) {
            let (Some(grown), Some(previous)) = (*next, *previous) else {
                continue;
            };
            if grown == previous {
                continue;
            }
            // Grown past the previous iteration: each bound that moved jumps
            // to the type's threshold (int32's range for an `int`); a
            // `float` keeps only that it is a Number.
            *next = Some(
                match (integer, grown.integer_bounds(), previous.integer_bounds()) {
                    (true, Some((minimum, maximum)), Some((old_minimum, old_maximum))) => {
                        let low = if minimum < old_minimum {
                            i32::MIN as i64
                        } else {
                            minimum
                        };
                        let high = if maximum > old_maximum {
                            i32::MAX as i64
                        } else {
                            maximum
                        };
                        NumberFacts::integer_range(low, high, false).unwrap_or(NumberFacts::I32)
                    }
                    (true, _, _) => NumberFacts::I32,
                    (false, _, _) => NumberFacts::NUMBER,
                },
            );
        }
    }
}

#[cfg(test)]
#[path = "ranges_tests.rs"]
mod tests;
