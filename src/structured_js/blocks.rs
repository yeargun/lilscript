//! Block structure on the finished target tree.
//!
//! * A block nested directly in another region adds only braces: its
//!   statements run once, in order, each time the enclosing region reaches it,
//!   exactly as they would inline. Its declarations move to the enclosing
//!   scope; references name bindings, not spellings, so naming keeps every
//!   spelling distinct there. A block declaring a function stays (a
//!   declaration hoists to its block's top), as does one declaring a pinned
//!   spelling (it could collide), and root statements keep their per-module
//!   bookkeeping.
use super::*;

impl Module {
    /// Splice nested blocks into their enclosing region. Returns how many.
    pub(crate) fn flatten_blocks(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut flattened = 0;
        for region in 0..self.regions.len() {
            if region == self.root.index() {
                continue;
            }
            let mut index = 0;
            while index < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                let Statement::Block(inner) = self.regions[region].statements[index] else {
                    index += 1;
                    continue;
                };
                if inner.index() == region || !self.flattenable(inner) {
                    index += 1;
                    continue;
                }
                let outer_scope = self.regions[region].scope;
                let inner_scope = self.regions[inner.index()].scope;
                budget.work(Analysis, (self.bindings.len() + self.scopes.len()) as u64)?;
                for binding in &mut self.bindings {
                    if binding.scope == inner_scope {
                        binding.scope = outer_scope;
                    }
                }
                for parent in self.scopes.iter_mut().flatten() {
                    if *parent == inner_scope {
                        *parent = outer_scope;
                    }
                }
                let moved = std::mem::take(&mut self.regions[inner.index()].statements);
                let count = moved.len();
                self.regions[region].statements.splice(index..=index, moved);
                flattened += 1;
                // The spliced statements may hold blocks of their own.
                if count == 0 {
                    continue;
                }
            }
        }
        Ok(flattened)
    }

    fn flattenable(&self, inner: RegionId) -> bool {
        // `{let i=v;for(;c;u)b}` prints as `for(let i=v;c;u)b`: keep it.
        if matches!(
            self.regions[inner.index()].statements.as_slice(),
            [
                Statement::Let { value: Some(_), .. },
                Statement::Loop { .. }
            ]
        ) {
            return false;
        }
        self.regions[inner.index()]
            .statements
            .iter()
            .all(|statement| match statement {
                Statement::Function { .. } => false,
                Statement::Let { binding, .. } => !self.bindings[binding.index()].pinned,
                _ => true,
            })
    }
}

/// Where a single-use call stands, and what its result feeds.
#[derive(Clone, Copy)]
enum Site {
    /// `f(a);`
    Discard,
    /// `let x=f(a);`
    Declare(BindingId),
    /// `x=f(a);`
    Assign(BindingId),
    /// `return f(a);`
    Return,
}

impl Module {
    /// Block inlining of functions with one call. `let f=(p)=>{…}` called
    /// once, as `f(a);`, `let x=f(a);`, `x=f(a);` or `return f(a);`, becomes
    /// `{let p=a;…}` at the call: the arguments evaluate once, in order,
    /// before the body, as a call evaluates them. The body's returns stay
    /// returns for `return f(a)`; for the other sites each is in tail
    /// position and stores its value, so the block ends right after it. An
    /// early return would need a one-iteration loop to leave
    /// (`for(;;){…x=v;break…}`), which costs more than the call it replaces:
    /// such functions stay. The body keeps no frame a strict caller could
    /// see, reads no `this`, `arguments` or `super` of its own, and does not
    /// suspend; its scopes are renewed under the call's. The declaring
    /// region's scope encloses the call, and no call can run before the
    /// declaration: the call stands in a statement after it that is not a
    /// hoisted function. Returns how many calls were inlined.
    pub(crate) fn inline_single_calls(
        &mut self,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        if !strict {
            return Ok(0);
        }
        let mut inlined = 0;
        // One call per round: each edit moves statements and scopes.
        for _ in 0..256 {
            let Some(found) = self.single_call_candidate(budget)? else {
                break;
            };
            self.inline_single_call(found, budget)?;
            inlined += 1;
        }
        Ok(inlined)
    }

