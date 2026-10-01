//! The program call graph (architecture §6 "Derived views"; plan M6.1).
//!
//! A derived view of a checked program: it is computed from the program's
//! structural uses (the one enumeration in `uses::walk`), keyed by the
//! program's revisions and never edited. It answers, per call, which body the
//! call runs; per body, which calls reach it when that set is complete; which
//! bodies escape as values; and the strongly connected components, callees
//! first, that effect summaries are computed over.
//!
//! **Resolution.** A callee value denotes a body when it is that body's
//! closure, or a load of storage that denotes the body. Storage denotes a body
//! when it is initialized exactly once, from a value that denotes the body,
//! and is never written, passed by reference or supplied by the host. Root
//! (module-level) storage is sealed in every build the compiler makes: a
//! module's roots are its own, and so, since owner answer Y5 (2026-09-29),
//! are an application script's (its top-level bindings are private to the
//! program unless declared global; a library is always a module). Under
//! `Seal::StructuralOnly`, kept for a script whose globals other scripts
//! share (no contract requests one today), a load of root storage denotes
//! nothing. A load that
//! runs before the one initialization throws before any call happens, so the
//! call can only ever run the denoted body. This is the rule
//! `callable_inputs` applies to one producer at a time, stated once for the
//! whole program.
use super::uses::{self, CellUse, Event, ValueUse};
use super::views::Deps;
use super::*;
use crate::catalog::callback_intrinsic;
use crate::check::BuiltinCall;
use crate::compilation_contract::JavaScriptExecution;
use crate::primitive::ResolvedIntrinsic;

use super::analysis_storage as storage;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

/// Whether root storage is sealed against the host (see the module comment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Seal {
    StructuralOnly = 0,
    Module = 1,
}

impl Seal {
    /// The sealing of a build's root storage: sealed for a module, and
    /// for a classic script, which is always an application's (owner answer
    /// Y5, 2026-09-29: its roots are the program's own unless declared
    /// global). `StructuralOnly` is for a script sharing its globals with
    /// other scripts, which no contract requests today.
    pub fn from_execution(execution: JavaScriptExecution) -> Self {
        match execution {
            JavaScriptExecution::Module | JavaScriptExecution::Script => Self::Module,
        }
    }
}

/// What one call runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// A body of this program, resolved from the callee value.
    Unit(UnitId),
    /// A declared host function; `pure` is its trusted declaration.
    Extern {
        cell: CellId,
        pure: bool,
    },
    Builtin(BuiltinCall),
    Intrinsic(ResolvedIntrinsic),
    /// A host member, a dynamic value, a parameter or storage that does not
    /// denote one body.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// The call operation runs the body.
    Call,
    /// An array intrinsic runs the body once per element.
    Callback,
}

/// A structural upper bound, not a measured hotness estimate. Unknown callers
/// and recursion never establish a single dynamic activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallFrequency {
    AtMostOnce,
    MayRepeat,
    Unknown,
}

/// One call of a program body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallEdge {
    pub caller: UnitId,
    pub operation: OpId,
    pub call: CallId,
    pub callee: UnitId,
    pub kind: EdgeKind,
}

/// Where a body's closure value escapes the calls the graph resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeAt {
    /// An operation of the using unit consumes the value.
    Operation(OpId),
    /// The value is the result of a region of the using unit.
    Region(RegionId),
}

/// One escape of a body, recorded in the unit where it happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Escape {
    pub body: UnitId,
    pub at: EscapeAt,
}

/// Structural facts about one cell, over every unit of the program.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellStorage {
    /// `Initialize` operations of the cell, in any unit.
    pub initializers: u32,
    /// Written by a `Store` (whole or through a field projection).
    pub stored: bool,
    /// Passed by reference to a call, which may write it.
    pub referenced: bool,
    /// Read, written or captured by a unit other than its owner.
    pub shared: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Unit(UnitId),
    Extern { pure: bool },
}

#[derive(Debug, Clone, Copy)]
enum Resolution {
    Pending,
    Visiting,
    Done(Option<Target>),
}

