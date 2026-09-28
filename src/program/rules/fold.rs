//! Literal and branch folding (plan M7.8a; architecture §8.3–§8.4), on the
//! exact values of `values.rs`, before any spelling is chosen.
//!
//! - An operation that computes an exact value and whose evaluation is not
//!   required becomes that constant: arithmetic, comparisons, copies and
//!   discardable calls. A plain load is left to root-constant forwarding
//!   (M7.4); its consumers fold here.
//! - `if` with an exact condition keeps the branch that runs, as a block (its
//!   lexical scope is kept); `?:` and a short circuit with an exact left
//!   operand keep the operand that runs; a loop whose test is exactly false
//!   and needs no evaluation never runs and goes.
//!
//! A fold never lengthens the code it replaces: the constant's text is at
//! most a lower bound of the replaced expression's text in any output (Oxc's
//! rule, `oxc@591966d crates/oxc_minifier/src/peephole/fold_constants.rs:419-421`),
//! so the rule holds under every codec and every later spelling (L3). In
//! that bound a literal the program spells once counts at its text, and every
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
use facts::{StoredExact, StoredString};
use std::collections::HashMap;

enum Fold {
    /// Keep `region` of the operation, as a block.
    Block { op: OpId, region: RegionId },
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
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
type Literals = HashMap<Literal, u32>;

#[derive(Default)]
struct Plan {
    constants: Vec<(OpId, Constant)>,
    calls: Vec<CallId>,
    folds: Vec<Fold>,
}

pub(super) fn apply(
    editor: &mut Editor<'_>,
    values: &ProgramValues,
    effects: &ProgramEffects,
    receipt: &mut RuleReceipt,
) -> bool {
    let program = editor.program();
    let created = created_units(program);
    let mut literals = Literals::new();
    for frozen in program
        .units
        .iter()
        .filter(|frozen| created[frozen.id().index()])
    {
        for operation in &frozen.data().operations {
            if let OperationKind::Constant(constant) = &operation.kind {
                if let Some(literal) = Literal::of(constant) {
                    *literals.entry(literal).or_default() += 1;
                }
            }
        }
    }
    let plans: Vec<(UnitId, Plan)> = program
        .units
        .iter()
        .filter(|frozen| created[frozen.id().index()])
        .map(|frozen| {
            let unit = frozen.id();
            (unit, plan(program, effects, values, &literals, unit))
        })
        .filter(|(_, plan)| !plan.constants.is_empty() || !plan.folds.is_empty())
        .collect();
    if plans.is_empty() {
        return false;
    }
    for (unit, plan) in plans {
        let (data, cells) = editor.unit_and_cells(unit);
        for (op, constant) in plan.constants {
            edit::make_constant(data, op, constant);
            receipt.folded_values += 1;
        }
        // A folded call's preparations leave with it.
        if !plan.calls.is_empty() {
            let preparations: Vec<OpId> = data
                .operations
                .iter()
                .enumerate()
                .filter(|(_, operation)| match operation.kind {
                    OperationKind::PrepareCall(call)
                    | OperationKind::PrepareReference { call, .. } => plan.calls.contains(&call),
                    _ => false,
                })
                .map(|(index, _)| OpId::from_index(index).unwrap())
                .collect();
            for op in preparations {
                edit::detach(data, op);
            }
        }
        for fold in plan.folds {
            match fold {
                Fold::Block { op, region } => edit::make_block(data, op, region),
                Fold::Remove { op } => edit::detach(data, op),
                Fold::Splice {
                    op,
                    region,
                    result,
                    yields,
                } => {
                    edit::splice(data, cells, unit, op, region);
                    edit::substitute(data, result, yields);
                }
                Fold::Replace { op, result, with } => {
                    edit::detach(data, op);
                    edit::substitute(data, result, with);
                }
            }
            receipt.folded_branches += 1;
        }
    }
    true
}

fn plan(
    program: &Program<'_>,
    effects: &ProgramEffects,
    values: &ProgramValues,
    literals: &Literals,
    unit: UnitId,
) -> Plan {
    let data = program.unit(unit).expect("a program unit");
    let behaviors = behaviors(program, effects, unit, Some(values));
    let mut in_prefix = vec![false; data.operations.len()];
    for op in prefix(data) {
        in_prefix[op.index()] = true;
    }
    let mut plan = Plan::default();
    // Structural folds apply to the outermost construct of a round; what a
    // fold keeps is folded again in the next round.
    let mut stack = vec![(data.entry, false)];
    while let Some((region, inside)) = stack.pop() {
        for &op in &data.regions[region.index()].operations {
            let operation = &data.operations[op.index()];
            let fold = if inside {
                None
            } else {
                structural(program, values, &behaviors, unit, data, op)
            };
            let folded = fold.is_some();
            plan.folds.extend(fold);
            for child in operation.kind.child_regions() {
                stack.push((child, inside || folded));
            }
            if in_prefix[op.index()] {
                continue;
            }
            if let Some(constant) = value(program, values, literals, &behaviors, unit, data, op) {
                if let OperationKind::Call(call) = operation.kind {
                    plan.calls.push(call);
                }
                plan.constants.push((op, constant));
            }
        }
    }
    plan
}

fn structural(
    program: &Program<'_>,
    values: &ProgramValues,
    behaviors: &[EvaluationBehavior],
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
        OperationKind::Loop { test, .. } => {
            let condition = data.regions[test.index()].result?;
            (values.exact(unit, condition)? == &StoredExact::Boolean(false)
                && region_unobservable(data, behaviors, test))
            .then_some(Fold::Remove { op })
        }
        _ => None,
    }
}

