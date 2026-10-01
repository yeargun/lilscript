//! Root constants read as their literal (013-T7.4; Closure's
//! `InlineVariables` for constants and `InferConsts`).
//!
//! A root `let c=L` of a literal that nothing assigns holds `L` wherever it
//! is read once initialized: at a later root statement, or in a function
//! that cannot run before the declaration (`Order::initialized_in`, by
//! initialization order). Those reads become `L` itself. A declaration
//! left unread is pruned later with the other unused ones; an exported one
//! stays for its importers.
//!
//! Source facts are consumed by shared program rules; physical storage created
//! by layouts and inlining is described by the shared target binding owner.
//!
//! A number, boolean, `null` or `undefined` moves by rule, whatever the
//! objective (Closure's `InlineVariables` for immutable values): a few
//! characters, the canonical form. A string moves as the artifact's
//! `string_constants` family says (M7.4: a longer value's forwarding is a
//! choice): measured on 2026-09-29, one reference port's main file is 238
//! Brotli bytes smaller with its string constants named and another's 264
//! larger (the plan record names them). Whether a literal repeated where it
//! stands is better read through a new name is the pooling family's choice
//! (Closure's `AliasStrings`).
use super::facts::Owner;
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// Which root constants a pass forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstantKind {
    /// Numbers, booleans, `null` and `undefined`: a few characters, by rule.
    Scalar,
    /// Strings: the `string_constants` family's choice (M7.4).
    String,
}

impl Module {
    /// Reads of literal root constants of `kind` become the literal. An
    /// observed literal, which has a scored spelling, stays where it is.
    /// Returns how many reads.
    pub(crate) fn forward_root_constants(
        &mut self,
        kind: ConstantKind,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let frames = self.frames(budget)?;
        let order = self.order(&frames, budget)?;
        // The binding owner includes representation-created roots and is
        // rebuilt after edits; this transform adds no value/order proof.
        let owners = self.expression_owners(budget)?;
        self.with_reach_tree(budget, |module, reach, budget| {
            let mut rewrites = Vec::new();
            for &(id, _) in &reach.expressions {
                budget.work(Analysis, 1)?;
                let Expr::Binding(binding) = module.expressions[id.index()] else {
                    continue;
                };
                let Some((declared, value)) = order.literal(binding) else {
                    continue;
                };
                if !matches!(&module.expressions[value.index()],Expr::Literal(literal)
                    if matches!(literal,Literal::String(_)) == (kind == ConstantKind::String))
                {
                    continue;
                }
                let initialized = match owners[id.index()] {
                    Some(Owner::Root(at)) => at > declared,
                    Some(Owner::Function(function)) => order.initialized_in(binding, function),
                    None => false,
                };
                if initialized {
                    rewrites.push((id, value));
                }
            }
            for &(read, value) in &rewrites {
                let node = module.expressions[value.index()].clone();
                module.set_expression(read, node);
            }
            Ok(rewrites.len())
        })?
    }
}
