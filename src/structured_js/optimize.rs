//! One bounded set of transformations, shared by both analysis contenders.
//! Legality is proved against an immutable input. Only then is its uniquely
//! owned target rewritten; Rust prevents a borrowed analysis from surviving
//! that mutation. Names and JavaScript text do not participate in any proof.

use super::*;
use crate::compilation_contract::{JavaScriptExecution, JavaScriptWorld};
use analysis::{Analysis, Mode, Snapshot, Work};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedData {
    Arrays,
    Scalars,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Report {
    pub removed_bindings: usize,
    pub removed_stores: usize,
    pub inlined_bindings: usize,
    pub discarded_expressions: usize,
    pub simplified_control: usize,
    pub folded_constants: usize,
    pub simplified_integer_arithmetic: usize,
    pub removed_allocations: usize,
    pub retained_for_depth: usize,
    pub edited_regions: usize,
    pub remapped_expressions: usize,
    pub inserted_expressions: usize,
    pub compacted: compact::Removed,
    pub work: Work,
    /// Full structural verifications in the release optimizer. The independent
    /// debug comparison of compaction metadata is deliberately additional.
    pub verified_states: usize,
    pub reused_structure: bool,
}
impl Report {
    pub fn changed(&self) -> bool {
        self.removed_bindings
            + self.removed_stores
            + self.inlined_bindings
            + self.discarded_expressions
            + self.simplified_control
            + self.folded_constants
            + self.simplified_integer_arithmetic
            != 0
    }
}

pub struct Outcome {
    pub report: Report,
    pub unchanged_facts: Option<Snapshot>,
}

pub fn optimize<'sem, 'src>(
    tree: &mut lower::AnnotatedTree<'sem, 'src>,
    mode: Mode,
    world: JavaScriptWorld,
) -> Result<Outcome, String> {
    optimize_with(tree, mode, world, OwnedData::Scalars)
}

pub fn optimize_with<'sem, 'src>(
    tree: &mut lower::AnnotatedTree<'sem, 'src>,
    mode: Mode,
    world: JavaScriptWorld,
    owned_data: OwnedData,
) -> Result<Outcome, String> {
    optimize_with_execution(tree, mode, world, JavaScriptExecution::Script, owned_data)
}

/// The caller owns the complete artifact's loading contract. Module execution
/// is independent of public visibility and must survive all subsequent phases.
pub fn optimize_in_execution<'sem, 'src>(
    tree: &mut lower::AnnotatedTree<'sem, 'src>,
    mode: Mode,
    world: JavaScriptWorld,
    execution: JavaScriptExecution,
) -> Result<Outcome, String> {
    optimize_with_execution(tree, mode, world, execution, OwnedData::Scalars)
}

pub fn optimize_with_execution<'sem, 'src>(
    tree: &mut lower::AnnotatedTree<'sem, 'src>,
    mode: Mode,
    world: JavaScriptWorld,
    execution: JavaScriptExecution,
    owned_data: OwnedData,
) -> Result<Outcome, String> {
    if !tree.target().imports.is_empty() && execution != JavaScriptExecution::Module {
        return Err("static imports require ECMAScript module execution".into());
    }
    let (structure, reused_structure) = tree.structure()?;
    let mut report = Report {
        verified_states: usize::from(!reused_structure),
        reused_structure,
        ..Report::default()
    };
    let (snapshot, plan, observations) = {
        let analysis = Analysis::new_in_execution(tree, mode, world, execution);
        if analysis.has_direct_eval() {
            report.work = analysis.work();
            return Ok(Outcome {
                report,
                unchanged_facts: Some(analysis.detach()),
            });
        }
        let mut planner = plan::Planner::new(tree.target(), structure, analysis, world, owned_data);
        planner.run(&mut report);
        planner.finish(&mut report)
    };
    if !report.changed() {
        return Ok(Outcome {
            report,
            unchanged_facts: Some(snapshot),
        });
    }
    drop(snapshot);
    let (substitutions, replacements, regions) = plan.into_parts();
    // No analysis or proof can borrow tree across this mutation. Operand order
    // remains postorder, and a substituted occurrence has exactly one owner.
    report.edited_regions = regions.len();
    report.compacted = tree.edit(|module| {
        for (id, edits) in regions {
            let statements = &mut module.regions[id.index()].statements;
            let mut edits = edits.into_iter().peekable();
            let mut position = 0;
            statements.retain_mut(|statement| {
                let at = position;
                position += 1;
                if !edits.peek().is_some_and(|(index, _)| *index == at) {
                    return true;
                }
                match edits.next().unwrap().1 {
                    Some(replacement) => {
                        *statement = replacement;
                        true
                    }
                    None => false,
                }
            });
            debug_assert!(edits.next().is_none());
        }
        let work = rewrite::apply(module, substitutions, replacements);
        report.remapped_expressions = work.remapped;
        report.inserted_expressions = work.inserted;
        if let Some(observations) = observations {
            plan::check_observations(module, &observations);
        }
    })?;
    report.verified_states += 1;
    Ok(Outcome {
        report,
        unchanged_facts: None,
    })
}

pub(super) fn plan_expressions(
    module: &Module,
    structure: &verify::Structure,
    analysis: &mut Analysis<'_, '_, '_>,
    plan: &plan::Plan,
    report: &mut Report,
) -> Vec<(usize, rewrite::Replacement)> {
    #[derive(Clone, Copy)]
    struct Offset {
        base: ExprId,
        amount: i32,
    }
    let mut offsets: Option<Vec<Option<Offset>>> = None;
    let mut replacements = Vec::new();
    for (index, expression) in module.expressions.iter().enumerate() {
        if !structure.live_expressions[index] {
            continue;
        }
        if plan.replacement(ExprId::new(index)).is_some() {
            continue;
        }
        // Follow only substitutions already proved legal by placement. An
        // arbitrary value-definition edge is not permission to duplicate or
        // move its initializer, even when its arithmetic value is known.
        if let Some(alias) = plan.aliases.as_ref().and_then(|aliases| aliases[index]) {
            if let Some(offsets) = &mut offsets {
                offsets[index] = offsets[alias.index()];
            }
        }
        if !matches!(
            expression,
            Expr::IntBinary { .. }
                | Expr::IntNegate(_)
                | Expr::ToInt32(_)
                | Expr::Intrinsic { .. }
                | Expr::Binary {
                    op: Binary::Add,
                    ..
                }
                | Expr::Template(_)
        ) {
            continue;
        }
        let facts = analysis.facts(ExprId::new(index));
        if facts.effects.discardable() {
            if let Some(value) = facts
                .integer
                .and_then(analysis::IntegerRange::singleton_i32)
            {
                replacements.push((index, rewrite::Replacement::Constant(value)));
                report.folded_constants += 1;
                continue;
            }
            if let Some(value) = analysis.constant(facts) {
                replacements.push((index, rewrite::Replacement::Literal(value)));
                report.folded_constants += 1;
                continue;
            }
        }
        if let Expr::Template(parts) = expression {
            let mut changes = Vec::new();
            for (index, part) in parts.iter().enumerate() {
                if let TemplatePart::Expression(value) = part {
                    let facts = analysis.facts(*value);
                    if facts.effects.discardable() {
                        if let Some(text) = analysis.constant_text(facts) {
                            changes.push((index, text));
                        }
                    }
                }
            }
            if !changes.is_empty() {
                replacements.push((index, rewrite::Replacement::TemplateText(changes)));
                report.folded_constants += 1;
            }
            continue;
        }
        let Expr::IntBinary {
            op: op @ (IntBinary::Add | IntBinary::Subtract),
            left,
            right,
        } = expression
        else {
            continue;
        };
        let left_facts = analysis.facts(*left);
        let right_facts = analysis.facts(*right);
        let constant = |facts: analysis::Facts| {
            facts
                .effects
                .discardable()
                .then(|| {
                    facts
                        .integer
                        .and_then(analysis::IntegerRange::singleton_i32)
                })
                .flatten()
        };
        let (base, amount, base_facts) = if let Some(value) = constant(right_facts) {
            (
                *left,
                if *op == IntBinary::Subtract {
                    value.wrapping_neg()
                } else {
                    value
                },
                left_facts,
            )
        } else if let (IntBinary::Add, Some(value)) = (op, constant(left_facts)) {
            (*right, value, right_facts)
        } else {
            continue;
        };
        if !base_facts.integer.is_some_and(|range| range.fits_i32()) {
            continue;
        }
        let nested = offsets.as_ref().and_then(|offsets| offsets[base.index()]);
        let offset = nested.map_or(Offset { base, amount }, |nested| Offset {
            base: nested.base,
            amount: nested.amount.wrapping_add(amount),
        });
        offsets.get_or_insert_with(|| vec![None; module.expressions.len()])[index] = Some(offset);
        // Signed addition/subtraction associate modulo 2^32. Multiplication
        // does not: its binary64 rounding is part of the source contract.
        if nested.is_some() || offset.amount == 0 {
            replacements.push((
                index,
                rewrite::Replacement::IntegerOffset {
                    base: offset.base,
                    offset: offset.amount,
                },
            ));
            report.simplified_integer_arithmetic += 1;
        }
    }
    replacements
}

pub(super) fn literal_truth(expression: &Expr) -> Option<bool> {
    match expression {
        Expr::Literal(Literal::Bool(value)) => Some(*value),
        Expr::Literal(Literal::Number(value)) => Some(*value != 0.0),
        Expr::Literal(Literal::String(value)) => Some(value.as_unicode() != Some("")),
        Expr::Literal(Literal::Null | Literal::Undefined) => Some(false),
        _ => None,
    }
}
