//! Literal and branch folding (plan M7.8a; architecture §8.3–§8.4), on the
//! exact values of `values.rs`, before any spelling is chosen.
//!
//! - An operation that computes an exact value and whose evaluation is not
//!   required becomes that constant: arithmetic, comparisons, copies and
//!   discardable calls. Initialized root scalar loads use the same facts;
//!   objective-sensitive string forwarding remains a representation family.
//! - `x | 0` of an `int` is `x`, and the other bitwise identities with 0.
//! - `if` with an exact condition keeps the branch that runs, as a block (its
//!   lexical scope is kept); `?:` and a short circuit with an exact left
//!   operand keep the operand that runs; a loop whose test is exactly false
//!   and needs no evaluation never runs and goes.
//!
//! A fold's default size heuristic compares the constant's text with a lower
//! bound of the replaced expression's text (inspired by Oxc's
//! `fold_constants.rs`). This is not a gzip/Brotli proof: shorter literals
//! can lose repeated text. Objective calibration must judge those defaults.
//! In that bound a literal the program spells once counts at its text, and every
//! other operand at one character: a target may read a repeated literal from
//! a named constant (Closure's `AliasStrings`, the raw objective's pooling),
//! a load is a name, and a nested expression may be named or folded itself.
//! Each operator counts one character. Closure folds constants in
//! `PeepholeFoldConstants` and branches in `PeepholeRemoveDeadCode`
//! (`closure-compiler@0da58e1`); Oxc in `remove_dead_code.rs:59` (`if`),
//! `:428` (conditional expressions) and `fold_constants.rs:119,196`.

use super::edit::{self, Editor};
use super::values::ProgramValues;
use super::*;
use crate::check::Type;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use facts::{StoredExact, StoredString};

#[derive(Clone, Copy)]
enum Fold {
    /// Keep `region` of the operation, as a block.
    Block { op: OpId, region: RegionId },
    /// Keep `region` of the operation in its place: its cells' scope is
    /// unobservable (they own no storage), so its operations join the
    /// enclosing region, where the exits they hold end it.
    Inline { op: OpId, region: RegionId },
    /// The operation does nothing.
    Remove { op: OpId },
    /// Keep `region` in the operation's place; its result replaces the
    /// operation's.
    Splice {
        op: OpId,
        region: RegionId,
        result: ValueId,
        yields: ValueId,
    },
    /// The operation's result is a value computed before it.
    Replace {
        op: OpId,
        result: ValueId,
        with: ValueId,
    },
}

/// A literal as a target spells it: numbers by value, strings by their
/// interned text.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Literal {
    Number(u64),
    String(StringId),
}

impl Literal {
    fn of(constant: &Constant) -> Option<Self> {
        match constant {
            Constant::Integer(value) => Some(Self::Number(f64::from(*value).to_bits())),
            Constant::Number(bits) => Some(Self::Number(*bits)),
            Constant::String(id) => Some(Self::String(*id)),
            _ => None,
        }
    }
}

/// How often the created units spell each literal.
type Literals = Vec<Literal>;

#[derive(Default)]
struct Plan {
    constants: Vec<(OpId, FoldedConstant)>,
    calls: Vec<CallId>,
    folds: Vec<Fold>,
}

enum FoldedConstant {
    Existing(Constant),
    String(std::sync::Arc<StringValue>),
}

impl FoldedConstant {
    fn text(&self, program: &Program<'_>) -> Option<usize> {
        match self {
            Self::Existing(constant) => constant_text(program, constant),
            Self::String(value) => crate::js_string::literal_length(value, '"'),
        }
    }
}

pub(super) fn apply(
    editor: &mut Editor<'_>,
    values: &ProgramValues,
    effects: &ProgramEffects,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| plans(editor.program(), effects, values, budget),
        |plans, editor, budget| execute(editor, plans, values, receipt, budget),
    )
}

