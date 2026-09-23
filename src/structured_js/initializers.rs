//! Field initializers become the stores they make.
//!
//! A class's `init` that only stores its parameters (and literals) into
//! fields of its receiver is a function `(o,a,b)=>{o.x=a;o.y=b}`, called once
//! per construction right after the object's own literal:
//! `let o={x:null,y:null};f(o,v,w);`. At such a call it becomes its stores,
//! `o.x=v;o.y=w;`, which the store fold then writes into the literal,
//! `let o={x:v,y:w}`: a construction is its literal, and a function every
//! construction called goes.
//!
//! The rewrite is exact where it applies:
//! * The object is fresh: the call directly follows its literal, and no
//!   argument mentions it, so nothing can observe the stores' interleaving
//!   with the arguments' evaluation.
//! * Arguments keep their order: those with effects are read by the stores
//!   in argument order, and an argument no store reads runs nothing.
//! * A parameter the body defaults (`if(a===void 0)a=D`) receives a literal
//!   at the call, so the default is decided here: `D` for `void 0`, else the
//!   literal itself. Any other argument keeps the call.
//! * The body reads no frame of its own and suspends nothing. Each such
//!   call is rewritten on its own; other uses keep calling the function,
//!   which its binding (never assigned) always holds.
//!
//! The store fold needs pristine builtins (a literal key defines a property
//! where a store could run an inherited setter), so the caller applies this
//! only under them.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// A field-initializing body: the defaults it applies to parameters, and its
/// stores in order.
struct Initializer {
    function: FunctionId,
    /// For each parameter, the literal the body gives it for `undefined`.
    defaults: Vec<Option<Literal>>,
    /// Each store: the key, and the parameter or literal it stores.
    stores: Vec<(String, Stored)>,
}

#[derive(Clone)]
enum Stored {
    Parameter(usize),
    Literal(Literal),
}

