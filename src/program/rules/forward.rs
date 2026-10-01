//! Flow-safe forwarding and common primitive computations (plan M7.1/M7.4).
//!
//! A local cell initialized once and never subsequently written or referenced
//! is an SSA transport inside its owner: a load after initialization is the
//! initializer operand. Replacing the load keeps the operand's original
//! evaluation point, so captures, throws and allocation identity do not move.
//!
//! Within one straight-line region, a second total primitive operation with
//! the same operation, operands and result type is the first result. Operand
//! evaluations remain separate program operations and DCE retains any that are
//! observable. Cross-region reuse waits for an explicit dominance proof.

use super::edit::{self, Editor};
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PrimitiveOperation {
    Copy,
    Int(crate::primitive::IntBinary),
    Binary(crate::ast::BinaryOp),
    Unary {
        op: crate::ast::UnaryOp,
        integer: bool,
    },
    IsUndefined(Option<u32>, bool),
    Intrinsic(crate::primitive::ResolvedIntrinsic),
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Operand<'a> {
    Value(ValueId),
    Literal(&'a Constant, TypeId, crate::representation::RegionalChoices),
}

fn operand(data: &UnitData, value: ValueId) -> Operand<'_> {
    let entry = &data.values[value.index()];
    let definition = &data.operations[entry.definition.index()];
    match &definition.kind {
        OperationKind::Constant(constant) => {
            Operand::Literal(constant, entry.ty, definition.authored)
        }
        _ => Operand::Value(value),
    }
}

/// Store only the original operation; its operands remain in the immutable
/// source arena. Probing compares complete keys, so hash collisions cannot
/// introduce a common expression. The table is at most half full.
fn common_slot(
    data: &UnitData,
    operation: OpId,
    available: &[Option<(u64, OpId)>],
    budget: &mut AllocationBudget<'_>,
) -> Result<(usize, u64, Option<ValueId>), AllocationError> {
    let candidate = &data.operations[operation.index()];
    let values = data.operands(candidate.operands).unwrap_or(&[]);
    let ty = data.values[candidate.result.unwrap().index()].ty;
    let kind = primitive(&candidate.kind);
    budget.work(WorkKind::Analysis, values.len() as u64 + 1)?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    candidate.authored.hash(&mut hash);
    kind.hash(&mut hash);
    ty.hash(&mut hash);
    values.len().hash(&mut hash);
    for &value in values {
        operand(data, value).hash(&mut hash);
    }
    let hash = hash.finish();
    let mut slot = hash as usize & (available.len() - 1);
    loop {
        budget.work(WorkKind::Analysis, values.len() as u64 + 1)?;
        let Some((found_hash, found)) = available[slot] else {
            return Ok((slot, hash, None));
        };
        let found = &data.operations[found.index()];
        if hash == found_hash
            && candidate.authored == found.authored
            && kind == primitive(&found.kind)
            && ty == data.values[found.result.unwrap().index()].ty
            && values.iter().map(|&value| operand(data, value)).eq(data
                .operands(found.operands)
                .unwrap_or(&[])
                .iter()
                .map(|&value| operand(data, value)))
        {
            return Ok((slot, hash, found.result));
        }
        slot = (slot + 1) & (available.len() - 1);
    }
}

#[derive(Clone, Copy, Default)]
struct CellState {
    generation: usize,
    loaded: Option<ValueId>,
    reaching: Option<ValueId>,
}
fn state(cells: &mut [CellState], cell: CellId, generation: usize) -> &mut CellState {
    let entry = &mut cells[cell.index()];
    if entry.generation != generation {
        *entry = CellState {
            generation,
            ..CellState::default()
        };
    }
    entry
}

#[derive(Debug, Clone, Copy)]
struct Replacement {
    operation: OpId,
    result: ValueId,
    with: ValueId,
    kind: ReplacementKind,
}

#[derive(Debug, Clone, Copy)]
enum ReplacementKind {
    Forward,
    Common,
}