fn plans(
    program: &Program<'_>,
    effects: &ProgramEffects,
    values: &ProgramValues,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<(UnitId, Plan)>, super::RuleError> {
    budget.retained_phase(|budget| {
        let created = created_units_in(program, budget)?;
        let mut literals = Literals::new();
        for frozen in &program.units {
            if !created[frozen.id().index()] {
                continue;
            }
            for operation in &frozen.data().operations {
                budget.work(WorkKind::Analysis, 1)?;
                if let OperationKind::Constant(constant) = &operation.kind {
                    if let Some(literal) = Literal::of(constant) {
                        budget.push(Scratch, &mut literals, literal)?;
                    }
                }
            }
        }
        budget.work(
            WorkKind::Analysis,
            (literals.len() as u64)
                .saturating_mul(u64::from(usize::BITS - literals.len().max(1).leading_zeros()) + 1),
        )?;
        literals.sort_unstable();
        let mut plans = Vec::new();
        for frozen in &program.units {
            if !created[frozen.id().index()] {
                continue;
            }
            let plan = plan(program, effects, values, &literals, frozen.id(), budget)?;
            if plan.constants.is_empty() && plan.folds.is_empty() {
                storage::release_vec(plan.constants, Retained, budget)?;
                storage::release_vec(plan.calls, Retained, budget)?;
                storage::release_vec(plan.folds, Retained, budget)?;
            } else {
                budget.push(Retained, &mut plans, (frozen.id(), plan))?;
            }
        }
        storage::release_vec(created, Retained, budget)?;
        Ok(plans)
    })
}

fn execute(
    editor: &mut Editor<'_>,
    plans: &[(UnitId, Plan)],
    values: &ProgramValues,
    receipt: &mut RuleReceipt,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    for (unit, plan) in plans {
        let unit = *unit;
        let mut constants = budget.vector(Scratch, plan.constants.len())?;
        for (op, constant) in &plan.constants {
            let constant = match constant {
                FoldedConstant::Existing(constant) => constant.clone(),
                FoldedConstant::String(value) => {
                    Constant::String(editor.intern_string_in(value, budget)?)
                }
            };
            constants.push((*op, constant));
        }
        let (data, cells): (&mut UnitData, &mut [Cell]) = if plan
            .folds
            .iter()
            .any(|fold| matches!(fold, Fold::Inline { .. } | Fold::Splice { .. }))
        {
            editor.unit_and_cells_in(unit, budget)?
        } else {
            (editor.unit_mut_in(unit, budget)?, &mut [])
        };
        for (op, constant) in constants.drain(..) {
            if let Some(result) = data.operations[op.index()].result {
                let origin = values.origin(unit, result);
                receipt.set_folds += u32::from(origin & super::values::FROM_SET != 0);
                receipt.path_folds += u32::from(origin & super::values::FROM_PATH != 0);
            }
            edit::make_constant_in(data, op, constant, budget)?;
            receipt.folded_values += 1;
        }
        storage::release_vec(constants, Scratch, budget)?;
        // A folded call's preparations leave with it.
        if !plan.calls.is_empty() {
            receipt.folded_calls += plan.calls.len() as u32;
            for index in 0..data.operations.len() {
                budget.work(WorkKind::Analysis, plan.calls.len() as u64 + 1)?;
                if match data.operations[index].kind {
                    OperationKind::PrepareCall(call)
                    | OperationKind::PrepareReference { call, .. } => plan.calls.contains(&call),
                    _ => false,
                } {
                    edit::detach(data, OpId::from_index(index).unwrap());
                }
            }
        }
        // A fold's value may be another fold's result in the same round
        // (`(x | 0) | 0`): each substitution follows the ones made before it.
        let mut substituted = budget.filled(Scratch, data.values.len(), None)?;
        let resolve = |substituted: &[Option<ValueId>],
                       mut value: ValueId,
                       budget: &mut AllocationBudget<'_>| {
            while let Some(next) = substituted[value.index()] {
                budget.work(WorkKind::Analysis, 1)?;
                value = next;
            }
            Ok::<_, AllocationError>(value)
        };
        for &fold in &plan.folds {
            budget.work(WorkKind::Edit, 1)?;
            let op = match &fold {
                Fold::Block { op, .. }
                | Fold::Inline { op, .. }
                | Fold::Remove { op }
                | Fold::Splice { op, .. }
                | Fold::Replace { op, .. } => *op,
            };
            let operation = &data.operations[op.index()];
            let origin = data
                .operands(operation.operands)
                .unwrap_or(&[])
                .iter()
                .fold(0, |bits, &value| bits | values.origin(unit, value));
            receipt.set_folds += u32::from(origin & super::values::FROM_SET != 0);
            receipt.path_folds += u32::from(origin & super::values::FROM_PATH != 0);
            if matches!(operation.kind, OperationKind::If { .. })
                && data
                    .operands(operation.operands)
                    .and_then(|inputs| inputs.first())
                    .is_some_and(|condition| {
                        matches!(
                            data.operations[data.values[condition.index()].definition.index()].kind,
                            OperationKind::IsUndefined { .. }
                        )
                    })
            {
                receipt.default_checks_removed += 1;
            }
            match fold {
                Fold::Block { op, region } => edit::make_block_in(data, op, region, budget)?,
                Fold::Inline { op, region } => {
                    edit::splice_in(data, cells, unit, op, region, budget)?
                }
                Fold::Remove { op } => edit::detach(data, op),
                Fold::Splice {
                    op,
                    region,
                    result,
                    yields,
                } => {
                    let yields = resolve(&substituted, yields, budget)?;
                    let parent = data.operations[op.index()].region;
                    let position = data.regions[parent.index()]
                        .operations
                        .iter()
                        .position(|found| *found == op)
                        .ok_or("selected operation missing")?;
                    let kept = data.regions[region.index()].operations.len();
                    edit::splice_in(data, cells, unit, op, region, budget)?;
                    if data.values[result.index()].ty == data.values[yields.index()].ty {
                        edit::substitute(data, result, yields);
                        substituted[result.index()] = Some(yields);
                    } else {
                        edit::make_value_view_in(data, op, yields, budget)?;
                        let list = &mut data.regions[parent.index()].operations;
                        budget.reserve_vec(
                            crate::output_budget::AllocationClass::Retained,
                            list,
                            1,
                        )?;
                        list.insert(position + kept, op);
                    }
                }
                Fold::Replace { op, result, with } => {
                    let with = resolve(&substituted, with, budget)?;
                    if data.values[result.index()].ty == data.values[with.index()].ty {
                        edit::detach(data, op);
                        edit::substitute(data, result, with);
                        substituted[result.index()] = Some(with);
                    } else {
                        edit::make_value_view_in(data, op, with, budget)?;
                    }
                }
            }
            receipt.folded_branches += 1;
        }
        storage::release_vec(substituted, Scratch, budget)?;
    }
    Ok(!plans.is_empty())
}