impl Module {
    /// Rewrite each field-initializer call that follows its object's literal
    /// into the stores it makes. Returns how many calls.
    pub(crate) fn inline_initializers(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let reach = self.reach(budget)?;
        // Every reference, and those that are a call statement's callee.
        let mut uses = vec![0usize; self.bindings.len()];
        let mut written = vec![false; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            match &self.expressions[id.index()] {
                Expr::Binding(binding) => uses[binding.index()] += 1,
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = self.expressions[target.index()] {
                        written[binding.index()] = true;
                    }
                }
                _ => {}
            }
        }
        for export in &self.exports {
            written[export.binding.index()] = true;
        }
        let mut call_statements = vec![0usize; self.bindings.len()];
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                if let Some((callee, _)) = self.call_statement(statement) {
                    call_statements[callee.index()] += 1;
                }
            }
        }
        let mut initializers: Vec<Option<(RegionId, usize, Initializer)>> = Vec::new();
        for &region in &reach.regions {
            for (at, statement) in self.regions[region.index()].statements.iter().enumerate() {
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
                // Each qualifying construction is rewritten on its own: other
                // uses keep calling the function, which the binding always
                // holds.
                if written[binding.index()]
                    || self.bindings[binding.index()].pinned
                    || call_statements[binding.index()] == 0
                {
                    continue;
                }
                if let Some(initializer) = self.initializer(function) {
                    if initializers.len() <= binding.index() {
                        initializers.resize_with(binding.index() + 1, || None);
                    }
                    initializers[binding.index()] = Some((region, at, initializer));
                }
            }
        }
        if initializers.iter().all(Option::is_none) {
            return Ok(0);
        }
        // A call must run after the declaration: the declaring region's
        // statement holding it follows the declaration and hoists nothing.
        let parents = self.region_parents(budget)?;
        let follows = |site: (RegionId, usize), declaring: RegionId, at: usize| {
            let mut path = site;
            for _ in 0..verify::MAX_NESTING * 4 {
                if path.0 == declaring {
                    return path.1 > at
                        && !matches!(
                            self.regions[declaring.index()].statements[path.1],
                            Statement::Function { .. }
                        );
                }
                match parents[path.0.index()] {
                    Some(parent) => path = parent,
                    None => return false,
                }
            }
            false
        };
        let mut sites = Vec::new();
        for &region in &reach.regions {
            let statements = &self.regions[region.index()].statements;
            for index in 1..statements.len() {
                budget.work(Analysis, 1)?;
                let Some((callee, call)) = self.call_statement(&statements[index]) else {
                    continue;
                };
                let Some(Some((declaring, at, initializer))) = initializers.get(callee.index())
                else {
                    continue;
                };
                if !follows((region, index), *declaring, *at) {
                    continue;
                }
                if let Some(stores) =
                    self.site_stores(initializer, call, &statements[index - 1], budget)?
                {
                    sites.push((region, index, stores));
                }
            }
        }
        let count = sites.len();
        for (region, index, stores) in sites.into_iter().rev() {
            let mut replacement = Vec::with_capacity(stores.len());
            for (object, key, value) in stores {
                let object = self.expression_in(Expr::Binding(object), None, budget)?;
                let target = self.expression_in(
                    Expr::Member {
                        object,
                        property: Property::Named(key),
                    },
                    None,
                    budget,
                )?;
                let value = match value {
                    Value::Moved(id) => id,
                    Value::Literal(literal) => {
                        self.expression_in(Expr::Literal(literal), None, budget)?
                    }
                };
                let assign = self.expression_in(Expr::Assign { target, value }, None, budget)?;
                replacement.push(Statement::Evaluate(assign));
            }
            let added = replacement.len() - 1;
            self.regions[region.index()]
                .statements
                .splice(index..=index, replacement);
            if region == self.root && index < self.root_modules.len() {
                let module = self.root_modules[index];
                for _ in 0..added {
                    self.root_modules.insert(index, module);
                }
            }
        }
        Ok(count)
    }

    /// An initializer's store that writes, before anything can see the
    /// object, the value every construction's fresh literal already holds
    /// is no store at all: `let o={a:null,m:new Map};init(o,x)` with
    /// `init=(p,x)=>{p.a=null;p.m=new Map;…}` keeps the literal and loses
    /// both stores. The literal's key order and layout stay as they are.
    ///
    /// A construction is `let o={…};init(o,…)` or `(o={…},init(o,…),o)`, the
    /// arguments not mentioning `o`; every use of `init` must be one (or the
    /// initializer is created at its one construction). In the body, a
    /// statement that does not mention the receiver cannot see the fresh
    /// object, so the walk passes it; it stops at the first other statement
    /// that mentions the receiver. Values are inert (literals, functions
    /// excepted, arrays and objects of them, empty Maps and Sets), so a
    /// fresh one equal in shape is as good as the literal's. Returns how
    /// many stores went.
    pub(crate) fn drop_redundant_init_stores(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let reach = self.reach(budget)?;
        let mut uses = vec![0usize; self.bindings.len()];
        let mut written = vec![false; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            match &self.expressions[id.index()] {
                Expr::Binding(binding) => uses[binding.index()] += 1,
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = self.expressions[target.index()] {
                        written[binding.index()] = true;
                    }
                }
                _ => {}
            }
        }
        for export in &self.exports {
            written[export.binding.index()] = true;
        }
        // Every construction: its initializer, its literal, and where its
        // call stands.
        let mut constructions: Vec<(Initialized, ExprId, Site)> = Vec::new();
        let mut callee_sites = vec![0usize; self.bindings.len()];
        let mut declared: Vec<Option<FunctionId>> = vec![None; self.bindings.len()];
        for &region in &reach.regions {
            let statements = &self.regions[region.index()].statements;
            for (index, statement) in statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                if let Statement::Let {
                    binding,
                    value: Some(value),
                } = *statement
                {
                    if let Expr::Function(function) = self.expressions[value.index()] {
                        declared[binding.index()] = Some(function);
                    }
                }
                // `let o={…};init(o,…)`
                if index > 0 {
                    if let (
                        Statement::Let {
                            binding: object,
                            value: Some(literal),
                        },
                        Statement::Evaluate(call),
                    ) = (&statements[index - 1], statement)
                    {
                        if let Some(site) = self.construction(*call, *object, *literal, budget)? {
                            if let Initialized::Named(callee) = site {
                                callee_sites[callee.index()] += 1;
                            }
                            constructions.push((site, *literal, Site::Statement(region, index, *call)));
                        }
                    }
                }
            }
        }
        // `(o={…},init(o,…),o)`
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Sequence(items) = &self.expressions[id.index()] else {
                continue;
            };
            for (at, pair) in items.windows(2).enumerate() {
                let Expr::Assign { target, value } = self.expressions[pair[0].index()] else {
                    continue;
                };
                let Expr::Binding(object) = self.expressions[target.index()] else {
                    continue;
                };
                if let Some(site) = self.construction(pair[1], object, value, budget)? {
                    if let Initialized::Named(callee) = site {
                        callee_sites[callee.index()] += 1;
                    }
                    constructions.push((site, value, Site::Item(id, at + 1, pair[1])));
                }
            }
        }
        // An initializer qualifies when all its uses are constructions: a
        // named one by count, one created at its call by being there.
        let mut sites: Vec<(FunctionId, Vec<ExprId>, Vec<Site>)> = Vec::new();
        for &(initialized, literal, site) in &constructions {
            budget.work(Analysis, 1)?;
            let function = match initialized {
                Initialized::Created(function) => function,
                Initialized::Named(binding) => {
                    let Some(function) = declared[binding.index()] else {
                        continue;
                    };
                    if written[binding.index()]
                        || self.bindings[binding.index()].pinned
                        || uses[binding.index()] != callee_sites[binding.index()]
                    {
                        continue;
                    }
                    function
                }
            };
            match sites.iter_mut().find(|(found, _, _)| *found == function) {
                Some((_, literals, calls)) => {
                    literals.push(literal);
                    calls.push(site);
                }
                None => sites.push((function, vec![literal], vec![site])),
            }
        }
        let mut dropped = 0;
        // Calls of initializers left with nothing to do.
        let mut emptied: Vec<Site> = Vec::new();
        for (function, literals, calls) in sites {
            budget.work(Analysis, 1)?;
            let Some(&receiver) = self.functions[function.index()].parameters.first() else {
                continue;
            };
            let body = self.functions[function.index()].body;
            let mut seen: Vec<String> = Vec::new();
            let mut redundant = Vec::new();
            for (at, statement) in self.regions[body.index()].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                if !self.statement_mentions(statement, receiver) {
                    continue;
                }
                let Statement::Evaluate(store) = *statement else {
                    break;
                };
                let Some((Property::Named(key), value)) = self.object_store(store, receiver) else {
                    break;
                };
                if self.mentions_within(value, receiver, budget)? {
                    break;
                }
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key.clone());
                if !self.inert_value(value, budget)?
                    || matches!(self.expressions[value.index()], Expr::Function(_))
                {
                    continue;
                }
                let mut everywhere = true;
                for &literal in &literals {
                    let Expr::Object(entries) = &self.expressions[literal.index()] else {
                        everywhere = false;
                        break;
                    };
                    let held = entries
                        .iter()
                        .rev()
                        .find(|(entry, _)| matches!(entry, Property::Named(name) if *name == key));
                    everywhere &= held.is_some_and(|&(_, held)| self.same_inert(held, value));
                }
                if everywhere {
                    redundant.push(at);
                }
            }
            for &at in redundant.iter().rev() {
                self.regions[body.index()].statements.remove(at);
            }
            dropped += redundant.len();
            if self.regions[body.index()].statements.is_empty()
                && !self.functions[function.index()]
                    .parameters
                    .iter()
                    .any(|parameter| self.bindings[parameter.index()].pinned)
            {
                for &site in &calls {
                    let call = match site {
                        Site::Statement(_, _, call) | Site::Item(_, _, call) => call,
                    };
                    let Expr::Call { arguments, .. } = &self.expressions[call.index()] else {
                        continue;
                    };
                    let mut inert = true;
                    for &argument in &arguments[1..] {
                        inert = inert && self.inert_value(argument, budget)?;
                    }
                    if inert {
                        emptied.push(site);
                    }
                }
            }
        }
        // Remove those calls: statements from the last, items likewise.
        emptied.sort_unstable_by_key(|site| match *site {
            Site::Statement(region, index, _) => std::cmp::Reverse((0, region.index(), index)),
            Site::Item(sequence, at, _) => std::cmp::Reverse((1, sequence.index(), at)),
        });
        for site in emptied {
            match site {
                Site::Statement(region, _, call) => {
                    // Found again: an emptied body may have shifted it.
                    let Some(index) = self.regions[region.index()]
                        .statements
                        .iter()
                        .position(|statement| matches!(statement, Statement::Evaluate(found) if *found == call))
                    else {
                        continue;
                    };
                    self.regions[region.index()].statements.remove(index);
                    if region == self.root && index < self.root_modules.len() {
                        self.root_modules.remove(index);
                    }
                }
                Site::Item(sequence, _, call) => {
                    if let Expr::Sequence(items) = &mut self.expressions[sequence.index()] {
                        if let Some(at) = items.iter().position(|&item| item == call) {
                            if items.len() > 2 {
                                items.remove(at);
                            }
                        }
                    }
                }
            }
        }
        Ok(dropped)
    }

    /// A construction: `init(o,…)` right after `o` receives the literal,
    /// with the other arguments not mentioning `o`: the initializer, created
    /// at the call or named by a binding.
    fn construction(
        &self,
        call: ExprId,
        object: BindingId,
        literal: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<Initialized>, AllocationError> {
        if !matches!(self.expressions[literal.index()], Expr::Object(_)) {
            return Ok(None);
        }
        let Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value | Invocation::Reference,
        } = &self.expressions[call.index()]
        else {
            return Ok(None);
        };
        if !arguments
            .first()
            .is_some_and(|first| matches!(self.expressions[first.index()], Expr::Binding(found) if found == object))
        {
            return Ok(None);
        }
        for &argument in &arguments[1..] {
            if self.mentions_within(argument, object, budget)? {
                return Ok(None);
            }
        }
        Ok(match self.expressions[callee.index()] {
            Expr::Function(function) => Some(Initialized::Created(function)),
            Expr::Binding(binding) => Some(Initialized::Named(binding)),
            _ => None,
        })
    }

    /// Two inert values of the same shape: equal literals (numbers by bits),
    /// arrays and objects of such, or empty Maps or Sets of one kind.
    fn same_inert(&self, left: ExprId, right: ExprId) -> bool {
        match (&self.expressions[left.index()], &self.expressions[right.index()]) {
            (Expr::Literal(Literal::Number(a)), Expr::Literal(Literal::Number(b))) => a.to_bits() == b.to_bits(),
            (Expr::Literal(a), Expr::Literal(b)) => a == b,
            (Expr::Array(a), Expr::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| self.same_inert(*x, *y))
            }
            (Expr::Object(a), Expr::Object(b)) => {
                a.len() == b.len()
                    && a.iter().zip(b).all(|((kx, x), (ky, y))| match (kx, ky) {
                        (Property::Named(kx), Property::Named(ky)) => kx == ky && self.same_inert(*x, *y),
                        _ => false,
                    })
            }
            (
                Expr::ConstructIntrinsic {
                    operation: a,
                    arguments: x,
                },
                Expr::ConstructIntrinsic {
                    operation: b,
                    arguments: y,
                },
            ) => a == b && x.is_empty() && y.is_empty(),
            _ => false,
        }
    }

    /// `f(…);` with a binding callee: the callee and the call.
    fn call_statement(&self, statement: &Statement) -> Option<(BindingId, ExprId)> {
        let Statement::Evaluate(call) = *statement else {
            return None;
        };
        let Expr::Call {
            callee,
            invocation: Invocation::Value | Invocation::Reference,
            ..
        } = &self.expressions[call.index()]
        else {
            return None;
        };
        match self.expressions[callee.index()] {
            Expr::Binding(binding) => Some((binding, call)),
            _ => None,
        }
    }

    /// The initializer `function` is, if its body only defaults parameters
    /// and then stores parameters or literals into its first parameter.
    fn initializer(&self, function: FunctionId) -> Option<Initializer> {
        let declared = &self.functions[function.index()];
        if declared.suspension != Suspension::None
            || declared.length.is_some()
            || declared.strict
            || declared.parameters.is_empty()
            || !self.frame_free(function)
        {
            return None;
        }
        let parameters = &declared.parameters;
        let position = |binding: BindingId| parameters.iter().position(|&p| p == binding);
        let mut defaults = vec![None; parameters.len()];
        let mut stores = Vec::new();
        let mut read = vec![false; parameters.len()];
        let statements = &self.regions[declared.body.index()].statements;
        for (at, statement) in statements.iter().enumerate() {
            match statement {
                // `if(p===void 0)p=D`, before any store.
                Statement::If {
                    condition,
                    yes,
                    no: None,
                } if stores.is_empty() => {
                    let Expr::Binary {
                        op: Binary::StrictEqual,
                        left,
                        right,
                    } = &self.expressions[condition.index()]
                    else {
                        return None;
                    };
                    let tested = match (
                        &self.expressions[left.index()],
                        &self.expressions[right.index()],
                    ) {
                        (Expr::Binding(tested), Expr::Literal(Literal::Undefined))
                        | (Expr::Literal(Literal::Undefined), Expr::Binding(tested)) => *tested,
                        _ => return None,
                    };
                    let index = position(tested).filter(|&index| index > 0)?;
                    let [Statement::Evaluate(assign)] = self.regions[yes.index()].statements[..]
                    else {
                        return None;
                    };
                    let Expr::Assign { target, value } = &self.expressions[assign.index()] else {
                        return None;
                    };
                    let (Expr::Binding(target), Expr::Literal(default)) = (
                        &self.expressions[target.index()],
                        &self.expressions[value.index()],
                    ) else {
                        return None;
                    };
                    if *target != tested || defaults[index].is_some() {
                        return None;
                    }
                    defaults[index] = Some(default.clone());
                }
                Statement::Evaluate(assign) => {
                    let Expr::Assign { target, value } = &self.expressions[assign.index()] else {
                        return None;
                    };
                    let Expr::Member {
                        object,
                        property: Property::Named(key),
                    } = &self.expressions[target.index()]
                    else {
                        return None;
                    };
                    if !matches!(self.expressions[object.index()], Expr::Binding(receiver) if receiver == parameters[0])
                        || key == "__proto__"
                    {
                        return None;
                    }
                    let stored = match &self.expressions[value.index()] {
                        Expr::Binding(binding) => {
                            let index = position(*binding).filter(|&index| index > 0)?;
                            if std::mem::replace(&mut read[index], true) {
                                return None;
                            }
                            Stored::Parameter(index)
                        }
                        Expr::Literal(literal) => Stored::Literal(literal.clone()),
                        _ => return None,
                    };
                    stores.push((key.clone(), stored));
                }
                Statement::Return(None) if at + 1 == statements.len() => {}
                _ => return None,
            }
        }
        (!stores.is_empty()).then_some(Initializer {
            function,
            defaults,
            stores,
        })
    }

    /// The stores the call makes at this site, when it follows its object's
    /// literal and its arguments allow: object, key and value per store.
    fn site_stores(
        &self,
        initializer: &Initializer,
        call: ExprId,
        previous: &Statement,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<Vec<(BindingId, String, Value)>>, AllocationError> {
        let Expr::Call { arguments, .. } = &self.expressions[call.index()] else {
            return Ok(None);
        };
        let parameters = &self.functions[initializer.function.index()].parameters;
        if arguments.len() != parameters.len()
            || arguments
                .iter()
                .any(|argument| matches!(self.expressions[argument.index()], Expr::Spread(_)))
        {
            return Ok(None);
        }
        let Expr::Binding(object) = self.expressions[arguments[0].index()] else {
            return Ok(None);
        };
        // The object's own literal, just before, with every stored key.
        let Statement::Let {
            binding,
            value: Some(literal),
        } = *previous
        else {
            return Ok(None);
        };
        let Expr::Object(entries) = &self.expressions[literal.index()] else {
            return Ok(None);
        };
        if binding != object
            || !initializer.stores.iter().all(|(key, _)| {
                entries
                    .iter()
                    .any(|(entry, _)| matches!(entry, Property::Named(name) if name == key))
            })
        {
            return Ok(None);
        }
        // No argument can see the object, and the ones that run code are read
        // in their own order.
        let mut last = 0;
        for (index, &argument) in arguments.iter().enumerate().skip(1) {
            budget.work(Analysis, 1)?;
            if self.mentions_within(argument, object, budget)? {
                return Ok(None);
            }
            let read = initializer
                .stores
                .iter()
                .position(|(_, stored)| matches!(stored, Stored::Parameter(p) if *p == index));
            let inert = self.inert_value(argument, budget)?;
            match read {
                None if !inert => return Ok(None),
                Some(at) if !inert => {
                    if at < last {
                        return Ok(None);
                    }
                    last = at;
                }
                _ => {}
            }
        }
        let mut stores = Vec::with_capacity(initializer.stores.len());
        for (key, stored) in &initializer.stores {
            let value = match stored {
                Stored::Literal(literal) => Value::Literal(literal.clone()),
                Stored::Parameter(index) => {
                    let argument = arguments[*index];
                    match (
                        &initializer.defaults[*index],
                        &self.expressions[argument.index()],
                    ) {
                        (None, _) => Value::Moved(argument),
                        (Some(default), Expr::Literal(Literal::Undefined)) => {
                            Value::Literal(default.clone())
                        }
                        // A typed argument is never `undefined`.
                        (Some(_), _)
                            if self.never_undefined(argument)
                                || self.defined_parameters.contains(&parameters[*index]) =>
                        {
                            Value::Moved(argument)
                        }
                        (Some(_), _) => return Ok(None),
                    }
                }
            };
            stores.push((object, key.clone(), value));
        }
        Ok(Some(stores))
    }
}

