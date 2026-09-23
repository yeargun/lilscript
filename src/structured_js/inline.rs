//! Target-level inlining of single-expression arrow functions, and the arena
//! renumbering that lets an edit splice a subtree anywhere.
//!
//! `let f=(a,b)=>E; … f(x,y) …` becomes `… E[x/a,y/b] …` when evaluating `E`
//! with the arguments in place runs exactly what the call ran, in the same
//! order. The call evaluates `x`, then `y`, then `E`. So `E` must read each
//! parameter once, in parameter order, outside any branch, and before its last
//! parameter read it may only evaluate what cannot observe or change state:
//! literals, other parameter reads and, with pristine builtins, standard
//! globals and their properties. Everything after that read runs after every
//! argument in both forms. An arrow has no own `this`, `arguments` or frame a
//! program can observe, and `E` creates no function, so nothing captures an
//! argument. `f` is never reassigned or exported.

use super::*;
use crate::compilation_policy::WorkKind::Analysis;
use crate::output_budget::{AllocationBudget, AllocationError};

/// Standard globals whose values and properties pristine builtins fix.
const STANDARD_GLOBALS: &[&str] = &[
    "Array",
    "Boolean",
    "Date",
    "Error",
    "Infinity",
    "JSON",
    "Map",
    "Math",
    "NaN",
    "Number",
    "Object",
    "Promise",
    "RangeError",
    "Reflect",
    "RegExp",
    "Set",
    "String",
    "Symbol",
    "SyntaxError",
    "TypeError",
    "WeakMap",
    "WeakSet",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "isFinite",
    "isNaN",
    "parseFloat",
    "parseInt",
];

/// Whether `name` is a standard global that pristine builtins fix.
pub(super) fn is_standard_global(name: &str) -> bool {
    STANDARD_GLOBALS.contains(&name)
}

/// Call sites deeper than this keep their call, so no chain of inlined
/// bodies approaches the verifier's nesting limit.
const SITE_DEPTH: usize = verify::MAX_NESTING / 2;

struct Template {
    parameters: Vec<BindingId>,
    body: ExprId,
    /// How often the body reads each parameter.
    reads: Vec<usize>,
    /// The evaluation position of each parameter's first read, and whether
    /// it sits in a branch.
    first: Vec<Option<(usize, bool)>>,
    /// How many leading evaluations are inert.
    prefix: usize,
}

/// Every expression and region that can run, with each expression's depth as
/// the verifier counts it.
pub(super) struct Reach {
    pub(super) expressions: Vec<(ExprId, usize)>,
    pub(super) regions: Vec<RegionId>,
    /// Bindings mentioned by a function other than the one declaring them:
    /// a call can reach such a binding, and may write it.
    pub(super) captured: Vec<bool>,
    /// Function parameters: initialized before any code reads them.
    parameters: Vec<bool>,
}

