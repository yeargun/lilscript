//! A single owner for arena reconstruction. Replacements may introduce new
//! operands before their result; all prior target handles are mapped explicitly.
//! Payloads move out of the previous arena instead of cloning an entire program.

use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationError};

/// A value-read rule shared with direct target formation. The caller must
/// supply a value occurrence, never an assignment place or receiver callee.
/// Every element is an inert literal and the selected property is own and
/// present, so this removes only an unobserved fresh allocation. It performs
/// no normalization and creates neither a proposal nor a copied payload.
pub(crate) fn literal_array_projection(
    module: &Module,
    value: ExprId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<ExprId>, AllocationError> {
    budget.work(WorkKind::Analysis, 1)?;
    let Expr::Member {
        object,
        property: Property::Computed(key),
    } = &module.expressions[value.index()]
    else {
        return Ok(None);
    };
    let (Expr::Array(elements), Expr::Literal(Literal::Number(index))) = (
        &module.expressions[object.index()],
        &module.expressions[key.index()],
    ) else {
        return Ok(None);
    };
    if *index < 0.0 || index.fract() != 0.0 || *index >= elements.len() as f64 {
        return Ok(None);
    }
    for element in elements {
        budget.work(WorkKind::Analysis, 1)?;
        if !matches!(module.expressions[element.index()], Expr::Literal(_)) {
            return Ok(None);
        }
    }
    Ok(Some(elements[*index as usize]))
}

#[derive(Debug)]
pub(super) enum Replacement {
    Constant(i32),
    Literal(Literal),
    TemplateText(Vec<(usize, StringValue)>),
    Value(ExprId),
    IntegerOffset { base: ExprId, offset: i32 },
    Effects(Vec<ExprId>),
}

impl Replacement {
    /// Semantic inputs retained by this replacement. Observation and effect
    /// clients consume this interface instead of reconstructing an edit from
    /// its eventual syntax or assuming all old children remain live.
    pub(super) fn visit_inputs(&self, original: &Expr, mut visit: impl FnMut(ExprId)) {
        match self {
            Self::Constant(_) | Self::Literal(_) => {}
            Self::Value(value) | Self::IntegerOffset { base: value, .. } => visit(*value),
            Self::Effects(values) => values.iter().copied().for_each(visit),
            Self::TemplateText(changes) => {
                let Expr::Template(parts) = original else {
                    unreachable!("template edits retain their owning operation")
                };
                let mut changes = changes.iter().peekable();
                for (index, part) in parts.iter().enumerate() {
                    if changes.peek().is_some_and(|(changed, _)| *changed == index) {
                        changes.next();
                    } else if let TemplatePart::Expression(value) = part {
                        visit(*value);
                    }
                }
                debug_assert!(changes.next().is_none());
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Work {
    pub remapped: usize,
    pub inserted: usize,
}

pub(super) fn apply(
    module: &mut Module,
    aliases: Option<Vec<Option<ExprId>>>,
    replacements: Vec<(usize, Replacement)>,
) -> Work {
    if aliases.is_none() && replacements.is_empty() {
        return Work::default();
    }
    let expressions = std::mem::take(&mut module.expressions);
    let origins = std::mem::take(&mut module.origins);
    let extra = replacements.iter().filter(|(_, replacement)| {
        matches!(replacement, Replacement::IntegerOffset { offset, .. } if *offset != 0)
    }).count();
    module.expressions = Vec::with_capacity(expressions.len() + extra);
    module.origins = Vec::with_capacity(origins.len() + extra);
    let mut mapped: Vec<ExprId> = Vec::with_capacity(expressions.len());
    let mut replacements = replacements.into_iter().peekable();
    let mut work = Work::default();
    for (index, (mut node, origin)) in expressions.into_iter().zip(origins).enumerate() {
        work.remapped += 1;
        if let Some(alias) = aliases.as_ref().and_then(|aliases| aliases[index]) {
            debug_assert!(alias.index() < index);
            debug_assert!(!replacements.peek().is_some_and(|(at, _)| *at == index));
            mapped.push(mapped[alias.index()]);
            continue;
        }
        if replacements.peek().is_some_and(|(at, _)| *at == index) {
            match replacements.next().unwrap().1 {
                Replacement::Value(value) => {
                    mapped.push(mapped[value.index()]);
                    continue;
                }
                Replacement::Effects(mut values) => {
                    for value in &mut values {
                        *value = mapped[value.index()];
                    }
                    let result = match values.len() {
                        0 => module.expression(Expr::Literal(Literal::Undefined), None),
                        1 => values[0],
                        _ => module.expression(Expr::Sequence(values), None),
                    };
                    // The allocation's value no longer exists. Its children
                    // retain provenance; an effect-only sequence cannot inherit
                    // the removed allocation's source value/type proof.
                    mapped.push(result);
                    continue;
                }
                Replacement::Constant(value) => {
                    node = Expr::Literal(Literal::Number(f64::from(value)))
                }
                Replacement::Literal(value) => node = Expr::Literal(value),
                Replacement::TemplateText(changes) => {
                    let Expr::Template(parts) = &mut node else {
                        unreachable!("template text changes retain their owning operation")
                    };
                    for (index, value) in changes {
                        parts[index] = TemplatePart::String(value);
                    }
                    // Adjacent chunks already serialize contiguously. Keeping
                    // their owned buffers avoids another concatenation pass.
                }
                Replacement::IntegerOffset { base, offset: 0 } => {
                    mapped.push(mapped[base.index()]);
                    continue;
                }
                Replacement::IntegerOffset { base, offset } => {
                    let (op, constant) = if offset < 0 && offset != i32::MIN {
                        (IntBinary::Subtract, -offset)
                    } else {
                        (IntBinary::Add, offset)
                    };
                    let right = module
                        .expression(Expr::Literal(Literal::Number(f64::from(constant))), None);
                    work.inserted += 1;
                    let result = module.expression(
                        Expr::IntBinary {
                            op,
                            left: mapped[base.index()],
                            right,
                        },
                        origin,
                    );
                    mapped.push(result);
                    continue;
                }
            }
        }
        node.remap_children(|child| mapped[child.index()]);
        mapped.push(module.expression(node, origin));
    }
    debug_assert!(replacements.next().is_none());
    for region in &mut module.regions {
        for statement in &mut region.statements {
            statement.remap_expressions(|value| mapped[value.index()]);
        }
    }
    work
}
