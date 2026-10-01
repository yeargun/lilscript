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
                for binding in 0..self.bindings.len() {
                    if self.bindings[binding].scope == inner_scope {
                        self.binding_mut(BindingId::new(binding)).scope = outer_scope;
                    }
                }
                self.reparent_scopes(inner_scope, outer_scope);
                let count = self.regions[inner.index()].statements.len();
                self.splice_block(region, index, inner, budget)?;
                flattened += 1;
                // The spliced statements may hold blocks of their own.
                if count == 0 {
                    continue;
                }
            }
        }
        Ok(flattened)
    }

    /// A nested block that declares nothing is its statements: `{for(…)…}`
    /// is `for(…)…`. It holds no binding, so its child scopes take the
    /// enclosing scope as parent and naming sees the same bindings in the
    /// same places. Unlike `flatten_blocks`, no declaration moves between
    /// scopes. Returns how many blocks went.
    pub(crate) fn drop_bare_blocks(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        budget.with_temporary_context(self, |module, budget| {
            budget.work(Analysis, (module.bindings.len() + module.scopes.len()) as u64)?;
            let mut declares = budget.filled(AllocationClass::Retained, module.scopes.len(), false)?;
            for binding in &module.bindings {
                if let Some(declared) = declares.get_mut(binding.scope.index()) { *declared = true; }
            }
            Ok(declares)
        }, |declares, module, budget| {
            let mut dropped = 0;
            for region in 0..module.regions.len() {
                if region == module.root.index() {
                    continue;
                }
                let mut index = 0;
                while index < module.regions[region].statements.len() {
                    budget.work(Analysis, 1)?;
                    let Statement::Block(inner) = module.regions[region].statements[index] else {
                        index += 1;
                        continue;
                    };
                    let inner_scope = module.regions[inner.index()].scope;
                    let bare = inner.index() != region
                        && !declares.get(inner_scope.index()).copied().unwrap_or(true)
                        && !module.regions[inner.index()]
                            .statements
                            .iter()
                            .any(|statement| {
                                matches!(
                                    statement,
                                    Statement::Let { .. } | Statement::Function { .. }
                                )
                        });
                if !bare {
                    index += 1;
                    continue;
                }
                let outer_scope = module.regions[region].scope;
                budget.work(Analysis, module.scopes.len() as u64)?;
                module.splice_block(region, index, inner, budget)?;
                module.reparent_scopes(inner_scope, outer_scope);
                dropped += 1;
                // The spliced statements may be bare blocks themselves.
            }
        }
        Ok(dropped)
        })
    }

    /// Move the contents while both region buffers belong to the target owner.
    fn splice_block(
        &mut self, region: usize, index: usize, inner: RegionId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let additional = self.regions[inner.index()].statements.len().saturating_sub(1);
        budget.reserve_vec(AllocationClass::Retained, &mut self.regions[region].statements, additional)?;
        let moved = std::mem::take(self.statements_mut(inner.index()));
        let bytes = crate::output_budget::vector_bytes(&moved)?;
        self.statements_mut(region).splice(index..=index, moved);
        budget.release(AllocationClass::Retained, bytes)
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

/// A function `place_single_calls` creates at its one call: its
/// declaration (region and index), its value and function, and the call and
/// the call's region.
struct Placement {
    declaring: RegionId,
    at: usize,
    value: ExprId,
    function: FunctionId,
    call: ExprId,
    region: RegionId,
}

/// Where a single-use call stands, and what its result feeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// One binding's occurrences and first supported call statement. The index
/// belongs to a single unchanged tree, so an edit cannot leave stale sites.
#[derive(Clone, Copy, Default)]
struct SingleCall {
    calls: usize,
    uses: usize,
    function: Option<FunctionId>,
    site: Option<(RegionId, usize, Site, ExprId)>,
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
    /// such functions stay. The contract hides the body's frame, and it
    /// reads no `this`, `arguments` or `super` of its own, and does not
    /// suspend; its scopes are renewed under the call's. The declaring
    /// region's scope encloses the call, and no call can run before the
    /// declaration: the call stands in a statement after it that is not a
    /// hoisted function. Returns how many calls were inlined.
    pub(crate) fn inline_single_calls(
        &mut self,
        frames_hidden: bool,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        if !frames_hidden {
            return Ok(0);
        }
        let mut inlined = 0;
        // One call per round: each edit moves statements and scopes.
        for _ in 0..256 {
            let Some(found) = self.single_call_candidate(strict, budget)? else {
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
        strict: bool,
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
        let mut scratch = budget.scope();
        let budget = &mut scratch;
        let reach = self.reach_tree(budget)?;
        // Every reference counts, a statement's own root included; the one
        // allowed is the single call.
        let mut calls = budget.filled(
            AllocationClass::Scratch,
            self.bindings.len(),
            SingleCall::default(),
        )?;
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            match &self.expressions[id.index()] {
                Expr::Binding(binding) => calls[binding.index()].uses += 1,
                Expr::Call {
                    callee,
                    invocation: Invocation::Value | Invocation::Reference,
                    ..
                } => {
                    if let Expr::Binding(binding) = self.expressions[callee.index()] {
                        calls[binding.index()].calls += 1;
                    }
                }
                _ => {}
            }
        }
        budget.work(Analysis, self.exports.len() as u64)?;
        for export in &self.exports {
            calls[export.binding.index()].uses += 1;
        }
        self.index_single_call_sites(&reach, &mut calls, budget)?;
        // Each region's parent statement: (region, index), or a function
        // created at that statement.
        let mut parents = None;
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
                if calls[binding.index()].calls != 1
                    || calls[binding.index()].uses != 1
                    || self.bindings[binding.index()].pinned
                {
                    continue;
                }
                let found = calls[binding.index()].site;
                #[cfg(test)]
                assert_eq!(
                    found.map(|(region, index, site, call)| {
                        let Expr::Call { arguments, .. } = &self.expressions[call.index()] else {
                            unreachable!()
                        };
                        (region, index, site, arguments.clone())
                    }),
                    self.single_call_site(&reach, binding, function),
                    "indexed call site differs from the independent scan"
                );
                let Some((region, index, site, call)) = found else {
                    continue;
                };
                if !self.block_inlinable(function) {
                    continue;
                }
                let Expr::Call { arguments, .. } = &self.expressions[call.index()] else {
                    unreachable!("indexed call site is a call")
                };
                // The call runs only after the declaration: the declaring
                // region's statement holding it follows the declaration and
                // hoists nothing. A call inside the function's own body is
                // its recursion, never a place for the body.
                let own = self.functions[function.index()].body;
                if !strict
                    && !super::inline::same_strictness(
                        reach.strict_regions[own.index()],
                        reach.strict_regions[region.index()],
                    )
                {
                    continue;
                }
                if parents.is_none() {
                    parents = Some(self.region_parents(budget)?);
                }
                let parents = parents.as_ref().expect("initialized above");
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
                    if self.mentions(&[own], arguments, declared, false) {
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
                    declaring,
                    at,
                    function,
                    region,
                    index,
                    site,
                    arguments.clone(),
                )));
            }
        }
        Ok(None)
    }

    /// A function with one call is created at it: `let f=(p)=>{…};…f(a)`
    /// becomes `…((p)=>{…})(a)` and the binding goes, as Terser places a
    /// single-use lambda (`reduce_funcs`). Creating a function runs no code
    /// and cannot throw, so it may happen where the call is, when:
    /// - the call runs only after the declaration: the declaring region's
    ///   statement holding it follows the declaration and hoists nothing;
    /// - the function has its own frame, or reads none: an arrow reading
    ///   `this`, `arguments` or `super` would read the caller's;
    /// - nothing observes its name;
    /// - no loop of the caller's own frame holds the call, which would
    ///   create the function each iteration (Terser's
    ///   `dont_inline_lambda_in_loop`): a function created in a loop is a
    ///   frame of its own;
    /// - the call is in no `for…in` or `for…of` head, whose loop binding a
    ///   function created there would see;
    /// - a root declaration and the root statement holding the call come
    ///   from one source module, which multi-file delivery keeps together;
    /// - the contract hides frames, and moving the function preserves its
    ///   inherited strictness;
    /// - the moved body fits within the nesting limit.
    ///
    /// The body's scopes are renewed under the call's region. Returns how
    /// many functions moved, and each old node's new id when the arena was
    /// renumbered.
    pub(crate) fn place_single_calls(
        &mut self,
        frames_hidden: bool,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        // A frame may move only when host reflection over it is outside
        // the contract. Its inherited execution mode must still be kept.
        if !frames_hidden {
            return Ok(0);
        }
        let mut placed = 0;
        let mut disordered = false;
        // A round moves functions whose calls stand outside the bodies of
        // the others it moves; a function moved into another's body is taken
        // with that body in a later round.
        for _ in 0..8 {
            let mut moves = self.single_call_placements(strict, budget)?;
            if moves.is_empty() {
                break;
            }
            // Later declarations first, so earlier indices hold.
            moves.sort_unstable_by_key(|placement| {
                std::cmp::Reverse((placement.declaring.index(), placement.at))
            });
            for placement in &moves {
                budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                // A function literal is a value, not a reference with a base.
                if let Expr::Call {
                    callee, invocation, ..
                } = self.expression_mut(placement.call)
                {
                    *callee = placement.value;
                    *invocation = Invocation::Value;
                }
                disordered |= placement.value.index() > placement.call.index();
                self.remove_statement(placement.declaring.index(), placement.at);
                let scope = self.regions[placement.region.index()].scope;
                let body = self.functions[placement.function.index()].body;
                self.rescope(body, scope, budget)?;
            }
            placed += moves.len();
        }
        if disordered { self.renumber(budget)?; }
        Ok(placed)
    }

    /// The functions `place_single_calls` moves in one round.
    fn single_call_placements(
        &self,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<Placement>, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut phase = budget.scope();
        let budget = &mut phase;
        let reach = self.reach_tree(budget)?;
        let mut uses = vec![0usize; self.bindings.len()];
        let mut calls: Vec<Option<(ExprId, usize)>> = vec![None; self.bindings.len()];
        for &(id, depth) in &reach.expressions {
            budget.work(Analysis, 1)?;
            match &self.expressions[id.index()] {
                Expr::Binding(binding) => uses[binding.index()] += 1,
                Expr::Call {
                    callee,
                    invocation: Invocation::Value | Invocation::Reference,
                    ..
                } => {
                    if let Expr::Binding(binding) = self.expressions[callee.index()] {
                        calls[binding.index()] = Some((id, depth));
                    }
                }
                _ => {}
            }
        }
        for export in &self.exports {
            uses[export.binding.index()] += 1;
        }
        // Each single call's statement, and whether it is a loop's, whose
        // test and update repeat.
        let wanted = |binding: BindingId| uses[binding.index()] == 1;
        let mut sites: Vec<Option<(RegionId, usize, bool)>> = vec![None; self.bindings.len()];
        for &region in &reach.regions {
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                let mut pending = Vec::new();
                statement.visit_expressions(|root| pending.push(root));
                while let Some(id) = pending.pop() {
                    budget.work(Analysis, 1)?;
                    let expression = &self.expressions[id.index()];
                    if let Expr::Call { callee, .. } = expression {
                        if let Expr::Binding(binding) = self.expressions[callee.index()] {
                            if wanted(binding)
                                && calls[binding.index()].is_some_and(|(call, _)| call == id)
                            {
                                // A loop's test and update repeat; a `for…in`
                                // or `for…of` head runs with the loop's binding
                                // in scope (and in its TDZ), which a function
                                // created there would close over.
                                let head = matches!(
                                    statement,
                                    Statement::Loop { .. }
                                        | Statement::ForIn { .. }
                                        | Statement::ForOf { .. }
                                );
                                sites[binding.index()] = Some((region, index, head));
                            }
                        }
                    }
                    // A created function's body is a region of its own.
                    if !expression.creates_function() {
                        let _ = expression.visit_children(|child| {
                            pending.push(child);
                            Ok::<_, ()>(())
                        });
                    }
                }
            }
        }
        let parents = self.region_parents(budget)?;
        let mut bodies = vec![false; self.regions.len()];
        for function in &self.functions {
            bodies[function.body.index()] = true;
        }
        let mut placements: Vec<Placement> = Vec::new();
        // Regions on the paths of this round's calls, and the bodies moving.
        let mut on_paths = vec![false; self.regions.len()];
        let mut moving = vec![false; self.regions.len()];
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
                let (Some((call, depth)), Some((region, index, head))) =
                    (calls[binding.index()], sites[binding.index()])
                else {
                    continue;
                };
                let declared = &self.functions[function.index()];
                if uses[binding.index()] != 1
                    || head
                    || self.bindings[binding.index()].pinned
                    || !matches!(declared.name, FunctionName::Unobserved)
                    || (declared.arrow && !self.frame_free(function))
                {
                    continue;
                }
                if !strict
                    && !declared.strict
                    && !super::inline::same_strictness(
                        reach.strict_regions[declared.body.index()],
                        reach.strict_regions[region.index()],
                    )
                {
                    continue;
                }
                // Up from the call to the declaration: no loop body of the
                // call's own frame, no body moving this round.
                let own = declared.body;
                let mut path = (region, index);
                let mut frame = true;
                let mut regions = Vec::new();
                let mut encloses = false;
                for _ in 0..verify::MAX_NESTING * 4 {
                    budget.work(Analysis, 1)?;
                    if path.0 == own || moving[path.0.index()] {
                        break;
                    }
                    regions.push(path.0);
                    if path.0 == declaring {
                        encloses = path.1 > at
                            && !matches!(
                                self.regions[declaring.index()].statements[path.1],
                                Statement::Function { .. }
                            )
                            && (declaring != self.root
                                || self.root_module(at) == self.root_module(path.1));
                        break;
                    }
                    let Some(parent) = parents[path.0.index()] else {
                        break;
                    };
                    if bodies[path.0.index()] {
                        frame = false;
                    } else if frame
                        && matches!(
                            self.regions[parent.0.index()].statements[parent.1],
                            Statement::Loop { .. }
                                | Statement::ForIn { .. }
                                | Statement::ForOf { .. }
                        )
                    {
                        break;
                    }
                    path = parent;
                }
                if !encloses || on_paths[own.index()] {
                    continue;
                }
                if depth + 3 + self.region_subtree_depth(own) > verify::MAX_NESTING {
                    continue;
                }
                for &on in &regions {
                    on_paths[on.index()] = true;
                }
                moving[own.index()] = true;
                placements.push(Placement {
                    declaring,
                    at,
                    value,
                    function,
                    call,
                    region,
                });
            }
        }
        Ok(placements)
    }

    /// A function whose body may stand in its caller's place: it keeps no
    /// frame a strict caller could see, reads no frame of its own, and does
    /// not suspend.
    fn block_inlinable(&self, function: FunctionId) -> bool {
        let declared = &self.functions[function.index()];
        declared.suspension == Suspension::None
            && declared.length.is_none()
            && self.frame_free(function)
    }

    /// Find every eligible binding's first whole-statement call in traversal
    /// order. A body cannot be placed into itself. Arguments stay borrowed
    /// until the selected candidate is returned, rather than being cloned for
    /// every rejected declaration.
    fn index_single_call_sites(
        &self,
        reach: &super::reach::ReachTree,
        calls: &mut [SingleCall],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                if let Statement::Let {
                    binding,
                    value: Some(value),
                } = *statement
                {
                    let entry = &mut calls[binding.index()];
                    if entry.calls == 1 && entry.uses == 1 && !self.bindings[binding.index()].pinned
                    {
                        if let Expr::Function(function) = self.expressions[value.index()] {
                            entry.function = Some(function);
                        }
                    }
                }
            }
        }
        for &region in &reach.regions {
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                let (call, site) = match *statement {
                    Statement::Evaluate(value) => {
                        let value = match self.expressions[value.index()] {
                            Expr::Unary {
                                op: Unary::Void,
                                value,
                            } => value,
                            _ => value,
                        };
                        match self.expressions[value.index()] {
                            Expr::Assign { target, value } => {
                                let Expr::Binding(target) = self.expressions[target.index()] else {
                                    continue;
                                };
                                (value, Site::Assign(target))
                            }
                            _ => (value, Site::Discard),
                        }
                    }
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => (value, Site::Declare(binding)),
                    Statement::Return(Some(value)) => (value, Site::Return),
                    _ => continue,
                };
                let Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Value | Invocation::Reference,
                } = &self.expressions[call.index()]
                else {
                    continue;
                };
                let Expr::Binding(binding) = self.expressions[callee.index()] else {
                    continue;
                };
                let entry = &mut calls[binding.index()];
                let Some(function) = entry.function.map(|id| &self.functions[id.index()]) else {
                    continue;
                };
                if entry.site.is_some()
                    || region == function.body
                    || arguments.len() != function.parameters.len()
                {
                    continue;
                }
                budget.work(Analysis, arguments.len() as u64)?;
                if arguments
                    .iter()
                    .any(|id| matches!(self.expressions[id.index()], Expr::Spread(_)))
                {
                    continue;
                }
                entry.site = Some((region, index, site, call));
            }
        }
        Ok(())
    }

    /// Independent scan oracle for the binding-indexed lookup.
    #[cfg(test)]
    fn single_call_site(
        &self,
        reach: &super::reach::ReachTree,
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
    pub(super) fn region_parents(
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
                    for function in expression.created_functions() {
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
                for function in expression.created_functions() {
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
        statements.append(self.statements_mut(body.index()));
        // `return f(a)` returns `undefined` from a body that ends.
        if matches!(site, Site::Return) && falls {
            statements.push(Statement::Return(None));
        }
        *self.statements_mut(body.index()) = statements;
        // Renew the scopes of everything now under the body, the arguments'
        // functions included, under the call's scope, parents first.
        self.rescope(body, self.regions[region.index()].scope, budget)?;
        let block = Statement::Block(body);
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
        // The block runs where the call did: its row.
        self.splice_statements(region.index(), index..index + 1, replacement, |rows| {
            vec![rows[0]; added + 1]
        });
        // The declaration goes: nothing else reads the function.
        let at = if declaring == region && index < at {
            at + added
        } else {
            at
        };
        self.remove_statement(declaring.index(), at);
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
    pub(super) fn rescope(
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
            self.set_region_scope(region, fresh);
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
                    for function in expression.created_functions() {
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
        for binding in 0..self.bindings.len() {
            if let Some(&Some(fresh)) = map.get(self.bindings[binding].scope.index()) {
                self.binding_mut(BindingId::new(binding)).scope = fresh;
            }
        }
        Ok(())
    }

    /// Scopes whose parent is `from` take `to` as their parent.
    pub(super) fn reparent_scopes(&mut self, from: ScopeId, to: ScopeId) {
        if self.scopes.iter().any(|parent| *parent == Some(from)) {
            for parent in self.tables_mut().scopes.iter_mut().flatten() {
                if *parent == from {
                    *parent = to;
                }
            }
        }
    }
}
