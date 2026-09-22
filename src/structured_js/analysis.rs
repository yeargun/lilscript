//! Immutable dependency views over the same structured program. All experiment
//! modes use identical semantics. Tree mode walks each query's dependencies once;
//! indexed mode computes each summary in postorder, and memoized mode stores
//! summaries as queries need them. All use explicit work or postorder iteration.
//! Query-local scratch avoids exponential revisits even without cross-query reuse.
//! This is an analysis experiment, not a second authoritative program or complete SSA.

pub use super::constants::Work as ConstantWork;
pub use super::flow::{ParameterWork, Work as FlowWork};
use super::*;
use crate::compilation_contract::{JavaScriptExecution, JavaScriptWorld};
use crate::scalar_transfer::NumberFacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Tree,
    Indexed,
    Memoized,
    /// Propagate current values directly through structured evaluation order.
    Regions,
    /// Retain producer and join dependencies, then derive the same facts.
    Values,
}

#[derive(Debug)]
enum Task {
    Visit(ExprId),
    Finish(ExprId, usize),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Work {
    pub queries: usize,
    pub expression_visits: usize,
    pub stored_summaries: usize,
    pub temporary_summaries: usize,
    pub flow: FlowWork,
    pub parameters: ParameterWork,
    pub constants: ConstantWork,
}

/// Allocation, throwing, mutable reads and foreign effects are independent.
/// In particular, a fresh object is not a repeatable scalar value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Effects(u8);
impl Effects {
    pub(super) const THROW: Self = Self(1);
    const FOREIGN: Self = Self(2);
    pub(super) const MUTABLE_READ: Self = Self(4);
    const WRITE: Self = Self(8);
    const ALLOCATE: Self = Self(16);
    const HEAP_READ: Self = Self(32);
    pub(super) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub fn discardable(self) -> bool {
        // A lexical read can be discarded without being repeatable. TDZ
        // throws remain independent from mutable-cell value dependencies.
        self.0 & !(Self::ALLOCATE.0 | Self::MUTABLE_READ.0 | Self::HEAP_READ.0) == 0
    }
    pub fn stable_scalar(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ValueKind {
    #[default]
    Unknown,
    Number,
    String,
    Bool,
    Null,
    Undefined,
    Object,
}
impl ValueKind {
    fn primitive(self) -> bool {
        !matches!(self, Self::Unknown | Self::Object)
    }
}

/// Exact integer bounds also exclude negative zero. An IEEE number type alone
/// cannot establish either property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerRange {
    pub minimum: i64,
    pub maximum: i64,
}
impl IntegerRange {
    pub(super) const I32: Self = Self {
        minimum: i32::MIN as i64,
        maximum: i32::MAX as i64,
    };
    fn literal(value: f64) -> Option<Self> {
        Self::from_number(NumberFacts::literal(value))
    }
    fn from_number(value: NumberFacts) -> Option<Self> {
        if value.may_negative_zero() {
            return None;
        }
        value
            .integer_bounds()
            .map(|(minimum, maximum)| Self { minimum, maximum })
    }
    fn number(self) -> NumberFacts {
        NumberFacts::integer_range(self.minimum, self.maximum, false).unwrap_or(NumberFacts::NUMBER)
    }
    pub fn fits_i32(self) -> bool {
        self.minimum >= Self::I32.minimum && self.maximum <= Self::I32.maximum
    }
    pub fn singleton_i32(self) -> Option<i32> {
        (self.minimum == self.maximum && self.fits_i32()).then_some(self.minimum as i32)
    }
    fn exact_i32(value: i32) -> Self {
        Self {
            minimum: i64::from(value),
            maximum: i64::from(value),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    /// The operation's contribution before its operands are combined. A
    /// transitive union cannot be inverted when a planned operand disappears.
    pub(super) own_effects: Effects,
    pub effects: Effects,
    pub value: ValueKind,
    pub integer: Option<IntegerRange>,
    pub(super) constant: Option<constants::Known>,
    /// A proof about this operation's unnormalized result, distinct from its
    /// required signed result. A signed output type alone cannot justify it.
    pub normalization_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Site {
    pub region: RegionId,
    pub statement: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    Value,
    /// This occurrence is the direct callee, not an argument, constructor,
    /// property base or escaping value. Retain the call that owns the role.
    Call(ExprId),
    ArrayLength(ExprId),
}

#[derive(Debug, Clone, Copy)]
pub struct Read {
    pub expression: ExprId,
    pub site: Site,
    pub observation: Observation,
}

#[derive(Debug, Clone, Copy)]
pub struct Write {
    pub operation: ExprId,
    pub expression: ExprId,
    pub site: Site,
}

#[derive(Debug, Default)]
pub struct BindingUses {
    /// An ESM live binding is an observation root even with no local read.
    pub exported: bool,
    /// Readonly local alias of a live externally written ESM binding. This is
    /// not a namespace property lookup and does not prove initialization.
    pub imported: bool,
    pub reads: Vec<Read>,
    pub writes: Vec<Write>,
    pub declaration: Option<Site>,
    /// Parameters, catch cells and hoisted function bindings are initialized
    /// whenever their lexical scope is entered; ordinary lets are not.
    pub initialized_on_scope_entry: bool,
    pub initializer: Option<ExprId>,
    pub function: Option<FunctionId>,
    pub captured: bool,
    /// Actual argument domain for an immutable parameter whose callable has
    /// complete private direct-call coverage. Never inferred from source types.
    pub(super) entry_value: ValueKind,
    pub(super) entry_i32: bool,
}

/// Owned analysis storage can move between phases without copying the program
/// or its facts. It exposes no proof until attached to the matching revision.
#[derive(Debug)]
pub struct Snapshot {
    revision: lower::Revision,
    world: JavaScriptWorld,
    execution: JavaScriptExecution,
    bindings: Vec<BindingUses>,
    direct_eval: bool,
    initialized_references: Vec<bool>,
    dependencies: Option<super::flow::Dependencies>,
    constants: constants::Values,
    summaries: Option<Vec<Facts>>,
    memoized: Option<Vec<Option<Facts>>>,
    retain_memoized: bool,
    temporary: Vec<ExprId>,
    tasks: Vec<Task>,
    values: Vec<(ExprId, Facts)>,
    work: Work,
}

/// A revision check happens once when resuming a snapshot. The immutable borrow
/// then prevents mutation for the entire query phase. This presently reuses
/// facts only for unchanged programs, not across semantic edits.
pub struct Analysis<'tree, 'sem, 'src> {
    tree: &'tree lower::AnnotatedTree<'sem, 'src>,
    snapshot: Snapshot,
}

impl<'tree, 'sem, 'src> Analysis<'tree, 'sem, 'src> {
    pub fn new(tree: &'tree lower::AnnotatedTree<'sem, 'src>, mode: Mode) -> Self {
        Self::new_in_world(tree, mode, JavaScriptWorld::ReusableLibrary)
    }

    pub fn new_in_world(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        mode: Mode,
        world: JavaScriptWorld,
    ) -> Self {
        Self::new_in_execution(tree, mode, world, JavaScriptExecution::Script)
    }

    /// Module-qualified facts require the resulting artifact to execute as an
    /// ECMAScript module. Consumer visibility alone cannot seal hidden caller
    /// access in an ordinary sloppy function.
    pub fn new_in_execution(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        mode: Mode,
        world: JavaScriptWorld,
        execution: JavaScriptExecution,
    ) -> Self {
        let module = tree.target();
        let mut scope_functions = vec![None; module.scopes.len()];
        for (id, function) in module.functions.iter().enumerate() {
            let scope = module.regions[function.body.index()].scope;
            scope_functions[scope.index()] = Some(FunctionId::new(id));
        }
        // Verified scopes each have one owning region. Ordinary blocks
        // inherit the function, while a function body introduces its owner.
        for (id, parent) in module.scopes.iter().enumerate() {
            if scope_functions[id].is_none() {
                scope_functions[id] = parent.and_then(|parent| scope_functions[parent.index()]);
            }
        }
        let mut bindings: Vec<_> = module
            .bindings
            .iter()
            .map(|binding| BindingUses {
                function: scope_functions[binding.scope.index()],
                ..BindingUses::default()
            })
            .collect();
        for import in &module.imports {
            bindings[import.binding.index()].imported = true;
        }
        for export in &module.exports {
            bindings[export.binding.index()].exported = true;
        }
        for function in &module.functions {
            for parameter in &function.parameters {
                bindings[parameter.index()].initialized_on_scope_entry = true;
            }
        }
        let mut direct_eval = false;
        // Each region owns its immediate execution, including deferred function
        // bodies. Assignments are collected globally before proving stability:
        // a closure write cannot be hidden by its position in source order.
        for (index, region) in module.regions.iter().enumerate() {
            for (statement, node) in region.statements.iter().enumerate() {
                let site = Site {
                    region: RegionId::new(index),
                    statement,
                };
                let mut scan = |value| {
                    scan_uses(
                        module,
                        value,
                        site,
                        Context::Value,
                        &scope_functions,
                        &mut bindings,
                        &mut direct_eval,
                    )
                };
                node.visit_expressions(&mut scan);
                match node {
                    Statement::Let { binding, value } => {
                        let entry = &mut bindings[binding.index()];
                        entry.declaration = Some(site);
                        entry.initializer = *value;
                    }
                    Statement::Function { binding, .. } => {
                        let entry = &mut bindings[binding.index()];
                        entry.initialized_on_scope_entry = true;
                        entry.declaration = Some(site);
                    }
                    Statement::Try {
                        catch:
                            Some(Catch {
                                binding: Some(binding),
                                ..
                            }),
                        ..
                    }
                    | Statement::ForIn { binding, .. }
                    | Statement::ForOf { binding, .. } => {
                        bindings[binding.index()].initialized_on_scope_entry = true
                    }
                    _ => {}
                }
            }
        }
        let mut initialized_references = vec![false; module.expressions.len()];
        for binding in bindings.iter() {
            for (expression, site) in binding
                .reads
                .iter()
                .map(|read| (read.expression, read.site))
                .chain(
                    binding
                        .writes
                        .iter()
                        .map(|write| (write.expression, write.site)),
                )
            {
                initialized_references[expression.index()] = binding.initialized_on_scope_entry
                    || binding.declaration.is_some_and(|declaration| {
                        declaration.region == site.region && declaration.statement < site.statement
                    });
            }
        }
        let mut result = Self {
            tree,
            snapshot: Snapshot {
                revision: tree.revision.clone(),
                world,
                execution,
                bindings,
                direct_eval,
                initialized_references,
                dependencies: None,
                constants: constants::Values::default(),
                summaries: None,
                memoized: None,
                retain_memoized: true,
                temporary: vec![],
                tasks: vec![],
                values: vec![],
                work: Work::default(),
            },
        };
        if !direct_eval && execution.guarantees_strict_execution() {
            let (domains, mut work) = super::flow::parameter_domains(&mut result);
            work.summary_visits = result.snapshot.work.expression_visits;
            work.memoized_bytes = result.snapshot.memoized.as_ref().map_or(0, |cache| {
                cache.capacity() * std::mem::size_of::<Option<Facts>>()
            });
            result.snapshot.work.parameters = work;
            for (binding, domain) in result.snapshot.bindings.iter_mut().zip(domains) {
                binding.entry_value = domain.kind;
                binding.entry_i32 = domain.signed_i32;
            }
        }
        // Argument queries used the ordinary transfer before any parameter
        // seed was installed. Start the requested strategy with those seeds;
        // there is no circular inference from a callee back into its callers.
        if let Some(cache) = &mut result.snapshot.memoized {
            cache.fill(None);
        }
        result.snapshot.work.stored_summaries = 0;
        result.snapshot.retain_memoized = mode == Mode::Memoized;
        if !matches!(mode, Mode::Tree | Mode::Memoized) {
            result.snapshot.memoized = None;
        }
        if matches!(mode, Mode::Regions | Mode::Values) && !direct_eval {
            let output = super::flow::build(
                tree,
                &result.snapshot.bindings,
                world,
                mode == Mode::Values,
                &mut result.snapshot.initialized_references,
                &mut result.snapshot.constants,
            );
            result.snapshot.work.flow = output.work;
            result.snapshot.work.expression_visits += output.work.expressions;
            result.snapshot.work.stored_summaries = output.summaries.len();
            result.snapshot.summaries = Some(output.summaries);
            result.snapshot.dependencies = output.dependencies;
        } else if matches!(mode, Mode::Indexed | Mode::Regions | Mode::Values) {
            let mut summaries = Vec::with_capacity(module.expressions.len());
            for (index, node) in module.expressions.iter().enumerate() {
                summaries.push(
                    result.combine(ExprId::new(index), node, |child| summaries[child.index()]),
                );
            }
            result.snapshot.work.expression_visits += summaries.len();
            result.snapshot.work.stored_summaries = summaries.len();
            result.snapshot.summaries = Some(summaries);
        }
        result
    }

    pub fn resume(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        snapshot: Snapshot,
    ) -> Result<Self, &'static str> {
        Self::resume_in_execution(tree, snapshot, JavaScriptExecution::Script)
    }

    pub fn resume_in_execution(
        tree: &'tree lower::AnnotatedTree<'sem, 'src>,
        mut snapshot: Snapshot,
        execution: JavaScriptExecution,
    ) -> Result<Self, &'static str> {
        if !tree.revision.same(&snapshot.revision) {
            return Err("analysis snapshot belongs to a different program revision");
        }
        if snapshot.execution != execution {
            return Err("analysis snapshot belongs to a different JavaScript execution contract");
        }
        // Counters describe work performed in this phase. Retained summaries
        // are reused storage, not summaries computed a second time.
        snapshot.work = Work::default();
        // Retained values keep their resource accounting; resetting that
        // budget on reuse would allow unbounded derived storage.
        Ok(Self { tree, snapshot })
    }

