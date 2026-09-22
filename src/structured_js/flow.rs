//! Two value analyses over one evaluation-order walk. Direct propagation keeps
//! value facts in local cells. The dependency contender keeps producer and join
//! references and evaluates them in construction order. Neither copies syntax.
//! Loops deliberately forget incoming/current values; loop SSA is not claimed.

use super::*;
use crate::compilation_contract::JavaScriptWorld;
use analysis::{BindingUses, Effects, Facts, IntegerRange, ValueKind};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Work {
    pub expressions: usize,
    pub cell_updates: usize,
    pub merged_cells: usize,
    pub invalidated_cells: usize,
    pub producer_edges: usize,
    pub joins: usize,
    pub dependency_bytes: usize,
    pub peak_live_values: usize,
}

/// Observed work/storage for the legacy incoming-argument proof. This reports
/// actual traversal and temporary capacities; it is not the new Compilation's
/// admission ledger and does not certify that legacy analysis is budgeted.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ParameterWork {
    pub initialized_slots: usize,
    pub expressions: usize,
    pub statements: usize,
    pub call_uses: usize,
    pub argument_slots: usize,
    pub argument_queries: usize,
    pub summary_visits: usize,
    pub temporary_bytes: usize,
    /// Peak query-memo capacity while qualifying actual argument producers.
    /// Tree/memo strategies may retain this allocation for subsequent queries.
    pub memoized_bytes: usize,
}