#[derive(Debug)]
pub struct CallGraph {
    deps: Deps,
    seal: Seal,
    storage: Vec<CellStorage>,
    /// (unit, operation) of each cell's first initialization.
    first_initializer: Vec<Option<(UnitId, OpId)>>,
    targets: Vec<Option<Target>>,
    /// Per unit, per call id.
    callees: Vec<Vec<Callee>>,
    /// Per unit, per call id: the `Call` operation.
    call_operations: Vec<Vec<Option<OpId>>>,
    outgoing: Vec<Vec<CallEdge>>,
    incoming: Vec<Vec<CallEdge>>,
    address_taken: Vec<bool>,
    /// Per using unit, the escapes of bodies its operations make.
    escapes: Vec<Vec<Escape>>,
    /// Per body: handed to the host through an export or an `import()`
    /// namespace.
    interface: Vec<bool>,
    components: Vec<Vec<UnitId>>,
    component: Vec<u32>,
    recursive: Vec<bool>,
}

struct Scan {
    storage: Vec<CellStorage>,
    /// (unit, operation) of each cell's first initialization.
    first_initializer: Vec<Option<(UnitId, OpId)>>,
    /// Per unit: uses of values defined by a closure or a cell load.
    denoting_uses: Vec<Vec<(ValueId, ValueUse)>>,
    call_operations: Vec<Vec<Option<OpId>>>,
}

impl CallGraph {
    pub(crate) fn frequency(
        &self,
        program: &Program<'_>,
        mut body: UnitId,
    ) -> (CallFrequency, u32) {
        let mut work = 0u32;
        for _ in 0..program.units.len() {
            work += 1;
            if work >= 65_536 {
                return (CallFrequency::Unknown, work);
            }
            let data = program.unit(body).expect("call graph unit");
            if data.kind == UnitKind::ModuleInitialization {
                return (CallFrequency::AtMostOnce, work);
            }
            if self.recursive(body) {
                return (CallFrequency::Unknown, work);
            }
            let Some(calls) = self.complete_callers(body) else {
                return (CallFrequency::Unknown, work);
            };
            let [call] = calls else {
                return (
                    if calls.is_empty() {
                        CallFrequency::AtMostOnce
                    } else {
                        CallFrequency::MayRepeat
                    },
                    work,
                );
            };
            let caller = program.unit(call.caller).expect("call graph caller");
            let mut region = Some(caller.operations[call.operation.index()].region);
            while let Some(id) = region {
                for operation in &caller.operations {
                    work += 1;
                    if work >= 65_536 {
                        return (CallFrequency::Unknown, work);
                    }
                    if matches!(
                        operation.kind,
                        OperationKind::Loop { .. }
                            | OperationKind::ForIn { .. }
                            | OperationKind::ForOf { .. }
                    ) && operation.kind.child_regions().any(|child| child == id)
                    {
                        return (CallFrequency::MayRepeat, work);
                    }
                }
                region = caller.regions[id.index()].parent;
            }
            body = call.caller;
        }
        (CallFrequency::Unknown, work)
    }

    pub fn build(program: &Program<'_>, seal: Seal) -> Self {
        Self::build_in(program, seal, &mut AllocationBudget::new(None))
            .expect("inspection call graph")
    }

