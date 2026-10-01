//! Single-evaluation placement under the shared target binding and operation
//! facts. This walker owns evaluation order only; facts own initialization,
//! mutation and per-operation behavior. Moving a single-use initializer removes
//! its declaration, so the rule converges by decreasing live storage.
use super::facts::BindingFacts;
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// Where the reference was found.
enum Walk {
    /// Everything evaluated so far is quiet and holds no reference.
    Quiet,
    Found(Leaf, usize),
    Stop,
}

/// The statement being searched, and what the moved value can assign.
struct Search<'a> {
    binding: BindingId,
    region: RegionId,
    index: usize,
    owner: Option<FunctionId>,
    order: &'a BindingFacts,
    frames: &'a Frames,
    captured: &'a [bool],
    assigns: Vec<BindingId>,
    /// The value only reads, and member reads run no code.
    reads_only: bool,
}

impl Module {
    /// Where the one read of `binding` sits in statement `index + 1` of
    /// `region`, when everything that statement evaluates before it is quiet
    /// with respect to `value` (the value of the `let` at `index`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn quiet_leaf(
        &self,
        region: RegionId,
        index: usize,
        binding: BindingId,
        value: ExprId,
        order: &BindingFacts,
        frames: &Frames,
        captured: &[bool],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<(Leaf, usize)>, AllocationError> {
        let Some(owner) = order.owner[region.index()] else {
            return Ok(None);
        };
        let root = match &self.regions[region.index()].statements[index + 1] {
            Statement::Return(Some(value))
            | Statement::Evaluate(value)
            | Statement::Throw(value)
            | Statement::Let {
                value: Some(value), ..
            }
            | Statement::If {
                condition: value, ..
            }
            | Statement::ForIn { object: value, .. }
            | Statement::ForOf {
                iterable: value, ..
            } => *value,
            _ => return Ok(None),
        };
        // The bindings the value assigns itself; a call inside it reaches
        // only bindings some closure mentions, which quiet reads exclude.
        let mut assigns = Vec::new();
        let mut pending = vec![value];
        while let Some(id) = pending.pop() {
            budget.work(Analysis, 1)?;
            let expression = &self.expressions[id.index()];
            if let Expr::Assign { target, .. } = expression {
                if let Expr::Binding(assigned) = self.expressions[target.index()] {
                    assigns.push(assigned);
                }
            }
            if !expression.creates_function() {
                let _ = expression.visit_children(|child| {
                    pending.push(child);
                    Ok::<_, ()>(())
                });
            }
        }
        let reads_only = self.pure_property_reads && self.read_only(value, budget)?;
        let search = Search {
            binding,
            region,
            index,
            owner,
            order,
            frames,
            captured,
            assigns,
            reads_only,
        };
        Ok(
            match self.walk_quiet(root, Leaf::Root, 0, &search, budget)? {
                Walk::Found(leaf, depth) => Some((leaf, depth)),
                Walk::Quiet | Walk::Stop => None,
            },
        )
    }

