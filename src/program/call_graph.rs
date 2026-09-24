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
//! (module-level) storage is sealed only in module execution: a classic
//! script's globals are writable by other scripts, so under
//! `Seal::StructuralOnly` a load of root storage denotes nothing. A load that
//! runs before the one initialization throws before any call happens, so the
//! call can only ever run the denoted body. This is the rule
//! `callable_inputs` applies to one producer at a time, stated once for the
//! whole program.
use super::uses::{self, CellUse, Event, ValueUse};
use super::views::Deps;
use super::*;
use crate::check::BuiltinCall;
use crate::compilation_contract::JavaScriptExecution;
use crate::primitive::{Intrinsic, ResolvedIntrinsic};

/// Whether root storage is sealed against the host (see the module comment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Seal {
    StructuralOnly = 0,
    Module = 1,
}

impl Seal {
    pub fn from_execution(execution: JavaScriptExecution) -> Self {
        if execution == JavaScriptExecution::Module {
            Self::Module
        } else {
            Self::StructuralOnly
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

/// Intrinsics that call their first argument once per element and return.
pub(super) fn callback_intrinsic(operation: ResolvedIntrinsic) -> bool {
    matches!(
        operation,
        ResolvedIntrinsic::Method(
            Intrinsic::ArrayMap
                | Intrinsic::ArrayFilter
                | Intrinsic::ArrayReduce
                | Intrinsic::ArrayForEach
                | Intrinsic::ArraySome
                | Intrinsic::ArrayEvery
                | Intrinsic::ArrayFindIndex
        )
    )
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
    pub fn build(program: &Program<'_>, seal: Seal) -> Self {
        let scan = Scan::run(program);
        let mut resolution = vec![Resolution::Pending; program.cells.len()];
        for cell in 0..program.cells.len() {
            resolve_cell(
                program,
                seal,
                &scan,
                CellId::from_index(cell).unwrap(),
                &mut resolution,
            );
        }
        let targets: Vec<Option<Target>> = resolution
            .iter()
            .map(|state| match state {
                Resolution::Done(target) => *target,
                _ => None,
            })
            .collect();
        let units = program.units.len();
        let mut graph = Self {
            deps: Deps::of_program(program),
            seal,
            storage: scan.storage,
            first_initializer: scan.first_initializer,
            targets,
            callees: Vec::with_capacity(units),
            call_operations: scan.call_operations,
            outgoing: vec![Vec::new(); units],
            incoming: vec![Vec::new(); units],
            address_taken: vec![false; units],
            escapes: vec![Vec::new(); units],
            interface: vec![false; units],
            components: Vec::new(),
            component: vec![0; units],
            recursive: Vec::new(),
        };
        for (index, frozen) in program.units.iter().enumerate() {
            let unit = UnitId::from_index(index).unwrap();
            let data = frozen.data();
            let mut callees = Vec::with_capacity(data.calls.len());
            for (call_index, site) in data.calls.iter().enumerate() {
                let call = CallId::from_index(call_index).unwrap();
                let callee = graph.site_callee(program, data, site);
                if let Some(operation) = graph.call_operations[index][call_index] {
                    if let Callee::Unit(body) = callee {
                        graph.edge(unit, operation, call, body, EdgeKind::Call);
                    }
                    if let Callee::Intrinsic(intrinsic) = callee {
                        if callback_intrinsic(intrinsic) {
                            if let Some(body) = data
                                .arguments(site.arguments)
                                .and_then(|arguments| arguments.first())
                                .and_then(|argument| match *argument {
                                    CallArgument::Value(value) => {
                                        graph.denoted(program, data, value)
                                    }
                                    CallArgument::Reference(_) => None,
                                })
                                .and_then(|target| match target {
                                    Target::Unit(body) => Some(body),
                                    Target::Extern { .. } => None,
                                })
                            {
                                graph.edge(unit, operation, call, body, EdgeKind::Callback);
                            }
                        }
                    }
                }
                callees.push(callee);
            }
            graph.callees.push(callees);
        }
        graph.mark_address_taken(program, &scan.denoting_uses);
        graph.strongly_connected();
        graph
    }

    fn edge(
        &mut self,
        caller: UnitId,
        operation: OpId,
        call: CallId,
        callee: UnitId,
        kind: EdgeKind,
    ) {
        let edge = CallEdge {
            caller,
            operation,
            call,
            callee,
            kind,
        };
        self.outgoing[caller.index()].push(edge);
        self.incoming[callee.index()].push(edge);
    }

    fn site_callee(&self, program: &Program<'_>, data: &UnitData, site: &CallSite) -> Callee {
        match site.target {
            CallTarget::Value {
                callee,
                invocation: Invocation::Value,
            } => match self.denoted(program, data, callee) {
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
        }
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

    fn mark_address_taken(&mut self, program: &Program<'_>, uses: &[Vec<(ValueId, ValueUse)>]) {
        for (index, frozen) in program.units.iter().enumerate() {
            let data = frozen.data();
            for &(value, usage) in &uses[index] {
                let Some(Target::Unit(body)) = self.denoted(program, data, value) else {
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
                    self.escapes[index].push(Escape { body, at });
                }
            }
        }
        // Exported storage and `import()` namespaces hand bodies to the host.
        let exported = program
            .modules
            .iter()
            .flat_map(|module| {
                program.exports[module.exports.clone()]
                    .iter()
                    .filter_map(|export| match export.target {
                        InterfaceTarget::Value(cell) => Some(cell),
                        InterfaceTarget::Struct(_) => None,
                    })
                    .chain(module.namespace.iter().map(|(_, cell)| *cell))
            })
            .collect::<Vec<_>>();
        for cell in exported {
            if let Some(Target::Unit(body)) = self.targets.get(cell.index()).copied().flatten() {
                self.address_taken[body.index()] = true;
                self.interface[body.index()] = true;
            }
        }
    }

    /// Tarjan's algorithm, iteratively; components come out callees first.
    fn strongly_connected(&mut self) {
        let count = self.outgoing.len();
        const UNVISITED: u32 = u32::MAX;
        let mut index = vec![UNVISITED; count];
        let mut low = vec![0u32; count];
        let mut on_stack = vec![false; count];
        let mut stack = Vec::new();
        let mut next = 0u32;
        let mut self_edge = vec![false; count];
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
            let mut work: Vec<(usize, usize)> = vec![(root, 0)];
            index[root] = next;
            low[root] = next;
            next += 1;
            stack.push(root);
            on_stack[root] = true;
            while let Some(&(node, cursor)) = work.last() {
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
                        members.push(UnitId::from_index(member).unwrap());
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
                    self.components.push(members);
                    self.recursive.push(recursive);
                }
            }
        }
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

/// The target a value denotes, given how cells resolve.
fn denote(
    data: &UnitData,
    value: ValueId,
    mut cell: impl FnMut(CellId) -> Option<Target>,
) -> Option<Target> {
    let mut value = value;
    // CopyValue chains are acyclic (a value is defined before its uses);
    // the bound keeps a malformed unit from looping.
    for _ in 0..=data.values.len() {
        let definition = &data.operations[data.values.get(value.index())?.definition.index()];
        match definition.kind {
            OperationKind::Closure(body) => return Some(Target::Unit(body)),
            OperationKind::Load(place) => {
                return match data.places[place.index()] {
                    Place::Cell(storage) => cell(storage),
                    _ => None,
                }
            }
            OperationKind::CopyValue => {
                value = *data.operands(definition.operands)?.first()?;
            }
            _ => return None,
        }
    }
    None
}

fn resolve_cell(
    program: &Program<'_>,
    seal: Seal,
    scan: &Scan,
    cell: CellId,
    resolution: &mut Vec<Resolution>,
) -> Option<Target> {
    match resolution[cell.index()] {
        Resolution::Done(target) => return target,
        // A cycle of aliases never settles on one body.
        Resolution::Visiting => return None,
        Resolution::Pending => {}
    }
    resolution[cell.index()] = Resolution::Visiting;
    let target = (|| {
        let storage = &program.cells[cell.index()];
        let facts = scan.storage[cell.index()];
        if storage.binding == CellBinding::Foreign {
            return matches!(
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
            return None;
        }
        let owner = program.unit(storage.owner)?;
        if owner.kind == UnitKind::ModuleInitialization && seal != Seal::Module {
            return None;
        }
        let (unit, operation) = scan.first_initializer[cell.index()]?;
        let data = program.unit(unit)?;
        let initializer = &data.operations[operation.index()];
        let &[value] = data.operands(initializer.operands)? else {
            return None;
        };
        denote(data, value, |other| {
            resolve_cell(program, seal, scan, other, resolution)
        })
    })();
    resolution[cell.index()] = Resolution::Done(target);
    target
}

impl Scan {
    fn run(program: &Program<'_>) -> Self {
        let cells = program.cells.len();
        let mut scan = Self {
            storage: vec![CellStorage::default(); cells],
            first_initializer: vec![None; cells],
            denoting_uses: Vec::with_capacity(program.units.len()),
            call_operations: Vec::with_capacity(program.units.len()),
        };
        for (index, frozen) in program.units.iter().enumerate() {
            let unit = UnitId::from_index(index).unwrap();
            let data = frozen.data();
            let mut denoting = Vec::new();
            let mut calls = vec![None; data.calls.len()];
            let storage = &mut scan.storage;
            let first = &mut scan.first_initializer;
            let walked = uses::walk(data, |event| {
                match event {
                    Event::Cell(cell, usage) => {
                        let Some(facts) = storage.get_mut(cell.index()) else {
                            return Ok(());
                        };
                        if program.cells[cell.index()].owner != unit
                            && !matches!(usage, CellUse::Initialize(_))
                        {
                            facts.shared = true;
                        }
                        match usage {
                            CellUse::Initialize(operation) => {
                                facts.initializers = facts.initializers.saturating_add(1);
                                first[cell.index()].get_or_insert((unit, operation));
                            }
                            CellUse::Write { .. } => facts.stored = true,
                            CellUse::Reference { .. } => facts.referenced = true,
                            // A loop or catch binding is written by its
                            // construct on every entry.
                            CellUse::CatchBinding { .. } => facts.stored = true,
                            CellUse::Read { .. } | CellUse::Parameter(_) | CellUse::Capture => {}
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
                            denoting.push((value, usage));
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
            });
            if walked.is_err() {
                // A unit the use walk rejects is unverified input: nothing
                // it holds resolves, and every body it could reach escapes.
                denoting.clear();
                calls.iter_mut().for_each(|slot| *slot = None);
            }
            scan.denoting_uses.push(denoting);
            scan.call_operations.push(calls);
        }
        scan
    }
}