impl Module {
    /// Whether `id` can never evaluate to `undefined`: a value the body's
    /// default would leave as it is.
    fn never_undefined(&self, id: ExprId) -> bool {
        match &self.expressions[id.index()] {
            Expr::Literal(Literal::Undefined) => false,
            Expr::Literal(_)
            | Expr::Function(_)
            | Expr::Object(_)
            | Expr::Array(_)
            | Expr::Template(_)
            | Expr::Regex(_)
            | Expr::Construct { .. }
            | Expr::ToInt32(_)
            | Expr::IntBinary { .. }
            | Expr::IntNegate(_) => true,
            // Only `&&`, `||` and `??` can yield an operand as it is.
            Expr::Binary { op, .. } => !matches!(op, Binary::And | Binary::Or | Binary::Nullish),
            Expr::Unary { op, .. } => *op != Unary::Void,
            _ => self.known(id).is_some(),
        }
    }
}

/// Where a construction's call stands: a call statement (region, index), or
/// an item of a sequence expression (sequence, position).
#[derive(Clone, Copy)]
enum Site {
    Statement(RegionId, usize, ExprId),
    Item(ExprId, usize, ExprId),
}

/// The initializer of a construction.
#[derive(Clone, Copy)]
enum Initialized {
    /// Created at the call: its one use.
    Created(FunctionId),
    /// A binding's function; every use must be a construction.
    Named(BindingId),
}

/// A store's value at a site: an argument moved in, or a literal.
enum Value {
    Moved(ExprId),
    Literal(Literal),
}
