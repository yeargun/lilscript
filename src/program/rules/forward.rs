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
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PrimitiveOperation {
    Copy,
    Int(crate::primitive::IntBinary),
    Binary(crate::ast::BinaryOp),
    Unary {
        op: crate::ast::UnaryOp,
        integer: bool,
    },
    IsUndefined,
    Intrinsic(crate::primitive::ResolvedIntrinsic),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    operation: PrimitiveOperation,
    operands: Vec<Operand>,
    ty: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Operand {
    Value(ValueId),
    Literal(Constant, TypeId),
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
) -> bool {
    let program = editor.program();
    if program.units.iter().any(|unit| {
        unit.data().calls.iter().any(|call| {
            matches!(
                call.target,
                CallTarget::Value {
                    invocation: Invocation::DirectEval,
                    ..
                }
            )
        })
    }) {
        return false;
    }
    let created = created_units(program);
    let plans: Vec<(UnitId, Vec<Replacement>)> = program
        .units
        .iter()
        .filter(|frozen| created[frozen.id().index()])
        .filter_map(|frozen| {
            let replacements = plan(program, effects, frozen.id());
            (!replacements.is_empty()).then_some((frozen.id(), replacements))
        })
        .collect();
    if plans.is_empty() {
        return false;
    }
    for (unit, replacements) in plans {
        let data = editor.unit_mut(unit);
        let mut substituted: HashMap<ValueId, ValueId> = HashMap::new();
        let resolve = |substituted: &HashMap<ValueId, ValueId>, mut value: ValueId| {
            while let Some(&next) = substituted.get(&value) {
                value = next;
            }
            value
        };
        for replacement in replacements {
            let with = resolve(&substituted, replacement.with);
            edit::detach(data, replacement.operation);
            edit::substitute(data, replacement.result, with);
            substituted.insert(replacement.result, with);
            match replacement.kind {
                ReplacementKind::Forward => receipt.forwarded_definitions += 1,
                ReplacementKind::Common => receipt.common_computations += 1,
            }
        }
    }
    true
}

fn plan(program: &Program<'_>, effects: &ProgramEffects, unit: UnitId) -> Vec<Replacement> {
    let data = program.unit(unit).expect("a program unit");
    let graph = effects.graph();
    let initialization = super::super::initialization::UnitInitialization::build(data);
    let behaviors = behaviors(program, effects, unit, None);
    let classes = program.primitive_classes(graph.seal());
    let mut uses = vec![0u8; data.values.len()];
    super::super::uses::walk(data, |event| {
        if let super::super::uses::Event::Value(value, _) = event {
            uses[value.index()] = uses[value.index()].saturating_add(1);
        }
        Ok(())
    }).expect("verified program occurrences");
    let mut replacements = Vec::new();
    let mut replaced = vec![false; data.operations.len()];
    let arguments_free = super::super::defaults::arguments_free(program, unit);

    // The operand keeps its original evaluation and identity, including a
    // CopyValue already expressed by the IR. Field/ref writes make storage
    // ineligible just like whole-cell writes.
    let mut initial: HashMap<CellId, ValueId> = HashMap::new();
    for initialized in &data.operations {
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
                initial.insert(id, value);
            }
        }
    }
    for (index, operation) in data.operations.iter().enumerate() {
        let OperationKind::Load(place) = operation.kind else {
            continue;
        };
        let Some(&Place::Cell(cell)) = data.places.get(place.index()) else {
            continue;
        };
        let (Some(&with), Some(result)) = (initial.get(&cell), operation.result) else {
            continue;
        };
        let op = OpId::from_index(index).expect("an operation index");
        if initialization.after(data, cell, op)
            && data.values[with.index()].ty == data.values[result.index()].ty
        {
            replacements.push(Replacement {
                operation: op,
                result,
                with,
                kind: ReplacementKind::Forward,
            });
            replaced[op.index()] = true;
        }
    }

    // Straight-line common primitive operations. Loads are handled above;
    // calls and allocations deliberately keep their separate evaluations.
    for region in &data.regions {
        let mut available: HashMap<Key, ValueId> = HashMap::new();
        let mut loaded: HashMap<CellId, ValueId> = HashMap::new();
        let mut reaching: HashMap<CellId, ValueId> = HashMap::new();
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
                    let definition = &data.operations[data.values[value.index()].definition.index()];
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
                        reaching.insert(cell, value);
                    } else {
                        reaching.remove(&cell);
                    }
                }
            }
            // A child can write a cell or establish a narrower path. Keep
            // its new loads so range/call consumers see that path and each
            // argument's original evaluation site. Reusing a load across a
            // call can otherwise create a snapshot or hide a useful inline
            // boundary even when the storage itself stays immutable.
            if operation.kind.child_regions().next().is_some()
                || matches!(operation.kind, OperationKind::PrepareCall(_) | OperationKind::Call(_))
            {
                reaching.clear();
                loaded.clear();
            }
            let Some(result) = operation.result else {
                continue;
            };
            if let OperationKind::Load(place) = operation.kind {
                if let Some(&Place::Cell(cell)) = data.places.get(place.index()) {
                    if let Some(&with) = reaching.get(&cell) {
                        if initialization.after(data, cell, op)
                            && data.values[with.index()].ty == data.values[result.index()].ty
                            && !replaced[op.index()]
                        {
                            replacements.push(Replacement {
                                operation: op,
                                result,
                                with,
                                kind: ReplacementKind::Forward,
                            });
                            replaced[op.index()] = true;
                            continue;
                        }
                    }
                    let storage = graph.storage(cell);
                    let binding = &program.cells[cell.index()];
                    let closed =
                        data.kind != UnitKind::ModuleInitialization || graph.seal() == Seal::Module;
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
                            || matches!(program.ty(binding.ty), Some(Type::Int | Type::Float | Type::Bool | Type::Enum(_))))
                    {
                        if let Some(&with) = loaded.get(&cell) {
                            if data.values[with.index()].ty == data.values[result.index()].ty
                                && !replaced[op.index()]
                            {
                                replacements.push(Replacement {
                                    operation: op,
                                    result,
                                    with,
                                    kind: ReplacementKind::Forward,
                                });
                                replaced[op.index()] = true;
                            }
                        } else {
                            loaded.insert(cell, result);
                        }
                    }
                }
                continue;
            }
            let Some(kind) = primitive(&operation.kind) else {
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
            let key = Key {
                operation: kind,
                operands: data
                    .operands(operation.operands)
                    .unwrap_or(&[])
                    .iter()
                    .map(|&value| {
                        let entry = &data.values[value.index()];
                        match &data.operations[entry.definition.index()].kind {
                            OperationKind::Constant(constant) => {
                                Operand::Literal(constant.clone(), entry.ty)
                            }
                            _ => Operand::Value(value),
                        }
                    })
                    .collect(),
                ty: data.values[result.index()].ty,
            };
            if let Some(&with) = available.get(&key) {
                replacements.push(Replacement {
                    operation: op,
                    result,
                    with,
                    kind: ReplacementKind::Common,
                });
            } else if uses[result.index()] > 1 {
                // Sharing an expression with one use introduces a new
                // temporary and can grow every printed objective. Reuse
                // an already shared value here; introducing new storage
                // belongs to a judged representation alternative.
                available.insert(key, result);
            }
        }
    }
    replacements.sort_by_key(|replacement| replacement.operation);
    replacements
}

fn primitive(kind: &OperationKind) -> Option<PrimitiveOperation> {
    Some(match *kind {
        OperationKind::CopyValue => PrimitiveOperation::Copy,
        OperationKind::IntBinary(operation) => PrimitiveOperation::Int(operation),
        OperationKind::Binary(operation) => PrimitiveOperation::Binary(operation),
        OperationKind::Unary { op, integer } => PrimitiveOperation::Unary { op, integer },
        OperationKind::IsUndefined => PrimitiveOperation::IsUndefined,
        OperationKind::Intrinsic(operation) => PrimitiveOperation::Intrinsic(operation),
        _ => return None,
    })
}
