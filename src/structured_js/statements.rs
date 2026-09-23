//! Statements as expressions, a raw objective's choice: Terser's
//! `conditionals`, `if_return` and `sequences`, which a leave-one-out on
//! jquerylil's raw build ranks first (1,452, 630 and 450 bytes).
//!
//! * `if(c){a;b}` is `c&&(a,b)`, `if(c)a;else b` is `c?a:b`, and an empty
//!   branch negates: `if(c);else b` is `c||b`. Branches hold only expression
//!   statements, so they declare nothing and leave nothing: evaluating the
//!   expression runs exactly the statements' evaluations, in order.
//! * `if(c)x=a;else x=b` is `x=c?a:b` for one binding.
//! * `if(c)return a;e;return b` is `return c?a:(e,b)`, and so is an `if`
//!   whose branches both return: the same evaluations, then the same return.
//!
//! A codec matches the statement forms' repeated shapes nearly for free
//! (Terser's compression over our Brotli output made it larger), so only a
//! raw objective chooses these.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

impl Module {
    /// Rewrite statements into expressions throughout, until nothing more
    /// applies. Returns how many rewrites.
    pub(crate) fn compress_statements(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut total = 0;
        // Inner regions first: a branch that became one expression lets its
        // `if` become one too. A few rounds reach the fixed point.
        for _ in 0..8 {
            let reach = self.reach(budget)?;
            let depths = self.region_depths(budget)?;
            let mut changed = 0;
            for &region in reach.regions.iter().rev() {
                budget.work(Analysis, 1)?;
                // Far below the nesting limit only, since a chain of
                // conditionals nests as deep as the statements did.
                if depths[region.index()].is_none_or(|depth| depth + 64 > verify::MAX_NESTING) {
                    continue;
                }
                changed += self.compress_region(region, budget)?;
            }
            total += changed;
            if changed == 0 {
                break;
            }
        }
        Ok(total)
    }

