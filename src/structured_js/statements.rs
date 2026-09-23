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
//! * `if(c)o.k=a;else o.k=b` is `o.k=c?a:b` where reading `o` (and a key)
//!   before the condition changes nothing (`same_store`).
//! * `while(c){…;u}` is `for(;c;u){…}` when no `continue` skips `u`.
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
            let frames = self.frames(budget)?;
            let mut changed = 0;
            for &region in reach.regions.iter().rev() {
                budget.work(Analysis, 1)?;
                // Far below the nesting limit only, since a chain of
                // conditionals nests as deep as the statements did.
                if depths[region.index()].is_none_or(|depth| depth + 64 > verify::MAX_NESTING) {
                    continue;
                }
                changed += self.compress_region(region, &frames, &reach.captured, budget)?;
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
        frames: &Frames,
        captured: &[bool],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut changed = 0;
        let mut index = 0;
        while index < self.regions[region.index()].statements.len() {
            budget.work(Analysis, 1)?;
            let statement = self.regions[region.index()].statements[index].clone();
            // `while(c){…;u}` is `for(;c;u){…}` when nothing continues the
            // loop: the update runs where the body's last statement did.
            if let Statement::Loop {
                condition,
                update: None,
                body,
            } = statement
            {
                if let Some(&Statement::Evaluate(last)) = self.regions[body.index()].statements.last() {
                    if !self.continues(body, budget)? && self.outside_body(last, body, budget)? {
                        self.regions[body.index()].statements.pop();
                        self.regions[region.index()].statements[index] = Statement::Loop {
                            condition,
                            update: Some(last),
                            body,
                        };
                        changed += 1;
                    }
                }
                index += 1;
                continue;
            }
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
                    // `x=c?a:b` when each branch assigns one binding, and
                    // `o.k=c?a:b` when each stores to the same property.
                    let assigned = |values: &[ExprId]| match values {
                        [value] => match &self.expressions[value.index()] {
                            Expr::Assign { target, value } => Some((*target, *value)),
                            _ => None,
                        },
                        _ => None,
                    };
                    let same = match (assigned(&yes_values), assigned(&no_values)) {
                        (Some((left, a)), Some((right, b))) => {
                            self.same_store(left, right, condition, region, index, frames, captured, budget)?
                                .then_some((left, a, b))
                        }
                        _ => None,
                    };
                    match same {
                        Some((target, a, b)) => {
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

    /// Whether a `continue` in `body` (not in a loop or function inside it)
    /// continues the loop owning it.
    fn continues(
        &self,
        body: RegionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let mut regions = vec![body];
        while let Some(region) = regions.pop() {
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                match statement {
                    Statement::Continue => return Ok(true),
                    Statement::Loop { .. }
                    | Statement::ForIn { .. }
                    | Statement::ForOf { .. }
                    | Statement::Function { .. } => {}
                    _ => statement.visit_regions(|child| regions.push(child)),
                }
            }
        }
        Ok(false)
    }

    /// Whether `value` can move from `body`'s end into its loop's update,
    /// which runs in the loop's enclosing scope: it reads no binding the
    /// body declares, and creates no function (whose scope would move).
    fn outside_body(
        &self,
        value: ExprId,
        body: RegionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let scope = self.regions[body.index()].scope;
        let mut pending = vec![value];
        while let Some(id) = pending.pop() {
            budget.work(Analysis, 1)?;
            let expression = &self.expressions[id.index()];
            match expression {
                Expr::Binding(binding) if self.bindings[binding.index()].scope == scope => {
                    return Ok(false)
                }
                _ if expression.created_function().is_some() => return Ok(false),
                _ => {}
            }
            let _ = expression.visit_children(|child| {
                pending.push(child);
                Ok::<_, ()>(())
            });
        }
        Ok(true)
    }

    /// Whether two assignment targets name the same place, so one store of
    /// a conditional replaces a store in each branch of `condition`. That
    /// store reads the target's object binding (and a key binding) before
    /// the condition instead of after it, which changes nothing when the
    /// binding is initialized there and only this code can assign it: no
    /// closure reaches it and the condition does not. A key converts at the
    /// store in current engines, but earlier ones converted it at the read,
    /// so a key binding also needs a condition that runs no code.
    #[allow(clippy::too_many_arguments)]
    fn same_store(
        &self,
        left: ExprId,
        right: ExprId,
        condition: ExprId,
        region: RegionId,
        index: usize,
        frames: &Frames,
        captured: &[bool],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let expressions = &self.expressions;
        let (object, key) = match (&expressions[left.index()], &expressions[right.index()]) {
            (Expr::Binding(a), Expr::Binding(b)) => return Ok(a == b),
            (
                Expr::Member {
                    object: left_object,
                    property: left_property,
                },
                Expr::Member {
                    object: right_object,
                    property: right_property,
                },
            ) => {
                let (Expr::Binding(object), Expr::Binding(other)) = (
                    &expressions[left_object.index()],
                    &expressions[right_object.index()],
                ) else {
                    return Ok(false);
                };
                if object != other {
                    return Ok(false);
                }
                let key = match (left_property, right_property) {
                    (Property::Named(a), Property::Named(b)) if a == b => None,
                    (Property::Computed(a), Property::Computed(b)) => {
                        match (&expressions[a.index()], &expressions[b.index()]) {
                            (Expr::Literal(a), Expr::Literal(b))
                                if matches!(a, Literal::String(_) | Literal::Number(_))
                                    && a == b =>
                            {
                                None
                            }
                            (Expr::Binding(a), Expr::Binding(b)) if a == b => {
                                let quiet = match &expressions[condition.index()] {
                                    Expr::Binding(_) | Expr::Literal(_) => true,
                                    Expr::Unary {
                                        op: Unary::Not,
                                        value,
                                    } => matches!(expressions[value.index()], Expr::Binding(_)),
                                    _ => false,
                                };
                                if !quiet {
                                    return Ok(false);
                                }
                                Some(*a)
                            }
                            _ => return Ok(false),
                        }
                    }
                    _ => return Ok(false),
                };
                (*object, key)
            }
            _ => return Ok(false),
        };
        let read = [Some(object), key];
        let reads: Vec<BindingId> = read.into_iter().flatten().collect();
        for &binding in &reads {
            if captured[binding.index()]
                || !self.initialized_at(binding, region, index, frames, budget)?
            {
                return Ok(false);
            }
        }
        Ok(!self.statement_assigns(&Statement::Evaluate(condition), &reads))
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