    #[allow(clippy::type_complexity)]
    fn single_call_candidate(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<
        Option<(
            RegionId,
            usize,
            FunctionId,
            RegionId,
            usize,
            Site,
            Vec<ExprId>,
        )>,
        AllocationError,
    > {
        use crate::compilation_policy::WorkKind::Analysis;
        let reach = self.reach(budget)?;
        // Every reference counts, a statement's own root included; the one
        // allowed is the single call.
        let mut calls = vec![0usize; self.bindings.len()];
        let mut uses = vec![0usize; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            match &self.expressions[id.index()] {
                Expr::Binding(binding) => uses[binding.index()] += 1,
                Expr::Call {
                    callee,
                    invocation: Invocation::Value | Invocation::Reference,
                    ..
                } => {
                    if let Expr::Binding(binding) = self.expressions[callee.index()] {
                        calls[binding.index()] += 1;
                    }
                }
                _ => {}
            }
        }
        for export in &self.exports {
            uses[export.binding.index()] += 1;
        }
        // Each region's parent statement: (region, index), or a function
        // created at that statement.
        let parents = self.region_parents(budget)?;
        let mut depths = None;
        for &declaring in &reach.regions {
            for (at, statement) in self.regions[declaring.index()]
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
                let Expr::Function(function) = self.expressions[value.index()] else {
                    continue;
                };
                if calls[binding.index()] != 1
                    || uses[binding.index()] != 1
                    || self.bindings[binding.index()].pinned
                    || !self.block_inlinable(function)
                {
                    continue;
                }
                let Some((region, index, site, arguments)) =
                    self.single_call_site(&reach, binding, function)
                else {
                    continue;
                };
                // The call runs only after the declaration: the declaring
                // region's statement holding it follows the declaration and
                // hoists nothing. A call inside the function's own body is
                // its recursion, never a place for the body.
                let own = self.functions[function.index()].body;
                let mut path = (region, index);
                let mut encloses = false;
                for _ in 0..verify::MAX_NESTING * 4 {
                    if path.0 == own {
                        break;
                    }
                    if path.0 == declaring {
                        encloses = path.1 > at
                            && !matches!(
                                self.regions[declaring.index()].statements[path.1],
                                Statement::Function { .. }
                            );
                        break;
                    }
                    let Some(parent) = parents[path.0.index()] else {
                        break;
                    };
                    path = parent;
                }
                if !encloses {
                    continue;
                }
                // Returns other than `return f(a)`'s must be in tail position.
                // `x=f(a)` stores `undefined` when the body ends without one,
                // so there the body must not reach its end.
                if !matches!(site, Site::Return) {
                    match self.tail_returns(own) {
                        Some(true) if matches!(site, Site::Assign(_)) => continue,
                        Some(_) => {}
                        None => continue,
                    }
                }
                // `let x=f(a)` reads no `x` before it exists: inlined, the
                // body and arguments would see `undefined` where the call
                // throws.
                if let Site::Declare(declared) = site {
                    if self.mentions(&[own], &arguments, declared, false) {
                        continue;
                    }
                }
                // The body must fit under the call within the nesting limit.
                if depths.is_none() {
                    depths = Some(self.region_depths(budget)?);
                }
                let Some(Some(depth)) = depths.as_ref().map(|depths| depths[region.index()]) else {
                    continue;
                };
                if depth + 3 + self.region_subtree_depth(own) > verify::MAX_NESTING {
                    continue;
                }
                return Ok(Some((
                    declaring, at, function, region, index, site, arguments,
                )));
            }
        }
        Ok(None)
    }