    /// Walk `id` in evaluation order: the reference, or whether all of it is
    /// quiet.
    fn walk_quiet(
        &self,
        id: ExprId,
        parent: Leaf,
        depth: usize,
        search: &Search<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Walk, AllocationError> {
        budget.work(Analysis, 1)?;
        let here = Leaf::Child(id);
        let below = depth + 1;
        // Operands in order: the reference, a stop, or all quiet. What the
        // operation itself does is each case's to judge.
        let operands = |module: &Self, operands: &[ExprId], budget: &mut AllocationBudget<'_>| {
            for &operand in operands {
                match module.walk_quiet(operand, here, below, search, budget)? {
                    Walk::Quiet => {}
                    found => return Ok(found),
                }
            }
            Ok::<_, AllocationError>(Walk::Quiet)
        };
        // An operation that may run code: quiet operands stop there.
        let runs = |walk: Walk| match walk {
            Walk::Quiet => Walk::Stop,
            found => found,
        };
        // An operation the program's facts show quiet (M5.2's behaviour
        // column) is quiet with its operands; any other may run code.
        let judged = |module: &Self, walk: Walk| match walk {
            Walk::Quiet if module.operation_behaviour(id).is_some_and(Behaviour::quiet) => {
                Walk::Quiet
            }
            walk => runs(walk),
        };
        Ok(match &self.expressions[id.index()] {
            Expr::Binding(binding) if *binding == search.binding => Walk::Found(parent, depth),
            Expr::Binding(binding) => {
                if self.quiet_read(*binding, search, budget)? {
                    Walk::Quiet
                } else {
                    Walk::Stop
                }
            }
            // A created function is not searched: the one reference inside
            // it would then be found nowhere, which fails the search.
            Expr::Literal(_) | Expr::Regex(_) | Expr::Function(_) | Expr::This => Walk::Quiet,
            Expr::Host(name) => {
                if self.pristine_builtins
                    && matches!(name.kind, crate::catalog::HostKind::Standard(_))
                {
                    Walk::Quiet
                } else {
                    Walk::Stop
                }
            }
            Expr::Member { object, property } => {
                let mut parts = vec![*object];
                if let Property::Computed(key) = property {
                    parts.push(*key);
                }
                match operands(self, &parts, budget)? {
                    // A named path into a standard global runs no getter of ours.
                    Walk::Quiet if self.pristine_builtins && self.standard_member(id) => {
                        Walk::Quiet
                    }
                    // Nor does any read, by assumption, and the value changes
                    // nothing it could read.
                    Walk::Quiet if search.reads_only => Walk::Quiet,
                    walk => runs(walk),
                }
            }
            // Creating an array or object runs nothing; a spread item stops
            // on its own.
            Expr::Array(items) | Expr::Sequence(items) => operands(self, items, budget)?,
            Expr::Object(entries) => {
                let mut parts = Vec::with_capacity(entries.len() * 2);
                for (key, value) in entries {
                    if let Property::Computed(key) = key {
                        parts.push(*key);
                    }
                    parts.push(*value);
                }
                match operands(self, &parts, budget)? {
                    // A computed key converts to a string: only a literal's
                    // conversion runs no code.
                    Walk::Quiet
                        if entries.iter().all(|(key, _)| match key {
                            Property::Named(_) => true,
                            Property::Computed(key) => {
                                matches!(self.expressions[key.index()], Expr::Literal(_))
                            }
                        }) =>
                    {
                        Walk::Quiet
                    }
                    walk => runs(walk),
                }
            }
            Expr::Unary { op, value } => match operands(self, &[*value], budget)? {
                Walk::Quiet if matches!(op, Unary::Not | Unary::Void | Unary::TypeOf) => {
                    Walk::Quiet
                }
                walk => runs(walk),
            },
            Expr::Binary { op, left, right } => match op {
                Binary::And | Binary::Or | Binary::Nullish => {
                    match self.walk_quiet(*left, here, below, search, budget)? {
                        Walk::Quiet => {}
                        found => return Ok(found),
                    }
                    // The right operand may not run: a reference there is
                    // not evaluated exactly once.
                    match self.walk_quiet(*right, here, below, search, budget)? {
                        Walk::Quiet => Walk::Quiet,
                        _ => Walk::Stop,
                    }
                }
                // No conversion: strict equality runs no code.
                Binary::StrictEqual | Binary::StrictNotEqual => {
                    operands(self, &[*left, *right], budget)?
                }
                _ => judged(self, operands(self, &[*left, *right], budget)?),
            },
            Expr::Conditional { condition, yes, no } => {
                match self.walk_quiet(*condition, here, below, search, budget)? {
                    Walk::Quiet => {}
                    found => return Ok(found),
                }
                for branch in [*yes, *no] {
                    match self.walk_quiet(branch, here, below, search, budget)? {
                        Walk::Quiet => {}
                        _ => return Ok(Walk::Stop),
                    }
                }
                Walk::Quiet
            }
            Expr::Call {
                callee, arguments, ..
            }
            | Expr::Construct { callee, arguments } => {
                let mut parts = Vec::with_capacity(arguments.len() + 1);
                parts.push(*callee);
                parts.extend_from_slice(arguments);
                judged(self, operands(self, &parts, budget)?)
            }
            Expr::Intrinsic {
                receiver,
                arguments,
                ..
            } => {
                let mut parts = Vec::with_capacity(arguments.len() + 1);
                parts.push(*receiver);
                parts.extend_from_slice(arguments);
                runs(operands(self, &parts, budget)?)
            }
            Expr::ConstructIntrinsic { arguments, .. } => runs(operands(self, arguments, budget)?),
            // A conversion calls `valueOf` unless the facts show the operand
            // a primitive; a spread iterates, a suspension lets other code
            // run.
            Expr::ToInt32(value) | Expr::IntNegate(value) => {
                judged(self, operands(self, &[*value], budget)?)
            }
            Expr::Spread(value) | Expr::Await(value) | Expr::Yield { value, .. } => {
                runs(operands(self, &[*value], budget)?)
            }
            Expr::IntBinary { left, right, .. } => {
                judged(self, operands(self, &[*left, *right], budget)?)
            }
            Expr::Assign { target, value } => {
                // The target's parts, below the target, then the value; the
                // store itself is an effect.
                if let Expr::Member { object, property } = &self.expressions[target.index()] {
                    let mut parts = vec![*object];
                    if let Property::Computed(key) = property {
                        parts.push(*key);
                    }
                    for part in parts {
                        match self.walk_quiet(
                            part,
                            Leaf::Child(*target),
                            below + 1,
                            search,
                            budget,
                        )? {
                            Walk::Quiet => {}
                            found => return Ok(found),
                        }
                    }
                } else if !matches!(self.expressions[target.index()], Expr::Binding(_)) {
                    return Ok(Walk::Stop);
                }
                runs(operands(self, &[*value], budget)?)
            }
            Expr::Template(parts) => {
                // Each substitution converts to a string where it stands;
                // only a literal's conversion runs no code.
                for part in parts {
                    if let TemplatePart::Expression(value) = part {
                        match self.walk_quiet(*value, here, below, search, budget)? {
                            Walk::Quiet
                                if matches!(self.expressions[value.index()], Expr::Literal(_)) => {}
                            Walk::Quiet => return Ok(Walk::Stop),
                            found => return Ok(found),
                        }
                    }
                }
                Walk::Quiet
            }
            Expr::Class { .. } | Expr::SuperCall { .. } | Expr::LoadModule { .. } => Walk::Stop,
        })
    }

