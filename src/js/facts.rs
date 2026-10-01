//! Physical binding facts after formation. Every query reads the current
//! target tree, including storage/functions created by representation choices.
//! Source initialization points refine this transfer; unknown points use the
//! same lexical/control-flow owner, never a separate optimization proof.
//! Results are query-local and cannot survive edits. The edit journal provides
//! dependency invalidation for future cached clients.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// Physical storage facts, accumulated by the admitted reachable-tree walk.
/// Source annotations describe values; these rows describe all writes and
/// observations of the bindings that actually survive target formation.
/// A row is valid only for that walk's tree revision, never across an edit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct BindingUse {
    pub(super) mentions: usize,
    pub(super) calls: usize,
    pub(super) assignments: usize,
    pub(super) written: bool,
    pub(super) implicit_writes: bool,
    pub(super) external: bool,
    /// None: no initialized value yet; Some(false): a non-array value exists.
    arrays: Option<bool>,
    factory: Option<FunctionId>,
}

impl BindingUse {
    pub(super) fn fixed(self) -> bool { !self.written && !self.external }

    pub(super) fn literal_array(self) -> bool {
        !self.external && self.arrays == Some(true)
    }

    pub(super) fn factory(self, module: &Module) -> bool {
        self.fixed() && self.factory.is_some_and(|function| {
            matches!(module.regions[module.functions[function.index()].body.index()].statements[..],
                [Statement::Return(Some(value))] if matches!(module.expressions[value.index()], Expr::Function(_)))
        })
    }

    pub(super) fn value(&mut self, module: &Module, value: ExprId) {
        let array = matches!(module.expressions[value.index()], Expr::Array(_));
        self.arrays = Some(self.arrays.unwrap_or(true) && array);
        self.factory = match module.expressions[value.index()] {
            Expr::Function(function) => Some(function),
            _ => None,
        };
    }

    pub(super) fn parameter(&mut self) { self.arrays = Some(false); }

    pub(super) fn implicit_write(&mut self) {
        self.written = true;
        self.implicit_writes = true;
        self.arrays = Some(false);
        self.factory = None;
    }

    pub(super) fn declaration(&mut self, function: FunctionId) {
        self.arrays = Some(false);
        self.factory = Some(function);
    }
}

/// When a root declaration initializes its binding, or when a function is
/// first created: a hoisted declaration before any root statement runs,
/// otherwise during that root statement.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Moment {
    Hoisted,
    At(usize),
}

/// Facts about the whole module that the quiet test reads.
pub(super) struct BindingFacts {
    /// Bindings some code assigns, a loop head binds, or an import holds.
    written: Vec<bool>,
    literals: Vec<Option<(usize, ExprId)>>,
    /// Root declarations.
    declared: Vec<Option<Moment>>,
    /// The root statement whose evaluation first creates each function (a
    /// function nested in another is created no earlier than it).
    created: Vec<Option<Moment>>,
    /// The function each region belongs to; `Some(None)` for the root's.
    pub(super) owner: Vec<Option<Option<FunctionId>>>,
    /// For each root statement, the first one from there on that may run
    /// program code (the root's length when none does). A function created
    /// by root statement `c` cannot run before `runs_from[c]`: only running
    /// code can call it, and importers call exports after the whole root.
    runs_from: Vec<usize>,
    /// The program's order (M6.5), where formation carried it: the root
    /// point settling each binding's module cell, and the first point
    /// during which each function may run.
    settled: Vec<Option<u32>>,
    first_runs: Vec<Option<u32>>,
    /// A multi-entry or lazy program has no single end after which all
    /// root cells are initialized. Syntax alone cannot supply that proof.
    separate_initialization: bool,
}