impl Module {
    /// Inline each single-expression arrow whose body has at most `limit`
    /// nodes, or whose only call is inlined, at every call that passes its
    /// parameters. Returns the number of inlined calls and each old node's
    /// new id when the arena was renumbered.
    pub(crate) fn inline_expression_functions(
        &mut self,
        limit: usize,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(usize, Option<Vec<Option<ExprId>>>), AllocationError> {
        let reach = self.reach(budget)?;
        let mut written = vec![false; self.bindings.len()];
        let mut calls = vec![0usize; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            match &self.expressions[id.index()] {
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = self.expressions[target.index()] {
                        written[binding.index()] = true;
                    }
                }
                Expr::Call { callee, .. } => {
                    if let Expr::Binding(binding) = self.expressions[callee.index()] {
                        calls[binding.index()] += 1;
                    }
                }
                _ => {}
            }
        }
        for export in &self.exports {
            written[export.binding.index()] = true;
        }
        // Root constants declared with a literal before any root statement can
        // run code: every read inside a function finds them initialized.
        let mut early = vec![false; self.bindings.len()];
        for statement in &self.regions[self.root.index()].statements {
            budget.work(Analysis, 1)?;
            let Statement::Let { binding, value } = *statement else {
                break;
            };
            match value {
                Some(value) if !self.inert_value(value, budget)? => break,
                Some(value) => {
                    early[binding.index()] = !written[binding.index()]
                        && matches!(self.expressions[value.index()], Expr::Literal(_));
                }
                None => {}
            }
        }
        let mut templates: Vec<Option<Template>> = Vec::new();
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
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
                if written[binding.index()] || self.bindings[binding.index()].pinned {
                    continue;
                }
                let Some((body, nodes, reads, first, prefix)) =
                    self.template_body(function, binding, budget)?
                else {
                    continue;
                };
                // A sloppy script's user code, run from the body by a getter,
                // conversion or call, would see the arrow's frame as its
                // `arguments.callee.caller`: only a body that runs none loses it.
                if !strict && !self.runs_no_user_code(body) {
                    continue;
                }
                if nodes > limit && calls[binding.index()] != 1 {
                    continue;
                }
                if templates.len() <= binding.index() {
                    templates.resize_with(binding.index() + 1, || None);
                }
                templates[binding.index()] = Some(Template {
                    parameters: self.functions[function.index()].parameters.clone(),
                    body,
                    reads,
                    first,
                    prefix,
                });
            }
        }
        if templates.iter().all(Option::is_none) {
            return Ok((0, None));
        }
        let template = |binding: BindingId| templates.get(binding.index()).and_then(Option::as_ref);
        let mut sites = Vec::new();
        for &(id, depth) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Call {
                callee,
                arguments,
                invocation: Invocation::Value | Invocation::Reference,
            } = &self.expressions[id.index()]
            else {
                continue;
            };
            let Expr::Binding(binding) = self.expressions[callee.index()] else {
                continue;
            };
            let Some(found) = template(binding) else {
                continue;
            };
            if depth > SITE_DEPTH
                || arguments.len() != found.parameters.len()
                || arguments
                    .iter()
                    .any(|argument| matches!(self.expressions[argument.index()], Expr::Spread(_)))
            {
                continue;
            }
            // An argument read exactly once is read in argument order, outside a
            // branch, with only inert evaluations before the last such read.
            // One whose value cannot change may also be read again or not at
            // all. Where the call is the function's only one (so the function
            // goes), a stable argument may also be read at any time: measured,
            // doing that at every call grows markedlil and zodlil.
            let single = calls[binding.index()] == 1;
            let mut fits = true;
            let mut last: Option<usize> = None;
            for (index, &argument) in arguments.iter().enumerate() {
                let reads = found.reads[index];
                let stable = if single {
                    self.stable(argument, index, arguments, &reach, &written, &early, budget)?
                } else {
                    reads != 1 && self.repeatable(argument, index, arguments, &reach, budget)?
                };
                if (single && stable) || (reads == 0 && stable) {
                    continue;
                }
                if reads != 1 && !stable {
                    fits = false;
                    break;
                }
                match found.first[index] {
                    Some((position, false)) if last.is_none_or(|last| position > last) => {
                        last = Some(position);
                    }
                    _ => {
                        fits = false;
                        break;
                    }
                }
            }
            if !fits || last.is_some_and(|last| last >= found.prefix) {
                continue;
            }
            sites.push((id, binding));
        }
        // A site inside another template's body is edited in place, so later
        // copies of that body carry it inlined: the same evaluations either way.
        for &(site, binding) in &sites {
            let found = template(binding).unwrap();
            let Expr::Call { arguments, .. } = self.expressions[site.index()].clone() else {
                unreachable!("a recorded site is a call");
            };
            let origin = self.origins[site.index()];
            let mut placed = vec![false; arguments.len()];
            let root =
                self.clone_template(found, found.body, &arguments, &mut placed, origin, budget)?;
            self.expressions[site.index()] = root;
        }
        if sites.is_empty() {
            return Ok((0, None));
        }
        let map = self.renumber(budget)?;
        Ok((sites.len(), Some(map)))
    }

    /// `E` and its node count, when `function` is an arrow `(…)=>E` that a
    /// call site may take in place.
    fn template_body(
        &self,
        function: FunctionId,
        binding: BindingId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<(ExprId, usize, Vec<usize>, Vec<Option<(usize, bool)>>, usize)>, AllocationError>
    {
        let frame_free = self.frame_free(function);
        let function = &self.functions[function.index()];
        // A strict body keeps strict `delete` and assignment semantics that a
        // sloppy call site would not. A function that is not an arrow is its
        // body only while that reads no frame of its own.
        if !(function.arrow || frame_free)
            || function.strict
            || function.suspension != Suspension::None
            || function.length.is_some()
        {
            return Ok(None);
        }
        let [Statement::Return(Some(body))] = self.regions[function.body.index()].statements[..]
        else {
            return Ok(None);
        };
        // Evaluation order: each node after its operands, as JavaScript runs them.
        let mut events = Vec::new();
        if !self.evaluation_order(body, &function.parameters, false, &mut events, budget)? {
            return Ok(None);
        }
        let mut reads = vec![0usize; function.parameters.len()];
        let mut first = vec![None; function.parameters.len()];
        for (position, &(id, branch)) in events.iter().enumerate() {
            if let Expr::Binding(read) = self.expressions[id.index()] {
                if read == binding {
                    return Ok(None);
                }
                if let Some(index) = function.parameters.iter().position(|&p| p == read) {
                    first[index].get_or_insert((position, branch));
                    reads[index] += 1;
                }
            }
        }
        let prefix = events
            .iter()
            .take_while(|&&(id, _)| self.inert(id, &function.parameters))
            .count();
        Ok(Some((body, events.len(), reads, first, prefix)))
    }

    /// Appends `root`'s nodes in evaluation order, with whether each sits in
    /// a branch. False for anything a call site cannot take in place.
    fn evaluation_order(
        &self,
        root: ExprId,
        parameters: &[BindingId],
        branch: bool,
        events: &mut Vec<(ExprId, bool)>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(Analysis, 1)?;
        // A bound on the walk, which is linear: a single-use body moves whole
        // (a kernel's default object literal has sixty entries).
        if events.len() > 1024 {
            return Ok(false);
        }
        let expression = &self.expressions[root.index()];
        let operands_then_self = |this: &Self,
                                  operands: &[ExprId],
                                  events: &mut Vec<(ExprId, bool)>,
                                  budget: &mut AllocationBudget<'_>|
         -> Result<bool, AllocationError> {
            for &operand in operands {
                if !this.evaluation_order(operand, parameters, branch, events, budget)? {
                    return Ok(false);
                }
            }
            events.push((root, branch));
            Ok(true)
        };
        match expression {
            Expr::Literal(_) => events.push((root, branch)),
            Expr::Binding(_) => events.push((root, branch)),
            Expr::Host(name) => {
                if name == "arguments" || name == "eval" {
                    return Ok(false);
                }
                events.push((root, branch));
            }
            Expr::Unary {
                op: Unary::Delete, ..
            } => return Ok(false),
            Expr::Unary { value, .. }
            | Expr::ToInt32(value)
            | Expr::IntNegate(value) => {
                return operands_then_self(self, &[*value], events, budget);
            }
            Expr::Binary {
                op: Binary::And | Binary::Or | Binary::Nullish,
                left,
                right,
            } => {
                if !self.evaluation_order(*left, parameters, branch, events, budget)?
                    || !self.evaluation_order(*right, parameters, true, events, budget)?
                {
                    return Ok(false);
                }
                events.push((root, branch));
            }
            Expr::Binary { left, right, .. } | Expr::IntBinary { left, right, .. } => {
                return operands_then_self(self, &[*left, *right], events, budget);
            }
            Expr::Conditional { condition, yes, no } => {
                if !self.evaluation_order(*condition, parameters, branch, events, budget)?
                    || !self.evaluation_order(*yes, parameters, true, events, budget)?
                    || !self.evaluation_order(*no, parameters, true, events, budget)?
                {
                    return Ok(false);
                }
                events.push((root, branch));
            }
            Expr::Member { object, property } => {
                let mut operands = vec![*object];
                if let Property::Computed(key) = property {
                    operands.push(*key);
                }
                return operands_then_self(self, &operands, events, budget);
            }
            Expr::Call {
                callee,
                arguments,
                invocation: Invocation::Value | Invocation::Reference,
            } => {
                let mut operands = vec![*callee];
                operands.extend_from_slice(arguments);
                return operands_then_self(self, &operands, events, budget);
            }
            Expr::Construct { callee, arguments } => {
                let mut operands = vec![*callee];
                operands.extend_from_slice(arguments);
                return operands_then_self(self, &operands, events, budget);
            }
            Expr::Intrinsic {
                receiver,
                arguments,
                ..
            } => {
                let mut operands = vec![*receiver];
                operands.extend_from_slice(arguments);
                return operands_then_self(self, &operands, events, budget);
            }
            Expr::ConstructIntrinsic { arguments, .. } | Expr::Array(arguments) => {
                return operands_then_self(self, arguments, events, budget);
            }
            Expr::Sequence(items) => return operands_then_self(self, items, events, budget),
            Expr::Template(parts) => {
                let operands: Vec<ExprId> = parts
                    .iter()
                    .filter_map(|part| match part {
                        TemplatePart::Expression(value) => Some(*value),
                        TemplatePart::String(_) => None,
                    })
                    .collect();
                return operands_then_self(self, &operands, events, budget);
            }
            Expr::Object(entries) => {
                let mut operands = Vec::new();
                for (key, value) in entries {
                    if let Property::Computed(key) = key {
                        operands.push(*key);
                    }
                    operands.push(*value);
                }
                return operands_then_self(self, &operands, events, budget);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// At a call that is not the function's only one, the narrower test that
    /// measured best: a literal, or a binding no other function mentions (no
    /// call reaches it) that the call's other arguments do not mention. Its
    /// first read keeps the order rule, so an uninitialized binding throws
    /// where the argument would have, and later reads see the same value.
    fn repeatable(
        &self,
        argument: ExprId,
        index: usize,
        arguments: &[ExprId],
        reach: &Reach,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        match self.expressions[argument.index()] {
            Expr::Literal(_) => Ok(true),
            Expr::Binding(binding) if !reach.captured[binding.index()] => {
                for (other, &value) in arguments.iter().enumerate() {
                    if other != index && self.mentions_within(value, binding, budget)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Whether this argument's value is the same whenever, and however often,
    /// the body reads it. A literal is. So is a parameter nothing assigns, or
    /// one no other function mentions (no call reaches it) that the call's
    /// other arguments do not mention: a parameter is initialized before any
    /// code reads it. So is a root constant initialized before any code runs.
    fn stable(
        &self,
        argument: ExprId,
        index: usize,
        arguments: &[ExprId],
        reach: &Reach,
        written: &[bool],
        early: &[bool],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        match self.expressions[argument.index()] {
            Expr::Literal(_) => Ok(true),
            Expr::Binding(binding) if early[binding.index()] => Ok(true),
            Expr::Binding(binding) if reach.parameters[binding.index()] => {
                if !written[binding.index()] {
                    return Ok(true);
                }
                if reach.captured[binding.index()] {
                    return Ok(false);
                }
                for (other, &value) in arguments.iter().enumerate() {
                    if other != index && self.mentions_within(value, binding, budget)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Whether evaluating this node can neither observe nor change state,
    /// nor throw: it may run before an argument instead of after it.
    fn inert(&self, id: ExprId, parameters: &[BindingId]) -> bool {
        match &self.expressions[id.index()] {
            Expr::Literal(_) => true,
            Expr::Binding(binding) => {
                parameters.contains(binding)
                    || self.pristine_builtins && self.standard_global(*binding)
            }
            Expr::Host(name) => {
                self.pristine_builtins && STANDARD_GLOBALS.contains(&name.as_str())
            }
            Expr::Member { object, property } => {
                self.pristine_builtins && self.literal_key(property) && self.standard_path(*object)
            }
            Expr::Unary {
                op: Unary::Not | Unary::TypeOf | Unary::Void,
                ..
            } => true,
            Expr::Binary {
                op: Binary::StrictEqual | Binary::StrictNotEqual,
                ..
            } => true,
            // A fresh literal of inert parts runs no code and cannot fail
            // (resource exhaustion aside, D3.10); a spread would iterate.
            Expr::Array(elements) => elements.iter().all(|element| {
                !matches!(self.expressions[element.index()], Expr::Spread(_))
                    && self.inert(*element, parameters)
            }),
            Expr::Object(entries) => entries
                .iter()
                .all(|(key, value)| self.literal_key(key) && self.inert(*value, parameters)),
            Expr::Regex(_) => true,
            _ => false,
        }
    }

    /// A declared extern naming a standard global: pinned to its spelling,
    /// never assigned, and not an import.
    pub(super) fn standard_global(&self, binding: BindingId) -> bool {
        let declared = &self.bindings[binding.index()];
        declared.pinned
            && STANDARD_GLOBALS.contains(&declared.spelling.as_str())
            && !self.imports.iter().any(|import| import.binding == binding)
    }

    /// Whether evaluating `root` runs no user code: no getter outside the
    /// standard library, conversion hook or call.
    fn runs_no_user_code(&self, root: ExprId) -> bool {
        let expression = &self.expressions[root.index()];
        let own = match expression {
            Expr::Literal(_) => true,
            // A global may be an accessor; only standard ones are fixed.
            Expr::Binding(binding) => {
                !self.bindings[binding.index()].pinned
                    || self.pristine_builtins && self.standard_global(*binding)
            }
            Expr::Host(name) => {
                self.pristine_builtins && STANDARD_GLOBALS.contains(&name.as_str())
            }
            Expr::Member { object, property } => {
                self.pristine_builtins && self.literal_key(property) && self.standard_path(*object)
            }
            Expr::Unary {
                op: Unary::Not | Unary::TypeOf | Unary::Void,
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
            Expr::Conditional { .. } | Expr::Array(_) | Expr::Sequence(_) => true,
            Expr::Object(entries) => entries.iter().all(|(key, _)| self.literal_key(key)),
            _ => false,
        };
        let mut children = true;
        let _ = expression.visit_children(|child| {
            children &= self.runs_no_user_code(child);
            Ok::<_, ()>(())
        });
        own && children
    }

    /// A standard global or a named property path from one.
    fn standard_path(&self, id: ExprId) -> bool {
        match &self.expressions[id.index()] {
            Expr::Host(name) => STANDARD_GLOBALS.contains(&name.as_str()),
            Expr::Binding(binding) => self.standard_global(*binding),
            Expr::Member { object, property } => {
                self.literal_key(property) && self.standard_path(*object)
            }
            _ => false,
        }
    }

    /// A property named by syntax: `.name` or a literal string key.
    fn literal_key(&self, property: &Property) -> bool {
        match property {
            Property::Named(_) => true,
            Property::Computed(key) => {
                matches!(self.expressions[key.index()], Expr::Literal(Literal::String(_)))
            }
        }
    }

    /// The node that replaces a call: `root` copied with each parameter read
    /// replaced by its argument. New nodes follow every existing one.
    fn clone_template(
        &mut self,
        template: &Template,
        root: ExprId,
        arguments: &[ExprId],
        placed: &mut [bool],
        origin: Option<SourceNodeId>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Expr, AllocationError> {
        budget.work(Analysis, 1)?;
        let mut node = self.expressions[root.index()].clone();
        if let Expr::Binding(binding) = node {
            if let Some(index) = template.parameters.iter().position(|&p| p == binding) {
                placed[index] = true;
                return Ok(self.expressions[arguments[index].index()].clone());
            }
        }
        let mut children = Vec::new();
        let _ = node.visit_children(|child| {
            children.push(child);
            Ok::<_, ()>(())
        });
        let mut replaced = Vec::with_capacity(children.len());
        for child in children {
            let argument = match self.expressions[child.index()] {
                Expr::Binding(binding) => template.parameters.iter().position(|&p| p == binding),
                _ => None,
            };
            let id = match argument {
                // The first read takes the argument itself; a repeated one,
                // allowed only for a literal or a stable binding, a copy.
                Some(index) if !std::mem::replace(&mut placed[index], true) => arguments[index],
                Some(index) => {
                    let copy = self.expressions[arguments[index].index()].clone();
                    self.expression_in(copy, origin, budget)?
                }
                None => {
                    let copy =
                        self.clone_template(template, child, arguments, placed, origin, budget)?;
                    self.expression_in(copy, origin, budget)?
                }
            };
            replaced.push(id);
        }
        let mut next = replaced.into_iter();
        node.remap_children(|_| next.next().expect("one replacement per child"));
        Ok(node)
    }

    /// Every expression and region reachable from the root, each expression
    /// with its depth as the verifier counts it.
    pub(super) fn reach(&self, budget: &mut AllocationBudget<'_>) -> Result<Reach, AllocationError> {
        let mut seen_regions = vec![false; self.regions.len()];
        let mut seen = vec![false; self.expressions.len()];
        let mut reach = Reach {
            expressions: Vec::new(),
            regions: Vec::new(),
            captured: vec![false; self.bindings.len()],
            parameters: vec![false; self.bindings.len()],
        };
        // The function each binding is declared in, and each reference's.
        let mut declared: Vec<Option<Option<FunctionId>>> = vec![None; self.bindings.len()];
        let mut references: Vec<(BindingId, Option<FunctionId>)> = Vec::new();
        let mut regions = vec![(self.root, 0usize, None::<FunctionId>)];
        let mut pending: Vec<(ExprId, usize)> = Vec::new();
        while let Some((region, depth, owner)) = regions.pop() {
            budget.work(Analysis, 1)?;
            if std::mem::replace(&mut seen_regions[region.index()], true) {
                continue;
            }
            reach.regions.push(region);
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                match statement {
                    Statement::Let { binding, .. }
                    | Statement::ForIn { binding, .. }
                    | Statement::ForOf { binding, .. }
                    | Statement::Function { binding, .. } => {
                        declared[binding.index()] = Some(owner);
                    }
                    Statement::Try {
                        catch:
                            Some(Catch {
                                binding: Some(binding),
                                ..
                            }),
                        ..
                    } => declared[binding.index()] = Some(owner),
                    _ => {}
                }
                statement.visit_expressions(|root| pending.push((root, depth + 1)));
                statement.visit_regions(|child| regions.push((child, depth + 1, owner)));
                if let Statement::Function { function, .. } = statement {
                    for parameter in &self.functions[function.index()].parameters {
                        declared[parameter.index()] = Some(Some(*function));
                        reach.parameters[parameter.index()] = true;
                    }
                    regions.push((self.functions[function.index()].body, depth + 2, Some(*function)));
                }
                while let Some((id, at)) = pending.pop() {
                    budget.work(Analysis, 1)?;
                    if std::mem::replace(&mut seen[id.index()], true) {
                        continue;
                    }
                    reach.expressions.push((id, at));
                    let expression = &self.expressions[id.index()];
                    if let Expr::Binding(binding) = expression {
                        references.push((*binding, owner));
                    }
                    if let Some(function) = expression.created_function() {
                        for parameter in &self.functions[function.index()].parameters {
                            declared[parameter.index()] = Some(Some(function));
                            reach.parameters[parameter.index()] = true;
                        }
                        regions.push((self.functions[function.index()].body, at + 2, Some(function)));
                    }
                    let _ = expression.visit_children(|child| {
                        pending.push((child, at + 1));
                        Ok::<_, ()>(())
                    });
                }
            }
        }
        budget.work(Analysis, references.len() as u64)?;
        for (binding, owner) in references {
            // An undeclared binding (an import or host) counts as captured.
            if declared[binding.index()] != Some(owner) {
                reach.captured[binding.index()] = true;
            }
        }
        Ok(reach)
    }

    /// Rebuild the expression arena in postorder from the code that can run,
    /// so children again precede their parents after an edit. Unreachable
    /// nodes go, and unreachable regions lose their statements. Returns each
    /// surviving old node's new id.
    pub(crate) fn renumber(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<Option<ExprId>>, AllocationError> {
        let old = std::mem::take(&mut self.expressions);
        let old_origins = std::mem::take(&mut self.origins);
        let mut map: Vec<Option<ExprId>> = vec![None; old.len()];
        let mut reached = vec![false; self.regions.len()];
        let mut regions = vec![self.root];
        let mut pending: Vec<(ExprId, bool)> = Vec::new();
        self.expressions = Vec::with_capacity(old.len());
        self.origins = Vec::with_capacity(old.len());
        while let Some(region) = regions.pop() {
            budget.work(Analysis, 1)?;
            if std::mem::replace(&mut reached[region.index()], true) {
                continue;
            }
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                statement.visit_expressions(|root| pending.push((root, false)));
                statement.visit_regions(|child| regions.push(child));
                if let Statement::Function { function, .. } = statement {
                    regions.push(self.functions[function.index()].body);
                }
                while let Some((id, expanded)) = pending.pop() {
                    budget.work(Analysis, 1)?;
                    if map[id.index()].is_some() {
                        continue;
                    }
                    if !expanded {
                        pending.push((id, true));
                        let expression = &old[id.index()];
                        if let Some(function) = expression.created_function() {
                            regions.push(self.functions[function.index()].body);
                        }
                        let _ = expression.visit_children(|child| {
                            if map[child.index()].is_none() {
                                pending.push((child, false));
                            }
                            Ok::<_, ()>(())
                        });
                        continue;
                    }
                    let mut expression = old[id.index()].clone();
                    expression.remap_children(|child| {
                        map[child.index()].expect("an operand precedes its parent")
                    });
                    let new = ExprId::try_new(self.expressions.len())
                        .ok_or(AllocationError::Capacity)?;
                    self.expressions.push(expression);
                    self.origins.push(old_origins[id.index()]);
                    map[id.index()] = Some(new);
                }
            }
        }
        for (index, region) in self.regions.iter_mut().enumerate() {
            if !reached[index] {
                region.statements.clear();
                continue;
            }
            for statement in &mut region.statements {
                statement.remap_expressions(|id| {
                    map[id.index()].expect("a reachable statement's roots were placed")
                });
            }
        }
        Ok(map)
    }
}

impl Module {
    /// `f(x);` becomes `f`'s statements with `x` in place, when `f` is a
    /// `let`-bound arrow nothing reassigns or exports, this is its only call,
    /// and its body is only expression statements. Every argument is stable
    /// (a literal, or a binding no call can reach that no other argument
    /// mentions, declared earlier in the call's region or a parameter): the
    /// call evaluated it first, and every later read sees that same value.
    /// A classic script keeps a frame its body's user code could observe.
    /// Returns the number of inlined calls and the arena renumbering.
    pub(crate) fn inline_statement_functions(
        &mut self,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(usize, Option<Vec<Option<ExprId>>>), AllocationError> {
        let reach = self.reach(budget)?;
        let mut written = vec![false; self.bindings.len()];
        let mut calls = vec![0usize; self.bindings.len()];
        let mut other_use = vec![false; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            match &self.expressions[id.index()] {
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = self.expressions[target.index()] {
                        written[binding.index()] = true;
                    }
                }
                Expr::Call { callee, .. } => {
                    if let Expr::Binding(binding) = self.expressions[callee.index()] {
                        calls[binding.index()] += 1;
                    }
                }
                _ => {}
            }
        }
        for export in &self.exports {
            written[export.binding.index()] = true;
        }
        // Bindings read other than as a callee (they must keep their value).
        for &(id, _) in &reach.expressions {
            let _ = self.expressions[id.index()].visit_children(|child| {
                if let Expr::Binding(binding) = self.expressions[child.index()] {
                    let callee = matches!(
                        &self.expressions[id.index()],
                        Expr::Call { callee, .. } if *callee == child
                    );
                    if !callee {
                        other_use[binding.index()] = true;
                    }
                }
                Ok::<_, ()>(())
            });
        }
        // Candidate bodies, by binding.
        let mut bodies: Vec<Option<(FunctionId, Vec<ExprId>)>> = vec![None; self.bindings.len()];
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
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
                if written[binding.index()]
                    || other_use[binding.index()]
                    || calls[binding.index()] != 1
                    || self.bindings[binding.index()].pinned
                {
                    continue;
                }
                let declared = &self.functions[function.index()];
                if !declared.arrow
                    || declared.strict
                    || declared.suspension != Suspension::None
                    || declared.length.is_some()
                {
                    continue;
                }
                let mut roots = Vec::new();
                let mut plain = true;
                let statements = &self.regions[declared.body.index()].statements;
                for (position, statement) in statements.iter().enumerate() {
                    match statement {
                        Statement::Evaluate(root) => roots.push(*root),
                        Statement::Return(None) if position + 1 == statements.len() => {}
                        _ => plain = false,
                    }
                }
                if !plain || roots.is_empty() {
                    continue;
                }
                let mut fits = true;
                for &root in &roots {
                    fits = fits
                        && self.movable_statement(root, &declared.parameters, binding)
                        && (strict || self.runs_no_user_code(root));
                }
                if fits {
                    bodies[binding.index()] = Some((function, roots));
                }
            }
        }
        // The one call of each candidate, as a statement.
        let mut sites = Vec::new();
        for &region in &reach.regions {
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                let Statement::Evaluate(mut call) = *statement else {
                    continue;
                };
                // A discarded `void f()` is the call.
                if let Expr::Unary {
                    op: Unary::Void,
                    value,
                } = self.expressions[call.index()]
                {
                    call = value;
                }
                // A binding callee has no receiver: `Reference` and `Value`
                // invocations of it are the same call.
                let Expr::Call {
                    callee,
                    ref arguments,
                    invocation: Invocation::Value | Invocation::Reference,
                } = self.expressions[call.index()]
                else {
                    continue;
                };
                let Expr::Binding(binding) = self.expressions[callee.index()] else {
                    continue;
                };
                let Some((function, _)) = &bodies[binding.index()] else {
                    continue;
                };
                let parameters = &self.functions[function.index()].parameters;
                if arguments.len() != parameters.len() {
                    continue;
                }
                let mut stable = true;
                for (position, &argument) in arguments.iter().enumerate() {
                    stable = stable
                        && self.settled(argument, position, arguments, region, index, &reach, budget)?;
                }
                if stable {
                    sites.push((region, index, binding));
                }
            }
        }
        if sites.is_empty() {
            return Ok((0, None));
        }
        // Splice from the last site backwards so indices stay valid.
        sites.sort_unstable_by(|a, b| (a.0.index(), a.1).cmp(&(b.0.index(), b.1)).reverse());
        for &(region, index, binding) in &sites {
            let Statement::Evaluate(mut call) = self.regions[region.index()].statements[index]
            else {
                unreachable!("a recorded site is a call statement");
            };
            if let Expr::Unary {
                op: Unary::Void,
                value,
            } = self.expressions[call.index()]
            {
                call = value;
            }
            let Expr::Call { arguments, .. } = self.expressions[call.index()].clone() else {
                unreachable!("a recorded site is a call");
            };
            let (function, roots) = bodies[binding.index()].clone().unwrap();
            let parameters = self.functions[function.index()].parameters.clone();
            let origin = self.origins[call.index()];
            let mut statements = Vec::with_capacity(roots.len());
            for root in roots {
                let copy = self.substitute(root, &parameters, &arguments, origin, budget)?;
                let id = self.expression_in(copy, origin, budget)?;
                statements.push(Statement::Evaluate(id));
            }
            let root = region == self.root;
            let module = root.then(|| self.root_modules.get(index).copied()).flatten();
            let count = statements.len();
            self.regions[region.index()]
                .statements
                .splice(index..=index, statements);
            if let Some(module) = module {
                if index < self.root_modules.len() {
                    self.root_modules
                        .splice(index..=index, std::iter::repeat_n(module, count));
                }
            }
        }
        let map = self.renumber(budget)?;
        Ok((sites.len(), Some(map)))
    }

    /// Whether a body statement may run at a call site: no own `this`,
    /// `arguments`, function creation, suspension or assignment to a
    /// parameter, and no mention of the function itself.
    fn movable_statement(&self, root: ExprId, parameters: &[BindingId], own: BindingId) -> bool {
        let expression = &self.expressions[root.index()];
        let own_ok = match expression {
            Expr::This
            | Expr::Function(_)
            | Expr::Class { .. }
            | Expr::SuperCall { .. }
            | Expr::Await(_)
            | Expr::Yield { .. }
            | Expr::LoadModule { .. } => false,
            Expr::Host(name) => name != "arguments" && name != "eval",
            Expr::Binding(binding) => *binding != own,
            Expr::Assign { target, .. } => !matches!(
                self.expressions[target.index()],
                Expr::Binding(binding) if parameters.contains(&binding)
            ),
            Expr::Unary {
                op: Unary::Delete,
                value,
            } => !matches!(
                self.expressions[value.index()],
                Expr::Binding(_)
            ),
            _ => true,
        };
        let mut children = true;
        let _ = expression.visit_children(|child| {
            children &= self.movable_statement(child, parameters, own);
            Ok::<_, ()>(())
        });
        own_ok && children
    }

    /// Whether this argument keeps one value from the call through every
    /// statement of the body: a literal, or a binding no call can reach that
    /// no other argument mentions, initialized before the call (a parameter,
    /// or declared earlier in the call's own region).
    #[allow(clippy::too_many_arguments)]
    fn settled(
        &self,
        argument: ExprId,
        position: usize,
        arguments: &[ExprId],
        region: RegionId,
        index: usize,
        reach: &Reach,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        match self.expressions[argument.index()] {
            Expr::Literal(_) => Ok(true),
            Expr::Binding(binding) if !reach.captured[binding.index()] => {
                let initialized = reach.parameters[binding.index()]
                    || self.regions[region.index()].statements[..index]
                        .iter()
                        .any(|statement| matches!(statement, Statement::Let { binding: found, .. } if *found == binding));
                if !initialized {
                    return Ok(false);
                }
                for (other, &value) in arguments.iter().enumerate() {
                    if other != position && self.mentions_within(value, binding, budget)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// `root` copied with every parameter read replaced by a copy of its
    /// (settled) argument.
    fn substitute(
        &mut self,
        root: ExprId,
        parameters: &[BindingId],
        arguments: &[ExprId],
        origin: Option<SourceNodeId>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Expr, AllocationError> {
        budget.work(Analysis, 1)?;
        let mut node = self.expressions[root.index()].clone();
        if let Expr::Binding(binding) = node {
            if let Some(index) = parameters.iter().position(|&p| p == binding) {
                return Ok(self.expressions[arguments[index].index()].clone());
            }
        }
        let mut children = Vec::new();
        let _ = node.visit_children(|child| {
            children.push(child);
            Ok::<_, ()>(())
        });
        let mut replaced = Vec::with_capacity(children.len());
        for child in children {
            let copy = self.substitute(child, parameters, arguments, origin, budget)?;
            replaced.push(self.expression_in(copy, origin, budget)?);
        }
        let mut next = replaced.into_iter();
        node.remap_children(|_| next.next().expect("one replacement per child"));
        Ok(node)
    }

    /// `let c=d` goes, and `c` reads as `d`, when neither is ever assigned
    /// and `d` is initialized there (a parameter, or declared earlier in the
    /// same region): both always hold the same value. Nothing before the
    /// declaration mentions `c`, so no read of it met its temporal dead zone.
    /// Returns the number of aliases removed.
    pub(crate) fn eliminate_aliases(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let reach = self.reach(budget)?;
        let mut written = vec![false; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            if let Expr::Assign { target, .. } = &self.expressions[id.index()] {
                if let Expr::Binding(binding) = self.expressions[target.index()] {
                    written[binding.index()] = true;
                }
            }
        }
        for export in &self.exports {
            written[export.binding.index()] = true;
        }
        let mut replacement: Vec<Option<BindingId>> = vec![None; self.bindings.len()];
        let mut removed = 0;
        for &region in &reach.regions {
            budget.work(Analysis, 1 + self.regions[region.index()].statements.len() as u64)?;
            let aliases: Vec<(usize, BindingId, BindingId)> = self.regions[region.index()]
                .statements
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| match *statement {
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => match self.expressions[value.index()] {
                        Expr::Binding(source)
                            if !written[binding.index()]
                                && !written[source.index()]
                                && !self.bindings[binding.index()].pinned
                                && source != binding =>
                        {
                            Some((index, binding, source))
                        }
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            if aliases.is_empty() {
                continue;
            }
            let wanted: Vec<BindingId> = aliases.iter().map(|&(_, alias, _)| alias).collect();
            let first = self.first_mentions(region, &wanted, budget)?;
            let mut remove = Vec::new();
            for (index, alias, source) in aliases {
                let initialized = reach.parameters[source.index()]
                    || self.regions[region.index()].statements[..index].iter().any(
                        |statement| matches!(statement, Statement::Let { binding, .. } if *binding == source),
                    );
                // A source that is itself an alias being removed reads as its
                // own source.
                let source = replacement[source.index()].unwrap_or(source);
                if initialized && first.get(&alias).is_none_or(|&mention| mention > index) {
                    replacement[alias.index()] = Some(source);
                    remove.push(index);
                }
            }
            let root = region == self.root;
            for &index in remove.iter().rev() {
                self.regions[region.index()].statements.remove(index);
                if root && index < self.root_modules.len() {
                    self.root_modules.remove(index);
                }
                removed += 1;
            }
        }
        if removed != 0 {
            // Follow chains of aliases to the binding that remains.
            for index in 0..replacement.len() {
                let mut target = replacement[index];
                let mut steps = 0;
                while let Some(next) = target.and_then(|t| replacement[t.index()]) {
                    target = Some(next);
                    steps += 1;
                    if steps > replacement.len() {
                        break;
                    }
                }
                replacement[index] = target;
            }
            budget.work(Analysis, self.expressions.len() as u64)?;
            for expression in &mut self.expressions {
                if let Expr::Binding(binding) = expression {
                    if let Some(source) = replacement[binding.index()] {
                        *binding = source;
                    }
                }
            }
        }
        Ok(removed)
    }
}

impl Module {
    /// A constant namespace object is its members: `let M={k:f};…M.k(x)`
    /// reads as `f(x)`. `M` is never assigned, exported or pinned, and every
    /// use of it reads a literal key, so nothing can change or observe its
    /// properties: each read yields the value the literal gave, which is
    /// the same `f` when `f` is never assigned (it was initialized when the
    /// literal read it). A method call passes `M` as `this`, so there `f`
    /// must be an arrow. A literal member value is copied. When every use is
    /// replaced and the literal only reads bindings declared before it in
    /// its region, the declaration goes. Returns the replaced reads.
    pub(crate) fn flatten_constant_objects(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let reach = self.reach(budget)?;
        let mut written = vec![false; self.bindings.len()];
        let mut arrows = vec![false; self.bindings.len()];
        // Where each binding is assigned: a statement of a region, or (None)
        // somewhere a later evaluation could run it.
        let mut writes: Vec<Vec<Option<(RegionId, usize)>>> = vec![Vec::new(); self.bindings.len()];
        let mut assigned_arrow = vec![true; self.bindings.len()];
        // Each reachable node's parent, and how it uses the node.
        #[derive(Clone, Copy, PartialEq)]
        enum Role {
            Object,
            Callee,
            Target,
            Other,
        }
        let mut parent: Vec<Option<(ExprId, Role)>> = vec![None; self.expressions.len()];
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let expression = &self.expressions[id.index()];
            if let Expr::Assign { target, value } = expression {
                if let Expr::Binding(binding) = self.expressions[target.index()] {
                    written[binding.index()] = true;
                    let arrow = matches!(
                        self.expressions[value.index()],
                        Expr::Function(function) if self.functions[function.index()].arrow
                    );
                    assigned_arrow[binding.index()] &= arrow;
                }
            }
            let _ = expression.visit_children(|child| {
                let role = match expression {
                    Expr::Member { object, .. } if *object == child => Role::Object,
                    Expr::Call { callee, .. } if *callee == child => Role::Callee,
                    Expr::Assign { target, .. } if *target == child => Role::Target,
                    Expr::Unary {
                        op: Unary::Delete, ..
                    } => Role::Target,
                    _ => Role::Other,
                };
                parent[child.index()] = Some((id, role));
                Ok::<_, ()>(())
            });
        }
        for export in &self.exports {
            written[export.binding.index()] = true;
        }
        // Statement roots and loop heads also use bindings.
        let mut root_use = vec![false; self.bindings.len()];
        let mut literals: Vec<(RegionId, usize, BindingId, ExprId)> = Vec::new();
        for &region in &reach.regions {
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                statement.visit_expressions(|root| {
                    if let Expr::Binding(binding) = self.expressions[root.index()] {
                        root_use[binding.index()] = true;
                    }
                });
                if let Statement::Evaluate(root) = *statement {
                    if let Expr::Assign { target, .. } = self.expressions[root.index()] {
                        if let Expr::Binding(binding) = self.expressions[target.index()] {
                            writes[binding.index()].push(Some((region, index)));
                        }
                    }
                }
                match *statement {
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => {
                        match &self.expressions[value.index()] {
                            Expr::Object(_) => literals.push((region, index, binding, value)),
                            Expr::Function(function) => {
                                arrows[binding.index()] = self.functions[function.index()].arrow;
                            }
                            _ => {}
                        }
                    }
                    Statement::ForIn { binding, .. } | Statement::ForOf { binding, .. } => {
                        written[binding.index()] = true;
                    }
                    _ => {}
                }
            }
        }
        // Where each binding is read.
        let mut uses: Vec<Vec<ExprId>> = vec![Vec::new(); self.bindings.len()];
        for &(id, _) in &reach.expressions {
            if let Expr::Binding(binding) = self.expressions[id.index()] {
                uses[binding.index()].push(id);
            }
        }
        // A binding holds one value from the literal's creation on when
        // nothing assigns it, or when only statements before the literal in
        // its own region do: those ran before the literal read it.
        let assignments = |binding: BindingId| {
            // Writes found as statement roots, against all reachable writes.
            writes[binding.index()].len()
        };
        let mut all_writes = vec![0usize; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            if let Expr::Assign { target, .. } = &self.expressions[id.index()] {
                if let Expr::Binding(binding) = self.expressions[target.index()] {
                    all_writes[binding.index()] += 1;
                }
            }
        }
        let settled_at = |binding: BindingId, region: RegionId, index: usize| {
            !written[binding.index()]
                || assignments(binding) == all_writes[binding.index()]
                    && writes[binding.index()]
                        .iter()
                        .all(|site| matches!(site, Some((found, at)) if *found == region && *at < index))
        };
        // A read of `M` before its declaration throws; the flattened read
        // would not. So nothing earlier in its region may mention `M`, not
        // even a function that could be called before the declaration runs.
        let mut first: Vec<Option<usize>> = vec![None; self.bindings.len()];
        {
            let mut by_region: Vec<(RegionId, Vec<BindingId>)> = Vec::new();
            for &(region, _, object, _) in &literals {
                match by_region.iter_mut().find(|(found, _)| *found == region) {
                    Some((_, list)) => list.push(object),
                    None => by_region.push((region, vec![object])),
                }
            }
            for (region, objects) in by_region {
                for (binding, at) in self.first_mentions(region, &objects, budget)? {
                    first[binding.index()] = Some(at);
                }
            }
        }
        let mut replaced = 0;
        let mut removals: Vec<(RegionId, usize)> = Vec::new();
        for (region, index, object, value) in literals {
            budget.work(Analysis, 1)?;
            if written[object.index()]
                || root_use[object.index()]
                || self.bindings[object.index()].pinned
                || first[object.index()].is_some_and(|at| at <= index)
            {
                continue;
            }
            let Expr::Object(entries) = self.expressions[value.index()].clone() else {
                continue;
            };
            let mut members: Vec<(StringValue, ExprId)> = Vec::with_capacity(entries.len());
            let mut plain = true;
            for (key, item) in &entries {
                let name = match key {
                    Property::Named(name) => StringValue::from(name.as_str()),
                    Property::Computed(key) => match &self.expressions[key.index()] {
                        Expr::Literal(Literal::String(name)) => name.clone(),
                        _ => {
                            plain = false;
                            break;
                        }
                    },
                };
                if name.as_unicode() == Some("__proto__") {
                    plain = false;
                    break;
                }
                // The last entry for a key wins.
                members.retain(|(known, _)| *known != name);
                members.push((name, *item));
            }
            if !plain {
                continue;
            }
            // Every use reads a literal key and stores nothing.
            let mut sites = Vec::new();
            let mut all_reads = true;
            for &site in &uses[object.index()] {
                let Some((member, Role::Object)) = parent[site.index()] else {
                    all_reads = false;
                    break;
                };
                let Expr::Member { property, .. } = &self.expressions[member.index()] else {
                    all_reads = false;
                    break;
                };
                let key = match property {
                    Property::Named(name) => Some(StringValue::from(name.as_str())),
                    Property::Computed(key) => match &self.expressions[key.index()] {
                        Expr::Literal(Literal::String(name)) => Some(name.clone()),
                        _ => None,
                    },
                };
                let role = parent[member.index()].map(|(_, role)| role);
                if key.is_none() || role == Some(Role::Target) {
                    all_reads = false;
                    break;
                }
                sites.push((member, key.unwrap(), role == Some(Role::Callee)));
            }
            if !all_reads {
                continue;
            }
            let mut every = true;
            for (member, key, callee) in sites {
                let Some(&(_, item)) = members.iter().find(|(known, _)| *known == key) else {
                    every = false;
                    continue;
                };
                let replacement = match self.expressions[item.index()] {
                    Expr::Literal(ref literal) if !callee => Some(Expr::Literal(literal.clone())),
                    Expr::Binding(binding)
                        if binding != object
                            && settled_at(binding, region, index)
                            && (!callee
                                || arrows[binding.index()]
                                || written[binding.index()] && assigned_arrow[binding.index()]) =>
                    {
                        Some(Expr::Binding(binding))
                    }
                    _ => None,
                };
                match replacement {
                    Some(replacement) => {
                        self.expressions[member.index()] = replacement;
                        replaced += 1;
                    }
                    None => every = false,
                }
            }
            // The literal goes when nothing reads it and evaluating it could
            // not throw: it reads only bindings declared before it here.
            if every {
                let earlier = &self.regions[region.index()].statements[..index];
                let settled = members.iter().all(|&(_, item)| match self.expressions[item.index()] {
                    Expr::Literal(_) | Expr::Function(_) => true,
                    Expr::Binding(binding) => earlier.iter().any(|statement| {
                        matches!(statement, Statement::Let { binding: found, .. } if *found == binding)
                    }),
                    _ => false,
                });
                if settled {
                    removals.push((region, index));
                }
            }
        }
        removals.sort_unstable_by(|a, b| (a.0.index(), a.1).cmp(&(b.0.index(), b.1)).reverse());
        for (region, index) in removals {
            self.regions[region.index()].statements.remove(index);
            if region == self.root && index < self.root_modules.len() {
                self.root_modules.remove(index);
            }
        }
        Ok(replaced)
    }

    /// A function bound by `let`, never assigned or exported, whose every
    /// reference calls it, has a name nothing can read. Earlier edits (a
    /// flattened namespace) can leave a formerly escaping function so.
    pub(crate) fn unobserve_called_names(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let reach = self.reach(budget)?;
        let mut escaped = vec![false; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let expression = &self.expressions[id.index()];
            let _ = expression.visit_children(|child| {
                if let Expr::Binding(binding) = self.expressions[child.index()] {
                    if !matches!(expression, Expr::Call { callee, .. } if *callee == child) {
                        escaped[binding.index()] = true;
                    }
                }
                Ok::<_, ()>(())
            });
        }
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
                statement.visit_expressions(|root| {
                    if let Expr::Binding(binding) = self.expressions[root.index()] {
                        escaped[binding.index()] = true;
                    }
                });
            }
        }
        for export in &self.exports {
            escaped[export.binding.index()] = true;
        }
        let mut changed = 0;
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
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
                if !escaped[binding.index()]
                    && !self.bindings[binding.index()].pinned
                    && self.functions[function.index()].name != FunctionName::Unobserved
                {
                    self.functions[function.index()].name = FunctionName::Unobserved;
                    changed += 1;
                }
            }
        }
        Ok(changed)
    }
}