/// Whether every operation of `region`, and of the regions they own, can go
/// unevaluated.
fn region_unobservable(
    data: &UnitData,
    behaviors: &[EvaluationBehavior],
    region: RegionId,
) -> bool {
    let mut stack = vec![region];
    while let Some(region) = stack.pop() {
        for &op in &data.regions[region.index()].operations {
            if behaviors[op.index()].requires_evaluation() {
                return false;
            }
            stack.extend(data.operations[op.index()].kind.child_regions());
        }
    }
    true
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
) -> Option<Constant> {
    let operation = &data.operations[op.index()];
    let result = operation.result?;
    if !matches!(
        operation.kind,
        OperationKind::IntBinary(_)
            | OperationKind::Binary(_)
            | OperationKind::Unary { .. }
            | OperationKind::CopyValue
            | OperationKind::Call(_)
            | OperationKind::Intrinsic(_)
    ) || behaviors[op.index()].requires_evaluation()
    {
        return None;
    }
    let known = values.exact(unit, result)?;
    let constant = constant(program, data.values[result.index()].ty, known)?;
    (constant_text(program, &constant)? <= replaced_text(program, literals, data, operation))
        .then_some(constant)
}

/// The constant of `ty` that is `known`; none when its literal would not have
/// exactly that type.
fn constant(program: &Program<'_>, ty: TypeId, known: &StoredExact) -> Option<Constant> {
    Some(match (program.ty(ty)?, known) {
        (Type::Int, StoredExact::Integer(value)) => Constant::Integer(*value),
        (Type::Float, StoredExact::Number(bits)) => Constant::Number(*bits),
        (Type::Float, StoredExact::Integer(value)) => Constant::Number(f64::from(*value).to_bits()),
        (Type::Bool, StoredExact::Boolean(value)) => Constant::Boolean(*value),
        (Type::String, StoredExact::String(StoredString::Source(id))) => Constant::String(*id),
        (Type::Enum(declaration), StoredExact::Integer(value))
            if program
                .enum_definition(declaration.identity)?
                .variants
                .iter()
                .any(|variant| variant.value == *value) =>
        {
            Constant::Integer(*value)
        }
        _ => return None,
    })
}

/// The shortest text of a constant; none for a number with no finite
/// numeral, which folding never introduces.
fn constant_text(program: &Program<'_>, constant: &Constant) -> Option<usize> {
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
            if Literal::of(constant)
                .is_some_and(|literal| literals.get(&literal).copied().unwrap_or(0) <= 1) =>
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
                    CallArgument::Reference(_) => 1,
                })
                .sum();
            // `f()`, with a separator between arguments.
            3 + written + arguments.len().saturating_sub(1)
        }
        OperationKind::Intrinsic(_) => operands + 2,
        _ => operands,
    }
}

/// The length of the shortest decimal numeral of a finite number: the
/// shortest round-trip digits, plain or with an exponent, whichever is
/// shorter. None for NaN and the infinities.
fn numeral(value: f64) -> Option<usize> {
    if !value.is_finite() {
        return None;
    }
    let sign = usize::from(value.is_sign_negative());
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e')?;
    let exponent: i32 = exponent.parse().ok()?;
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let significant = digits.trim_end_matches('0').len().max(1);
    // Value = 0.DIGITS x 10^(exponent + 1).
    let point = exponent + 1;
    let count = significant as i32;
    let plain = if point <= 0 {
        1 + (-point) as usize + significant
    } else if point >= count {
        point as usize
    } else {
        significant + 1
    };
    let shift = point - count;
    let exponential = significant + 1 + shift.to_string().len();
    Some(
        sign + if shift != 0 {
            plain.min(exponential)
        } else {
            plain
        },
    )
}