impl BindingFacts {
    pub(super) fn literal(&self, binding: BindingId) -> Option<(usize, ExprId)> {
        if self.written(binding) {
            None
        } else {
            self.literals[binding.index()]
        }
    }

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
    /// value whenever `function` runs. The program's initialization owner
    /// answers first: the cell settles before the first point during which
    /// the function's unit may run, by the call graph rather than by which
    /// root statements run code at all.
    pub(super) fn initialized_in(&self, binding: BindingId, function: FunctionId) -> bool {
        let settled = self.settled.get(binding.index()).copied().flatten();
        let first = self.first_runs.get(function.index()).copied().flatten();
        if let Some((settled, first)) = settled.zip(first) {
            return first > settled;
        }
        if self.separate_initialization {
            return matches!(self.declared[binding.index()], Some(Moment::Hoisted));
        }
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

impl Module {
    pub(super) fn order(
        &self,
        frames: &Frames,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<BindingFacts, AllocationError> {
        let retained = budget.retained_bytes(AllocationClass::Retained);
        let reach = self.reach_tree(budget)?;
        let reach_bytes = budget.retained_bytes(AllocationClass::Retained) - retained;
        let mut written = budget.vector(AllocationClass::Scratch, reach.bindings.len())?;
        budget.work(Analysis, reach.bindings.len() as u64)?;
        written.extend(reach.bindings.iter().map(|row| row.written));
        let mut literals = budget.filled(AllocationClass::Scratch, self.bindings.len(), None)?;
        let mut declared = budget.filled(AllocationClass::Scratch, self.bindings.len(), None)?;
        let mut created = budget.filled(AllocationClass::Scratch, self.functions.len(), None)?;
        for (index, statement) in self.regions[self.root.index()]
            .statements
            .iter()
            .enumerate()
        {
            let moment = match statement {
                Statement::Function { binding, .. } => {
                    declared[binding.index()] = Some(Moment::Hoisted);
                    Moment::Hoisted
                }
                Statement::Let { binding, value } => {
                    if let Some(value) = value {
                        if matches!(self.expressions[value.index()], Expr::Literal(_))
                            && !self.observed(*value)
                        {
                            literals[binding.index()] = Some((index, *value));
                        }
                    }
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
                    for function in expression.created_functions() {
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
        let mut runs_from =
            budget.filled(AllocationClass::Scratch, statements.len(), statements.len())?;
        let mut next = statements.len();
        for index in (0..statements.len()).rev() {
            budget.work(Analysis, 1)?;
            // Quiet: cannot call program code. A read that throws (a global
            // that is missing, a binding before its declaration) stops the
            // root before any function could run, so it is quiet too.
            let quiet = match statements[index] {
                Statement::Function { .. } | Statement::Let { value: None, .. } => true,
                Statement::Let {
                    value: Some(value), ..
                }
                | Statement::Evaluate(value) => self.creates_only(value, &reach.bindings, budget)?,
                _ => false,
            };
            if !quiet {
                next = index;
            }
            runs_from[index] = next;
        }
        drop(reach);
        budget.release(AllocationClass::Retained, reach_bytes)?;
        Ok(BindingFacts {
            written,
            literals,
            declared,
            created,
            owner,
            runs_from,
            settled: budget.copy_slice(AllocationClass::Scratch, &self.settled)?,
            first_runs: budget.copy_slice(AllocationClass::Scratch, &self.first_runs)?,
            separate_initialization: !self.entries.is_empty()
                || self.expressions.iter().any(|expression| matches!(expression, Expr::LoadModule { .. })),
        })
    }

    /// Whether evaluating `value` cannot call program code: a binding read, a
    /// standard global (pristine builtins), an inert value, a call of a
    /// function factory with such arguments, and arrays and objects of those.
    fn creates_only(
        &self,
        value: ExprId,
        bindings: &[BindingUse],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(Analysis, 1)?;
        // A regular expression literal creates an object and calls nothing.
        if matches!(
            self.expressions[value.index()],
            Expr::Binding(_) | Expr::Regex(_)
        ) || self.pristine_builtins && self.standard_member(value)
            || self.inert_value(value, budget)?
        {
            return Ok(true);
        }
        Ok(match &self.expressions[value.index()] {
            Expr::Call {
                callee, arguments, ..
            } => {
                let factory = matches!(self.expressions[callee.index()], Expr::Binding(binding) if bindings[binding.index()].factory(self));
                let mut quiet = factory;
                for &argument in arguments {
                    quiet = quiet && self.creates_only(argument, bindings, budget)?;
                }
                quiet
            }
            Expr::Array(items) => {
                let mut quiet = true;
                for &item in items {
                    quiet = quiet && self.creates_only(item, bindings, budget)?;
                }
                quiet
            }
            Expr::Object(entries) => {
                let mut quiet = true;
                for (key, item) in entries {
                    quiet = quiet
                        && !matches!(key, Property::Computed(_))
                        && self.creates_only(*item, bindings, budget)?;
                }
                quiet
            }
            _ => false,
        })
    }

    /// Whether code of `owner` runs only after root statement `index` has
    /// run: a later root statement, or a function created by one.
    pub(super) fn runs_after_root(&self, owner: Owner, index: usize, order: &BindingFacts) -> bool {
        match owner {
            Owner::Root(at) => at > index && (!order.separate_initialization
                || self.root_rows.get(at).zip(self.root_rows.get(index)).is_some_and(|(at, before)|
                    at.module == before.module || at.point.zip(before.point).is_some_and(|(at, before)| at > before))),
            Owner::Function(function) => {
                // The program's answer first (M6.5): the function's unit
                // first runs after the statement's last operation.
                let first = order.first_runs.get(function.index()).copied().flatten();
                let point = self.root_rows.get(index).and_then(|row| row.point);
                match first.zip(point) {
                    Some((first, point)) => first > point,
                    None => !order.separate_initialization
                        && order.first_run(function).is_some_and(|first| first > index),
                }
            }
        }
    }

    /// Each reachable expression's owner.
    pub(super) fn expression_owners(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<Option<Owner>>, AllocationError> {
        let mut owners = budget.filled(AllocationClass::Scratch, self.expressions.len(), None)?;
        let mut regions: Vec<(RegionId, Option<FunctionId>, usize)> = Vec::new();
        for (index, statement) in self.regions[self.root.index()]
            .statements
            .iter()
            .enumerate()
        {
            let mut expressions = Vec::new();
            statement.visit_expressions(|root| expressions.push((root, None)));
            statement.visit_regions(|child| regions.push((child, None, index)));
            if let Statement::Function { function, .. } = statement {
                regions.push((
                    self.functions[function.index()].body,
                    Some(*function),
                    index,
                ));
            }
            loop {
                if let Some((id, function)) = expressions.pop() {
                    budget.work(Analysis, 1)?;
                    owners[id.index()] = Some(match function {
                        Some(function) => Owner::Function(function),
                        None => Owner::Root(index),
                    });
                    let expression = &self.expressions[id.index()];
                    for created in expression.created_functions() {
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
                        regions.push((
                            self.functions[function.index()].body,
                            Some(*function),
                            index,
                        ));
                    }
                }
            }
        }
        Ok(owners)
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
        order: &BindingFacts,
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

    /// A pinned standard host binding, distinct from a live module import.
    pub(super) fn standard_global(&self, binding: BindingId) -> bool {
        let declared = &self.bindings[binding.index()];
        declared.pinned
            && matches!(crate::catalog::host_kind(&declared.spelling), crate::catalog::HostKind::Standard(_))
            && !self.imports.iter().any(|import| import.binding == binding)
    }

    /// A checked standard global or a statically named path from one.
    pub(super) fn static_property_key(&self, property: &Property) -> bool {
        match property {
            Property::Named(_) => true,
            Property::Computed(key) => matches!(self.expressions[key.index()], Expr::Literal(Literal::String(_))),
        }
    }

    pub(super) fn standard_member(&self, id: ExprId) -> bool {
        match &self.expressions[id.index()] {
            Expr::Host(host) => matches!(host.kind, crate::catalog::HostKind::Standard(_)),
            Expr::Binding(binding) => self.standard_global(*binding),
            Expr::Member { object, property } => self.static_property_key(property) && self.standard_member(*object),
            _ => false,
        }
    }

    /// Frame-elision queries share the operation transfer. Binding reads can
    /// throw on TDZ without running user code; moving/discarding such a read
    /// still requires the separate initialization proof.
    pub(super) fn runs_no_user_code(&self, root: ExprId) -> bool {
        let node = &self.expressions[root.index()];
        let own = match node {
            Expr::Binding(binding) => !self.bindings[binding.index()].pinned
                || self.pristine_builtins && self.standard_global(*binding),
            _ => self.operation_behaviour(root).is_some_and(Behaviour::quiet),
        };
        let mut children = true;
        let _ = node.visit_children(|child| {
            children &= self.runs_no_user_code(child);
            Ok::<_, ()>(())
        });
        own && children
    }

}

impl Module {
    /// Evaluation of this operation alone. Child evaluation and binding
    /// initialization remain separate obligations. Stamped source rows take
    /// precedence; newly generated syntax uses the same target transfer.
    pub(crate) fn operation_behaviour(&self, id: ExprId) -> Option<Behaviour> {
        if let Some(known) = self.behaviour(id) {
            return Some(known);
        }
        let quiet = match &self.expressions[id.index()] {
            Expr::Literal(_)
            | Expr::Regex(_)
            | Expr::Function(_)
            | Expr::This
            | Expr::Array(_)
            | Expr::Sequence(_)
            | Expr::Conditional { .. } => true,
            Expr::Object(entries) => entries.iter().all(|(key, _)| match key {
                Property::Named(_) => true,
                Property::Computed(key) => {
                    matches!(self.expressions[key.index()], Expr::Literal(_))
                }
            }),
            Expr::Unary {
                op: Unary::Not | Unary::Void | Unary::TypeOf,
                ..
            } => true,
            Expr::Binary {
                op:
                    Binary::StrictEqual
                    | Binary::StrictNotEqual
                    | Binary::And
                    | Binary::Or
                    | Binary::Nullish,
                ..
            } => true,
            Expr::Host(host) => {
                self.pristine_builtins && matches!(host.kind, crate::catalog::HostKind::Standard(_))
            }
            Expr::Member { .. } => self.pristine_builtins && self.standard_member(id),
            _ => false,
        };
        quiet.then_some(Behaviour {
            reads: false,
            writes: false,
            throws: false,
            diverges: false,
            reenters: false,
            suspends: false,
        })
    }
}