    pub fn detach(self) -> Snapshot {
        self.snapshot
    }

    pub fn work(&self) -> Work {
        Work {
            constants: self.snapshot.constants.work,
            ..self.snapshot.work
        }
    }

    pub(super) fn constant(&mut self, facts: Facts) -> Option<Literal> {
        self.snapshot
            .constants
            .materialize(self.tree.target(), facts)
    }

    pub(super) fn constant_text(&mut self, facts: Facts) -> Option<StringValue> {
        self.snapshot
            .constants
            .materialize_text(self.tree.target(), facts)
    }

    pub(super) fn tree(&self) -> &'tree lower::AnnotatedTree<'sem, 'src> {
        self.tree
    }

    pub(super) fn world(&self) -> JavaScriptWorld {
        self.snapshot.world
    }

    pub fn execution(&self) -> JavaScriptExecution {
        self.snapshot.execution
    }

    pub(super) fn binding_uses(&self, symbol: BindingId) -> &BindingUses {
        &self.snapshot.bindings[symbol.index()]
    }

    /// A cell can disappear only when its value is unobserved and every
    /// lexical store is known initialized. RHS values/effects remain owned by
    /// their original evaluation sites. Compute this once, not per store.
    pub(super) fn unobserved_cell_removable(
        &self,
        binding: BindingId,
        world: JavaScriptWorld,
    ) -> bool {
        let module = self.tree.target();
        let root_scope = module.regions[module.root.index()].scope;
        !self.snapshot.bindings[binding.index()].exported
            && !self.snapshot.bindings[binding.index()].imported
            && (world == JavaScriptWorld::ClosedApplication
                || module.bindings[binding.index()].scope != root_scope)
            && self.snapshot.bindings[binding.index()]
                .writes
                .iter()
                .all(|write| self.snapshot.initialized_references[write.expression.index()])
    }

