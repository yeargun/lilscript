//! Raw-byte spellings of repeated strings, a raw objective's choice.
//!
//! * An array of plain strings reads `"a b c".split(" ")` when that is
//!   shorter, under pristine builtins: every evaluation still creates a fresh
//!   array of the same strings, and the separator is a character none of
//!   them contains.
//! * A string or number literal repeated often enough is read from a root
//!   constant, `let s="text"`, declared ahead of every other statement
//!   (Closure's `AliasStrings`, for numbers too). A primitive read yields
//!   the literal's value wherever it stood, and the constant is initialized
//!   before any code runs. Scalar root constants were canonicalized into
//!   their literals first (`forward_root_constants`), and string ones where
//!   the `string_constants` family says so, so this is where a literal
//!   repeated where it stands gets a name.
//!
//! Codec seeds usually keep repeated literal text. Explicit @pool requests
//! instead pin shared storage for surviving decoded string values, even when
//! a raw estimate or a codec seed would retain their literals.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// A pooled constant's name, as spelled: the root often has more than 54
/// bindings, so the estimate assumes two characters.
const POOLED_NAME: usize = 2;

/// Separators tried in order for a packed array.
const SEPARATORS: [&str; 6] = [" ", ",", "|", ";", "~", "!"];

impl Module {
    pub(crate) fn pinned_string(&self, value: &StringValue, budget: &mut AllocationBudget<'_>) -> Result<bool, AllocationError> {
        if self.authored_pool.is_empty() { return Ok(false); }
        let levels = u64::from(usize::BITS - self.authored_pool.len().leading_zeros()) + 1;
        budget.work(Analysis, levels.saturating_mul(value.storage_bytes() as u64 + 1))?;
        Ok(self.authored_pool.binary_search(value).is_ok())
    }