    fn compress_region(
        &mut self,
        region: RegionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut changed = 0;
        let mut index = 0;
        while index < self.regions[region.index()].statements.len() {
            budget.work(Analysis, 1)?;
            let statement = self.regions[region.index()].statements[index].clone();
            let Statement::If { condition, yes, no } = statement else {
                index += 1;
                continue;
            };
            let root = region == self.root;
            // Both branches return: one return of a conditional.
            if let (Some([Statement::Return(Some(a))]), Some([Statement::Return(Some(b))])) =
                (self.only(yes, 1), no.and_then(|no| self.only(no, 1)))
            {
                let (a, b) = (*a, *b);
                self.adopt(yes, region, budget)?;
                if let Some(no) = no {
                    self.adopt(no, region, budget)?;
                }
                let value = self.expression_in(
                    Expr::Conditional {
                        condition,
                        yes: a,
                        no: b,
                    },
                    None,
                    budget,
                )?;
                self.regions[region.index()].statements[index] = Statement::Return(Some(value));
                changed += 1;
                index += 1;
                continue;
            }
            // `if(c)return a;e…;return b`.
            if let (Some([Statement::Return(Some(a))]), None) = (self.only(yes, 1), no) {
                let a = *a;
                let statements = &self.regions[region.index()].statements;
                let mut end = index + 1;
                while matches!(statements.get(end), Some(Statement::Evaluate(_))) {
                    end += 1;
                }
                if let Some(&Statement::Return(Some(b))) = statements.get(end) {
                    let mut rest: Vec<ExprId> = statements[index + 1..end]
                        .iter()
                        .map(|statement| match statement {
                            Statement::Evaluate(value) => *value,
                            _ => unreachable!("only expression statements were counted"),
                        })
                        .collect();
                    rest.push(b);
                    self.adopt(yes, region, budget)?;
                    let otherwise = self.sequence(rest, budget)?;
                    let value = self.expression_in(
                        Expr::Conditional {
                            condition,
                            yes: a,
                            no: otherwise,
                        },
                        None,
                        budget,
                    )?;
                    self.regions[region.index()]
                        .statements
                        .splice(index..=end, [Statement::Return(Some(value))]);
                    if root && end < self.root_modules.len() {
                        self.root_modules.drain(index + 1..=end);
                    }
                    changed += 1;
                    index += 1;
                    continue;
                }
            }
            // Branches of expression statements: one expression.
            let yes_values = self.evaluations(yes);
            let no_values = match no {
                Some(no) => self.evaluations(no).map(Some),
                None => Some(None),
            };
            let (Some(yes_values), Some(no_values)) = (yes_values, no_values) else {
                index += 1;
                continue;
            };
            let replacement = match (yes_values.is_empty(), no_values) {
                (true, None) => None,
                (false, None) => {
                    let value = self.sequence(yes_values, budget)?;
                    Some(Expr::Binary {
                        op: Binary::And,
                        left: condition,
                        right: value,
                    })
                }
                (true, Some(no_values)) if no_values.is_empty() => None,
                (true, Some(no_values)) => {
                    let value = self.sequence(no_values, budget)?;
                    Some(Expr::Binary {
                        op: Binary::Or,
                        left: condition,
                        right: value,
                    })
                }
                (false, Some(no_values)) if no_values.is_empty() => {
                    let value = self.sequence(yes_values, budget)?;
                    Some(Expr::Binary {
                        op: Binary::And,
                        left: condition,
                        right: value,
                    })
                }
                (false, Some(no_values)) => {
                    // `x=c?a:b` when each branch assigns one binding.
                    let assigned = |values: &[ExprId]| match values {
                        [value] => match &self.expressions[value.index()] {
                            Expr::Assign { target, value } => {
                                match self.expressions[target.index()] {
                                    Expr::Binding(binding) => Some((binding, *target, *value)),
                                    _ => None,
                                }
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    match (assigned(&yes_values), assigned(&no_values)) {
                        (Some((left, target, a)), Some((right, _, b))) if left == right => {
                            let value = self.expression_in(
                                Expr::Conditional {
                                    condition,
                                    yes: a,
                                    no: b,
                                },
                                None,
                                budget,
                            )?;
                            Some(Expr::Assign { target, value })
                        }
                        _ => {
                            let a = self.sequence(yes_values, budget)?;
                            let b = self.sequence(no_values, budget)?;
                            Some(Expr::Conditional {
                                condition,
                                yes: a,
                                no: b,
                            })
                        }
                    }
                }
            };
            if let Some(replacement) = replacement {
                self.adopt(yes, region, budget)?;
                if let Some(no) = no {
                    self.adopt(no, region, budget)?;
                }
                let value = self.expression_in(replacement, None, budget)?;
                self.regions[region.index()].statements[index] = Statement::Evaluate(value);
                changed += 1;
            }
            index += 1;
        }
        Ok(changed)
    }

    /// Scopes nested in `inner`'s now nest in `outer`'s: its expressions
    /// moved there, and the functions they create with them.
    fn adopt(
        &mut self,
        inner: RegionId,
        outer: RegionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let (inner, outer) = (
            self.regions[inner.index()].scope,
            self.regions[outer.index()].scope,
        );
        budget.work(Analysis, self.scopes.len() as u64)?;
        for parent in self.scopes.iter_mut().flatten() {
            if *parent == inner {
                *parent = outer;
            }
        }
        Ok(())
    }

    /// The region's statements when there are exactly `count`.
    fn only(&self, region: RegionId, count: usize) -> Option<&[Statement]> {
        let statements = &self.regions[region.index()].statements;
        (statements.len() == count).then_some(statements.as_slice())
    }

    /// The region's values when it holds only expression statements.
    fn evaluations(&self, region: RegionId) -> Option<Vec<ExprId>> {
        self.regions[region.index()]
            .statements
            .iter()
            .map(|statement| match statement {
                Statement::Evaluate(value) => Some(*value),
                _ => None,
            })
            .collect()
    }

    /// `a` for one value, `(a,b,…)` for more.
    fn sequence(
        &mut self,
        values: Vec<ExprId>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<ExprId, AllocationError> {
        match values.as_slice() {
            [value] => Ok(*value),
            _ => self.expression_in(Expr::Sequence(values), None, budget),
        }
    }
}