    pub(super) fn has_direct_eval(&self) -> bool {
        self.snapshot.direct_eval
    }

    pub fn facts(&mut self, id: ExprId) -> Facts {
        self.snapshot.work.queries += 1;
        if let Some(summaries) = &self.snapshot.summaries {
            return summaries[id.index()];
        }
        // The shared incoming-argument proof may have no eligible call at all.
        // Allocate query memo storage on its first real use, not when selecting
        // a strategy that will instead build direct/indexed summaries.
        if self.snapshot.memoized.is_none() {
            self.snapshot.memoized = Some(vec![None; self.tree.target().expressions.len()]);
        }
        let (facts, visits) = self.evaluate(id);
        self.snapshot.work.expression_visits += visits;
        if self.snapshot.retain_memoized {
            self.snapshot.work.stored_summaries += visits;
        } else {
            self.snapshot.work.temporary_summaries += visits;
            let cache = self.snapshot.memoized.as_mut().unwrap();
            for id in self.snapshot.temporary.drain(..) {
                cache[id.index()] = None;
            }
        }
        facts
    }

    // Explicit work/value stacks also handle long def-use chains. A program
    // with shallow syntax can have deep value dependencies; the host call
    // stack is not the compiler's resource limit. Scratch capacity is reused.
    fn evaluate(&mut self, root: ExprId) -> (Facts, usize) {
        let mut tasks = std::mem::take(&mut self.snapshot.tasks);
        let mut values = std::mem::take(&mut self.snapshot.values);
        let mut visits = 0;
        tasks.push(Task::Visit(root));
        while let Some(task) = tasks.pop() {
            match task {
                Task::Visit(id) => {
                    if let Some(value) = self
                        .snapshot
                        .memoized
                        .as_ref()
                        .and_then(|cache| cache[id.index()])
                    {
                        values.push((id, value));
                        continue;
                    }
                    tasks.push(Task::Finish(id, values.len()));
                    let first = tasks.len();
                    if let Some(definition) = self.definition(id) {
                        tasks.push(Task::Visit(definition));
                    }
                    self.tree.target().expressions[id.index()]
                        .visit_children::<()>(|child| {
                            tasks.push(Task::Visit(child));
                            Ok(())
                        })
                        .unwrap();
                    tasks[first..].reverse();
                }
                Task::Finish(id, first) => {
                    let mut next = first;
                    let result =
                        self.combine(id, &self.tree.target().expressions[id.index()], |child| {
                            let (computed, facts) = values[next];
                            debug_assert_eq!(computed, child);
                            next += 1;
                            facts
                        });
                    debug_assert_eq!(next, values.len());
                    values.truncate(first);
                    values.push((id, result));
                    visits += 1;
                    if let Some(cache) = &mut self.snapshot.memoized {
                        cache[id.index()] = Some(result);
                        if !self.snapshot.retain_memoized {
                            self.snapshot.temporary.push(id);
                        }
                    }
                }
            }
        }
        let result = values.pop().unwrap().1;
        debug_assert!(values.is_empty());
        self.snapshot.values = values;
        self.snapshot.tasks = tasks;
        (result, visits)
    }