    /// Whether reading `binding` in the searched statement is quiet: it is
    /// initialized there, and the moved value cannot assign it.
    fn quiet_read(
        &self,
        binding: BindingId,
        search: &Search<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        if search.assigns.contains(&binding) {
            return Ok(false);
        }
        // A declared extern naming a standard global, never assigned.
        if self.pristine_builtins && self.standard_global(binding) {
            return Ok(true);
        }
        // A value that only reads assigns nothing: the binding need only be
        // initialized.
        if search.reads_only {
            return self.initialized_for(binding, search, budget);
        }
        // Only its own function's code can assign it, and the value does not.
        if !search.captured[binding.index()] {
            return self.initialized_at(
                binding,
                search.region,
                search.index,
                search.frames,
                budget,
            );
        }
        self.constant_at(
            binding,
            search.region,
            search.index,
            search.order,
            search.frames,
            budget,
        )
    }

    /// Whether `binding` is initialized wherever the searched statement runs,
    /// whoever else assigns it.
    fn initialized_for(
        &self,
        binding: BindingId,
        search: &Search<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        match search.owner {
            _ if !search.captured[binding.index()] => {
                self.initialized_at(binding, search.region, search.index, search.frames, budget)
            }
            None => {
                self.initialized_at(binding, search.region, search.index, search.frames, budget)
            }
            Some(function) => Ok(search.order.initialized_in(binding, function)),
        }
    }

    /// Whether evaluating `value` only reads: literals, bindings, created
    /// functions, member reads (by the assumption) and standard globals,
    /// arrays and objects of them, and operators that convert nothing.
    fn read_only(
        &self,
        value: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let mut pending = vec![value];
        while let Some(id) = pending.pop() {
            budget.work(Analysis, 1)?;
            let expression = &self.expressions[id.index()];
            let reads = matches!(expression, Expr::Binding(_) | Expr::Member { .. })
                || self.operation_behaviour(id).is_some_and(Behaviour::quiet);
            if !reads {
                return Ok(false);
            }
            if !expression.creates_function() {
                let _ = expression.visit_children(|child| {
                    pending.push(child);
                    Ok::<_, ()>(())
                });
            }
        }
        Ok(true)
    }
}
