//! Statement forms, in two sets (plan M9.3, record 013-T7.2).
//!
//! **Rules that remove operations** run under every objective, since they
//! pay under every codec (architecture L3). They are Closure's
//! PeepholeMinimizeConditions exit rules and repeated-statement removal
//! (`closure-compiler@0da58e1 PeepholeMinimizeConditions.java:240-345, 722-777`)
//! and the redundant-exit half of MinimizeExitPoints
//! (`MinimizeExitPoints.java:84-94`):
//!
//! * same-exit merge: `if(c){A;return x}return x` is `if(c){A}return x`, and
//!   a `return;` (a `continue`) ending a branch at a function body's (a loop
//!   body's) end goes, since that end exits the same way;
//! * trailing-statement dedup: `if(c){A;S}else{B;S}` is `if(c){A}else{B}S`;
//! * exit to `break`: `while(c){…return x…}return x` is
//!   `while(c){…break…}return x`.
//!
//! **Spellings** re-spell the same evaluations. Each group is a family the
//! objective seeds and the terminal stage offers as a challenger
//! (`StatementSpellings`): Terser's `conditionals`, `if_return` and
//! `sequences`, which a leave-one-out on jquerylil's raw build ranks first
//! (1,452, 630 and 450 bytes), and Closure's late MinimizeExitPoints,
//! MinimizeConditions and StatementFusion:
//!
//! * `if(c){a;b}` is `c&&(a,b)`, `if(c)a;else b` is `c?a:b`, and an empty
//!   branch negates: `if(c);else b` is `c||b`. Branches hold only expression
//!   statements, so they declare nothing and leave nothing: evaluating the
//!   expression runs exactly the statements' evaluations, in order
//!   (logical branches).
//! * `if(c)x=a;else x=b` is `x=c?a:b` for one binding, and
//!   `if(c)o.k=a;else o.k=b` is `o.k=c?a:b` where reading `o` (and a key)
//!   before the condition changes nothing (`same_store`) (conditional values).
//! * `if(c)return a;e;return b` is `return c?a:(e,b)`, and so is an `if`
//!   whose branches both return: the same evaluations, then the same return
//!   (conditional returns).
//! * `while(c){…;u}` is `for(;c;u){…}` when no `continue` skips `u` (loop
//!   fusion).
//! * `if(c){A;return}R` ending a function body, or `if(c){A;continue}R`
//!   ending a loop body, is `if(c){A}else{R}` (`if(!c){R}` for an empty `A`)
//!   (exit points).
//! * `if(a){if(b)S}` is `if(a&&b)S`, and conditionals with a repeated
//!   binding or a boolean condition and branch are `&&`/`||`
//!   (`compress_conditionals`) (logical branches).
//!
//! A codec matches the statement forms' repeated shapes nearly for free
//! (Terser's compression over our Brotli output made it larger), so the
//! exact codec, not the objective, decides each group per artifact.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