    pub(super) fn build_in(
        program: &Program<'_>,
        seal: Seal,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.retained_phase(|budget| {
            let scan = Scan::run_in(program, budget)?;
            let mut resolution =
                budget.filled(Scratch, program.cells.len(), Resolution::Pending)?;
            let mut pending = budget.vector(Scratch, program.cells.len())?;
            for cell in 0..program.cells.len() {
                resolve_cell(
                    program,
                    seal,
                    &scan,
                    CellId::from_index(cell).unwrap(),
                    &mut resolution,
                    &mut pending,
                    budget,
                )?;
            }
            let targets = storage::collect(
                resolution.iter().map(|state| match state {
                    Resolution::Done(target) => *target,
                    _ => None,
                }),
                Retained,
                budget,
            )?;
            let units = program.units.len();
            let mut graph = Self {
                deps: Deps::of_program_in(program, budget)?,
                seal,
                storage: scan.storage,
                first_initializer: scan.first_initializer,
                targets,
                callees: budget.vector(Retained, units)?,
                call_operations: scan.call_operations,
                outgoing: storage::collect((0..units).map(|_| Vec::new()), Retained, budget)?,
                incoming: storage::collect((0..units).map(|_| Vec::new()), Retained, budget)?,
                address_taken: budget.filled(Retained, units, false)?,
                escapes: storage::collect((0..units).map(|_| Vec::new()), Retained, budget)?,
                interface: budget.filled(Retained, units, false)?,
                components: Vec::new(),
                component: budget.filled(Retained, units, 0)?,
                recursive: Vec::new(),
            };
            for (index, frozen) in program.units.iter().enumerate() {
                let unit = UnitId::from_index(index).unwrap();
                let data = frozen.data();
                let mut callees = budget.vector(Retained, data.calls.len())?;
                for (call_index, site) in data.calls.iter().enumerate() {
                    let call = CallId::from_index(call_index).unwrap();
                    let callee = graph.site_callee_in(program, data, site, budget)?;
                    if let Some(operation) = graph.call_operations[index][call_index] {
                        if let Callee::Unit(body) = callee {
                            graph.edge(unit, operation, call, body, EdgeKind::Call, budget)?;
                        }
                        if let Callee::Intrinsic(intrinsic) = callee {
                            if callback_intrinsic(intrinsic) {
                                if let Some(CallArgument::Value(value)) =
                                    data.arguments(site.arguments).and_then(|args| args.first())
                                {
                                    if let Some(Target::Unit(body)) =
                                        graph.denoted_in(program, data, *value, budget)?
                                    {
                                        graph.edge(
                                            unit,
                                            operation,
                                            call,
                                            body,
                                            EdgeKind::Callback,
                                            budget,
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                    budget.push(Retained, &mut callees, callee)?;
                }
                budget.push(Retained, &mut graph.callees, callees)?;
            }
            graph.mark_address_taken(program, &scan.denoting_uses, budget)?;
            // Scanning and alias resolution no longer overlap SCC scratch.
            for uses in scan.denoting_uses {
                storage::release(uses, Scratch, budget)?;
            }
            // The outer occurrence table remains scope-owned until return.
            storage::release(resolution, Scratch, budget)?;
            storage::release(pending, Scratch, budget)?;
            graph.strongly_connected(budget)?;
            Ok(graph)
        })
    }

    fn edge(
        &mut self,
        caller: UnitId,
        operation: OpId,
        call: CallId,
        callee: UnitId,
        kind: EdgeKind,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let edge = CallEdge {
            caller,
            operation,
            call,
            callee,
            kind,
        };
        budget.push(Retained, &mut self.outgoing[caller.index()], edge)?;
        budget.push(Retained, &mut self.incoming[callee.index()], edge)
    }

    fn site_callee_in(
        &self,
        program: &Program<'_>,
        data: &UnitData,
        site: &CallSite,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Callee, AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        Ok(match site.target {
            CallTarget::Value {
                callee,
                invocation: Invocation::Value,
            } => match self.denoted_in(program, data, callee, budget)? {
                Some(Target::Unit(body)) => Callee::Unit(body),
                Some(Target::Extern { pure }) => match self.foreign_cell(data, callee) {
                    Some(cell) => Callee::Extern { cell, pure },
                    None => Callee::Unknown,
                },
                None => Callee::Unknown,
            },
            CallTarget::Value { .. } | CallTarget::Reference { .. } => Callee::Unknown,
            CallTarget::Builtin(builtin) => Callee::Builtin(builtin),
            CallTarget::Intrinsic { operation, .. } => Callee::Intrinsic(operation),
        })
    }

    fn foreign_cell(&self, data: &UnitData, value: ValueId) -> Option<CellId> {
        let definition = &data.operations[data.values[value.index()].definition.index()];
        match definition.kind {
            OperationKind::Load(place) => match data.places[place.index()] {
                Place::Cell(cell) => Some(cell),
                _ => None,
            },
            _ => None,
        }
    }

    /// The body or host function a value denotes, if any.
    fn denoted(&self, program: &Program<'_>, data: &UnitData, value: ValueId) -> Option<Target> {
        denote(data, value, |cell| {
            self.targets.get(cell.index()).copied().flatten()
        })
        .filter(|target| match target {
            Target::Unit(body) => program.units.get(body.index()).is_some(),
            Target::Extern { .. } => true,
        })
    }

    fn denoted_in(
        &self,
        program: &Program<'_>,
        data: &UnitData,
        value: ValueId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<Target>, AllocationError> {
        let target = match denote_using(data, value, || budget.work(WorkKind::Analysis, 1))? {
            Some(Denotation::Target(target)) => Some(target),
            Some(Denotation::Cell(cell)) => self.targets.get(cell.index()).copied().flatten(),
            None => None,
        };
        Ok(target.filter(|target| match target {
            Target::Unit(body) => program.units.get(body.index()).is_some(),
            Target::Extern { .. } => true,
        }))
    }

    fn mark_address_taken(
        &mut self,
        program: &Program<'_>,
        uses: &[Vec<(ValueId, ValueUse)>],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        for (index, frozen) in program.units.iter().enumerate() {
            let data = frozen.data();
            for &(value, usage) in &uses[index] {
                let Some(Target::Unit(body)) = self.denoted_in(program, data, value, budget)?
                else {
                    continue;
                };
                let direct = match usage {
                    ValueUse::CallCallee { call, .. } => matches!(
                        data.calls[call.index()].target,
                        CallTarget::Value {
                            callee,
                            invocation: Invocation::Value,
                        } if callee == value
                    ),
                    // Initializing storage that itself denotes the body is an
                    // alias, not an escape: its loads are checked in turn.
                    ValueUse::Operand {
                        operation,
                        position: 0,
                    } => matches!(
                        data.operations[operation.index()].kind,
                        OperationKind::Initialize(cell)
                            if self.targets[cell.index()] == Some(Target::Unit(body))
                    ),
                    _ => false,
                };
                if !direct {
                    self.address_taken[body.index()] = true;
                    let at = match usage {
                        ValueUse::Operand { operation, .. }
                        | ValueUse::CallArgument { operation, .. }
                        | ValueUse::PlaceReceiver { operation, .. }
                        | ValueUse::PlaceKey { operation, .. } => EscapeAt::Operation(operation),
                        ValueUse::CallCallee { prepare, .. }
                        | ValueUse::CallReceiver { prepare, .. } => EscapeAt::Operation(prepare),
                        ValueUse::RegionResult(region) => EscapeAt::Region(region),
                    };
                    budget.push(Retained, &mut self.escapes[index], Escape { body, at })?;
                }
            }
        }
        // Exported storage and `import()` namespaces hand bodies to the host,
        // and so does a published class: its constructor and its prototype
        // methods, which JavaScript calls with any arguments.
        let exported = program.exports[program.public.clone()]
            .iter()
            .filter_map(|export| match export.target {
                InterfaceTarget::Value(cell) => Some(cell),
                InterfaceTarget::Type(_) => None,
            })
            .chain(
                program
                    .modules
                    .iter()
                    .flat_map(|module| module.namespace.iter().map(|(_, cell)| *cell)),
            )
            .chain(
                program
                    .classes
                    .iter()
                    .filter(|class| class.published)
                    .flat_map(|class| {
                        class
                            .value
                            .into_iter()
                            .chain(class.prototype.iter().map(|(_, cell)| *cell))
                    }),
            );
        for cell in exported {
            budget.work(WorkKind::Analysis, 1)?;
            if let Some(Target::Unit(body)) = self.targets.get(cell.index()).copied().flatten() {
                self.address_taken[body.index()] = true;
                self.interface[body.index()] = true;
            }
        }
        Ok(())
    }

    /// Tarjan's algorithm, iteratively; components come out callees first.
    fn strongly_connected(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let count = self.outgoing.len();
        const UNVISITED: u32 = u32::MAX;
        let mut index = budget.filled(Scratch, count, UNVISITED)?;
        let mut low = budget.filled(Scratch, count, 0u32)?;
        let mut on_stack = budget.filled(Scratch, count, false)?;
        let mut stack = budget.vector(Scratch, count)?;
        let mut next = 0u32;
        let mut self_edge = budget.filled(Scratch, count, false)?;
        let mut work: Vec<(usize, usize)> = budget.vector(Scratch, count)?;
        for edges in &self.outgoing {
            for edge in edges {
                if edge.caller == edge.callee {
                    self_edge[edge.caller.index()] = true;
                }
            }
        }
        for root in 0..count {
            if index[root] != UNVISITED {
                continue;
            }
            // (node, next outgoing edge to explore)
            work.clear();
            work.push((root, 0));
            index[root] = next;
            low[root] = next;
            next += 1;
            stack.push(root);
            on_stack[root] = true;
            while let Some(&(node, cursor)) = work.last() {
                budget.work(WorkKind::Analysis, 1)?;
                if let Some(edge) = self.outgoing[node].get(cursor) {
                    work.last_mut().unwrap().1 += 1;
                    let target = edge.callee.index();
                    if index[target] == UNVISITED {
                        index[target] = next;
                        low[target] = next;
                        next += 1;
                        stack.push(target);
                        on_stack[target] = true;
                        work.push((target, 0));
                    } else if on_stack[target] {
                        low[node] = low[node].min(index[target]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[node]);
                }
                if low[node] == index[node] {
                    let mut members = Vec::new();
                    loop {
                        let member = stack.pop().unwrap();
                        on_stack[member] = false;
                        budget.push(Retained, &mut members, UnitId::from_index(member).unwrap())?;
                        if member == node {
                            break;
                        }
                    }
                    members.sort_unstable();
                    let id = self.components.len() as u32;
                    let recursive = members.len() > 1 || self_edge[node];
                    for member in &members {
                        self.component[member.index()] = id;
                    }
                    budget.push(Retained, &mut self.components, members)?;
                    budget.push(Retained, &mut self.recursive, recursive)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn component_index(&self, unit: UnitId) -> usize {
        self.component[unit.index()] as usize
    }
    pub fn deps(&self) -> &Deps {
        &self.deps
    }
    pub fn seal(&self) -> Seal {
        self.seal
    }
    /// What call `call` of `unit` runs.
    pub fn callee(&self, unit: UnitId, call: CallId) -> Callee {
        self.callees
            .get(unit.index())
            .and_then(|calls| calls.get(call.index()))
            .copied()
            .unwrap_or(Callee::Unknown)
    }
    /// The body a callee value of `data` runs, without the value's unit id:
    /// the per-operation effect transfer resolves calls through this.
    pub fn callee_of_value(
        &self,
        program: &Program<'_>,
        data: &UnitData,
        value: ValueId,
    ) -> Callee {
        match self.denoted(program, data, value) {
            Some(Target::Unit(body)) => Callee::Unit(body),
            Some(Target::Extern { pure }) => match self.foreign_cell(data, value) {
                Some(cell) => Callee::Extern { cell, pure },
                None => Callee::Unknown,
            },
            None => Callee::Unknown,
        }
    }
    /// Structural facts about a cell's storage.
    pub fn storage(&self, cell: CellId) -> CellStorage {
        self.storage
            .get(cell.index())
            .copied()
            .unwrap_or(CellStorage {
                initializers: 0,
                stored: true,
                referenced: true,
                shared: true,
            })
    }
    /// The body a cell holds whenever it is read, if it denotes one.
    pub fn cell_body(&self, cell: CellId) -> Option<UnitId> {
        match self.targets.get(cell.index()).copied().flatten() {
            Some(Target::Unit(body)) => Some(body),
            _ => None,
        }
    }
    /// Calls and callbacks this unit makes to program bodies.
    pub fn calls_from(&self, unit: UnitId) -> &[CallEdge] {
        self.outgoing.get(unit.index()).map_or(&[], Vec::as_slice)
    }

    /// Inputs the effect transfer reads outside the unit being summarized.
    /// Interface/escape changes feed initialization separately. Compare exact
    /// storage and resolution tables, never a collision-prone digest.
    pub(super) fn same_effect_storage(&self, other: &Self) -> bool {
        self.seal == other.seal && self.storage == other.storage && self.targets == other.targets
    }

    pub(super) fn same_effect_calls(&self, other: &Self, unit: UnitId) -> bool {
        self.callees.get(unit.index()) == other.callees.get(unit.index())
            && self.outgoing.get(unit.index()) == other.outgoing.get(unit.index())
            && self.recursive(unit) == other.recursive(unit)
    }

    pub(super) fn component_of(&self, unit: UnitId) -> &[UnitId] {
        self.component
            .get(unit.index())
            .and_then(|&index| self.components.get(index as usize))
            .map_or(&[], Vec::as_slice)
    }

    pub(super) fn same_component(&self, left: UnitId, right: UnitId) -> bool {
        self.component.get(left.index()) == self.component.get(right.index())
    }

    pub(super) fn callers_of(&self, unit: UnitId) -> &[CallEdge] {
        self.incoming.get(unit.index()).map_or(&[], Vec::as_slice)
    }
    /// Whether the body escapes as a value: handed to the host, stored,
    /// passed as an argument or returned. Its calls are then not all known.
    pub fn address_taken(&self, unit: UnitId) -> bool {
        self.address_taken
            .get(unit.index())
            .copied()
            .unwrap_or(true)
    }
    /// The escapes the operations of `unit` make: where a body's value
    /// leaves the calls this graph resolves.
    pub fn escapes_in(&self, unit: UnitId) -> &[Escape] {
        self.escapes.get(unit.index()).map_or(&[], Vec::as_slice)
    }
    /// Whether the body is handed to the host through the program's
    /// interface (an export or an `import()` namespace), with no operation.
    pub fn escapes_through_interface(&self, unit: UnitId) -> bool {
        self.interface.get(unit.index()).copied().unwrap_or(true)
    }
    /// The one `Initialize` of a cell initialized exactly once, anywhere.
    pub fn initializer(&self, cell: CellId) -> Option<(UnitId, OpId)> {
        (self.storage(cell).initializers == 1)
            .then(|| self.first_initializer.get(cell.index()).copied().flatten())
            .flatten()
    }
    /// Every call that can run this body, when that set is complete: the
    /// body does not escape, and every call of it is a direct call.
    pub fn complete_callers(&self, unit: UnitId) -> Option<&[CallEdge]> {
        if self.address_taken(unit) {
            return None;
        }
        let incoming = self.incoming.get(unit.index())?;
        incoming
            .iter()
            .all(|edge| edge.kind == EdgeKind::Call)
            .then_some(incoming.as_slice())
    }
    /// Strongly connected components, callees before callers.
    pub fn components(&self) -> &[Vec<UnitId>] {
        &self.components
    }
    /// Whether the body can call itself, directly or through its component.
    pub fn recursive(&self, unit: UnitId) -> bool {
        self.component
            .get(unit.index())
            .and_then(|component| self.recursive.get(*component as usize))
            .copied()
            .unwrap_or(true)
    }
    /// The call operation of a call id, when the unit schedules it.
    pub fn call_operation(&self, unit: UnitId, call: CallId) -> Option<OpId> {
        self.call_operations
            .get(unit.index())
            .and_then(|calls| calls.get(call.index()))
            .copied()
            .flatten()
    }
}

#[derive(Clone, Copy)]
enum Denotation {
    Target(Target),
    Cell(CellId),
}

/// Shared, allocation-free value-chain walk. Queries use the same producer;
/// construction supplies its existing work owner.
fn denote_using<E>(
    data: &UnitData,
    mut value: ValueId,
    mut work: impl FnMut() -> Result<(), E>,
) -> Result<Option<Denotation>, E> {
    for _ in 0..=data.values.len() {
        work()?;
        let Some(value_data) = data.values.get(value.index()) else {
            return Ok(None);
        };
        let definition = &data.operations[value_data.definition.index()];
        match definition.kind {
            OperationKind::Closure(body) => {
                return Ok(Some(Denotation::Target(Target::Unit(body))))
            }
            OperationKind::Load(place) => match data.places[place.index()] {
                Place::Value(input) => value = input,
                Place::Cell(cell) => return Ok(Some(Denotation::Cell(cell))),
                _ => return Ok(None),
            },
            OperationKind::CopyValue => {
                let Some(input) = data.operands(definition.operands).and_then(|v| v.first()) else {
                    return Ok(None);
                };
                value = *input;
            }
            _ => return Ok(None),
        }
    }
    Ok(None)
}
fn denote(
    data: &UnitData,
    value: ValueId,
    mut cell: impl FnMut(CellId) -> Option<Target>,
) -> Option<Target> {
    match denote_using(data, value, || Ok::<(), std::convert::Infallible>(())).unwrap()? {
        Denotation::Target(target) => Some(target),
        Denotation::Cell(storage) => cell(storage),
    }
}

fn resolve_cell(
    program: &Program<'_>,
    seal: Seal,
    scan: &Scan,
    mut cell: CellId,
    resolution: &mut [Resolution],
    pending: &mut Vec<CellId>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    debug_assert!(pending.is_empty());
    let target = loop {
        budget.work(WorkKind::Analysis, 1)?;
        match resolution[cell.index()] {
            Resolution::Done(target) => break target,
            Resolution::Visiting => break None,
            Resolution::Pending => {}
        }
        resolution[cell.index()] = Resolution::Visiting;
        // Each cell enters at most once, within the pre-admitted capacity.
        pending.push(cell);
        let storage = &program.cells[cell.index()];
        let facts = scan.storage[cell.index()];
        if storage.binding == CellBinding::Foreign {
            break matches!(
                program.types[storage.ty.index()],
                Type::Function(_) | Type::GenericFunction(_)
            )
            .then_some(Target::Extern {
                pure: storage.declared_pure,
            });
        }
        if matches!(storage.binding, CellBinding::Parameter(_))
            || facts.initializers != 1
            || facts.stored
            || facts.referenced
        {
            break None;
        }
        let Some(owner) = program.unit(storage.owner) else {
            break None;
        };
        if owner.kind == UnitKind::ModuleInitialization && seal != Seal::Module {
            break None;
        }
        let Some((unit, operation)) = scan.first_initializer[cell.index()] else {
            break None;
        };
        let Some(data) = program.unit(unit) else {
            break None;
        };
        let Some(&[value]) = data.operands(data.operations[operation.index()].operands) else {
            break None;
        };
        match denote_using(data, value, || budget.work(WorkKind::Analysis, 1))? {
            Some(Denotation::Target(target)) => break Some(target),
            Some(Denotation::Cell(other)) => cell = other,
            None => break None,
        }
    };
    for cell in pending.drain(..) {
        resolution[cell.index()] = Resolution::Done(target);
    }
    Ok(())
}

impl Scan {
    fn run_in(
        program: &Program<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let cells = program.cells.len();
        let mut scan = Self {
            storage: budget.filled(Retained, cells, CellStorage::default())?,
            first_initializer: budget.filled(Retained, cells, None)?,
            denoting_uses: budget.vector(Scratch, program.units.len())?,
            call_operations: budget.vector(Retained, program.units.len())?,
        };
        for (index, frozen) in program.units.iter().enumerate() {
            let unit = UnitId::from_index(index).unwrap();
            let data = frozen.data();
            let mut denoting = Vec::new();
            let mut calls = budget.filled(Retained, data.calls.len(), None)?;
            let storage = &mut scan.storage;
            let first = &mut scan.first_initializer;
            let mut allocation_error = None;
            let walked = uses::walk(data, |event| {
                let visit = (|| -> Result<(), AllocationError> {
                    budget.work(WorkKind::Analysis, 1)?;
                    match event {
                        Event::Cell(cell, usage) => {
                            let Some(facts) = storage.get_mut(cell.index()) else {
                                return Ok(());
                            };
                            if program.cells[cell.index()].owner != unit
                                && !matches!(usage, CellUse::Initialize(_) | CellUse::Declare(_))
                            {
                                facts.shared = true;
                            }
                            match usage {
                                CellUse::Initialize(operation) => {
                                    facts.initializers = facts.initializers.saturating_add(1);
                                    first[cell.index()].get_or_insert((unit, operation));
                                }
                                // `let x;`: its value comes from its stores (R3).
                                CellUse::Write { .. } | CellUse::Declare(_) => facts.stored = true,
                                CellUse::Reference { .. } => facts.referenced = true,
                                // A loop or catch binding is written by its
                                // construct on every entry.
                                CellUse::CatchBinding { .. } => facts.stored = true,
                                CellUse::Read { .. } | CellUse::Parameter(_) | CellUse::Capture => {
                                }
                            }
                        }
                        Event::Value(value, usage) => {
                            let definition = data
                                .values
                                .get(value.index())
                                .and_then(|value| data.operations.get(value.definition.index()));
                            if definition.is_some_and(|definition| {
                                matches!(
                                    definition.kind,
                                    OperationKind::Closure(_)
                                        | OperationKind::Load(_)
                                        | OperationKind::CopyValue
                                )
                            }) {
                                budget.push(Scratch, &mut denoting, (value, usage))?;
                            }
                        }
                        Event::Call(call, operation) => {
                            if let Some(slot) = calls.get_mut(call.index()) {
                                *slot = Some(operation);
                            }
                        }
                        Event::Closure(..) | Event::Tick => {}
                    }
                    Ok(())
                })();
                visit.map_err(|error| {
                    allocation_error = Some(error);
                    uses::UseError::Capacity
                })
            });
            if let Some(error) = allocation_error {
                return Err(error);
            }
            if walked.is_err() {
                // A unit the use walk rejects is unverified input: nothing
                // it holds resolves, and every body it could reach escapes.
                denoting.clear();
                calls.iter_mut().for_each(|slot| *slot = None);
            }
            budget.push(Scratch, &mut scan.denoting_uses, denoting)?;
            budget.push(Retained, &mut scan.call_operations, calls)?;
        }
        Ok(scan)
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    #[test]
    fn q2_iterative_alias_resolution_handles_long_pending_chains_and_cycles() {
        let mut source = String::from("int target(){return 7;}");
        for index in 0..512 {
            let next = if index == 0 {
                "target".to_string()
            } else {
                format!("a{}", index - 1)
            };
            source.push_str(&format!("func()->int a{index}={next};"));
        }
        source.push_str("int run(){return a511();}print(run());");
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, &source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let cell = |name| {
            CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == name)
                    .unwrap(),
            )
            .unwrap()
        };
        let target = match program.cells[cell("target").index()].binding {
            CellBinding::Function(unit) => Target::Unit(unit),
            _ => unreachable!(),
        };
        let mut budget = AllocationBudget::new(None);
        let mut scan = Scan::run_in(&program, &mut budget).unwrap();
        let mut resolution = vec![Resolution::Pending; program.cells.len()];
        let mut pending = Vec::with_capacity(program.cells.len());
        // Start at the chain's last alias, before any earlier cell has resolved.
        resolve_cell(
            &program,
            Seal::Module,
            &scan,
            cell("a511"),
            &mut resolution,
            &mut pending,
            &mut budget,
        )
        .unwrap();
        assert!(
            matches!(resolution[cell("a511").index()], Resolution::Done(Some(answer)) if answer == target)
        );
        assert!(pending.is_empty());
        // A synthetic scan cycle exercises fail-closed handling independently
        // of the frontend's prohibition on reading an undeclared local.
        scan.first_initializer[cell("a0").index()] = scan.first_initializer[cell("a2").index()];
        resolution.fill(Resolution::Pending);
        resolve_cell(
            &program,
            Seal::Module,
            &scan,
            cell("a511"),
            &mut resolution,
            &mut pending,
            &mut budget,
        )
        .unwrap();
        assert!(matches!(
            resolution[cell("a511").index()],
            Resolution::Done(None)
        ));
        assert!(pending.is_empty());
    }
}