    /// Another representation cannot absorb the literals of a pinned pool.
    pub(crate) fn contains_pooled_string(&self, root: ExprId, budget: &mut AllocationBudget<'_>) -> Result<bool, AllocationError> {
        if self.authored_pool.is_empty() { return Ok(false); }
        let mut phase = budget.scope();
        let mut pending = phase.vector(AllocationClass::Scratch, 1)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            phase.work(Analysis, 1)?;
            let expression = &self.expressions[id.index()];
            if let Expr::Literal(Literal::String(value)) = expression {
                if self.pinned_string(value, &mut phase)? { return Ok(true); }
            }
            expression.visit_children(|child| phase.push(AllocationClass::Scratch, &mut pending, child))?;
        }
        Ok(false)
    }

    /// Pack arrays of plain strings into one split string where shorter.
    /// Returns how many, and the renumbering map when it edited.
    pub(crate) fn pack_string_arrays(
        &mut self,
        permission: ArrayPacking,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(usize, Option<Vec<Option<ExprId>>>), AllocationError> {
        if !self.pristine_builtins || permission == ArrayPacking::Disabled {
            return Ok((0, None));
        }
        let recurring = if permission == ArrayPacking::Startup {
            Some(self.function_expressions(budget)?)
        } else {
            None
        };
        // Arena growth belongs to the module's existing allocation owner.
        // A child scope cannot release that owner's old vector on growth.
        let result = self.pack_string_arrays_at_sites(recurring.as_deref(), budget);
        if let Some(recurring) = recurring {
            let bytes = recurring.capacity() as u64;
            drop(recurring);
            budget.release(AllocationClass::Retained, bytes)?;
        }
        result
    }

    fn pack_string_arrays_at_sites(
        &mut self,
        recurring: Option<&[bool]>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(usize, Option<Vec<Option<ExprId>>>), AllocationError> {
        self.with_reach_tree(budget, |module, reach, budget| {
            let mut packed = 0;
            for &(id, _) in &reach.expressions {
                budget.work(Analysis, 1)?;
                if recurring.is_some_and(|recurring| recurring[id.index()]) {
                    continue;
                }
                let Expr::Array(elements) = &module.expressions[id.index()] else {
                    continue;
                };
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    match &module.expressions[element.index()] {
                        Expr::Literal(Literal::String(value)) if !module.observed(*element) => {
                            if module.pinned_string(value, budget)? { break; }
                            let Some(value) = value.as_unicode() else {
                                break;
                            };
                            values.push(value.to_string());
                        }
                        _ => break,
                    }
                }
                if values.len() != elements.len() || values.is_empty() {
                    continue;
                }
                let Some(separator) = SEPARATORS
                    .iter()
                    .find(|separator| values.iter().all(|value| !value.contains(**separator)))
                else {
                    continue;
                };
                let spelled: usize = values
                    .iter()
                    .map(|value| printed(&StringValue::from(value.as_str())))
                    .sum::<usize>()
                    + values.len()
                    + 1;
                let joined = StringValue::from(values.join(*separator).as_str());
                let separator = StringValue::from(*separator);
                if printed(&joined) + ".split()".len() + printed(&separator) >= spelled {
                    continue;
                }
                let object =
                    module.expression_in(Expr::Literal(Literal::String(joined)), None, budget)?;
                let callee = module.expression_in(
                    Expr::Member {
                        object,
                        property: Property::Named("split".into()),
                    },
                    None,
                    budget,
                )?;
                let separator = module.expression_in(
                    Expr::Literal(Literal::String(separator)),
                    None,
                    budget,
                )?;
                module.set_expression(
                    id,
                    Expr::Call {
                        callee,
                        arguments: vec![separator],
                        invocation: Invocation::Reference,
                    },
                );
                packed += 1;
            }
            if packed == 0 {
                return Ok((0, None));
            }
            let map = module.renumber(budget)?;
            Ok((packed, Some(map)))
        })?
    }

    /// Every expression reachable from any function body. A shared subtree
    /// used both at startup and in a callback is recurring. No call-count or
    /// termination guess can turn a function body into startup-only work.
    fn function_expressions(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<bool>, AllocationError> {
        use AllocationClass::{Retained, Scratch};
        let mut phase = budget.scope();
        let budget = &mut phase;
        let mut expressions = budget.filled(Retained, self.expressions.len(), false)?;
        let mut seen = budget.filled(Scratch, self.regions.len(), false)?;
        let mut regions = budget.vector(Scratch, self.functions.len())?;
        for function in &self.functions {
            budget.work(Analysis, 1)?;
            regions.push(function.body);
        }
        let mut pending = budget.vector(Scratch, 0)?;
        while let Some(region) = regions.pop() {
            budget.work(Analysis, 1)?;
            if std::mem::replace(&mut seen[region.index()], true) {
                continue;
            }
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                let mut result = Ok(());
                statement.visit_regions(|child| {
                    if result.is_ok() {
                        result = budget.push(Scratch, &mut regions, child);
                    }
                });
                result?;
                let mut result = Ok(());
                statement.visit_expressions(|root| {
                    if result.is_ok() {
                        result = budget.push(Scratch, &mut pending, root);
                    }
                });
                result?;
                while let Some(id) = pending.pop() {
                    budget.work(Analysis, 1)?;
                    if std::mem::replace(&mut expressions[id.index()], true) {
                        continue;
                    }
                    self.expressions[id.index()]
                        .visit_children(|child| budget.push(Scratch, &mut pending, child))?;
                }
            }
        }
        drop(seen);
        drop(regions);
        drop(pending);
        phase.finish_retained()?;
        Ok(expressions)
    }

    /// Read each string repeated often enough from a root constant declared
    /// first in the root. Returns how many strings.
    pub(crate) fn pool_strings(
        &mut self,
        automatic: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        self.with_reach_tree(budget, |module, reach, budget| {
            // Each literal's uses, in the order the arena holds them.
            let mut uses: Vec<(Pooled, Vec<ExprId>)> = Vec::new();
            let mut index: std::collections::HashMap<(Pooled, Option<AltId>), usize> =
                std::collections::HashMap::new();
            for &(id, _) in &reach.expressions {
                budget.work(Analysis, 1)?;
                let pooled = match &module.expressions[id.index()] {
                    Expr::Literal(Literal::String(value)) => Pooled::String(value.clone()),
                    Expr::Literal(Literal::Number(value)) => Pooled::Number(value.to_bits()),
                    _ => continue,
                };
                if module.observed(id) {
                    continue;
                }
                // One pooled initializer cannot satisfy opposite quote pins.
                // Keep those representation classes separate; ordinary literals
                // still share exactly as before.
                let quote = module.expression_choices(id).get(ChoiceFamily::QuoteDelimiter);
                let key = (pooled.clone(), quote);
                match index.get(&key) {
                    Some(&at) => uses[at].1.push(id),
                    None => {
                        index.insert(key, uses.len());
                        uses.push((pooled, vec![id]));
                    }
                }
            }
            // An author pin admits a decoded string value, even for one use.
            // Ordinary candidates retain their existing raw estimate.
            let mut kept = 0;
            for index in 0..uses.len() {
                let (value, sites) = &uses[index];
                let pinned = match value {
                    Pooled::String(value) => !module.authored_pool_formed && module.pinned_string(value, budget)?,
                    Pooled::Number(_) => false,
                };
                let (count, length) = (sites.len(), value.printed());
                if pinned || automatic && count.saturating_mul(length)
                    > count.saturating_mul(POOLED_NAME).saturating_add(POOLED_NAME + length + 2) {
                    uses.swap(kept, index);
                    kept += 1;
                }
            }
            uses.truncate(kept);
            // The terminal scheduler may revisit this pass. The literal in a
            // pool's own initializer must not request another pool forever.
            // Keep the value constraints for other encoders, but form them once.
            module.authored_pool_formed = true;
            if uses.is_empty() {
                return Ok(0);
            }
            // Most saved first, then by first use, so the order is stable.
            uses.sort_by(|(a, left), (b, right)| {
                let saved = |value: &Pooled, sites: &Vec<ExprId>| {
                    sites.len().saturating_mul(value.printed().saturating_sub(POOLED_NAME))
                };
                saved(b, right)
                    .cmp(&saved(a, left))
                    .then_with(|| left[0].cmp(&right[0]))
            });
            let root = module.root.index();
            let scope = module.regions[root].scope;
            let mut statements = Vec::with_capacity(uses.len());
            for (value, sites) in &uses {
                budget.work(Analysis, sites.len() as u64)?;
                let binding = module.binding_in(
                    Binding {
                        source_symbol: None,
                        scope,
                        spelling: "s".into(),
                        pinned: false,
                        class: None,
                        defined: false,
                    },
                    budget,
                )?;
                for &site in sites {
                    module.set_expression(site, Expr::Binding(binding));
                }
                let literal = module.expression_in(Expr::Literal(value.literal()), None, budget)?;
                module.copy_author_choices(sites[0], literal);
                statements.push(Statement::Let {
                    binding,
                    value: Some(literal),
                });
            }
            let count = statements.len();
            budget.reserve_vec(
                AllocationClass::Retained,
                &mut module.regions[root].statements,
                count,
            )?;
            // A pool is a rule's definition, beside the first module (design §6).
            let first = module.root_rows.first().map_or(0, |row| row.module);
            if !module.root_rows.is_empty() {
                budget.reserve_vec(AllocationClass::Retained, &mut module.root_rows, count)?;
            }
            module.prepend_roots(
                statements,
                std::iter::repeat_n(RootRow::synthetic(first), count),
            );
            Ok(count)
        })?
    }
}

/// A literal the pool can share: a string, or a number by its bits.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Pooled {
    String(StringValue),
    Number(u64),
}

impl Pooled {
    /// Printed length, quotes included.
    fn printed(&self) -> usize {
        match self {
            Self::String(value) => printed(value),
            Self::Number(bits) => super::print::number_spelling(f64::from_bits(*bits)).len(),
        }
    }

    fn literal(&self) -> Literal {
        match self {
            Self::String(value) => Literal::String(value.clone()),
            Self::Number(bits) => Literal::Number(f64::from_bits(*bits)),
        }
    }
}

/// A string literal's printed length, quotes included.
fn printed(value: &StringValue) -> usize {
    let mut text = String::new();
    let _ = crate::js_string::contents(&mut text, value, '"', false);
    text.len() + 2
}
