//! Raw-byte spellings of repeated strings, a raw objective's choice.
//!
//! * An array of plain strings reads `"a b c".split(" ")` when that is
//!   shorter, under pristine builtins: every evaluation still creates a fresh
//!   array of the same strings, and the separator is a character none of
//!   them contains.
//! * A string literal repeated often enough is read from a root constant,
//!   `let s="text"`, declared ahead of every other statement. A string is a
//!   primitive, so the read yields the literal's value wherever it stood, and
//!   the constant is initialized before any code runs.
//!
//! A codec already matches repeated text, and a joined string or a name
//! breaks the match, so a codec objective keeps the literals.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// A pooled constant's name, as spelled: the root often has more than 54
/// bindings, so the estimate assumes two characters.
const POOLED_NAME: usize = 2;

/// Separators tried in order for a packed array.
const SEPARATORS: [&str; 6] = [" ", ",", "|", ";", "~", "!"];

impl Module {
    /// Pack arrays of plain strings into one split string where shorter.
    /// Returns how many, and the renumbering map when it edited.
    pub(crate) fn pack_string_arrays(
        &mut self,
        protected: &[ExprId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(usize, Option<Vec<Option<ExprId>>>), AllocationError> {
        if !self.pristine_builtins {
            return Ok((0, None));
        }
        let reach = self.reach(budget)?;
        let mut packed = 0;
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Array(elements) = &self.expressions[id.index()] else {
                continue;
            };
            let mut values = Vec::with_capacity(elements.len());
            for element in elements {
                match &self.expressions[element.index()] {
                    Expr::Literal(Literal::String(value))
                        if protected.binary_search(element).is_err() =>
                    {
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
                self.expression_in(Expr::Literal(Literal::String(joined)), None, budget)?;
            let callee = self.expression_in(
                Expr::Member {
                    object,
                    property: Property::Named("split".into()),
                },
                None,
                budget,
            )?;
            let separator =
                self.expression_in(Expr::Literal(Literal::String(separator)), None, budget)?;
            self.expressions[id.index()] = Expr::Call {
                callee,
                arguments: vec![separator],
                invocation: Invocation::Reference,
            };
            packed += 1;
        }
        if packed == 0 {
            return Ok((0, None));
        }
        let map = self.renumber(budget)?;
        Ok((packed, Some(map)))
    }

    /// Read each string repeated often enough from a root constant declared
    /// first in the root. Returns how many strings.
    pub(crate) fn pool_strings(
        &mut self,
        protected: &[ExprId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let reach = self.reach(budget)?;
        // Each string's uses, in the order the arena holds them.
        let mut uses: Vec<(StringValue, Vec<ExprId>)> = Vec::new();
        let mut index: std::collections::HashMap<StringValue, usize> =
            std::collections::HashMap::new();
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Literal(Literal::String(value)) = &self.expressions[id.index()] else {
                continue;
            };
            if protected.binary_search(&id).is_ok() {
                continue;
            }
            match index.get(value) {
                Some(&at) => uses[at].1.push(id),
                None => {
                    index.insert(value.clone(), uses.len());
                    uses.push((value.clone(), vec![id]));
                }
            }
        }
        // `k` uses of `L` bytes against `k` names plus `n="…",`.
        uses.retain(|(value, sites)| {
            let (count, length) = (sites.len(), printed(value));
            count * length > count * POOLED_NAME + POOLED_NAME + length + 2
        });
        if uses.is_empty() {
            return Ok(0);
        }
        // Most saved first, then by value, so the order is stable.
        uses.sort_by(|(a, left), (b, right)| {
            let saved = |value: &StringValue, sites: &Vec<ExprId>| {
                sites.len() * (printed(value) - POOLED_NAME)
            };
            saved(b, right)
                .cmp(&saved(a, left))
                .then_with(|| left[0].cmp(&right[0]))
        });
        let root = self.root.index();
        let scope = self.regions[root].scope;
        let mut statements = Vec::with_capacity(uses.len());
        for (value, sites) in &uses {
            budget.work(Analysis, sites.len() as u64)?;
            let binding = self.binding_in(
                Binding {
                    source_symbol: None,
                    scope,
                    spelling: "s".into(),
                    pinned: false,
                },
                budget,
            )?;
            for site in sites {
                self.expressions[site.index()] = Expr::Binding(binding);
            }
            let literal =
                self.expression_in(Expr::Literal(Literal::String(value.clone())), None, budget)?;
            statements.push(Statement::Let {
                binding,
                value: Some(literal),
            });
        }
        let count = statements.len();
        budget.reserve_vec(
            AllocationClass::Retained,
            &mut self.regions[root].statements,
            count,
        )?;
        self.regions[root].statements.splice(0..0, statements);
        if let Some(&first) = self.root_modules.first() {
            budget.reserve_vec(AllocationClass::Retained, &mut self.root_modules, count)?;
            self.root_modules
                .splice(0..0, std::iter::repeat_n(first, count));
        }
        Ok(count)
    }
}

/// A string literal's printed length, quotes included.
fn printed(value: &StringValue) -> usize {
    let mut text = String::new();
    let _ = crate::js_string::contents(&mut text, value, '"', false);
    text.len() + 2
}
