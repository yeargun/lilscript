//! Where a single-use value may move past the start of the statement that
//! reads it: `let v=V;f(a,v)` is `f(a,V)` when everything the statement
//! evaluates before `v` is quiet. A quiet evaluation runs no code, cannot
//! throw, and yields a value `V` cannot change, so running `V` after it
//! instead of before it is invisible:
//!
//! * a literal, a regular expression, a function created (not called), and
//!   arrays and objects of quiet values;
//! * a standard global, or a named path into one, under pristine builtins;
//! * a read of a binding that is initialized there and that `V` cannot
//!   assign. `V` may run any code, which can assign any binding a closure
//!   reaches, so such a binding must never be assigned at all. A root
//!   binding read inside a function is initialized there when the root
//!   statement declaring it precedes the first root statement that may run
//!   code from the one creating the function on: the function cannot run
//!   before it exists, nor before code calls it (013-T7.4, by
//!   initialization order).
//!
//! When the contract assumes member reads run no code, a value that only
//! reads (`read_only`) also moves past member reads and past reads of
//! bindings other code assigns: neither side changes anything the other
//! reads. Only which of them throws first could differ, which the
//! assumption, like Terser's `pure_getters`, sets aside.
//!
//! The read of `v` must also be evaluated exactly once: a branch of `&&`,
//! `||`, `??` or `?:` might skip `V`.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// When a root declaration initializes its binding, or when a function is
/// first created: a hoisted declaration before any root statement runs,
/// otherwise during that root statement.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Moment {
    Hoisted,
    At(usize),
}

/// Facts about the whole module that the quiet test reads.
pub(super) struct Order {
    /// Bindings some code assigns, a loop head binds, or an import holds.
    written: Vec<bool>,
    /// Root declarations.
    declared: Vec<Option<Moment>>,
    /// The root statement whose evaluation first creates each function (a
    /// function nested in another is created no earlier than it).
    created: Vec<Option<Moment>>,
    /// The function each region belongs to; `Some(None)` for the root's.
    owner: Vec<Option<Option<FunctionId>>>,
    /// For each root statement, the first one from there on that may run
    /// program code (the root's length when none does). A function created
    /// by root statement `c` cannot run before `runs_from[c]`: only running
    /// code can call it, and importers call exports after the whole root.
    runs_from: Vec<usize>,
}

impl Order {
    /// Whether some code assigns `binding`, a loop head binds it, or an
    /// import holds it.
    pub(super) fn written(&self, binding: BindingId) -> bool {
        self.written[binding.index()]
    }

    /// The first root statement during which `function` can run.
    fn first_run(&self, function: FunctionId) -> Option<usize> {
        match self.created[function.index()]? {
            Moment::Hoisted => Some(self.runs_from.first().copied().unwrap_or(0)),
            Moment::At(created) => Some(self.runs_from[created]),
        }
    }

    /// Whether a root binding declared by statement `declared` holds its
    /// value whenever `function` runs.
    pub(super) fn initialized_in(&self, binding: BindingId, function: FunctionId) -> bool {
        match (self.declared[binding.index()], self.first_run(function)) {
            (Some(Moment::Hoisted), _) => true,
            (Some(Moment::At(declared)), Some(first)) => declared < first,
            _ => false,
        }
    }
}

/// The code an expression belongs to: a root statement's own evaluation,
/// or a function's body (the innermost function around it).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Owner {
    Root(usize),
    Function(FunctionId),
}

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
    order: &'a Order,
    frames: &'a Frames,
    captured: &'a [bool],
    assigns: Vec<BindingId>,
    /// The value only reads, and member reads run no code.
    reads_only: bool,
}

