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
//! Every literal moves, whatever its length and whatever the objective
//! (Closure's `InlineVariables` for immutable values): this is the
//! canonical form. Whether a repeated literal is better read through a name
//! is a separate choice, made once for all literals of the program, by the
//! objective: `pool_strings` shares them under the raw objective (Closure's
//! `AliasStrings`), and a codec objective keeps the repeated text it
//! matches.
use super::quiet::Owner;
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

impl Module {
    /// Reads of literal root constants become the literal. `protected`
    /// (ascending) lists literals with scored spellings, which stay where
    /// they are. Returns how many reads.
    pub(crate) fn forward_root_constants(
        &mut self,
        protected: &[ExprId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let frames = self.frames(budget)?;
        let order = self.order(&frames, budget)?;
        let mut values: Vec<Option<(usize, ExprId)>> = vec![None; self.bindings.len()];
        let mut any = false;
        for (index, statement) in self.regions[self.root.index()]
            .statements
            .iter()
            .enumerate()
        {
            budget.work(Analysis, 1)?;
            let Statement::Let {
                binding,
                value: Some(value),
            } = *statement
            else {
                continue;
            };
            if order.written(binding) || protected.binary_search(&value).is_ok() {
                continue;
            }
            if matches!(self.expressions[value.index()], Expr::Literal(_)) {
                values[binding.index()] = Some((index, value));
                any = true;
            }
        }
        if !any {
            return Ok(0);
        }
        let owners = self.expression_owners(budget)?;
        let reach = self.reach(budget)?;
        let mut rewrites = Vec::new();
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Binding(binding) = self.expressions[id.index()] else {
                continue;
            };
            let Some((declared, value)) = values[binding.index()] else {
                continue;
            };
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
            self.expressions[read.index()] = self.expressions[value.index()].clone();
        }
        Ok(rewrites.len())
    }
}