pub(super) fn apply(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| plans(editor.program(), effects, budget),
        |plans, editor, budget| {
            for (unit, replacements) in plans {
                let data = editor.unit_mut_in(*unit, budget)?;
                let mut phase = budget.scope();
                let mut substituted: Vec<Option<ValueId>> =
                    phase.filled(Scratch, data.values.len(), None)?;
                for replacement in replacements {
                    let mut with = replacement.with;
                    while let Some(next) = substituted[with.index()] {
                        phase.work(WorkKind::Analysis, 1)?;
                        with = next;
                    }
                    phase.work(
                        WorkKind::Edit,
                        data.operands.len() as u64 + data.operations.len() as u64 + 1,
                    )?;
                    edit::detach(data, replacement.operation);
                    edit::substitute(data, replacement.result, with);
                    substituted[replacement.result.index()] = Some(with);
                    match replacement.kind {
                        ReplacementKind::Forward => receipt.forwarded_definitions += 1,
                        ReplacementKind::Common => receipt.common_computations += 1,
                    }
                }
            }
            Ok(!plans.is_empty())
        },
    )
}

fn plans(
    program: &Program<'_>,
    effects: &ProgramEffects,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<(UnitId, Vec<Replacement>)>, super::RuleError> {
    budget.retained_phase(|budget| {
        for unit in &program.units {
            for call in &unit.data().calls {
                budget.work(WorkKind::Analysis, 1)?;
                if matches!(
                    call.target,
                    CallTarget::Value {
                        invocation: Invocation::DirectEval,
                        ..
                    }
                ) {
                    return Ok(Vec::new());
                }
            }
        }
        let created = created_units_in(program, budget)?;
        let free = super::super::defaults::arguments_free_all_in(program, budget)?;
        let mut plans = Vec::new();
        for frozen in &program.units {
            let unit = frozen.id();
            if !created[unit.index()] {
                continue;
            }
            let replacements = budget.with_temporary(
                |budget| {
                    Ok::<_, super::RuleError>(
                        super::super::initialization::UnitInitialization::build_in(
                            frozen.data(),
                            budget,
                        )?,
                    )
                },
                |initialization, budget| {
                    plan(
                        program,
                        effects,
                        unit,
                        free[unit.index()],
                        initialization,
                        budget,
                    )
                },
            )?;
            if replacements.is_empty() {
                storage::release_vec(replacements, Retained, budget)?;
            } else {
                budget.push(Retained, &mut plans, (unit, replacements))?;
            }
        }
        storage::release_vec(created, Retained, budget)?;
        storage::release_vec(free, Retained, budget)?;
        Ok(plans)
    })
}

fn plan(
    program: &Program<'_>,
    effects: &ProgramEffects,
    unit: UnitId,
    arguments_free: bool,
    initialization: &super::super::initialization::UnitInitialization,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Replacement>, super::RuleError> {
    budget.retained_phase(|budget| {
        let data = program.unit(unit).expect("a program unit");
        let graph = effects.graph();
        let behaviors = behaviors_in(program, effects, unit, None, budget)?;
        let classes = program.primitive_classes(graph.seal());
        let mut uses = budget.filled(Scratch, data.values.len(), 0u8)?;
        let mut refusal = None;
        let walked = super::super::uses::walk(data, |event| {
            if let Err(error) = budget.work(WorkKind::Analysis, 1) {
                refusal = Some(error);
                return Err(super::super::uses::UseError::Capacity);
            }
            if let super::super::uses::Event::Value(value, _) = event {
                uses[value.index()] = uses[value.index()].saturating_add(1);
            }
            Ok(())
        });
        if let Some(error) = refusal {
            return Err(error.into());
        }
        walked.map_err(|_| "invalid forward occurrences")?;
        let mut replacements = Vec::new();
        let mut replaced = budget.filled(Scratch, data.operations.len(), false)?;

        // The operand keeps its original evaluation and identity, including a
        // CopyValue already expressed by the IR. Field/ref writes make storage
        // ineligible just like whole-cell writes.
        let mut initial = budget.filled(Scratch, program.cells.len(), None)?;
        for initialized in &data.operations {
            budget.work(WorkKind::Analysis, 1)?;
            let OperationKind::Initialize(id) = initialized.kind else {
                continue;
            };
            let cell = &program.cells[id.index()];
            let storage = graph.storage(id);
            if cell.owner != unit
                || cell.binding != CellBinding::Local
                || (data.kind == UnitKind::ModuleInitialization && graph.seal() != Seal::Module)
                || storage.initializers != 1
                || storage.stored
                || storage.referenced
            {
                continue;
            }
            if let Some(&value) = data
                .operands(initialized.operands)
                .and_then(|operands| operands.first())
            {
                // Keep the storage owner of an allocation or a closure. Its
                // identity/layout and callable ABI are representation choices;
                // turning the owner into a naked SSA value can force a second
                // target temporary and hide the existing scalar/inline family.
                // A load alias keeps that original owner and is safe to forward.
                let definition = &data.operations[data.values[value.index()].definition.index()];
                if matches!(definition.kind, OperationKind::Load(_))
                    || (super::scalar(program, data.values[value.index()].ty)
                        && !matches!(definition.kind, OperationKind::Call(_)))
                {
                    initial[id.index()] = Some(value);
                }
            }
        }
        for (index, operation) in data.operations.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            let OperationKind::Load(place) = operation.kind else {
                continue;
            };
            let Some(&Place::Cell(cell)) = data.places.get(place.index()) else {
                continue;
            };
            let (Some(with), Some(result)) = (initial[cell.index()], operation.result) else {
                continue;
            };
            let op = OpId::from_index(index).expect("an operation index");
            if initialization.after(data, cell, op)
                && data.values[with.index()].ty == data.values[result.index()].ty
            {
                budget.push(
                    Retained,
                    &mut replacements,
                    Replacement {
                        operation: op,
                        result,
                        with,
                        kind: ReplacementKind::Forward,
                    },
                )?;
                replaced[op.index()] = true;
            }
        }

        // Straight-line common primitive operations. Loads are handled above;
        // calls and allocations deliberately keep their separate evaluations.
        let mut cells = budget.filled(Scratch, program.cells.len(), CellState::default())?;
        let mut generation = 0usize;
        for region in &data.regions {
            generation += 1;
            let capacity = region
                .operations
                .len()
                .max(1)
                .checked_mul(2)
                .and_then(usize::checked_next_power_of_two)
                .ok_or(AllocationError::Capacity)?;
            let mut available = budget.filled(Scratch, capacity, None)?;
            let own_scalar = |cell: CellId| {
                let binding = &program.cells[cell.index()];
                let storage = graph.storage(cell);
                binding.owner == unit
                    && super::scalar(program, binding.ty)
                    && binding.binding == CellBinding::Local
                    && !storage.shared
                    && !storage.referenced
                    && (data.kind != UnitKind::ModuleInitialization || graph.seal() == Seal::Module)
            };
            for &op in &region.operations {
                budget.work(WorkKind::Analysis, 1)?;
                let operation = &data.operations[op.index()];
                let written = match operation.kind {
                    OperationKind::Initialize(cell) => Some(cell),
                    OperationKind::Store(place) => match data.places[place.index()] {
                        Place::Cell(cell) => Some(cell),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(cell) = written.filter(|&cell| own_scalar(cell)) {
                    if let Some(&value) = data
                        .operands(operation.operands)
                        .and_then(|values| values.first())
                    {
                        // A mutable cell already stores this evaluation. Only
                        // reuse a literal or an immutable cell read here: an
                        // arbitrary producer would need another snapshot when
                        // later branches still read/write the original cell.
                        let definition =
                            &data.operations[data.values[value.index()].definition.index()];
                        let reusable = match definition.kind {
                            OperationKind::Constant(_) => true,
                            OperationKind::Load(place) => match data.places[place.index()] {
                                Place::Cell(source) => {
                                    let storage = graph.storage(source);
                                    !storage.stored && !storage.referenced
                                }
                                _ => false,
                            },
                            _ => false,
                        };
                        if reusable {
                            state(&mut cells, cell, generation).reaching = Some(value);
                        } else {
                            state(&mut cells, cell, generation).reaching = None;
                        }
                    }
                }
                // A child can write a cell or establish a narrower path. Keep
                // its new loads so range/call consumers see that path and each
                // argument's original evaluation site. Reusing a load across a
                // call can otherwise create a snapshot or hide a useful inline
                // boundary even when the storage itself stays immutable.
                if operation.kind.child_regions().next().is_some()
                    || matches!(
                        operation.kind,
                        OperationKind::PrepareCall(_) | OperationKind::Call(_)
                    )
                {
                    generation += 1;
                }
                let Some(result) = operation.result else {
                    continue;
                };
                if let OperationKind::Load(place) = operation.kind {
                    if let Some(&Place::Cell(cell)) = data.places.get(place.index()) {
                        if let Some(with) = state(&mut cells, cell, generation).reaching {
                            if initialization.after(data, cell, op)
                                && data.values[with.index()].ty == data.values[result.index()].ty
                                && !replaced[op.index()]
                            {
                                budget.push(
                                    Retained,
                                    &mut replacements,
                                    Replacement {
                                        operation: op,
                                        result,
                                        with,
                                        kind: ReplacementKind::Forward,
                                    },
                                )?;
                                replaced[op.index()] = true;
                                continue;
                            }
                        }
                        let storage = graph.storage(cell);
                        let binding = &program.cells[cell.index()];
                        let closed = data.kind != UnitKind::ModuleInitialization
                            || graph.seal() == Seal::Module;
                        let initialized = match binding.binding {
                            CellBinding::Parameter(_) => {
                                arguments_free && !program.is_reference_parameter(cell)
                            }
                            CellBinding::Local => initialization.after(data, cell, op),
                            _ => false,
                        };
                        if binding.owner == unit
                            && closed
                            && initialized
                            && !storage.stored
                            && !storage.referenced
                            && (classes.value(unit, result).primitive()
                                || matches!(
                                    program.ty(binding.ty),
                                    Some(Type::Int | Type::Float | Type::Bool | Type::Enum(_))
                                ))
                        {
                            if let Some(with) = state(&mut cells, cell, generation).loaded {
                                if data.values[with.index()].ty == data.values[result.index()].ty
                                    && !replaced[op.index()]
                                {
                                    budget.push(
                                        Retained,
                                        &mut replacements,
                                        Replacement {
                                            operation: op,
                                            result,
                                            with,
                                            kind: ReplacementKind::Forward,
                                        },
                                    )?;
                                    replaced[op.index()] = true;
                                }
                            } else {
                                state(&mut cells, cell, generation).loaded = Some(result);
                            }
                        }
                    }
                    continue;
                }
                let Some(_) = primitive(&operation.kind) else {
                    continue;
                };
                if behaviors[op.index()] != EvaluationBehavior::TOTAL {
                    continue;
                }
                if let OperationKind::Intrinsic(intrinsic) = operation.kind {
                    if intrinsic
                        != crate::primitive::ResolvedIntrinsic::Property(
                            crate::primitive::Intrinsic::StringLength,
                        )
                        || !data
                            .operands(operation.operands)
                            .unwrap_or(&[])
                            .iter()
                            .all(|&value| {
                                classes.value(unit, value) == super::super::classes::Class::STRING
                            })
                    {
                        continue;
                    }
                }
                // The source effect summary may rely on a checked annotation.
                // Reusing a raw operation also needs producer evidence: a host
                // value with a conversion hook cannot lose its second conversion.
                let primitive_inputs = data
                    .operands(operation.operands)
                    .unwrap_or(&[])
                    .iter()
                    .all(|&value| classes.value(unit, value).primitive());
                if facts::primitive_evaluation_behavior(program, data, operation, primitive_inputs)
                    .is_some_and(|behavior| behavior != EvaluationBehavior::TOTAL)
                {
                    continue;
                }
                let (slot, hash, with) = common_slot(data, op, &available, budget)?;
                if let Some(with) = with {
                    budget.push(
                        Retained,
                        &mut replacements,
                        Replacement {
                            operation: op,
                            result,
                            with,
                            kind: ReplacementKind::Common,
                        },
                    )?;
                } else if uses[result.index()] > 1 {
                    // Sharing an expression with one use introduces a new
                    // temporary and can grow every printed objective. Reuse
                    // an already shared value here; introducing new storage
                    // belongs to a judged representation alternative.
                    available[slot] = Some((hash, op));
                }
            }
            storage::release_vec(available, Scratch, budget)?;
        }
        budget.work(
            WorkKind::Analysis,
            (replacements.len() as u64).saturating_mul(
                u64::from(usize::BITS - replacements.len().max(1).leading_zeros()) + 1,
            ),
        )?;
        replacements.sort_unstable_by_key(|replacement| replacement.operation);
        storage::release_vec(behaviors, Retained, budget)?;
        Ok(replacements)
    })
}

fn primitive(kind: &OperationKind) -> Option<PrimitiveOperation> {
    Some(match *kind {
        OperationKind::CopyValue => PrimitiveOperation::Copy,
        OperationKind::IntBinary(operation) => PrimitiveOperation::Int(operation),
        OperationKind::Binary(operation) => PrimitiveOperation::Binary(operation),
        OperationKind::Unary { op, integer } => PrimitiveOperation::Unary { op, integer },
        OperationKind::IsUndefined { parameter, nullish } => {
            PrimitiveOperation::IsUndefined(parameter, nullish)
        }
        OperationKind::Intrinsic(operation) => PrimitiveOperation::Intrinsic(operation),
        _ => return None,
    })
}