impl Module {
    pub(super) fn order(
        &self,
        frames: &Frames,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Order, AllocationError> {
        let mut written = budget.filled(AllocationClass::Scratch, self.bindings.len(), false)?;
        budget.work(Analysis, (self.expressions.len() + self.regions.len()) as u64)?;
        for expression in &self.expressions {
            if let Expr::Assign { target, .. } = expression {
                if let Expr::Binding(binding) = self.expressions[target.index()] {
                    written[binding.index()] = true;
                }
            }
        }
        for region in &self.regions {
            for statement in &region.statements {
                if let Statement::ForIn { binding, .. } | Statement::ForOf { binding, .. } = statement {
                    written[binding.index()] = true;
                }
            }
        }
        for import in &self.imports {
            written[import.binding.index()] = true;
        }
        let mut declared = budget.filled(AllocationClass::Scratch, self.bindings.len(), None)?;
        let mut created = budget.filled(AllocationClass::Scratch, self.functions.len(), None)?;
        for (index, statement) in self.regions[self.root.index()].statements.iter().enumerate() {
            let moment = match statement {
                Statement::Function { binding, .. } => {
                    declared[binding.index()] = Some(Moment::Hoisted);
                    Moment::Hoisted
                }
                Statement::Let { binding, .. } => {
                    declared[binding.index()] = Some(Moment::At(index));
                    Moment::At(index)
                }
                _ => Moment::At(index),
            };
            // Every function this statement creates, at any depth.
            let mut expressions = Vec::new();
            let mut regions = Vec::new();
            statement.visit_expressions(|root| expressions.push(root));
            statement.visit_regions(|child| regions.push(child));
            if let Statement::Function { function, .. } = statement {
                created[function.index()] = Some(moment);
                regions.push(self.functions[function.index()].body);
            }
            loop {
                if let Some(id) = expressions.pop() {
                    budget.work(Analysis, 1)?;
                    let expression = &self.expressions[id.index()];
                    if let Some(function) = expression.created_function() {
                        created[function.index()] = Some(moment);
                        regions.push(self.functions[function.index()].body);
                    }
                    let _ = expression.visit_children(|child| {
                        expressions.push(child);
                        Ok::<_, ()>(())
                    });
                    continue;
                }
                let Some(region) = regions.pop() else {
                    break;
                };
                for statement in &self.regions[region.index()].statements {
                    budget.work(Analysis, 1)?;
                    statement.visit_expressions(|root| expressions.push(root));
                    statement.visit_regions(|child| regions.push(child));
                    if let Statement::Function { function, .. } = statement {
                        created[function.index()] = Some(moment);
                        regions.push(self.functions[function.index()].body);
                    }
                }
            }
        }
        let mut owner = budget.filled(AllocationClass::Scratch, self.regions.len(), None)?;
        for start in 0..self.regions.len() {
            budget.work(Analysis, 1)?;
            let mut region = RegionId::new(start);
            owner[start] = loop {
                if let Some(function) = frames.bodies[region.index()] {
                    break Some(Some(function));
                }
                if region == self.root {
                    break Some(None);
                }
                match frames.parents[region.index()] {
                    Some((parent, _)) => region = parent,
                    None => break None,
                }
            };
        }
        let statements = &self.regions[self.root.index()].statements;
        let mut runs_from = budget.filled(AllocationClass::Scratch, statements.len(), statements.len())?;
        let mut next = statements.len();
        for index in (0..statements.len()).rev() {
            budget.work(Analysis, 1)?;
            let quiet = match statements[index] {
                Statement::Function { .. } | Statement::Let { value: None, .. } => true,
                Statement::Let { value: Some(value), .. } => {
                    matches!(self.expressions[value.index()], Expr::Binding(_))
                        || self.standard_member(value)
                        || self.inert_value(value, budget)?
                }
                _ => false,
            };
            if !quiet {
                next = index;
            }
            runs_from[index] = next;
        }
        Ok(Order {
            written,
            declared,
            created,
            owner,
            runs_from,
        })
    }

    /// Whether code of `owner` runs only after root statement `index` has
    /// run: a later root statement, or a function created by one.
    pub(super) fn runs_after_root(&self, owner: Owner, index: usize, order: &Order) -> bool {
        match owner {
            Owner::Root(at) => at > index,
            Owner::Function(function) => order.first_run(function).is_some_and(|first| first > index),
        }
    }

    /// Each reachable expression's owner.
    pub(super) fn expression_owners(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<Option<Owner>>, AllocationError> {
        let mut owners = budget.filled(AllocationClass::Scratch, self.expressions.len(), None)?;
        let mut regions: Vec<(RegionId, Option<FunctionId>, usize)> = Vec::new();
        for (index, statement) in self.regions[self.root.index()].statements.iter().enumerate() {
            let mut expressions = Vec::new();
            statement.visit_expressions(|root| expressions.push((root, None)));
            statement.visit_regions(|child| regions.push((child, None, index)));
            if let Statement::Function { function, .. } = statement {
                regions.push((self.functions[function.index()].body, Some(*function), index));
            }
            loop {
                if let Some((id, function)) = expressions.pop() {
                    budget.work(Analysis, 1)?;
                    owners[id.index()] = Some(match function {
                        Some(function) => Owner::Function(function),
                        None => Owner::Root(index),
                    });
                    let expression = &self.expressions[id.index()];
                    if let Some(created) = expression.created_function() {
                        regions.push((self.functions[created.index()].body, Some(created), index));
                    }
                    let _ = expression.visit_children(|child| {
                        expressions.push((child, function));
                        Ok::<_, ()>(())
                    });
                    continue;
                }
                let Some((region, function, _)) = regions.pop() else {
                    break;
                };
                for statement in &self.regions[region.index()].statements {
                    budget.work(Analysis, 1)?;
                    statement.visit_expressions(|root| expressions.push((root, function)));
                    statement.visit_regions(|child| regions.push((child, function, index)));
                    if let Statement::Function { function, .. } = statement {
                        regions.push((self.functions[function.index()].body, Some(*function), index));
                    }
                }
            }
        }
        Ok(owners)
    }

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
        order: &Order,
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
                value: Some(value),
                ..
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
            if expression.created_function().is_none() {
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
        Ok(match self.walk_quiet(root, Leaf::Root, 0, &search, budget)? {
            Walk::Found(leaf, depth) => Some((leaf, depth)),
            Walk::Quiet | Walk::Stop => None,
        })
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
                if self.pristine_builtins && inline::is_standard_global(name) {
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
                    Walk::Quiet if self.pristine_builtins && self.standard_member(id) => Walk::Quiet,
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
                Walk::Quiet if matches!(op, Unary::Not | Unary::Void | Unary::TypeOf) => Walk::Quiet,
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
                _ => runs(operands(self, &[*left, *right], budget)?),
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
                runs(operands(self, &parts, budget)?)
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
            // A spread iterates, a conversion calls `valueOf`, a suspension
            // lets other code run.
            Expr::ToInt32(value)
            | Expr::IntNegate(value)
            | Expr::Spread(value)
            | Expr::Await(value)
            | Expr::Yield { value, .. } => runs(operands(self, &[*value], budget)?),
            Expr::IntBinary { left, right, .. } => runs(operands(self, &[*left, *right], budget)?),
            Expr::Assign { target, value } => {
                // The target's parts, below the target, then the value; the
                // store itself is an effect.
                if let Expr::Member { object, property } = &self.expressions[target.index()] {
                    let mut parts = vec![*object];
                    if let Property::Computed(key) = property {
                        parts.push(*key);
                    }
                    for part in parts {
                        match self.walk_quiet(part, Leaf::Child(*target), below + 1, search, budget)? {
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
            return self.initialized_at(binding, search.region, search.index, search.frames, budget);
        }
        self.constant_at(binding, search.region, search.index, search.order, search.frames, budget)
    }

    /// Whether `binding` is never assigned and holds its value wherever
    /// statement `index` of `region` runs: a root declaration that precedes
    /// the root statement creating the region's function (or, in the root,
    /// the statement itself).
    pub(super) fn constant_at(
        &self,
        binding: BindingId,
        region: RegionId,
        index: usize,
        order: &Order,
        frames: &Frames,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        if order.written[binding.index()] {
            return Ok(false);
        }
        match order.owner[region.index()] {
            None => Ok(false),
            Some(None) => self.initialized_at(binding, region, index, frames, budget),
            Some(Some(function)) => Ok(order.initialized_in(binding, function)),
        }
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
            None => self.initialized_at(binding, search.region, search.index, search.frames, budget),
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
            let reads = match expression {
                Expr::Literal(_)
                | Expr::Regex(_)
                | Expr::Function(_)
                | Expr::This
                | Expr::Binding(_)
                | Expr::Member { .. }
                | Expr::Conditional { .. }
                | Expr::Sequence(_) => true,
                Expr::Host(name) => self.pristine_builtins && inline::is_standard_global(name),
                Expr::Array(items) => items
                    .iter()
                    .all(|item| !matches!(self.expressions[item.index()], Expr::Spread(_))),
                Expr::Object(entries) => entries.iter().all(|(key, _)| match key {
                    Property::Named(_) => true,
                    Property::Computed(key) => {
                        matches!(self.expressions[key.index()], Expr::Literal(_))
                    }
                }),
                Expr::Unary { op, .. } => matches!(op, Unary::Not | Unary::TypeOf | Unary::Void),
                Expr::Binary { op, .. } => matches!(
                    op,
                    Binary::StrictEqual
                        | Binary::StrictNotEqual
                        | Binary::And
                        | Binary::Or
                        | Binary::Nullish
                ),
                _ => false,
            };
            if !reads {
                return Ok(false);
            }
            if expression.created_function().is_none() {
                let _ = expression.visit_children(|child| {
                    pending.push(child);
                    Ok::<_, ()>(())
                });
            }
        }
        Ok(true)
    }

    /// `Math.max` under pristine builtins: a named path into a standard global.
    pub(super) fn standard_member(&self, id: ExprId) -> bool {
        match &self.expressions[id.index()] {
            Expr::Host(name) => inline::is_standard_global(name),
            Expr::Member {
                object,
                property: Property::Named(_),
            } => self.standard_member(*object),
            _ => false,
        }
    }
}