fn plan(
    program: &Program<'_>,
    effects: &ProgramEffects,
    values: &ProgramValues,
    literals: &Literals,
    unit: UnitId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Plan, AllocationError> {
    budget.retained_phase(|budget| {
        let data = program.unit(unit).expect("a program unit");
        let behaviors = behaviors_in(program, effects, unit, Some(values), budget)?;
        let unobservable = unobservable_regions(data, &behaviors, budget)?;
        let mut in_prefix = budget.filled(Scratch, data.operations.len(), false)?;
        for op in prefix(data) {
            in_prefix[op.index()] = true;
        }
        let mut plan = Plan::default();
        // Structural folds apply to the outermost construct of a round; what a
        // fold keeps is folded again in the next round.
        let mut stack = budget.vector(Scratch, data.regions.len())?;
        stack.push((data.entry, false));
        while let Some((region, inside)) = stack.pop() {
            for &op in &data.regions[region.index()].operations {
                let operation = &data.operations[op.index()];
                budget.work(
                    WorkKind::Analysis,
                    data.operands(operation.operands)
                        .map_or(0, |values| values.len()) as u64
                        + 1,
                )?;
                let fold = if inside {
                    None
                } else {
                    structural(program, values, &unobservable, unit, data, op)
                };
                let folded = fold.is_some();
                if matches!(fold, Some(Fold::Block { .. })) {
                    budget.work(WorkKind::Analysis, program.cells.len() as u64)?;
                }
                // A kept branch whose own cells own no storage needs no scope.
                let fold = fold.map(|fold| match fold {
                    Fold::Block { op, region }
                        if program.cells.iter().all(|cell| {
                            cell.owner != unit
                                || cell.region != region
                                || super::scalar(program, cell.ty)
                        }) =>
                    {
                        Fold::Inline { op, region }
                    }
                    fold => fold,
                });
                if let Some(fold) = fold {
                    budget.push(Retained, &mut plan.folds, fold)?;
                }
                for child in operation.kind.child_regions() {
                    budget.push(Scratch, &mut stack, (child, inside || folded))?;
                }
                if in_prefix[op.index()] {
                    continue;
                }
                if let Some(constant) = value(program, values, literals, &behaviors, unit, data, op)
                {
                    if let OperationKind::Call(call) = operation.kind {
                        budget.push(Retained, &mut plan.calls, call)?;
                    }
                    budget.push(Retained, &mut plan.constants, (op, constant))?;
                }
            }
        }
        storage::release_vec(behaviors, Retained, budget)?;
        Ok(plan)
    })
}

fn structural(
    program: &Program<'_>,
    values: &ProgramValues,
    unobservable: &[bool],
    unit: UnitId,
    data: &UnitData,
    op: OpId,
) -> Option<Fold> {
    let operation = &data.operations[op.index()];
    let operands = data.operands(operation.operands)?;
    match operation.kind {
        OperationKind::If { yes, no } => match values.exact(unit, *operands.first()?)? {
            StoredExact::Boolean(true) => Some(Fold::Block { op, region: yes }),
            StoredExact::Boolean(false) => Some(match no {
                Some(region) => Fold::Block { op, region },
                None => Fold::Remove { op },
            }),
            _ => None,
        },
        OperationKind::Select { yes, no } => {
            let region = match values.exact(unit, *operands.first()?)? {
                StoredExact::Boolean(true) => yes,
                StoredExact::Boolean(false) => no,
                _ => return None,
            };
            Some(Fold::Splice {
                op,
                region,
                result: operation.result?,
                yields: data.regions[region.index()].result?,
            })
        }
        OperationKind::ShortCircuit { kind, right } => {
            let left = *operands.first()?;
            let known = values.exact(unit, left)?;
            let take_right = match kind {
                ShortCircuit::Nullish => {
                    matches!(known, StoredExact::Null | StoredExact::Undefined)
                }
                ShortCircuit::BooleanAnd | ShortCircuit::JavaScriptAnd => {
                    facts::truthy(program, known)
                }
                ShortCircuit::BooleanOr | ShortCircuit::JavaScriptOr => {
                    !facts::truthy(program, known)
                }
            };
            let result = operation.result?;
            Some(if take_right {
                Fold::Splice {
                    op,
                    region: right,
                    result,
                    yields: data.regions[right.index()].result?,
                }
            } else {
                Fold::Replace {
                    op,
                    result,
                    with: left,
                }
            })
        }
        // An `int` is its own ToInt32: `x | 0`, `x ^ 0`, `x << 0` and
        // `x >> 0` are `x`, as are `0 | x` and `0 ^ x`. Evaluating an `int`
        // runs nothing (R1), so the operation needs no evaluation of its own.
        OperationKind::Binary(
            kind @ (BinaryOp::BitOr | BinaryOp::Xor | BinaryOp::ShiftLeft | BinaryOp::ShiftRight),
        ) => {
            let [left, right] = operands[..] else {
                return None;
            };
            let int = |value: ValueId| {
                matches!(
                    program.types[data.values[value.index()].ty.index()],
                    Type::Int
                )
            };
            let zero = |value: ValueId| values.exact(unit, value) == Some(&StoredExact::Integer(0));
            let with = if zero(right) && int(left) {
                left
            } else if matches!(kind, BinaryOp::BitOr | BinaryOp::Xor) && zero(left) && int(right) {
                right
            } else {
                return None;
            };
            Some(Fold::Replace {
                op,
                result: operation.result?,
                with,
            })
        }
        OperationKind::Loop { test, .. } => {
            let condition = data.regions[test.index()].result?;
            (values.exact(unit, condition)? == &StoredExact::Boolean(false)
                && unobservable[test.index()])
            .then_some(Fold::Remove { op })
        }
        _ => None,
    }
}

/// Propagate required evaluation to containing regions once per body. The
/// ownership edges come from operations, not lexical-scope parent metadata.
fn unobservable_regions(
    data: &UnitData,
    behaviors: &[EvaluationBehavior],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<bool>, AllocationError> {
    let mut unobservable = budget.filled(Scratch, data.regions.len(), true)?;
    let mut parents = budget.filled(Scratch, data.regions.len(), None)?;
    let mut pending = budget.vector(Scratch, data.regions.len())?;
    for (index, region) in data.regions.iter().enumerate() {
        for &op in &region.operations {
            budget.work(WorkKind::Analysis, 1)?;
            if behaviors[op.index()].requires_evaluation()
                && std::mem::replace(&mut unobservable[index], false)
            {
                pending.push(index);
            }
            for child in data.operations[op.index()].kind.child_regions() {
                parents[child.index()] = Some(index);
            }
        }
    }
    let mut cursor = 0;
    while cursor < pending.len() {
        budget.work(WorkKind::Analysis, 1)?;
        if let Some(parent) = parents[pending[cursor]] {
            if std::mem::replace(&mut unobservable[parent], false) {
                pending.push(parent);
            }
        }
        cursor += 1;
    }
    storage::release_vec(parents, Scratch, budget)?;
    storage::release_vec(pending, Scratch, budget)?;
    Ok(unobservable)
}

/// The constant an operation folds to, if it computes an exact value, its
/// evaluation is not required, and the constant's text is no longer than the
/// operation's shortest text.
fn value(
    program: &Program<'_>,
    values: &ProgramValues,
    literals: &Literals,
    behaviors: &[EvaluationBehavior],
    unit: UnitId,
    data: &UnitData,
    op: OpId,
) -> Option<FoldedConstant> {
    let operation = &data.operations[op.index()];
    let result = operation.result?;
    let root_scalar = matches!(operation.kind, OperationKind::Load(place)
        if matches!(data.places.get(place.index()), Some(Place::Cell(cell))
            if program.unit(program.cells[cell.index()].owner).is_some_and(|owner|
                owner.kind == UnitKind::ModuleInitialization)))
        && values
            .exact(unit, result)
            .is_some_and(|known| !matches!(known, StoredExact::String(_)));
    let value_view = matches!(operation.kind, OperationKind::Load(place)
        if matches!(data.places.get(place.index()), Some(Place::Value(_))));
    if !matches!(
        operation.kind,
        OperationKind::IntBinary(_)
            | OperationKind::Binary(_)
            | OperationKind::Unary { .. }
            | OperationKind::CopyValue
            | OperationKind::Call(_)
            | OperationKind::Intrinsic(_)
            | OperationKind::Enum { .. }
    ) && !root_scalar
        && !value_view
        || (behaviors[op.index()].requires_evaluation() && !values.evaluated_call(unit, op))
    {
        return None;
    }
    let known = values.exact(unit, result)?;
    let constant = match known {
        StoredExact::String(StoredString::Computed(value))
            if matches!(
                program.ty(data.values[result.index()].ty),
                Some(Type::String)
            ) =>
        {
            FoldedConstant::String(value.clone())
        }
        _ => FoldedConstant::Existing(constant(program, data.values[result.index()].ty, known)?),
    };
    (root_scalar || constant.text(program)? <= replaced_text(program, literals, data, operation))
        .then_some(constant)
}

/// The constant of `ty` that is `known`; none when its literal would not have
/// exactly that type.
pub(super) fn constant(program: &Program<'_>, ty: TypeId, known: &StoredExact) -> Option<Constant> {
    fn for_type(program: &Program<'_>, ty: &Type<'_>, known: &StoredExact) -> Option<Constant> {
        Some(match (ty, known) {
            (Type::Dynamic | Type::Unknown, StoredExact::Integer(value)) => {
                Constant::Integer(*value)
            }
            (Type::Dynamic | Type::Unknown, StoredExact::Number(bits)) => Constant::Number(*bits),
            (Type::Dynamic | Type::Unknown, StoredExact::Boolean(value)) => {
                Constant::Boolean(*value)
            }
            (Type::Dynamic | Type::Unknown, StoredExact::String(StoredString::Source(id))) => {
                Constant::String(*id)
            }
            (Type::Null | Type::Nullable(_) | Type::Dynamic | Type::Unknown, StoredExact::Null) => {
                Constant::Null
            }
            (
                Type::Null | Type::Nullable(_) | Type::Dynamic | Type::Unknown,
                StoredExact::Undefined,
            ) => Constant::Undefined,
            (Type::Nullable(inner), known) => return for_type(program, inner, known),
            (Type::Union(members), known) => {
                return members.iter().find_map(|ty| for_type(program, ty, known))
            }
            (Type::Int, StoredExact::Integer(value)) => Constant::Integer(*value),
            (Type::Float, StoredExact::Number(bits)) => Constant::Number(*bits),
            (Type::Float, StoredExact::Integer(value)) => {
                Constant::Number(f64::from(*value).to_bits())
            }
            (Type::Bool, StoredExact::Boolean(value)) => Constant::Boolean(*value),
            (Type::String, StoredExact::String(StoredString::Source(id))) => Constant::String(*id),
            (Type::Enum(declaration), known) => {
                let definition = program.enum_definition(declaration.identity)?;
                if definition.abi == crate::ast::EnumAbi::Flags {
                    let StoredExact::Integer(value) = known else {
                        return None;
                    };
                    if (*value as u32) & !definition.flag_mask != 0 {
                        return None;
                    }
                    Constant::Integer(*value)
                } else {
                    definition
                        .variants
                        .iter()
                        .find(|variant| {
                            super::super::enums::literal(variant.value).as_ref() == Some(known)
                        })?
                        .value
                }
            }
            _ => return None,
        })
    }
    for_type(program, program.ty(ty)?, known)
}

/// The shortest text of a constant; none for a number with no finite
/// numeral, which folding never introduces.
pub(super) fn constant_text(program: &Program<'_>, constant: &Constant) -> Option<usize> {
    Some(match constant {
        Constant::Integer(value) => numeral(f64::from(*value))?,
        Constant::Number(bits) => numeral(f64::from_bits(*bits))?,
        Constant::Boolean(_) => 2,
        Constant::String(id) => program.strings.get(id.index())?.code_units().count() + 2,
        Constant::Null => 4,
        Constant::Undefined => 6,
    })
}

/// A lower bound of an operand's text: a literal the program spells once at
/// its text, anything else at one character.
fn operand_text(
    program: &Program<'_>,
    literals: &Literals,
    data: &UnitData,
    value: ValueId,
) -> usize {
    let definition = data.values[value.index()].definition;
    match &data.operations[definition.index()].kind {
        OperationKind::Constant(constant)
            if Literal::of(constant).is_some_and(|literal| {
                literals.partition_point(|entry| *entry <= literal)
                    - literals.partition_point(|entry| *entry < literal)
                    <= 1
            }) =>
        {
            constant_text(program, constant).unwrap_or(1)
        }
        _ => 1,
    }
}

/// A lower bound of the replaced expression's text in any output: its
/// operands at `operand_text`, each operator, name and parenthesis at one
/// character.
fn replaced_text(
    program: &Program<'_>,
    literals: &Literals,
    data: &UnitData,
    operation: &Operation,
) -> usize {
    let text = |value: &ValueId| operand_text(program, literals, data, *value);
    let operands: usize = data
        .operands(operation.operands)
        .unwrap_or(&[])
        .iter()
        .map(text)
        .sum();
    match operation.kind {
        OperationKind::IntBinary(_) | OperationKind::Binary(_) | OperationKind::Unary { .. } => {
            operands + 1
        }
        OperationKind::Call(call) => {
            let arguments = data
                .arguments(data.calls[call.index()].arguments)
                .unwrap_or(&[]);
            let written: usize = arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Value(value) => text(value),
                    CallArgument::Spread(value) => 3 + text(value),
                    CallArgument::Reference(_) => 1,
                })
                .sum();
            // A builtin's receiver is another evaluated input, even when
            // a target spells the operation through a shared helper.
            let receiver = match data.calls[call.index()].target {
                CallTarget::Intrinsic {
                    receiver: Some(receiver),
                    ..
                } => text(&receiver) + usize::from(!arguments.is_empty()),
                _ => 0,
            };
            3 + receiver + written + arguments.len().saturating_sub(1)
        }
        OperationKind::Intrinsic(_) => operands + 2,
        _ => operands,
    }
}

/// The length of the shortest decimal numeral of a finite number: the
/// shortest round-trip digits, plain or with an exponent, whichever is
/// shorter. None for NaN and the infinities.
fn numeral(value: f64) -> Option<usize> {
    // The printer handles negative zero at its expression boundary. Preserve
    // this heuristic's former sign byte when measuring that value alone.
    crate::js::number_spelling_length(value)
        .map(|length| length + usize::from(value == 0.0 && value.is_sign_negative()))
}