    /// A function whose body may stand in its caller's place.
    fn block_inlinable(&self, function: FunctionId) -> bool {
        let declared = &self.functions[function.index()];
        if declared.suspension != Suspension::None || declared.length.is_some() || declared.strict {
            return false;
        }
        // No own `this`/`arguments`/`super`: an arrow's are its creator's,
        // which the caller does not share; a function's are the call's.
        let mut expressions = Vec::new();
        let mut regions = vec![declared.body];
        while let Some(region) = regions.pop() {
            for statement in &self.regions[region.index()].statements {
                statement.visit_expressions(|root| expressions.push(root));
                if !matches!(statement, Statement::Function { .. }) {
                    statement.visit_regions(|child| regions.push(child));
                }
            }
            while let Some(id) = expressions.pop() {
                match &self.expressions[id.index()] {
                    Expr::This | Expr::SuperCall { .. } => return false,
                    Expr::Host(name) if name == "arguments" || name == "eval" => return false,
                    // An arrow shares the body's `this`; a function has its own.
                    Expr::Function(inner) if self.functions[inner.index()].arrow => {
                        regions.push(self.functions[inner.index()].body)
                    }
                    _ => {}
                }
                let _ = self.expressions[id.index()].visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, ()>(())
                });
            }
        }
        true
    }

    /// The one call of `binding`, when it stands as a whole statement form.
    fn single_call_site(
        &self,
        reach: &super::inline::Reach,
        binding: BindingId,
        function: FunctionId,
    ) -> Option<(RegionId, usize, Site, Vec<ExprId>)> {
        let parameters = self.functions[function.index()].parameters.len();
        let body = self.functions[function.index()].body;
        let call = |id: ExprId| match &self.expressions[id.index()] {
            Expr::Call {
                callee,
                arguments,
                invocation: Invocation::Value | Invocation::Reference,
            } if matches!(self.expressions[callee.index()], Expr::Binding(found) if found == binding)
                && arguments.len() == parameters
                && !arguments.iter().any(|argument| {
                    matches!(self.expressions[argument.index()], Expr::Spread(_))
                }) =>
            {
                Some(arguments.clone())
            }
            _ => None,
        };
        for &region in &reach.regions {
            if region == body {
                continue;
            }
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                let found = match *statement {
                    Statement::Evaluate(value) => {
                        let value = match self.expressions[value.index()] {
                            Expr::Unary {
                                op: Unary::Void,
                                value,
                            } => value,
                            _ => value,
                        };
                        call(value)
                            .map(|arguments| (Site::Discard, arguments))
                            .or_else(|| match &self.expressions[value.index()] {
                                Expr::Assign { target, value } => {
                                    match self.expressions[target.index()] {
                                        Expr::Binding(target) => call(*value)
                                            .map(|arguments| (Site::Assign(target), arguments)),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            })
                    }
                    Statement::Let {
                        binding: declared,
                        value: Some(value),
                    } => call(value).map(|arguments| (Site::Declare(declared), arguments)),
                    Statement::Return(Some(value)) => {
                        call(value).map(|arguments| (Site::Return, arguments))
                    }
                    _ => None,
                };
                if let Some((site, arguments)) = found {
                    return Some((region, index, site, arguments));
                }
            }
        }
        None
    }

    /// For every region, the statement that holds it: its region and index.
    /// A function body's parent is the statement creating the function.
    fn region_parents(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<Option<(RegionId, usize)>>, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut parents = vec![None; self.regions.len()];
        let mut regions = vec![self.root];
        let mut seen = vec![false; self.regions.len()];
        while let Some(region) = regions.pop() {
            if std::mem::replace(&mut seen[region.index()], true) {
                continue;
            }
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                let mut children = Vec::new();
                statement.visit_regions(|child| children.push(child));
                if let Statement::Function { function, .. } = statement {
                    children.push(self.functions[function.index()].body);
                }
                let mut expressions = Vec::new();
                statement.visit_expressions(|root| expressions.push(root));
                while let Some(id) = expressions.pop() {
                    let expression = &self.expressions[id.index()];
                    if let Some(function) = expression.created_function() {
                        children.push(self.functions[function.index()].body);
                    }
                    let _ = expression.visit_children(|child| {
                        expressions.push(child);
                        Ok::<_, ()>(())
                    });
                }
                for child in children {
                    parents[child.index()].get_or_insert((region, index));
                    regions.push(child);
                }
            }
        }
        Ok(parents)
    }

    /// How deep the regions and expressions under `region` reach below it.
    fn region_subtree_depth(&self, region: RegionId) -> usize {
        let mut deepest = 0;
        let mut regions = vec![(region, 0usize)];
        let mut expressions: Vec<(ExprId, usize)> = Vec::new();
        loop {
            if let Some((id, at)) = expressions.pop() {
                deepest = deepest.max(at);
                let expression = &self.expressions[id.index()];
                if let Some(function) = expression.created_function() {
                    regions.push((self.functions[function.index()].body, at + 2));
                }
                let _ = expression.visit_children(|child| {
                    expressions.push((child, at + 1));
                    Ok::<_, ()>(())
                });
                continue;
            }
            let Some((region, depth)) = regions.pop() else {
                return deepest;
            };
            deepest = deepest.max(depth);
            for statement in &self.regions[region.index()].statements {
                statement.visit_expressions(|root| expressions.push((root, depth + 1)));
                statement.visit_regions(|child| regions.push((child, depth + 1)));
                if let Statement::Function { function, .. } = statement {
                    regions.push((self.functions[function.index()].body, depth + 2));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn inline_single_call(
        &mut self,
        (declaring, at, function, region, index, site, arguments): (
            RegionId,
            usize,
            FunctionId,
            RegionId,
            usize,
            Site,
            Vec<ExprId>,
        ),
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let body = self.functions[function.index()].body;
        let parameters = self.functions[function.index()].parameters.clone();
        let falls = self.tail_returns(body) != Some(false);
        if !matches!(site, Site::Return) {
            self.store_returns(body, site, budget)?;
        }
        let mut statements: Vec<Statement> = parameters
            .iter()
            .zip(&arguments)
            .map(|(&parameter, &argument)| Statement::Let {
                binding: parameter,
                value: Some(argument),
            })
            .collect();
        statements.append(&mut self.regions[body.index()].statements);
        // `return f(a)` returns `undefined` from a body that ends.
        if matches!(site, Site::Return) && falls {
            statements.push(Statement::Return(None));
        }
        self.regions[body.index()].statements = statements;
        // Renew the scopes of everything now under the body, the arguments'
        // functions included, under the call's scope, parents first.
        self.rescope(body, self.regions[region.index()].scope, budget)?;
        let block = Statement::Block(body);
        let root = region == self.root;
        let replacement = match site {
            Site::Declare(declared) => vec![
                Statement::Let {
                    binding: declared,
                    value: None,
                },
                block,
            ],
            _ => vec![block],
        };
        let added = replacement.len() - 1;
        self.regions[region.index()]
            .statements
            .splice(index..=index, replacement);
        if root && index < self.root_modules.len() {
            let module = self.root_modules[index];
            for _ in 0..added {
                self.root_modules.insert(index, module);
            }
        }
        // The declaration goes: nothing else reads the function.
        let at = if declaring == region && index < at {
            at + added
        } else {
            at
        };
        self.regions[declaring.index()].statements.remove(at);
        if declaring == self.root && at < self.root_modules.len() {
            self.root_modules.remove(at);
        }
        Ok(())
    }

    /// `Some(reaches_end)` when every return of `body` (outside nested
    /// functions) is in tail position: the region's last statement, or in
    /// tail position within its last `if` or block. `reaches_end` says
    /// whether some path can reach the end without returning or throwing.
    fn tail_returns(&self, body: RegionId) -> Option<bool> {
        let mut returns = 0usize;
        let mut regions = vec![body];
        while let Some(region) = regions.pop() {
            for statement in &self.regions[region.index()].statements {
                match statement {
                    Statement::Return(_) => returns += 1,
                    Statement::Function { .. } => {}
                    other => other.visit_regions(|child| regions.push(child)),
                }
            }
        }
        let (tail, reaches_end) = self.tail(body);
        (tail == returns).then_some(reaches_end)
    }

    /// Returns in tail position under `region`, and whether its end is
    /// reachable without one.
    fn tail(&self, region: RegionId) -> (usize, bool) {
        match self.regions[region.index()].statements.last() {
            None => (0, true),
            Some(Statement::Return(_)) => (1, false),
            Some(Statement::Throw(_)) => (0, false),
            Some(Statement::If { yes, no, .. }) => {
                let (yes, yes_ends) = self.tail(*yes);
                let (no, no_ends) = no.map_or((0, true), |no| self.tail(no));
                (yes + no, yes_ends || no_ends)
            }
            Some(Statement::Block(inner)) => self.tail(*inner),
            Some(_) => (0, true),
        }
    }

    /// Every return of the body, each in tail position, stores its value for
    /// `site`: `x=v` for a declaration or an assignment, `v` for a discarded
    /// result.
    fn store_returns(
        &mut self,
        body: RegionId,
        site: Site,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let origin = None;
        let mut regions = vec![body];
        while let Some(region) = regions.pop() {
            let mut index = 0;
            while index < self.regions[region.index()].statements.len() {
                let statement = self.regions[region.index()].statements[index].clone();
                match statement {
                    Statement::Return(value) => {
                        let replacement = match (site, value) {
                            (Site::Declare(target) | Site::Assign(target), value) => {
                                let value = match value {
                                    Some(value) => value,
                                    None => self.expression_in(
                                        Expr::Literal(Literal::Undefined),
                                        origin,
                                        budget,
                                    )?,
                                };
                                let target =
                                    self.expression_in(Expr::Binding(target), origin, budget)?;
                                let assign = self.expression_in(
                                    Expr::Assign { target, value },
                                    origin,
                                    budget,
                                )?;
                                vec![Statement::Evaluate(assign)]
                            }
                            (_, Some(value)) => vec![Statement::Evaluate(value)],
                            (_, None) => vec![],
                        };
                        let count = replacement.len();
                        self.regions[region.index()]
                            .statements
                            .splice(index..=index, replacement);
                        index += count;
                    }
                    Statement::Function { .. } => index += 1,
                    other => {
                        other.visit_regions(|child| regions.push(child));
                        index += 1;
                    }
                }
            }
        }
        Ok(())
    }

    /// Give every region under `top` a fresh scope in preorder, `top`'s
    /// parented to `parent`, and move each binding to its region's new scope.
    fn rescope(
        &mut self,
        top: RegionId,
        parent: ScopeId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut map: Vec<Option<ScopeId>> = vec![None; self.scopes.len()];
        let mut pending = vec![(top, parent)];
        while let Some((region, parent)) = pending.pop() {
            budget.work(Analysis, 1)?;
            let old = self.regions[region.index()].scope;
            let fresh = ScopeId::try_new(self.scopes.len()).ok_or(AllocationError::Capacity)?;
            budget.reserve_vec(AllocationClass::Retained, &mut self.scopes, 1)?;
            self.scopes.push(Some(parent));
            if old.index() < map.len() {
                map[old.index()] = Some(fresh);
            }
            self.regions[region.index()].scope = fresh;
            let mut children = Vec::new();
            for statement in &self.regions[region.index()].statements {
                statement.visit_regions(|child| children.push(child));
                if let Statement::Function { function, .. } = statement {
                    children.push(self.functions[function.index()].body);
                }
                let mut expressions = Vec::new();
                statement.visit_expressions(|root| expressions.push(root));
                while let Some(id) = expressions.pop() {
                    let expression = &self.expressions[id.index()];
                    if let Some(function) = expression.created_function() {
                        children.push(self.functions[function.index()].body);
                    }
                    let _ = expression.visit_children(|child| {
                        expressions.push(child);
                        Ok::<_, ()>(())
                    });
                }
            }
            // Pushed in reverse so the first child is renewed first.
            for child in children.into_iter().rev() {
                pending.push((child, fresh));
            }
        }
        budget.work(Analysis, self.bindings.len() as u64)?;
        for binding in &mut self.bindings {
            if let Some(Some(fresh)) = map.get(binding.scope.index()) {
                binding.scope = *fresh;
            }
        }
        Ok(())
    }
}