impl Module {
    /// Remove redundant exits and repeated statements throughout, and
    /// rewrite statements into the spellings `spellings` selects, until
    /// nothing more applies. Returns how many rewrites.
    pub(crate) fn compress_statements(
        &mut self,
        spellings: StatementSpellings,
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
                changed += self.minimize_exits(region, &frames, budget)?;
                if spellings != StatementSpellings::NONE {
                    changed +=
                        self.compress_region(region, spellings, &frames, &reach.captured, budget)?;
                }
            }
            if spellings.logical_branches {
                changed += self.compress_conditionals(&reach, budget)?;
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
        spellings: StatementSpellings,
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
                if !spellings.loop_fusion {
                    index += 1;
                    continue;
                }
                if let Some(&Statement::Evaluate(last)) =
                    self.regions[body.index()].statements.last()
                {
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
            // `if(c){A;return}R` ending a function body is `if(c){A}else{R}`,
            // as is `if(c){A;continue}R` ending a loop body: both branches end
            // where the body does.
            if spellings.exit_points
                && no.is_none()
                && index + 1 < self.regions[region.index()].statements.len()
                && self.exits_at_end(region, yes, frames)
                && self.tail_movable(region, index, budget)?
            {
                self.regions[yes.index()].statements.pop();
                let rest = self.tail_region(region, index, budget)?;
                let statement = if self.regions[yes.index()].statements.is_empty() {
                    let negated = match self.negated(condition, budget)? {
                        Some(negated) => negated,
                        None => self.expression_in(
                            Expr::Unary {
                                op: Unary::Not,
                                value: condition,
                            },
                            None,
                            budget,
                        )?,
                    };
                    Statement::If {
                        condition: negated,
                        yes: rest,
                        no: None,
                    }
                } else {
                    Statement::If {
                        condition,
                        yes,
                        no: Some(rest),
                    }
                };
                self.regions[region.index()].statements[index] = statement;
                changed += 1;
                continue;
            }
            // `if(a){if(b)S}` is `if(a&&b)S`: `b` runs exactly when `a` holds.
            if spellings.logical_branches && no.is_none() {
                if let Some(
                    &[Statement::If {
                        condition: inner,
                        yes: inner_yes,
                        no: None,
                    }],
                ) = self.only(yes, 1)
                {
                    self.adopt(yes, region, budget)?;
                    let both = self.expression_in(
                        Expr::Binary {
                            op: Binary::And,
                            left: condition,
                            right: inner,
                        },
                        None,
                        budget,
                    )?;
                    self.regions[region.index()].statements[index] = Statement::If {
                        condition: both,
                        yes: inner_yes,
                        no: None,
                    };
                    changed += 1;
                    // The merged `if` may compress further.
                    continue;
                }
            }
            // Both branches return: one return of a conditional.
            if let (true, Some([Statement::Return(Some(a))]), Some([Statement::Return(Some(b))])) = (
                spellings.conditional_returns,
                self.only(yes, 1),
                no.and_then(|no| self.only(no, 1)),
            ) {
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
            if let (true, Some([Statement::Return(Some(a))]), None) =
                (spellings.conditional_returns, self.only(yes, 1), no)
            {
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
            let logical = spellings.logical_branches;
            let replacement = match (yes_values.is_empty(), no_values) {
                (true, None) => None,
                (false, None) if logical => {
                    let value = self.sequence(yes_values, budget)?;
                    Some(Expr::Binary {
                        op: Binary::And,
                        left: condition,
                        right: value,
                    })
                }
                (false, None) => None,
                (true, Some(no_values)) if no_values.is_empty() => None,
                (true, Some(no_values)) if logical => {
                    let value = self.sequence(no_values, budget)?;
                    Some(Expr::Binary {
                        op: Binary::Or,
                        left: condition,
                        right: value,
                    })
                }
                (true, Some(_)) => None,
                (false, Some(no_values)) if no_values.is_empty() => {
                    if logical {
                        let value = self.sequence(yes_values, budget)?;
                        Some(Expr::Binary {
                            op: Binary::And,
                            left: condition,
                            right: value,
                        })
                    } else {
                        None
                    }
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
                    let same = match (
                        spellings.conditional_values,
                        assigned(&yes_values),
                        assigned(&no_values),
                    ) {
                        (true, Some((left, a)), Some((right, b))) => self
                            .same_store(
                                left, right, condition, region, index, frames, captured, budget,
                            )?
                            .then_some((left, a, b)),
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
                        None if logical => {
                            let a = self.sequence(yes_values, budget)?;
                            let b = self.sequence(no_values, budget)?;
                            Some(Expr::Conditional {
                                condition,
                                yes: a,
                                no: b,
                            })
                        }
                        None => None,
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

    /// The rules that remove operations, on one region: same-exit merge,
    /// trailing-statement dedup and exit to `break` (module documentation).
    /// Each is exact: every path runs the same evaluations in the same order,
    /// and only a statement that a following one repeats goes. Returns how
    /// many statements went or moved.
    fn minimize_exits(
        &mut self,
        region: RegionId,
        frames: &Frames,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let end = self.end_exit(region, frames);
        let mut changed = 0;
        let mut index = 0;
        while index < self.regions[region.index()].statements.len() {
            budget.work(Analysis, 1)?;
            let statements = &self.regions[region.index()].statements;
            // What runs next: the following exit statement, or the exit the
            // region's own end takes.
            let follow = match statements.get(index + 1) {
                Some(next @ (Statement::Return(_) | Statement::Throw(_))) => Some(next.clone()),
                Some(_) => None,
                None => end.clone(),
            };
            match statements[index].clone() {
                // A body's final `return;`, or a loop body's final
                // `continue`, is where the body ends anyway.
                exit @ (Statement::Return(None) | Statement::Continue)
                    if index + 1 == statements.len() && end.as_ref() == Some(&exit) =>
                {
                    self.regions[region.index()].statements.pop();
                    if region == self.root && index < self.root_modules.len() {
                        self.root_modules.truncate(index);
                    }
                    changed += 1;
                    continue;
                }
                Statement::If { yes, no, .. } => {
                    if let Some(no) = no {
                        changed += self.hoist_repeated(region, index, yes, no, budget)?;
                    }
                    // Re-read: hoisting may have emptied both branches, and
                    // the `if` is then its condition alone.
                    if let (Some(follow), Statement::If { yes, no, .. }) = (
                        &follow,
                        self.regions[region.index()].statements[index].clone(),
                    ) {
                        let mut stripped = self.strip_exit(yes, follow, budget)?;
                        if let Some(no) = no {
                            stripped += self.strip_exit(no, follow, budget)?;
                        }
                        if stripped != 0 {
                            self.settle_if(region, index, budget)?;
                            changed += stripped;
                        }
                    }
                }
                Statement::Loop { body, .. } | Statement::ForIn { body, .. } => {
                    if let Some(follow @ (Statement::Return(_) | Statement::Throw(_))) = &follow {
                        changed += self.exits_to_break(body, follow, budget)?;
                    }
                }
                // Leaving a `for…of` closes its iterator: after a return's
                // value, but before the value read after a `break`, and a
                // throw's own completion wins over a failed close. Only a
                // return of nothing or of a literal moves.
                Statement::ForOf { body, .. } => {
                    if let Some(follow @ Statement::Return(value)) = &follow {
                        if value.is_none_or(|value| {
                            matches!(self.expressions[value.index()], Expr::Literal(_))
                        }) {
                            changed += self.exits_to_break(body, follow, budget)?;
                        }
                    }
                }
                _ => {}
            }
            index += 1;
        }
        Ok(changed)
    }

    /// The exit statement that runs when control falls off `region`'s end
    /// (Closure's `computeFollowNode`): `return;` for a function body,
    /// `continue` for a loop body, and for a block or a branch of an `if`,
    /// the exit statement right after it, or its own region's. Nothing for
    /// the root, a `try`'s regions (a finalizer runs in between) or anything
    /// else.
    fn end_exit(&self, region: RegionId, frames: &Frames) -> Option<Statement> {
        let mut region = region;
        loop {
            if frames.bodies[region.index()].is_some() {
                return Some(Statement::Return(None));
            }
            let (parent, index) = frames.parents[region.index()]?;
            let statements = &self.regions[parent.index()].statements;
            match statements.get(index)? {
                Statement::Loop { body, .. }
                | Statement::ForIn { body, .. }
                | Statement::ForOf { body, .. }
                    if *body == region =>
                {
                    return Some(Statement::Continue);
                }
                Statement::Block(inner) if *inner == region => {}
                Statement::If { yes, no, .. } if *yes == region || *no == Some(region) => {}
                _ => return None,
            }
            match statements.get(index + 1) {
                Some(next @ (Statement::Return(_) | Statement::Throw(_))) => {
                    return Some(next.clone())
                }
                Some(_) => return None,
                None => region = parent,
            }
        }
    }

    /// Same-exit merge: drop `exit` where it ends `region`, or ends a branch
    /// of an `if` ending it, and so on down, since falling out of the region
    /// reaches the same `exit` right after. Returns how many went. Iterative:
    /// an `else if` chain nests as deep as it is long.
    fn strip_exit(
        &mut self,
        region: RegionId,
        exit: &Statement,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut stripped = 0;
        // Each `if` whose branches were visited, in visiting order: settled
        // in reverse, so an inner `if` settles before the one holding it.
        let mut visited = Vec::new();
        let mut pending = vec![region];
        while let Some(region) = pending.pop() {
            budget.work(Analysis, 1)?;
            let Some(last) = self.regions[region.index()].statements.last().cloned() else {
                continue;
            };
            if self.same_statement(&last, exit, budget)? {
                self.regions[region.index()].statements.pop();
                stripped += 1;
                continue;
            }
            if let Statement::If { yes, no, .. } = last {
                let at = self.regions[region.index()].statements.len() - 1;
                visited.push((region, at));
                pending.push(yes);
                pending.extend(no);
            }
        }
        if stripped != 0 {
            for &(region, at) in visited.iter().rev() {
                self.settle_if(region, at, budget)?;
            }
        }
        Ok(stripped)
    }

    /// Trailing-statement dedup: `if(c){A;S}else{B;S}` is `if(c){A}else{B}S`
    /// for simple statements `S` (no declaration, no nested region). `S` is
    /// the same in both branches, so every binding it reads is declared
    /// outside them, and it runs at the same point on either path.
    fn hoist_repeated(
        &mut self,
        region: RegionId,
        index: usize,
        yes: RegionId,
        no: RegionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut moved = Vec::new();
        loop {
            let (Some(a), Some(b)) = (
                self.regions[yes.index()].statements.last(),
                self.regions[no.index()].statements.last(),
            ) else {
                break;
            };
            let (a, b) = (a.clone(), b.clone());
            if !self.same_statement(&a, &b, budget)? {
                break;
            }
            self.regions[yes.index()].statements.pop();
            self.regions[no.index()].statements.pop();
            moved.push(a);
        }
        if moved.is_empty() {
            return Ok(0);
        }
        let count = moved.len();
        moved.reverse();
        let at = index + 1;
        if region == self.root && index < self.root_modules.len() {
            let module = self.root_modules[index];
            self.root_modules
                .splice(at..at, std::iter::repeat_n(module, count));
        }
        self.regions[region.index()]
            .statements
            .splice(at..at, moved);
        self.settle_if(region, index, budget)?;
        Ok(count)
    }

    /// Exit to `break`: in a loop followed by `exit`, an `exit` the loop's
    /// body runs (outside any nested loop, function or `try`, whose
    /// `break` or finalizer would differ) is a `break`, which reaches the
    /// same `exit` right after the loop.
    fn exits_to_break(
        &mut self,
        body: RegionId,
        exit: &Statement,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut replaced = 0;
        let mut regions = vec![body];
        while let Some(region) = regions.pop() {
            for index in 0..self.regions[region.index()].statements.len() {
                budget.work(Analysis, 1)?;
                let statement = self.regions[region.index()].statements[index].clone();
                match statement {
                    Statement::If { yes, no, .. } => {
                        regions.push(yes);
                        regions.extend(no);
                    }
                    Statement::Block(inner) => regions.push(inner),
                    _ if self.same_statement(&statement, exit, budget)? => {
                        self.regions[region.index()].statements[index] = Statement::Break;
                        replaced += 1;
                    }
                    _ => {}
                }
            }
        }
        Ok(replaced)
    }

    /// An `if` left with an empty branch: `if(c){}` is `c`, `if(c){}else B`
    /// is `if(!c)B`, and `if(c)A;else{}` is `if(c)A`. The condition still
    /// runs exactly once.
    fn settle_if(
        &mut self,
        region: RegionId,
        index: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let Statement::If { condition, yes, no } =
            self.regions[region.index()].statements[index].clone()
        else {
            return Ok(());
        };
        let empty =
            |module: &Self, region: RegionId| module.regions[region.index()].statements.is_empty();
        let no = no.filter(|&no| !empty(self, no));
        let statement = match (empty(self, yes), no) {
            (true, None) => Statement::Evaluate(condition),
            (true, Some(no)) => {
                let negated = match self.negated(condition, budget)? {
                    Some(negated) => negated,
                    None => self.expression_in(
                        Expr::Unary {
                            op: Unary::Not,
                            value: condition,
                        },
                        None,
                        budget,
                    )?,
                };
                Statement::If {
                    condition: negated,
                    yes: no,
                    no: None,
                }
            }
            (false, no) => Statement::If { condition, yes, no },
        };
        self.regions[region.index()].statements[index] = statement;
        Ok(())
    }

    /// Two simple statements that run the same evaluations: the same kind,
    /// with structurally equal expressions.
    fn same_statement(
        &self,
        left: &Statement,
        right: &Statement,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        Ok(match (left, right) {
            (Statement::Evaluate(a), Statement::Evaluate(b))
            | (Statement::Throw(a), Statement::Throw(b))
            | (Statement::Return(Some(a)), Statement::Return(Some(b))) => {
                self.same_expression(*a, *b, budget)?
            }
            (Statement::Return(None), Statement::Return(None))
            | (Statement::Break, Statement::Break)
            | (Statement::Continue, Statement::Continue) => true,
            _ => false,
        })
    }

    /// Whether two expressions are the same tree: the same operations on the
    /// same bindings and literals (numbers by bits). One that creates a
    /// function or class, or loads a module, has its own identity and is
    /// never the same as another.
    pub(super) fn same_expression(
        &self,
        left: ExprId,
        right: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let mut pending = vec![(left, right)];
        while let Some((left, right)) = pending.pop() {
            budget.work(Analysis, 1)?;
            if left == right {
                continue;
            }
            let mut pairs = |a: &[ExprId], b: &[ExprId]| {
                let same = a.len() == b.len();
                if same {
                    pending.extend(a.iter().copied().zip(b.iter().copied()));
                }
                same
            };
            let property =
                |a: &Property,
                 b: &Property,
                 pairs: &mut dyn FnMut(&[ExprId], &[ExprId]) -> bool| match (a, b) {
                    (Property::Named(a), Property::Named(b)) => a == b,
                    (Property::Computed(a), Property::Computed(b)) => pairs(&[*a], &[*b]),
                    _ => false,
                };
            let same = match (
                &self.expressions[left.index()],
                &self.expressions[right.index()],
            ) {
                (Expr::Literal(Literal::Number(a)), Expr::Literal(Literal::Number(b))) => {
                    a.to_bits() == b.to_bits()
                }
                (Expr::Literal(a), Expr::Literal(b)) => a == b,
                (Expr::Binding(a), Expr::Binding(b)) => a == b,
                (Expr::Host(a), Expr::Host(b)) => a == b,
                (Expr::This, Expr::This) => true,
                (Expr::Regex(a), Expr::Regex(b)) => a == b,
                (
                    Expr::Unary { op, value },
                    Expr::Unary {
                        op: other,
                        value: with,
                    },
                ) => op == other && pairs(&[*value], &[*with]),
                (Expr::ToInt32(a), Expr::ToInt32(b))
                | (Expr::IntNegate(a), Expr::IntNegate(b))
                | (Expr::Spread(a), Expr::Spread(b))
                | (Expr::Await(a), Expr::Await(b)) => pairs(&[*a], &[*b]),
                (
                    Expr::Yield { value, delegate },
                    Expr::Yield {
                        value: with,
                        delegate: other,
                    },
                ) => delegate == other && pairs(&[*value], &[*with]),
                (
                    Expr::IntBinary { op, left, right },
                    Expr::IntBinary {
                        op: other,
                        left: l,
                        right: r,
                    },
                ) => op == other && pairs(&[*left, *right], &[*l, *r]),
                (
                    Expr::Binary { op, left, right },
                    Expr::Binary {
                        op: other,
                        left: l,
                        right: r,
                    },
                ) => op == other && pairs(&[*left, *right], &[*l, *r]),
                (
                    Expr::Intrinsic {
                        operation,
                        receiver,
                        arguments,
                    },
                    Expr::Intrinsic {
                        operation: other,
                        receiver: r,
                        arguments: a,
                    },
                ) => operation == other && pairs(&[*receiver], &[*r]) && pairs(arguments, a),
                (
                    Expr::ConstructIntrinsic {
                        operation,
                        arguments,
                    },
                    Expr::ConstructIntrinsic {
                        operation: other,
                        arguments: a,
                    },
                ) => operation == other && pairs(arguments, a),
                (
                    Expr::Member {
                        object,
                        property: p,
                    },
                    Expr::Member {
                        object: o,
                        property: q,
                    },
                ) => pairs(&[*object], &[*o]) && property(p, q, &mut pairs),
                (
                    Expr::Call {
                        callee,
                        arguments,
                        invocation,
                    },
                    Expr::Call {
                        callee: c,
                        arguments: a,
                        invocation: other,
                    },
                ) => invocation == other && pairs(&[*callee], &[*c]) && pairs(arguments, a),
                (
                    Expr::Construct { callee, arguments },
                    Expr::Construct {
                        callee: c,
                        arguments: a,
                    },
                ) => pairs(&[*callee], &[*c]) && pairs(arguments, a),
                (
                    Expr::Conditional { condition, yes, no },
                    Expr::Conditional {
                        condition: c,
                        yes: y,
                        no: n,
                    },
                ) => pairs(&[*condition, *yes, *no], &[*c, *y, *n]),
                (
                    Expr::Assign { target, value },
                    Expr::Assign {
                        target: t,
                        value: v,
                    },
                ) => pairs(&[*target, *value], &[*t, *v]),
                (Expr::Sequence(a), Expr::Sequence(b)) | (Expr::Array(a), Expr::Array(b)) => {
                    pairs(a, b)
                }
                (Expr::SuperCall { arguments }, Expr::SuperCall { arguments: a }) => {
                    pairs(arguments, a)
                }
                (Expr::Object(a), Expr::Object(b)) => {
                    a.len() == b.len()
                        && a.iter().zip(b).all(|((p, x), (q, y))| {
                            property(p, q, &mut pairs) && pairs(&[*x], &[*y])
                        })
                }
                (Expr::Template(a), Expr::Template(b)) => {
                    a.len() == b.len()
                        && a.iter().zip(b).all(|parts| match parts {
                            (TemplatePart::String(x), TemplatePart::String(y)) => x == y,
                            (TemplatePart::Expression(x), TemplatePart::Expression(y)) => {
                                pairs(&[*x], &[*y])
                            }
                            _ => false,
                        })
                }
                _ => false,
            };
            if !same {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Conditionals that say less: `x?x:y` is `x||y` and `x?y:x` is `x&&y`
    /// for a binding `x`. With a boolean condition `c`, `c?y:!1` is `c&&y`
    /// and `c?!0:y` is `c||y`; `c?!1:y` and `c?y:!0` negate `c` in place
    /// (an equality flips, `!d` drops its `!` when `d` is boolean) and are
    /// `c'&&y` and `c'||y`. Each keeps the conditional's node, so its parent
    /// still follows its children.
    fn compress_conditionals(
        &mut self,
        reach: &super::inline::Reach,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut changed = 0;
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Conditional { condition, yes, no } = self.expressions[id.index()] else {
                continue;
            };
            let same = |a: ExprId, b: ExprId| {
                matches!(
                    (&self.expressions[a.index()], &self.expressions[b.index()]),
                    (Expr::Binding(x), Expr::Binding(y)) if x == y
                )
            };
            let flag = |value: ExprId| match self.expressions[value.index()] {
                Expr::Literal(Literal::Bool(flag)) => Some(flag),
                _ => None,
            };
            let (op, left, right) = if same(condition, yes) {
                (Binary::Or, condition, no)
            } else if same(condition, no) {
                (Binary::And, condition, yes)
            } else if !self.boolean_valued(condition, budget)? {
                continue;
            } else {
                match (flag(yes), flag(no)) {
                    (_, Some(false)) => (Binary::And, condition, yes),
                    (Some(true), _) => (Binary::Or, condition, no),
                    (Some(false), _) => match self.negated(condition, budget)? {
                        Some(negated) => (Binary::And, negated, no),
                        None => continue,
                    },
                    (_, Some(true)) => match self.negated(condition, budget)? {
                        Some(negated) => (Binary::Or, negated, yes),
                        None => continue,
                    },
                    _ => continue,
                }
            };
            self.expressions[id.index()] = Expr::Binary { op, left, right };
            changed += 1;
        }
        Ok(changed)
    }

    /// Whether `value` is always `true` or `false`.
    fn boolean_valued(
        &self,
        value: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(Analysis, 1)?;
        Ok(match &self.expressions[value.index()] {
            Expr::Literal(Literal::Bool(_)) => true,
            Expr::Unary { op: Unary::Not, .. } => true,
            Expr::Binary { op, left, right } => match op {
                Binary::Equal
                | Binary::NotEqual
                | Binary::StrictEqual
                | Binary::StrictNotEqual
                | Binary::Less
                | Binary::LessEqual
                | Binary::Greater
                | Binary::GreaterEqual
                | Binary::In
                | Binary::InstanceOf => true,
                Binary::And | Binary::Or => {
                    self.boolean_valued(*left, budget)? && self.boolean_valued(*right, budget)?
                }
                _ => false,
            },
            Expr::Conditional { yes, no, .. } => {
                self.boolean_valued(*yes, budget)? && self.boolean_valued(*no, budget)?
            }
            _ => false,
        })
    }

    /// The boolean `condition` negated without a new node: an equality flips
    /// in place, and `!d` is `d` when `d` is boolean. An ordering does not
    /// flip, since `NaN` makes both `a<b` and `a>=b` false.
    fn negated(
        &mut self,
        condition: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<ExprId>, AllocationError> {
        let flipped = match &self.expressions[condition.index()] {
            Expr::Unary {
                op: Unary::Not,
                value,
            } => {
                let value = *value;
                return Ok(self.boolean_valued(value, budget)?.then_some(value));
            }
            Expr::Literal(Literal::Bool(flag)) => Expr::Literal(Literal::Bool(!flag)),
            Expr::Binary { op, left, right } => {
                let op = match op {
                    Binary::Equal => Binary::NotEqual,
                    Binary::NotEqual => Binary::Equal,
                    Binary::StrictEqual => Binary::StrictNotEqual,
                    Binary::StrictNotEqual => Binary::StrictEqual,
                    _ => return Ok(None),
                };
                Expr::Binary {
                    op,
                    left: *left,
                    right: *right,
                }
            }
            _ => return Ok(None),
        };
        self.expressions[condition.index()] = flipped;
        Ok(Some(condition))
    }

    /// Whether `yes` ends by leaving `region` the way `region`'s own end
    /// would: `return;` in a function body, `continue` in a loop body.
    fn exits_at_end(&self, region: RegionId, yes: RegionId, frames: &Frames) -> bool {
        match self.regions[yes.index()].statements.last() {
            Some(Statement::Return(None)) => frames.bodies[region.index()].is_some(),
            Some(Statement::Continue) => {
                let Some((parent, _)) = frames.parents[region.index()] else {
                    return false;
                };
                self.regions[parent.index()]
                    .statements
                    .iter()
                    .any(|statement| match statement {
                        Statement::Loop { body, .. }
                        | Statement::ForIn { body, .. }
                        | Statement::ForOf { body, .. } => *body == region,
                        _ => false,
                    })
            }
            _ => false,
        }
    }

    /// Whether the statements after `index` can move into a block of their
    /// own: none is a function declaration (whose hoisting a block would
    /// change), and no code up to `index` names a binding they declare.
    fn tail_movable(
        &self,
        region: RegionId,
        index: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let statements = &self.regions[region.index()].statements;
        let mut declared = Vec::new();
        for statement in &statements[index + 1..] {
            match statement {
                Statement::Function { .. } => return Ok(false),
                Statement::Let { binding, .. } => declared.push(*binding),
                _ => {}
            }
        }
        if declared.is_empty() {
            return Ok(true);
        }
        let mut expressions = Vec::new();
        let mut regions = Vec::new();
        for statement in &statements[..=index] {
            statement.visit_expressions(|root| expressions.push(root));
            statement.visit_regions(|child| regions.push(child));
            if let Statement::Function { function, .. } = statement {
                regions.push(self.functions[function.index()].body);
            }
        }
        loop {
            if let Some(id) = expressions.pop() {
                budget.work(Analysis, 1)?;
                let expression = &self.expressions[id.index()];
                if let Expr::Binding(binding) = expression {
                    if declared.contains(binding) {
                        return Ok(false);
                    }
                }
                if let Some(function) = expression.created_function() {
                    regions.push(self.functions[function.index()].body);
                }
                let _ = expression.visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, ()>(())
                });
                continue;
            }
            let Some(inner) = regions.pop() else {
                return Ok(true);
            };
            for statement in &self.regions[inner.index()].statements {
                budget.work(Analysis, 1)?;
                statement.visit_expressions(|root| expressions.push(root));
                statement.visit_regions(|child| regions.push(child));
                if let Statement::Function { function, .. } = statement {
                    regions.push(self.functions[function.index()].body);
                }
            }
        }
    }

    /// Move the statements after `index` into a new region nested in
    /// `region`'s scope. Its declarations move with it, and `rescope` gives
    /// the region and all it holds fresh scopes, so each parent still
    /// precedes its children.
    fn tail_region(
        &mut self,
        region: RegionId,
        index: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<RegionId, AllocationError> {
        let rest: Vec<Statement> = self.regions[region.index()]
            .statements
            .drain(index + 1..)
            .collect();
        let parent = self.regions[region.index()].scope;
        let tail = self.region_in(parent, budget)?;
        let scope = self.regions[tail.index()].scope;
        for statement in &rest {
            budget.work(Analysis, 1)?;
            if let Statement::Let { binding, .. } = statement {
                self.bindings[binding.index()].scope = scope;
            }
        }
        self.regions[tail.index()].statements = rest;
        self.rescope(tail, parent, budget)?;
        Ok(tail)
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

impl Module {
    /// `let t=A;if(t)t=B` is `let t=A&&B`, `if(!t)t=B` makes it `A||B`, and
    /// `if(t==null)t=B` makes it `A??B` (with `nullish`, an ES2020 target):
    /// JavaScript's value-returning operators, which a port spells as a
    /// temporary and a test. `A` still runs once and first, `B` runs exactly
    /// when the test passes, and `t` ends with the same value. `B` must not
    /// name `t`: it would read `t` in its TDZ, or its old value. The same
    /// holds after an assignment `t=A;`. Returns how many.
    pub(crate) fn fold_logical_assignments(
        &mut self,
        nullish: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut folded = 0;
        for region in 0..self.regions.len() {
            let root = region == self.root.index();
            let mut index = 0;
            while index + 1 < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                // The temporary and its first value.
                let first = match self.regions[region].statements[index] {
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => Some((binding, value, None)),
                    Statement::Evaluate(root_expression) => {
                        match self.expressions[root_expression.index()] {
                            Expr::Assign { target, value } => {
                                match self.expressions[target.index()] {
                                    Expr::Binding(binding) => {
                                        Some((binding, value, Some(root_expression)))
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let Some((binding, first_value, assignment)) = first else {
                    index += 1;
                    continue;
                };
                let Statement::If {
                    condition,
                    yes,
                    no: None,
                } = self.regions[region].statements[index + 1]
                else {
                    index += 1;
                    continue;
                };
                let op = self.logical_test(condition, binding, nullish);
                let second = match (op, &self.regions[yes.index()].statements[..]) {
                    (Some(op), [Statement::Evaluate(value)]) => match self.expressions
                        [value.index()]
                    {
                        Expr::Assign { target, value } => match self.expressions[target.index()] {
                            Expr::Binding(assigned) if assigned == binding => Some((op, value)),
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                let same_module =
                    !root || self.root_modules.get(index) == self.root_modules.get(index + 1);
                let Some((op, second_value)) = second.filter(|_| same_module) else {
                    index += 1;
                    continue;
                };
                budget.work(Analysis, 1)?;
                if self.mentions(&[], &[second_value], binding, false) {
                    index += 1;
                    continue;
                }
                let combined = self.expression_in(
                    Expr::Binary {
                        op,
                        left: first_value,
                        right: second_value,
                    },
                    None,
                    budget,
                )?;
                // The branch's scopes (functions `B` creates) nest here now.
                self.adopt(yes, RegionId::new(region), budget)?;
                match assignment {
                    None => {
                        self.regions[region].statements[index] = Statement::Let {
                            binding,
                            value: Some(combined),
                        };
                    }
                    Some(root_expression) => {
                        let target = self.expression_in(Expr::Binding(binding), None, budget)?;
                        let assigned = self.expression_in(
                            Expr::Assign {
                                target,
                                value: combined,
                            },
                            None,
                            budget,
                        )?;
                        let _ = root_expression;
                        self.regions[region].statements[index] = Statement::Evaluate(assigned);
                    }
                }
                self.regions[region].statements.remove(index + 1);
                if root && index + 1 < self.root_modules.len() {
                    self.root_modules.remove(index + 1);
                }
                folded += 1;
                // The folded statement may meet another test.
            }
        }
        Ok(folded)
    }

    /// The operator a test of `binding` alone stands for: `t` (or `!!t`) is
    /// `&&`, `!t` is `||`, `t==null` (or `null==t`, `t==void 0`) is `??`.
    /// `if(t)return t;return B` is `return t||B`, and likewise with the arms
    /// or the test the other way round, `&&` for the dual, and `??` for a
    /// loose test against `null` (with `nullish`, an ES2020 target): each
    /// operator returns the operand that decides it, which is what the arm
    /// taken returns. `t` is a binding, read once or twice alike; the other
    /// value runs exactly when its return would. The second return may be
    /// the `if`'s `else` or the statement after it. Returns how many.
    pub(crate) fn fold_logical_returns(
        &mut self,
        nullish: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut folded = 0;
        for region in 0..self.regions.len() {
            if region == self.root.index() {
                continue;
            }
            let mut index = 0;
            while index < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                let Statement::If { condition, yes, no } = self.regions[region].statements[index]
                else {
                    index += 1;
                    continue;
                };
                let returned = |module: &Self, arm: RegionId| match module.regions[arm.index()]
                    .statements[..]
                {
                    [Statement::Return(Some(value))] => Some(value),
                    _ => None,
                };
                let taken = returned(self, yes);
                let other = match no {
                    Some(no) => returned(self, no).map(|value| (value, false)),
                    None => match self.regions[region].statements.get(index + 1) {
                        Some(Statement::Return(Some(value))) => Some((*value, true)),
                        _ => None,
                    },
                };
                let (Some(a), Some((b, next))) = (taken, other) else {
                    index += 1;
                    continue;
                };
                let binding_of = |id: ExprId| match self.expressions[id.index()] {
                    Expr::Binding(binding) => Some(binding),
                    _ => None,
                };
                let mut fold = None;
                for tested in [binding_of(a), binding_of(b)].into_iter().flatten() {
                    let Some((truthiness, positive)) =
                        self.test_polarity(condition, tested, nullish)
                    else {
                        continue;
                    };
                    let first = binding_of(a) == Some(tested);
                    // `positive`: the `if` arm runs when `t` is truthy (or
                    // not nullish).
                    fold = match (truthiness, positive, first) {
                        (true, true, true) => Some((Binary::Or, a, b)),
                        (true, true, false) => Some((Binary::And, b, a)),
                        (true, false, true) => Some((Binary::And, a, b)),
                        (true, false, false) => Some((Binary::Or, b, a)),
                        (false, true, true) => Some((Binary::Nullish, a, b)),
                        (false, false, false) => Some((Binary::Nullish, b, a)),
                        _ => None,
                    };
                    if fold.is_some() {
                        break;
                    }
                }
                let Some((op, left, right)) = fold else {
                    index += 1;
                    continue;
                };
                let combined =
                    self.expression_in(Expr::Binary { op, left, right }, None, budget)?;
                // The arms' scopes (functions their values create) nest here.
                self.adopt(yes, RegionId::new(region), budget)?;
                if let Some(no) = no {
                    self.adopt(no, RegionId::new(region), budget)?;
                }
                self.regions[region].statements[index] = Statement::Return(Some(combined));
                if next {
                    self.regions[region].statements.remove(index + 1);
                }
                folded += 1;
                index += 1;
            }
        }
        Ok(folded)
    }

    /// How a condition tests `binding`: `(true, positive)` for its
    /// truthiness, `(false, positive)` for a loose comparison with `null`
    /// (with `nullish`), where `positive` means the condition holds when the
    /// binding is truthy (or not nullish). Each `!` flips it.
    fn test_polarity(
        &self,
        condition: ExprId,
        binding: BindingId,
        nullish: bool,
    ) -> Option<(bool, bool)> {
        let is_binding = |id: ExprId| matches!(self.expressions[id.index()], Expr::Binding(found) if found == binding);
        let mut positive = true;
        let mut tested = condition;
        while let Expr::Unary {
            op: Unary::Not,
            value,
        } = self.expressions[tested.index()]
        {
            positive = !positive;
            tested = value;
        }
        if is_binding(tested) {
            return Some((true, positive));
        }
        let Expr::Binary { op, left, right } = &self.expressions[tested.index()] else {
            return None;
        };
        let loose = match op {
            Binary::Equal => false,
            Binary::NotEqual => true,
            _ => return None,
        };
        let nothing = |id: ExprId| {
            matches!(
                self.expressions[id.index()],
                Expr::Literal(Literal::Null | Literal::Undefined)
            )
        };
        (nullish
            && ((is_binding(*left) && nothing(*right)) || (nothing(*left) && is_binding(*right))))
        .then_some((false, positive == loose))
    }

    fn logical_test(&self, condition: ExprId, binding: BindingId, nullish: bool) -> Option<Binary> {
        let is_binding = |id: ExprId| matches!(self.expressions[id.index()], Expr::Binding(found) if found == binding);
        // A test reads truthiness: each `!` flips it.
        let mut negations = 0;
        let mut tested = condition;
        while let Expr::Unary {
            op: Unary::Not,
            value,
        } = self.expressions[tested.index()]
        {
            negations += 1;
            tested = value;
        }
        if is_binding(tested) {
            return Some(if negations % 2 == 0 {
                Binary::And
            } else {
                Binary::Or
            });
        }
        match &self.expressions[condition.index()] {
            Expr::Binary {
                op: Binary::Equal,
                left,
                right,
            } if nullish => {
                let nothing = |id: ExprId| {
                    matches!(
                        self.expressions[id.index()],
                        Expr::Literal(Literal::Null | Literal::Undefined)
                    )
                };
                ((is_binding(*left) && nothing(*right)) || (nothing(*left) && is_binding(*right)))
                    .then_some(Binary::Nullish)
            }
            _ => None,
        }
    }
}
