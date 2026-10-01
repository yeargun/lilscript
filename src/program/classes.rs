//! Runtime primitive classes shared by program rules and target formation.
//!
//! Checked annotations do not prove a raw host value's class. These facts join
//! every producer of closed storage, complete-call-set arguments and results.
//! Starting at unknown and only narrowing makes each bounded round sound,
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
            let mut producers = budget.filled(Scratch, cells.len(), Class::EMPTY)?;
            // Each round is sound without convergence. A long dependency chain
            // keeps unknown facts after the bound rather than guessing a class.
            for _ in 0..16 {
                let mut changed = false;
                budget.work(WorkKind::Analysis, cells.len() as u64)?;
                producers.fill(Class::EMPTY);
                for frozen in &program.units {
                    let unit = frozen.id();
                    let data = frozen.data();
                    let mut returned = Class::UNDEFINED;
                    for operation in &data.operations {
                        budget.work(
                            WorkKind::Analysis,
                            data.operands(operation.operands)
                                .map_or(0, |operands| operands.len())
                                as u64
                                + 1,
                        )?;
                        if let Some(value) = operation.result {
                            let class =
                                transfer(data, operation, &units[unit.index()], &cells, |call| {
                                    match graph.callee(unit, call) {
                                        Callee::Unit(callee)
                                            if program.unit(callee).is_some_and(|body| {
                                                body.suspension == Suspension::None
                                                    && body.constructor_of.is_none()
                                            }) =>
                                        {
                                            results[callee.index()]
                                        }
                                        Callee::Intrinsic(
                                            crate::primitive::ResolvedIntrinsic::Method(
                                                crate::primitive::Intrinsic::FloatToInt,
                                            ),
                                        ) => Class::NUMBER,
                                        _ => Class::UNKNOWN,
                                    }
                                });
                            let next = units[unit.index()][value.index()].meet(class);
                            changed |= next != units[unit.index()][value.index()];
                            units[unit.index()][value.index()] = next;
                        }
                        let argument = data
                            .operands(operation.operands)
                            .and_then(|operands| operands.first())
                            .map_or(Class::UNKNOWN, |value| units[unit.index()][value.index()]);
                        match operation.kind {
                            OperationKind::Initialize(cell) => {
                                producers[cell.index()] = producers[cell.index()].join(argument)
                            }
                            OperationKind::Store(place) => {
                                if let Place::Cell(cell) = data.places[place.index()] {
                                    producers[cell.index()] =
                                        producers[cell.index()].join(argument);
                                }
                            }
                            OperationKind::ForIn { key, .. } => {
                                producers[key.index()] = producers[key.index()].join(Class::STRING)
                            }
                            OperationKind::ForOf { item, .. } => {
                                producers[item.index()] = Class::UNKNOWN
                            }
                            OperationKind::Try {
                                catch: Some((Some(cell), _)),
                                ..
                            } => producers[cell.index()] = Class::UNKNOWN,
                            OperationKind::Return => returned = returned.join(argument),
                            _ => {}
                        }
                    }
                    let next = results[unit.index()].meet(returned);
                    changed |= next != results[unit.index()];
                    results[unit.index()] = next;
                    for (position, &parameter) in data.parameters.iter().enumerate() {
                        let mut passed = Class::EMPTY;
                        match graph.complete_callers(unit) {
                            Some(edges) if !edges.is_empty() => {
                                for edge in edges {
                                    budget.work(WorkKind::Analysis, 1)?;
                                    let caller = program.unit(edge.caller).expect("a caller");
                                    let site = &caller.calls[edge.call.index()];
                                    let arguments = caller.arguments(site.arguments).unwrap_or(&[]);
                                    let class = if edge.kind != EdgeKind::Call
                                        || site.contract.instantiation.is_some()
                                    {
                                        Class::UNKNOWN
                                    } else {
                                        match arguments.get(position) {
                                            Some(CallArgument::Value(value)) => {
                                                units[edge.caller.index()][value.index()]
                                            }
                                            None => Class::UNDEFINED,
                                            _ => Class::UNKNOWN,
                                        }
                                    };
                                    passed = passed.join(class);
                                    if position
                                        >= arguments
                                            .len()
                                            .saturating_sub(site.omit_trailing as usize)
                                    {
                                        passed = passed.join(Class::UNDEFINED);
                                    }
                                }
                            }
                            _ => passed = Class::UNKNOWN,
                        }
                        producers[parameter.index()] = producers[parameter.index()].join(passed);
                    }
                }
                for (index, cell) in cells.iter_mut().enumerate() {
                    if closed[index] && producers[index] != Class::EMPTY {
                        let next = cell.meet(producers[index]);
                        changed |= next != *cell;
                        *cell = next;
                    }
                }
                if !changed {
                    break;
                }
            }
            Ok(Self {
                deps: Deps::of_program_in(program, budget)?,
                units,
                cells,
            })
        })
    }
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