    fn definition(&self, id: ExprId) -> Option<ExprId> {
        if self.snapshot.direct_eval {
            return None;
        }
        let Expr::Binding(symbol) = &self.tree.target().expressions[id.index()] else {
            return None;
        };
        let binding = &self.snapshot.bindings[symbol.index()];
        binding.initializer.filter(|value| {
            *value < id
                && binding.writes.is_empty()
                && self.snapshot.initialized_references[id.index()]
        })
    }

    fn combine(
        &mut self,
        id: ExprId,
        node: &Expr,
        mut child: impl FnMut(ExprId) -> Facts,
    ) -> Facts {
        // A producer supplies value knowledge, never its initialization effects.
        let definition = self.definition(id).map(&mut child).or_else(|| {
            let Expr::Binding(symbol) = node else {
                return None;
            };
            let binding = &self.snapshot.bindings[symbol.index()];
            (binding.entry_value != ValueKind::Unknown).then_some(Facts {
                value: binding.entry_value,
                integer: binding.entry_i32.then_some(IntegerRange::I32),
                ..Facts::default()
            })
        });
        let binding_effects = match node {
            Expr::Binding(symbol) => read_effects(
                &self.snapshot.bindings[symbol.index()],
                self.snapshot.initialized_references[id.index()],
            ),
            _ => Effects::default(),
        };
        transfer(
            self.tree,
            id,
            node,
            definition,
            binding_effects,
            &mut self.snapshot.constants,
            child,
        )
    }
}

pub(super) fn read_effects(binding: &BindingUses, initialized: bool) -> Effects {
    let mut effects = Effects::default();
    if !initialized {
        effects = effects.union(Effects::THROW);
    }
    if binding.imported || !binding.writes.is_empty() {
        effects = effects.union(Effects::MUTABLE_READ);
    }
    effects
}

/// Shared semantic transfer: neither analysis strategy owns a second set of
/// arithmetic, coercion or effect rules. Binding inputs describe this read.
pub(super) fn transfer(
    tree: &lower::AnnotatedTree<'_, '_>,
    id: ExprId,
    node: &Expr,
    definition: Option<Facts>,
    binding_effects: Effects,
    constants: &mut constants::Values,
    mut child: impl FnMut(ExprId) -> Facts,
) -> Facts {
    let mut inherited = Effects::default();
    // Each immediate child is queried exactly once in either mode. The
    // recursive contender must not pay for an accidentally exponential
    // implementation of this shared transfer function.
    let mut child = |id| {
        let facts = child(id);
        inherited = inherited.union(facts.effects);
        facts
    };
    let mut result = Facts::default();
    match node {
        Expr::Literal(literal) => {
            match literal {
                Literal::Number(value) => {
                    result.value = ValueKind::Number;
                    result.integer = IntegerRange::literal(*value);
                    // An exact integer range already owns this information.
                    // A second occurrence identity would make equal cell
                    // states appear different without adding a value proof.
                    if result.integer.is_none() {
                        result.constant = Some(constants::Known(id));
                    }
                }
                Literal::String(_) => {
                    result.value = ValueKind::String;
                    result.constant = Some(constants::Known(id));
                }
                Literal::Bool(_) => {
                    result.value = ValueKind::Bool;
                    result.constant = Some(constants::Known(id));
                }
                Literal::Null => result.value = ValueKind::Null,
                Literal::Undefined => result.value = ValueKind::Undefined,
            }
        }
        Expr::Binding(_) => {
            result.effects = binding_effects;
            if let Some(definition) = definition {
                result.value = definition.value;
                result.integer = definition.integer;
                result.constant = definition.constant;
            }
        }
        Expr::Host(_) => result.effects = Effects::THROW.union(Effects::FOREIGN),
        // A fresh object from a checked literal: no code runs.
        Expr::Regex(_) => {}
        Expr::This => {}
        Expr::ToInt32(value) => {
            let operand = child(*value);
            if !operand.value.primitive() {
                result.effects = result.effects.union(Effects::THROW).union(Effects::FOREIGN);
            }
            result.value = ValueKind::Number;
            let raw = number_facts(operand, false);
            result.integer = IntegerRange::from_number(raw.to_int32());
            result.normalization_redundant = raw.normalization_redundant();
        }
        Expr::Unary { op, value } => {
            let operand = child(*value);
            match op {
                Unary::Not => result.value = ValueKind::Bool,
                Unary::Void => result.value = ValueKind::Undefined,
                // A property deletion: a proxy trap or non-configurable
                // property (a strict-mode TypeError) is foreign behavior.
                Unary::Delete => {
                    result.value = ValueKind::Bool;
                    result.effects = Effects::HEAP_READ
                        .union(Effects::WRITE)
                        .union(Effects::THROW)
                        .union(Effects::FOREIGN);
                }
                Unary::TypeOf => result.value = ValueKind::String,
                Unary::Negate | Unary::Plus | Unary::BitNot => {
                    result.value = if operand.value.primitive() {
                        ValueKind::Number
                    } else {
                        ValueKind::Unknown
                    };
                    if !operand.value.primitive() {
                        result.effects =
                            result.effects.union(Effects::THROW).union(Effects::FOREIGN);
                    }
                    result.integer =
                        IntegerRange::from_number(number_facts(operand, true).unary(*op));
                }
            }
        }
        Expr::Binary { op, left, right } => {
            let left = child(*left);
            let right = child(*right);
            result = binary_facts(*op, left, right);
            if *op == Binary::Add {
                if let Some((a, b)) = constants
                    .string(tree.target(), left)
                    .zip(constants.string(tree.target(), right))
                {
                    let bytes = a.storage_bytes().saturating_add(b.storage_bytes());
                    result.constant = constants.evaluate_string(
                        id,
                        bytes.saturating_mul(2),
                        bytes.saturating_mul(2),
                        |values| {
                            values
                                .string(tree.target(), left)
                                .unwrap()
                                .concat(values.string(tree.target(), right).unwrap())
                        },
                    );
                }
            } else if matches!(op, Binary::And | Binary::Or | Binary::Nullish) {
                result.constant = constants.join(tree.target(), left.constant, right.constant);
                if *op == Binary::Nullish
                    && matches!(left.value, ValueKind::Null | ValueKind::Undefined)
                {
                    result.constant = right.constant;
                }
            }
        }
        Expr::IntBinary { op, left, right } => {
            let left = child(*left);
            let right = child(*right);
            result = binary_facts(op.javascript(), left, right);
            result.normalization_redundant = result.integer.is_some_and(|range| range.fits_i32());
            // The observable language result and the raw JavaScript result
            // are different proofs. Evaluate the former without making
            // normalization disappear from the latter.
            if let Some((left, right)) = left
                .integer
                .and_then(IntegerRange::singleton_i32)
                .zip(right.integer.and_then(IntegerRange::singleton_i32))
            {
                result.integer = Some(IntegerRange::exact_i32(op.evaluate(left, right)));
            } else if !result.normalization_redundant {
                result.integer = Some(IntegerRange::I32);
            }
            result.value = ValueKind::Number;
        }
        Expr::IntNegate(value) => {
            let operand = child(*value);
            if !operand.value.primitive() {
                result.effects = Effects::THROW.union(Effects::FOREIGN);
            }
            let raw_number = number_facts(operand, true).unary(Unary::Negate);
            let raw = IntegerRange::from_number(raw_number);
            result.normalization_redundant = raw_number.normalization_redundant();
            result.integer = Some(
                operand
                    .integer
                    .and_then(IntegerRange::singleton_i32)
                    .map(|value| IntegerRange::exact_i32(value.wrapping_neg()))
                    .or_else(|| raw.filter(|range| range.fits_i32()))
                    .unwrap_or(IntegerRange::I32),
            );
            result.value = ValueKind::Number;
        }
        Expr::ConstructIntrinsic { arguments, .. } => {
            for argument in arguments {
                child(*argument);
            }
            result.value = ValueKind::Object;
            result.effects = Effects::ALLOCATE
                .union(Effects::THROW)
                .union(Effects::FOREIGN);
        }
        Expr::Intrinsic {
            operation,
            receiver,
            arguments,
        } => {
            let receiver_facts = child(*receiver);
            for argument in arguments {
                child(*argument);
            }
            // These facts describe the selected JS recipe, including mutable
            // lookup and invocation. A source method identity does not prove
            // that the current property still names the built-in function.
            if integer_intrinsic(*operation) {
                // The typed printer emits |0 on every normal completion.
                // This says nothing about the unnormalized method result.
                result.value = ValueKind::Number;
                result.integer = Some(IntegerRange::I32);
            }
            match intrinsic_form(*operation) {
                IntrinsicForm::Method(_) => {
                    result.effects = Effects::HEAP_READ
                        .union(Effects::WRITE)
                        .union(Effects::THROW)
                        .union(Effects::FOREIGN);
                    // Noninteger methods return the actual invoked value. In
                    // particular, split need not return an array or fresh data,
                    // and slice/charAt need not return a primitive string.
                }
                IntrinsicForm::Property(_) => {
                    let length = match operation {
                        Intrinsic::StringLength if receiver_facts.value == ValueKind::String => {
                            let scan = constants
                                .string(tree.target(), receiver_facts)
                                .map(StringValue::storage_bytes);
                            if scan.is_some_and(|bytes| constants.charge(bytes)) {
                                constants
                                    .string(tree.target(), receiver_facts)
                                    .map(|value| value.code_units().count())
                            } else {
                                None
                            }
                        }
                        Intrinsic::ArrayLength => {
                            match &tree.target().expressions[receiver.index()] {
                                Expr::Array(elements)
                                    if !elements.iter().any(|element| {
                                        matches!(
                                            tree.target().expressions[element.index()],
                                            Expr::Spread(_)
                                        )
                                    }) =>
                                {
                                    Some(elements.len())
                                }
                                _ => {
                                    // Object/declared array is not an array-shape
                                    // proof: a proxy or foreign getter can reenter.
                                    result.effects = Effects::HEAP_READ
                                        .union(Effects::THROW)
                                        .union(Effects::FOREIGN);
                                    None
                                }
                            }
                        }
                        Intrinsic::StringLength => {
                            result.effects = Effects::HEAP_READ
                                .union(Effects::THROW)
                                .union(Effects::FOREIGN);
                            None
                        }
                        // Accessors on mutable prototypes (`size`, `flags`,
                        // `byteLength`): an invoked getter can write.
                        _ => {
                            result.effects = Effects::HEAP_READ
                                .union(Effects::WRITE)
                                .union(Effects::THROW)
                                .union(Effects::FOREIGN);
                            None
                        }
                    };
                    if let Some(length) = length {
                        result.integer = Some(IntegerRange::exact_i32(length as i32));
                        result.normalization_redundant = i32::try_from(length).is_ok();
                    }
                }
            }
        }
        // A class value evaluates its base, which throws unless it is a
        // constructor; `super(...)` runs host code and binds `this`.
        Expr::Class { .. } => {
            node.visit_children::<()>(|id| {
                child(id);
                Ok(())
            })
            .unwrap();
            result.value = ValueKind::Object;
            result.effects = Effects::ALLOCATE
                .union(Effects::THROW)
                .union(Effects::FOREIGN);
        }
        Expr::Member { .. } | Expr::Call { .. } | Expr::Construct { .. } | Expr::SuperCall { .. } => {
            node.visit_children::<()>(|id| {
                child(id);
                Ok(())
            })
            .unwrap();
            result.effects = if matches!(node, Expr::Member { .. }) {
                Effects::HEAP_READ
            } else {
                Effects::WRITE
            };
            result.effects = result.effects.union(Effects::THROW).union(Effects::FOREIGN);
        }
        Expr::Assign { target, value } => {
            child(*target);
            let value = child(*value);
            result.value = value.value;
            result.integer = value.integer;
            result.constant = value.constant;
            result.effects = result.effects.union(Effects::WRITE);
        }
        Expr::Conditional { condition, yes, no } => {
            child(*condition);
            let yes = child(*yes);
            let no = child(*no);
            if yes.value == no.value {
                result.value = yes.value;
            }
            result.constant = constants.join(tree.target(), yes.constant, no.constant);
            result.integer = yes.integer.zip(no.integer).map(|(a, b)| IntegerRange {
                minimum: a.minimum.min(b.minimum),
                maximum: a.maximum.max(b.maximum),
            });
        }
        Expr::Sequence(values) => {
            for value in values {
                let tail = child(*value);
                result.value = tail.value;
                result.integer = tail.integer;
                result.constant = tail.constant;
            }
        }
        Expr::Template(parts) => {
            result.value = ValueKind::String;
            let mut texts = Vec::new();
            let mut exact = true;
            for part in parts {
                match part {
                    TemplatePart::String(value) if exact => {
                        texts.push(constants::Text::Chunk(value))
                    }
                    TemplatePart::Expression(value) => {
                        let facts = child(*value);
                        if !facts.value.primitive() {
                            result.effects =
                                result.effects.union(Effects::THROW).union(Effects::FOREIGN);
                        }
                        if exact {
                            if let Some(text) = constants.text(tree.target(), facts) {
                                texts.push(text);
                            } else {
                                exact = false;
                                texts.clear();
                            }
                        }
                    }
                    _ => {}
                }
            }
            if exact {
                result.constant = constants.template(tree.target(), id, &texts);
            }
        }
        Expr::Array(values) => {
            for value in values {
                child(*value);
            }
            result.value = ValueKind::Object;
            result.effects = Effects::ALLOCATE;
        }
        // The spread operand's iterator protocol may be patched host code.
        Expr::Spread(value) => {
            child(*value);
            result.effects = Effects::HEAP_READ
                .union(Effects::WRITE)
                .union(Effects::THROW)
                .union(Effects::FOREIGN);
        }
        // A host load: its namespace is read later, and a chunk may run any
        // code; nothing about it is known now.
        Expr::LoadModule {
            members,
            promise,
            string,
            ..
        } => {
            child(*promise);
            child(*string);
            for (_, member) in members {
                child(*member);
            }
            result.value = ValueKind::Object;
            result.effects = Effects::HEAP_READ
                .union(Effects::WRITE)
                .union(Effects::THROW)
                .union(Effects::FOREIGN);
        }
        // Any code runs while the function is suspended, including closures
        // over this frame; the resumed value is unknown.
        Expr::Await(value) | Expr::Yield { value, .. } => {
            child(*value);
            result.effects = Effects::HEAP_READ
                .union(Effects::WRITE)
                .union(Effects::THROW)
                .union(Effects::FOREIGN);
        }
        Expr::Object(entries) => {
            result.value = ValueKind::Object;
            result.effects = Effects::ALLOCATE;
            for (key, value) in entries {
                if matches!(key, Property::Computed(id) if !child(*id).value.primitive()) {
                    result.effects = result.effects.union(Effects::THROW).union(Effects::FOREIGN);
                }
                child(*value);
            }
        }
        Expr::Function(_) => {
            result.value = ValueKind::Object;
            result.effects = Effects::ALLOCATE;
        }
    }
    // Source annotations retain language knowledge. They do not establish a
    // runtime domain for an unchecked parameter, foreign return or mutable
    // method result. Only this emitted recipe and its operand facts may do so.
    result.own_effects = result.effects;
    result.effects = result.effects.union(inherited);
    result
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Value,
    Place(ExprId),
    Reference,
    Call(ExprId),
    ArrayLength(ExprId),
}

fn scan_uses(
    module: &Module,
    id: ExprId,
    site: Site,
    context: Context,
    scope_functions: &[Option<FunctionId>],
    bindings: &mut [BindingUses],
    direct_eval: &mut bool,
) {
    match &module.expressions[id.index()] {
        Expr::Binding(symbol) => {
            let uses = &mut bindings[symbol.index()];
            let at = scope_functions[module.regions[site.region.index()].scope.index()];
            uses.captured |= at != uses.function;
            if let Context::Place(operation) = context {
                uses.writes.push(Write {
                    operation,
                    expression: id,
                    site,
                });
            } else {
                uses.reads.push(Read {
                    expression: id,
                    site,
                    observation: match context {
                        Context::Call(call) => Observation::Call(call),
                        Context::ArrayLength(length) => Observation::ArrayLength(length),
                        _ => Observation::Value,
                    },
                });
            }
        }
        Expr::Assign { target, value } => {
            // A property assignment reads its base/key; only a binding target
            // writes the lexical cell itself.
            scan_uses(
                module,
                *target,
                site,
                Context::Place(id),
                scope_functions,
                bindings,
                direct_eval,
            );
            scan_uses(
                module,
                *value,
                site,
                Context::Value,
                scope_functions,
                bindings,
                direct_eval,
            );
        }
        Expr::Intrinsic {
            operation: Intrinsic::ArrayLength,
            receiver,
            ..
        } => {
            scan_uses(
                module,
                *receiver,
                site,
                Context::ArrayLength(id),
                scope_functions,
                bindings,
                direct_eval,
            );
        }
        Expr::Member { object, property } => {
            // Collect the observation where the reference role is known.
            // Assignment places and receiver calls cannot masquerade as an
            // ordinary length read. The allocation proof belongs to planning.
            let observation = if context == Context::Value
                && matches!(property, Property::Named(name) if name == "length")
            {
                Context::ArrayLength(id)
            } else {
                Context::Value
            };
            scan_uses(
                module,
                *object,
                site,
                observation,
                scope_functions,
                bindings,
                direct_eval,
            );
            if let Property::Computed(key) = property {
                scan_uses(
                    module,
                    *key,
                    site,
                    Context::Value,
                    scope_functions,
                    bindings,
                    direct_eval,
                );
            }
        }
        Expr::Call {
            callee,
            arguments,
            invocation,
        } => {
            *direct_eval |= *invocation == Invocation::DirectEval;
            scan_uses(
                module,
                *callee,
                site,
                if matches!(module.expressions[callee.index()], Expr::Binding(_)) {
                    Context::Call(id)
                } else if *invocation == Invocation::Value {
                    Context::Value
                } else {
                    Context::Reference
                },
                scope_functions,
                bindings,
                direct_eval,
            );
            for argument in arguments {
                scan_uses(
                    module,
                    *argument,
                    site,
                    Context::Value,
                    scope_functions,
                    bindings,
                    direct_eval,
                );
            }
        }
        node => {
            node.visit_children::<()>(|child| {
                scan_uses(
                    module,
                    child,
                    site,
                    Context::Value,
                    scope_functions,
                    bindings,
                    direct_eval,
                );
                Ok(())
            })
            .unwrap();
        }
    }
}

/// Adapt the legacy no-negative-zero range view into the common raw Number
/// owner. Primitive coercion is admitted only by the calling JS operation;
/// this adapter does not clear the independently computed operation effects.
fn number_facts(facts: Facts, coerce_primitive: bool) -> NumberFacts {
    if let Some(range) = facts.integer {
        range.number()
    } else if facts.value == ValueKind::Number || (coerce_primitive && facts.value.primitive()) {
        NumberFacts::NUMBER
    } else {
        NumberFacts::UNKNOWN
    }
}

fn binary_facts(op: Binary, left: Facts, right: Facts) -> Facts {
    let mut result = Facts::default();
    if !matches!(
        op,
        Binary::StrictEqual | Binary::StrictNotEqual | Binary::And | Binary::Or | Binary::Nullish
    ) && !(left.value.primitive() && right.value.primitive())
    {
        result.effects = result.effects.union(Effects::THROW).union(Effects::FOREIGN);
    }
    if op == Binary::In {
        // A proxy `has` trap can run arbitrary code; a primitive right side throws.
        result.effects = result
            .effects
            .union(Effects::HEAP_READ)
            .union(Effects::WRITE)
            .union(Effects::THROW)
            .union(Effects::FOREIGN);
    }
    result.value = match op {
        Binary::Equal | Binary::NotEqual | Binary::In => ValueKind::Bool,
        Binary::StrictEqual
        | Binary::StrictNotEqual
        | Binary::Less
        | Binary::LessEqual
        | Binary::Greater
        | Binary::GreaterEqual => ValueKind::Bool,
        Binary::Nullish if matches!(left.value, ValueKind::Null | ValueKind::Undefined) => {
            right.value
        }
        Binary::And | Binary::Or | Binary::Nullish => {
            if left.value == right.value {
                left.value
            } else {
                ValueKind::Unknown
            }
        }
        Binary::Add if left.value == ValueKind::String || right.value == ValueKind::String => {
            ValueKind::String
        }
        Binary::Add
            if left.value == ValueKind::Unknown
                || right.value == ValueKind::Unknown
                || left.value == ValueKind::Object
                || right.value == ValueKind::Object =>
        {
            ValueKind::Unknown
        }
        _ => {
            if left.value.primitive() && right.value.primitive() {
                ValueKind::Number
            } else {
                ValueKind::Unknown
            }
        }
    };
    // Legacy annotations and effects remain owned by this analyzer. The raw
    // arithmetic transfer itself is shared with direct target formation and
    // does not inspect those annotations or assume pristine host methods.
    let coerce = !matches!(op, Binary::Add | Binary::And | Binary::Or | Binary::Nullish)
        || (op == Binary::Add && result.value == ValueKind::Number);
    let numeric = number_facts(left, coerce).binary(op, number_facts(right, coerce));
    result.integer = IntegerRange::from_number(numeric);
    if numeric.is_number() {
        result.value = ValueKind::Number;
    }
    result
}
