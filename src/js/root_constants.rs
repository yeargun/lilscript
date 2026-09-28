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
//! A string constant keeps its name where the name is shorter than the text
//! it would repeat: `k` reads of a literal printed in `P` bytes cost `k·P`,
//! and through a name `k·N + N + P + 2` with the declaration `n=L,`
//! (Terser's `inline.js` size rule; esbuild inlines only strings of three
//! characters or fewer). Every other literal moves (Closure's
//! `InlineVariables` for immutable values). Measured on katexlil, whose
//! symbol table reads 15 constants at 1,938 call arguments: the names are
//! smaller in every codec (raw −9,522, gzip −324, Brotli −99), so the rule
//! is the objective's own answer, not a raw-only one. Repeated literals the
//! source never named are `pool_strings`'s choice (Closure's
//! `AliasStrings`), under the raw objective.
use super::quiet::Owner;
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// A kept constant's name, as spelled: the estimate `pool_strings` uses.
const CONSTANT_NAME: usize = 2;

/// Whether `k` reads of a string printed in `printed` bytes are shorter
/// through a name than as the literal.
fn name_is_shorter(reads: usize, printed: usize) -> bool {
    reads * printed > reads * CONSTANT_NAME + CONSTANT_NAME + printed + 2
}

/// A string literal's printed length, quotes included.
fn printed(value: &StringValue) -> usize {
    let mut text = String::new();
    let _ = crate::js_string::contents(&mut text, value, '"', false);
    text.len() + 2
}

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
                rewrites.push((id, value, binding));
            }
        }
        // Prototype switch for A/B: `LILSCRIPT_ROOT_CONSTANT_NAMES=0` moves
        // every literal, as before.
        let names = std::env::var_os("LILSCRIPT_ROOT_CONSTANT_NAMES").is_none_or(|v| v != "0");
        let mut reads = vec![0usize; self.bindings.len()];
        for &(_, _, binding) in &rewrites {
            reads[binding.index()] += 1;
        }
        let mut moved = 0;
        for &(read, value, binding) in &rewrites {
            if names {
                if let Expr::Literal(Literal::String(text)) = &self.expressions[value.index()] {
                    if name_is_shorter(reads[binding.index()], printed(text)) {
                        continue;
                    }
                }
            }
            self.expressions[read.index()] = self.expressions[value.index()].clone();
            moved += 1;
        }
        Ok(moved)
    }
}