/// Deliberately coarse: actual argument producers can establish primitive
/// kind and signed range without forcing specialization to one literal value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct ParameterDomain {
    pub(super) kind: ValueKind,
    pub(super) signed_i32: bool,
}
impl ParameterDomain {
    fn from_facts(facts: Facts) -> Self {
        Self {
            kind: facts.value,
            signed_i32: facts.integer.is_some_and(|range| range.fits_i32()),
        }
    }
    fn value(self) -> Value {
        Value {
            kind: self.kind,
            integer: self.signed_i32.then_some(IntegerRange::I32),
            constant: None,
        }
    }
    fn from_binding(binding: &BindingUses) -> Self {
        Self {
            kind: binding.entry_value,
            signed_i32: binding.entry_i32,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Value {
    kind: ValueKind,
    integer: Option<IntegerRange>,
    constant: Option<constants::Known>,
}
impl Value {
    fn from_facts(facts: Facts) -> Self {
        Self {
            kind: facts.value,
            integer: facts.integer,
            constant: facts.constant,
        }
    }
    fn facts(self) -> Facts {
        Facts {
            value: self.kind,
            integer: self.integer,
            constant: self.constant,
            ..Facts::default()
        }
    }
    fn join(self, other: Self, module: &Module, constants: &mut constants::Values) -> Self {
        Self {
            kind: if self.kind == other.kind {
                self.kind
            } else {
                ValueKind::Unknown
            },
            constant: constants.join(module, self.constant, other.constant),
            integer: self.integer.zip(other.integer).map(|(a, b)| IntegerRange {
                minimum: a.minimum.min(b.minimum),
                maximum: a.maximum.max(b.maximum),
            }),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Reference {
    #[default]
    Unknown,
    Undefined,
    Parameter(ParameterDomain),
    Expression(ExprId),
    Join(u32),
}

/// Retained dependencies describe value production, not permission to move or
/// duplicate the referenced expression. The temporary schedule is not retained.
#[derive(Debug)]
pub(super) struct Dependencies {
    reads: Vec<Reference>,
    read_effects: Vec<Effects>,
    joins: Vec<(Reference, Reference)>,
}

pub(super) struct Output {
    pub summaries: Vec<Facts>,
    pub dependencies: Option<Dependencies>,
    pub work: Work,
}

trait Domain {
    type Current: Copy + Default + PartialEq;
    fn undefined(&self) -> Self::Current;
    fn expression(
        &mut self,
        tree: &lower::AnnotatedTree<'_, '_>,
        id: ExprId,
        input: Self::Current,
        effects: Effects,
    ) -> Self::Current;
    fn join(&mut self, module: &Module, a: Self::Current, b: Self::Current) -> Self::Current;
    fn finish(self, tree: &lower::AnnotatedTree<'_, '_>, work: Work) -> Output;
}

struct Direct<'a> {
    summaries: Vec<Facts>,
    constants: &'a mut constants::Values,
}
impl Domain for Direct<'_> {
    type Current = Value;
    fn undefined(&self) -> Value {
        Value {
            kind: ValueKind::Undefined,
            integer: None,
            constant: None,
        }
    }
    fn expression(
        &mut self,
        tree: &lower::AnnotatedTree<'_, '_>,
        id: ExprId,
        input: Value,
        effects: Effects,
    ) -> Value {
        let facts = analysis::transfer(
            tree,
            id,
            &tree.target().expressions[id.index()],
            Some(input.facts()),
            effects,
            self.constants,
            |child| self.summaries[child.index()],
        );
        self.summaries[id.index()] = facts;
        Value::from_facts(facts)
    }
    fn join(&mut self, module: &Module, a: Value, b: Value) -> Value {
        a.join(b, module, self.constants)
    }
    fn finish(self, _: &lower::AnnotatedTree<'_, '_>, work: Work) -> Output {
        Output {
            summaries: self.summaries,
            dependencies: None,
            work,
        }
    }
}

struct Indexed<'a> {
    constants: &'a mut constants::Values,
    dependencies: Dependencies,
    schedule: Vec<Reference>,
    visited: Vec<bool>,
}
impl Domain for Indexed<'_> {
    type Current = Reference;
    fn undefined(&self) -> Reference {
        Reference::Undefined
    }
    fn expression(
        &mut self,
        tree: &lower::AnnotatedTree<'_, '_>,
        id: ExprId,
        input: Reference,
        effects: Effects,
    ) -> Reference {
        if matches!(tree.target().expressions[id.index()], Expr::Binding(_)) {
            self.dependencies.reads[id.index()] = input;
            self.dependencies.read_effects[id.index()] = effects;
        }
        // Only literals may have multiple syntax owners. Their value/effects
        // do not depend on the environment at any of those occurrences.
        if !self.visited[id.index()] {
            self.visited[id.index()] = true;
            self.schedule.push(Reference::Expression(id));
        }
        Reference::Expression(id)
    }
    fn join(&mut self, _: &Module, a: Reference, b: Reference) -> Reference {
        if a == b {
            return a;
        }
        if a == Reference::Unknown || b == Reference::Unknown {
            return Reference::Unknown;
        }
        let id = u32::try_from(self.dependencies.joins.len()).expect("value join capacity");
        self.dependencies.joins.push((a, b));
        self.schedule.push(Reference::Join(id));
        Reference::Join(id)
    }
    fn finish(self, tree: &lower::AnnotatedTree<'_, '_>, mut work: Work) -> Output {
        let mut summaries = vec![Facts::default(); tree.target().expressions.len()];
        let mut joins = Vec::with_capacity(self.dependencies.joins.len());
        let constants = self.constants;
        let get = |reference, summaries: &[Facts], joins: &[Value]| match reference {
            Reference::Unknown => Value::default(),
            Reference::Parameter(domain) => domain.value(),
            Reference::Undefined => Value {
                kind: ValueKind::Undefined,
                integer: None,
                constant: None,
            },
            Reference::Expression(id) => Value::from_facts(summaries[id.index()]),
            Reference::Join(id) => joins[id as usize],
        };
        for next in self.schedule {
            match next {
                Reference::Expression(id) => {
                    let input = get(self.dependencies.reads[id.index()], &summaries, &joins);
                    summaries[id.index()] = analysis::transfer(
                        tree,
                        id,
                        &tree.target().expressions[id.index()],
                        Some(input.facts()),
                        self.dependencies.read_effects[id.index()],
                        constants,
                        |child| summaries[child.index()],
                    );
                }
                Reference::Join(id) => {
                    debug_assert_eq!(id as usize, joins.len());
                    let (a, b) = self.dependencies.joins[id as usize];
                    joins.push(get(a, &summaries, &joins).join(
                        get(b, &summaries, &joins),
                        tree.target(),
                        constants,
                    ));
                }
                _ => unreachable!(),
            }
        }
        work.producer_edges = self
            .dependencies
            .reads
            .iter()
            .filter(|value| matches!(value, Reference::Expression(_) | Reference::Join(_)))
            .count();
        work.joins = self.dependencies.joins.len();
        work.dependency_bytes = self.dependencies.reads.capacity()
            * std::mem::size_of::<Reference>()
            + self.dependencies.read_effects.capacity() * std::mem::size_of::<Effects>()
            + self.dependencies.joins.capacity() * std::mem::size_of::<(Reference, Reference)>();
        Output {
            summaries,
            dependencies: Some(self.dependencies),
            work,
        }
    }
}

/// Constant-time membership/update; invalidation visits current values only,
/// never every binding in the program for every region or function.
struct SparseSet {
    members: Vec<usize>,
    positions: Vec<usize>,
}
impl SparseSet {
    fn new(size: usize) -> Self {
        Self {
            members: Vec::new(),
            positions: vec![usize::MAX; size],
        }
    }
    fn set(&mut self, slot: usize, present: bool) {
        let position = self.positions[slot];
        if present && position == usize::MAX {
            self.positions[slot] = self.members.len();
            self.members.push(slot);
        } else if !present && position != usize::MAX {
            self.members.swap_remove(position);
            self.positions[slot] = usize::MAX;
            if let Some(moved) = self.members.get(position) {
                self.positions[*moved] = position;
            }
        }
    }
    fn clear(&mut self) {
        for slot in self.members.drain(..) {
            self.positions[slot] = usize::MAX;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Cell<V> {
    initialized: bool,
    value: V,
}

struct Environment<V: Copy + Default + PartialEq> {
    slots: Vec<(u32, Cell<V>)>,
    initialized_at_entry: Vec<bool>,
    entry_values: Vec<V>,
    exposed: Vec<bool>,
    run: u32,
    active: SparseSet,
    active_exposed: SparseSet,
    trail: Vec<(usize, Cell<V>)>,
    forks: usize,
    seen: Vec<u32>,
    rollback_id: u32,
    work: Work,
}
impl<V: Copy + Default + PartialEq> Environment<V> {
    fn new(
        bindings: &[BindingUses],
        module: &Module,
        world: JavaScriptWorld,
        entry: impl Fn(&BindingUses) -> V,
    ) -> Self {
        let count = bindings.len();
        Self {
            slots: vec![
                (
                    0,
                    Cell {
                        initialized: false,
                        value: V::default()
                    }
                );
                count
            ],
            entry_values: bindings.iter().map(entry).collect(),
            initialized_at_entry: bindings
                .iter()
                .map(|uses| uses.initialized_on_scope_entry)
                .collect(),
            exposed: bindings
                .iter()
                .enumerate()
                .map(|(id, uses)| {
                    uses.imported
                        || (uses.captured && !uses.writes.is_empty())
                        || (world == JavaScriptWorld::ReusableLibrary
                            && module.bindings[id].scope
                                == module.regions[module.root.index()].scope)
                })
                .collect(),
            run: 0,
            active: SparseSet::new(count),
            active_exposed: SparseSet::new(count),
            trail: Vec::new(),
            forks: 0,
            seen: vec![0; count],
            rollback_id: 0,
            work: Work::default(),
        }
    }
    fn next_function(&mut self) {
        debug_assert_eq!(self.forks, 0);
        self.run = self
            .run
            .checked_add(1)
            .expect("function environment capacity");
        self.active.clear();
        self.active_exposed.clear();
    }
    fn get(&self, slot: usize) -> Cell<V> {
        if self.slots[slot].0 == self.run {
            self.slots[slot].1
        } else {
            Cell {
                initialized: self.initialized_at_entry[slot],
                value: self.entry_values[slot],
            }
        }
    }
    fn write(&mut self, slot: usize, cell: Cell<V>) {
        let previous = self.get(slot);
        if previous == cell {
            return;
        }
        if self.forks != 0 {
            self.trail.push((slot, previous));
        }
        self.restore(slot, cell);
        self.work.cell_updates += 1;
    }
    fn restore(&mut self, slot: usize, cell: Cell<V>) {
        self.slots[slot] = (self.run, cell);
        let known = cell.value != V::default();
        self.active.set(slot, known);
        self.active_exposed.set(slot, known && self.exposed[slot]);
        self.work.peak_live_values = self.work.peak_live_values.max(self.active.members.len());
    }
    fn invalidate(&mut self, exposed_only: bool) {
        loop {
            let slot = if exposed_only {
                self.active_exposed.members.last()
            } else {
                self.active.members.last()
            };
            let Some(slot) = slot.copied() else {
                break;
            };
            let mut cell = self.get(slot);
            cell.value = V::default();
            self.write(slot, cell);
            self.work.invalidated_cells += 1;
        }
    }
    fn checkpoint(&mut self) -> usize {
        self.forks += 1;
        self.trail.len()
    }
    fn rollback(&mut self, checkpoint: usize) -> Vec<(usize, Cell<V>)> {
        self.rollback_id = self
            .rollback_id
            .checked_add(1)
            .expect("branch environment capacity");
        let mut changed = Vec::new();
        while self.trail.len() > checkpoint {
            let (slot, old) = self.trail.pop().unwrap();
            if self.seen[slot] != self.rollback_id {
                self.seen[slot] = self.rollback_id;
                changed.push((slot, self.get(slot)));
            }
            self.restore(slot, old);
        }
        self.forks -= 1;
        changed.sort_unstable_by_key(|(slot, _)| *slot);
        changed
    }
}

struct Walker<'a, 'sem, 'src, D: Domain> {
    tree: &'a lower::AnnotatedTree<'sem, 'src>,
    bindings: &'a [BindingUses],
    domain: D,
    environment: Environment<D::Current>,
    initialized_references: &'a mut [bool],
}
impl<D: Domain> Walker<'_, '_, '_, D> {
    fn expression(&mut self, id: ExprId) -> D::Current {
        let mut input = D::Current::default();
        let mut effects = Effects::default();
        match &self.tree.target().expressions[id.index()] {
            Expr::Template(parts) => {
                for part in parts {
                    if let TemplatePart::Expression(value) = part {
                        self.expression(*value);
                        // ToString can reenter through a foreign object's
                        // conversion hook before the next substitution reads.
                        self.environment.invalidate(true);
                    }
                }
            }
            Expr::ConstructIntrinsic { arguments, .. } => {
                // The implicit native constructor lookup can invoke a global
                // getter before arguments. The construction can reenter too.
                self.environment.invalidate(true);
                for argument in arguments {
                    self.expression(*argument);
                }
                self.environment.invalidate(true);
            }
            Expr::Intrinsic {
                operation,
                receiver,
                arguments,
            } => {
                self.expression(*receiver);
                // Method lookup occurs before argument evaluation. A getter
                // may mutate exposed cells whose snapshots those arguments
                // then read. Retain already evaluated receiver/argument values
                // in the existing expression domain, as for ordinary calls.
                self.environment.invalidate(true);
                for argument in arguments {
                    self.expression(*argument);
                }
                if matches!(intrinsic_form(*operation), IntrinsicForm::Method(_)) {
                    // The invoked method (and integer result coercion, if
                    // selected) may reenter again after all argument snapshots.
                    self.environment.invalidate(true);
                }
            }
            Expr::Binding(symbol) => {
                let uses = &self.bindings[symbol.index()];
                let cell = self.environment.get(symbol.index());
                self.initialized_references[id.index()] = cell.initialized;
                input = cell.value;
                effects = analysis::read_effects(uses, cell.initialized);
                if self.environment.exposed[symbol.index()] {
                    effects = effects.union(Effects::MUTABLE_READ);
                }
            }
            Expr::Assign { target, value } => {
                self.place(*target);
                let current = self.expression(*value);
                if let Expr::Binding(symbol) = self.tree.target().expressions[target.index()] {
                    let slot = symbol.index();
                    let mut cell = self.environment.get(slot);
                    // Assignment cannot initialize a lexical declaration in TDZ.
                    if cell.initialized {
                        cell.value = current;
                        self.environment.write(slot, cell);
                    }
                } else {
                    self.environment.invalidate(true);
                }
            }
            Expr::Conditional { condition, yes, no } => {
                self.expression(*condition);
                self.branch(
                    |walk| {
                        walk.expression(*yes);
                    },
                    |walk| {
                        walk.expression(*no);
                    },
                );
            }
            Expr::Binary {
                op: Binary::And | Binary::Or | Binary::Nullish,
                left,
                right,
            } => {
                self.expression(*left);
                self.branch(
                    |walk| {
                        walk.expression(*right);
                    },
                    |_| {},
                );
            }
            Expr::Object(entries) => {
                for (key, value) in entries {
                    if let Property::Computed(key) = key {
                        self.expression(*key);
                        // ToPropertyKey happens before this property's value.
                        self.environment.invalidate(true);
                    }
                    self.expression(*value);
                }
            }
            node => {
                node.visit_children::<()>(|child| {
                    self.expression(child);
                    Ok(())
                })
                .unwrap();
                if matches!(
                    node,
                    Expr::Host(_)
                        | Expr::Member { .. }
                        | Expr::Call { .. }
                        | Expr::Construct { .. }
                        | Expr::ToInt32(_)
                        | Expr::IntNegate(_)
                        | Expr::IntBinary { .. }
                        | Expr::Unary {
                            op: Unary::Negate | Unary::Plus | Unary::BitNot,
                            ..
                        }
                        | Expr::Binary {
                            op: Binary::Add
                                | Binary::Subtract
                                | Binary::Multiply
                                | Binary::Divide
                                | Binary::Remainder
                                | Binary::ShiftLeft
                                | Binary::ShiftRight
                                | Binary::UnsignedShiftRight
                                | Binary::Less
                                | Binary::LessEqual
                                | Binary::Greater
                                | Binary::GreaterEqual
                                | Binary::BitAnd
                                | Binary::BitOr
                                | Binary::BitXor,
                            ..
                        }
                ) {
                    // Both contenders use this conservative barrier policy.
                    // A future effect proof may remove barriers, independently
                    // of whether values are facts or retained producer links.
                    self.environment.invalidate(true);
                }
            }
        }
        self.environment.work.expressions += 1;
        self.domain.expression(self.tree, id, input, effects)
    }

    fn place(&mut self, id: ExprId) {
        if let Expr::Member { object, property } = &self.tree.target().expressions[id.index()] {
            self.expression(*object);
            if let Property::Computed(key) = property {
                self.expression(*key);
            }
            // Creating this reference performs neither GetValue nor key
            // coercion. For a simple assignment, PutValue coerces the key
            // after the RHS; Assign applies that barrier after evaluating it.
            // The conservative effect summary does not change this order.
            self.environment.work.expressions += 1;
            self.domain
                .expression(self.tree, id, D::Current::default(), Effects::default());
        } else {
            self.expression(id);
        }
    }

    fn branch(&mut self, yes: impl FnOnce(&mut Self), no: impl FnOnce(&mut Self)) {
        let checkpoint = self.environment.checkpoint();
        yes(self);
        let left = self.environment.rollback(checkpoint);
        let checkpoint = self.environment.checkpoint();
        no(self);
        let right = self.environment.rollback(checkpoint);
        let mut left = left.into_iter().peekable();
        let mut right = right.into_iter().peekable();
        while left.peek().is_some() || right.peek().is_some() {
            let slot = match (left.peek(), right.peek()) {
                (Some((a, _)), Some((b, _))) => *a.min(b),
                (Some((a, _)), None) | (None, Some((a, _))) => *a,
                _ => unreachable!(),
            };
            let previous = self.environment.get(slot);
            let a = if left.peek().is_some_and(|(at, _)| *at == slot) {
                left.next().unwrap().1
            } else {
                previous
            };
            let b = if right.peek().is_some_and(|(at, _)| *at == slot) {
                right.next().unwrap().1
            } else {
                previous
            };
            let initialized = a.initialized && b.initialized;
            let value = if initialized {
                self.domain.join(self.tree.target(), a.value, b.value)
            } else {
                D::Current::default()
            };
            self.environment.write(slot, Cell { initialized, value });
            self.environment.work.merged_cells += 1;
        }
    }

    fn region(&mut self, id: RegionId) {
        for statement in &self.tree.target().regions[id.index()].statements {
            match statement {
                Statement::Let { binding, value } => {
                    let value = match value {
                        Some(id) => self.expression(*id),
                        None => self.domain.undefined(),
                    };
                    self.environment.write(
                        binding.index(),
                        Cell {
                            initialized: true,
                            value,
                        },
                    );
                }
                Statement::Evaluate(value)
                | Statement::Throw(value)
                | Statement::Return(Some(value)) => {
                    self.expression(*value);
                }
                Statement::If { condition, yes, no } => {
                    self.expression(*condition);
                    self.branch(
                        |walk| walk.region(*yes),
                        |walk| {
                            if let Some(no) = no {
                                walk.region(*no);
                            }
                        },
                    );
                }
                Statement::Loop {
                    condition,
                    update,
                    body,
                } => {
                    // A finite, conservative loop policy: do not use entry
                    // values as if they held on every condition evaluation.
                    self.environment.invalidate(false);
                    if let Some(condition) = condition {
                        self.expression(*condition);
                    }
                    self.region(*body);
                    self.environment.invalidate(false);
                    if let Some(update) = update {
                        // The body walk also visits syntax following break or
                        // continue. Until abrupt successors are modeled, none
                        // of those current values is a proof at the update.
                        self.expression(*update);
                        self.environment.invalidate(false);
                    }
                }
                Statement::Block(body) => self.region(*body),
                Statement::ForIn {
                    binding,
                    object,
                    body,
                }
                | Statement::ForOf {
                    binding,
                    iterable: object,
                    body,
                } => {
                    self.expression(*object);
                    // Key enumeration can run proxy traps and iteration runs
                    // the iterator; each iteration binds an unknown value.
                    self.environment.invalidate(true);
                    self.environment.write(
                        binding.index(),
                        Cell {
                            initialized: true,
                            value: D::Current::default(),
                        },
                    );
                    self.region(*body);
                    self.environment.invalidate(false);
                }
                Statement::Try {
                    body,
                    catch,
                    finally,
                } => {
                    self.region(*body);
                    // A handler can be reached from any throwing evaluation,
                    // not just the syntactic end of the body. Do not transport
                    // its final current values across an unknown predecessor.
                    self.environment.invalidate(false);
                    if let Some(catch) = catch {
                        if let Some(binding) = catch.binding {
                            self.environment.write(
                                binding.index(),
                                Cell {
                                    initialized: true,
                                    value: D::Current::default(),
                                },
                            );
                        }
                        self.region(catch.body);
                    }
                    if let Some(finally) = finally {
                        // The finalizer also runs when the body or handler
                        // returns or throws before reaching its last syntax.
                        self.environment.invalidate(false);
                        self.region(*finally);
                    }
                    // Finally may replace a pending abrupt completion. Exact
                    // successor-sensitive propagation is separate work.
                    self.environment.invalidate(false);
                }
                Statement::Function { .. }
                | Statement::Return(None)
                | Statement::Break
                | Statement::Continue => {}
            }
        }
    }

    fn run(mut self) -> Output {
        self.environment.next_function();
        self.region(self.tree.target().root);
        for function in &self.tree.target().functions {
            self.environment.next_function();
            self.region(function.body);
        }
        self.domain.finish(self.tree, self.environment.work)
    }
}

pub(super) fn build(
    tree: &lower::AnnotatedTree<'_, '_>,
    bindings: &[BindingUses],
    world: JavaScriptWorld,
    indexed: bool,
    initialized_references: &mut [bool],
    constants: &mut constants::Values,
) -> Output {
    let count = tree.target().expressions.len();
    if indexed {
        Walker {
            tree,
            bindings,
            initialized_references,
            environment: Environment::new(bindings, tree.target(), world, |binding| {
                if binding.entry_value == ValueKind::Unknown {
                    Reference::Unknown
                } else {
                    Reference::Parameter(ParameterDomain::from_binding(binding))
                }
            }),
            domain: Indexed {
                constants,
                dependencies: Dependencies {
                    reads: vec![Reference::Unknown; count],
                    read_effects: vec![Effects::default(); count],
                    joins: Vec::new(),
                },
                schedule: Vec::with_capacity(count),
                visited: vec![false; count],
            },
        }
        .run()
    } else {
        Walker {
            tree,
            bindings,
            initialized_references,
            environment: Environment::new(bindings, tree.target(), world, |binding| {
                ParameterDomain::from_binding(binding).value()
            }),
            domain: Direct {
                summaries: vec![Facts::default(); count],
                constants,
            },
        }
        .run()
    }
}

/// One finite incoming-domain pass, using the existing expression transfer.
/// Only declaration-owned callable identities with complete direct-call uses
/// qualify. Public/escaping/reassigned callables and mutable parameters retain
/// Unknown. All arguments are queried before installing any result, so cycles
/// cannot bootstrap a primitive proof from declared types or from themselves.
/// The caller must supply a Module-qualified analysis. Sloppy functions can
/// escape through an opaque callback's Function.caller even when every explicit
/// source use is a direct call; no incoming-domain proof applies in that case.
pub(super) fn parameter_domains(
    analysis: &mut analysis::Analysis<'_, '_, '_>,
) -> (Vec<ParameterDomain>, ParameterWork) {
    debug_assert!(analysis.execution().guarantees_strict_execution());
    let module = analysis.tree().target();
    let mut domains = vec![ParameterDomain::default(); module.bindings.len()];
    let mut materializations = vec![0usize; module.functions.len()];
    let mut work = ParameterWork {
        initialized_slots: domains.len() + materializations.len(),
        temporary_bytes: domains.capacity() * std::mem::size_of::<ParameterDomain>()
            + materializations.capacity() * std::mem::size_of::<usize>(),
        ..ParameterWork::default()
    };
    for node in &module.expressions {
        work.expressions += 1;
        // The legacy target can be inspected as a sloppy script too. A raw
        // arguments reference may mutate mapped parameters without a Binding
        // write; until that access is modeled, no incoming-domain seed applies.
        if matches!(node, Expr::Host(name) if name == "arguments") {
            return (domains, work);
        }
        if let Some(function) = node.created_function() {
            materializations[function.index()] += 1;
        }
    }
    for region in &module.regions {
        for statement in &region.statements {
            work.statements += 1;
            if let Statement::Function { function, .. } = statement {
                materializations[function.index()] += 1;
            }
        }
    }
    for region in &module.regions {
        for statement in &region.statements {
            work.statements += 1;
            let Statement::Function { binding, function } = *statement else {
                continue;
            };
            let uses = analysis.binding_uses(binding);
            if materializations[function.index()] != 1
                || uses.exported
                || !uses.writes.is_empty()
                || uses.reads.is_empty()
                || (analysis.world() == JavaScriptWorld::ReusableLibrary
                    && module.bindings[binding.index()].scope
                        == module.regions[module.root.index()].scope)
                || uses.reads.iter().any(|read| {
                    work.call_uses += 1;
                    !matches!(read.observation, analysis::Observation::Call(_))
                })
            {
                continue;
            }
            let parameters = &module.functions[function.index()].parameters;
            let calls = uses.reads.len();
            for index in 0..calls {
                work.call_uses += 1;
                let read = analysis.binding_uses(binding).reads[index];
                let analysis::Observation::Call(call) = read.observation else {
                    unreachable!()
                };
                let Expr::Call {
                    callee, arguments, ..
                } = &module.expressions[call.index()]
                else {
                    for parameter in parameters {
                        domains[parameter.index()] = ParameterDomain::default();
                    }
                    break;
                };
                if *callee != read.expression || arguments.len() != parameters.len() {
                    for parameter in parameters {
                        domains[parameter.index()] = ParameterDomain::default();
                    }
                    break;
                }
                for (&parameter, &argument) in parameters.iter().zip(arguments) {
                    work.argument_slots += 1;
                    if !analysis.binding_uses(parameter).writes.is_empty()
                        || (index != 0 && domains[parameter.index()].kind == ValueKind::Unknown)
                    {
                        continue;
                    }
                    work.argument_queries += 1;
                    let incoming = ParameterDomain::from_facts(analysis.facts(argument));
                    let previous = domains[parameter.index()];
                    domains[parameter.index()] = if index == 0 {
                        incoming
                    } else if previous.kind == incoming.kind {
                        ParameterDomain {
                            kind: incoming.kind,
                            signed_i32: previous.signed_i32 && incoming.signed_i32,
                        }
                    } else {
                        ParameterDomain::default()
                    };
                }
            }
        }
    }
    (domains, work)
}
