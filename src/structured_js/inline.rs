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
struct Reach {
    expressions: Vec<(ExprId, usize)>,
    regions: Vec<RegionId>,
    /// Bindings mentioned by a function other than the one declaring them:
    /// a call can reach such a binding, and may write it.
    captured: Vec<bool>,
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
                invocation: Invocation::Value,
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
        let function = &self.functions[function.index()];
        // A strict body keeps strict `delete` and assignment semantics that a
        // sloppy call site would not.
        if !function.arrow
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
        if events.len() > 64 {
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
            _ => false,
        }
    }

    /// A declared extern naming a standard global: pinned to its spelling,
    /// never assigned, and not an import.
    fn standard_global(&self, binding: BindingId) -> bool {
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
    fn reach(&self, budget: &mut AllocationBudget<'_>) -> Result<Reach, AllocationError> {
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
