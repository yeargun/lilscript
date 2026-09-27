//! The one initialization owner (architecture §6 "initialization:
//! structured dominance plus module order", §7 "Initialization order"; plan
//! M6.5, seeded by the checker's proofs of M4.3).
//!
//! It answers three questions:
//! * **per root cell, the root statement that settles it**: the
//!   `Initialize` after which the cell is out of its temporal dead zone;
//! * **per unit, the first root statement during which it may run** ("not
//!   invoked before root statement S");
//! * **per access, whether the storage is initialized there**, the one
//!   answer effects, liveness and the unit-local facts read.
//!
//! **The root schedule.** Module initializers run in `Program::initialization`
//! order (the module contract owns it). Every module's instantiation prefix
//! runs first, before any module evaluates (`UnitData::instantiation_prefix`):
//! named functions exist from the start. Then each initializer's
//! entry-region operations run in order; each is a *root statement*, and all
//! of them are numbered in one global order. When the program imports host
//! modules, a point before the first root statement stands for their
//! evaluation: a host module the host resolves may import this program back
//! and call what it exports before any of it has run.
//!
//! **When a body may run.** A root statement runs program code only through
//! the calls it makes or the values it creates. So a body runs
//! * when a call the call graph resolves runs it: a root statement's call, or
//!   a call made by a body that runs (array callbacks included);
//! * when code the program does not see runs after the body's value escaped
//!   (the call graph's escape sites; for the host-visible interface, from the
//!   start in a script or beside host modules, at the end otherwise). That
//!   code runs at a *hazard*: a root statement that may run host code, a
//!   conversion hook or an unresolved call, or that may throw. A throw ends
//!   initialization, and host code holding an escaped body may call it later
//!   with every cell settled after the throw still in its dead zone.
//!
//! The first point of each body is a least fixed point over the call graph,
//! computed like a shortest path: every point propagated from a point is at
//! least that point.
//!
//! **Per access.** A parameter is initialized on entry; a named function
//! from its module's instantiation. A local is initialized at an access when
//! the checker proved every occurrence past its initialization (the seed), or
//! when the access runs after the cell's `Initialize` in its owner: by
//! structured dominance, or through the chain of closure creations that
//! leads from the access to the owner. A root cell is also initialized in
//! any code that cannot run before the statement settling it.
//!
//! **Two tiers.** `structural` needs no effects: every body may run from the
//! first point on. The effects owner summarizes with it, `schedule` reads the
//! root statements' effects from those summaries and computes each body's
//! first point, and the effects owner summarizes again with the complete
//! answer (`ProgramEffects::build`). Both rounds are sound; the second only
//! removes temporal-dead-zone throws.
//!
//! Prior art: esbuild propagates constants only through the chain of `const`
//! declarations that starts a statement list, "which doesn't have any TDZ
//! considerations because no other statements come before it"
//! (esbuild@f6058f8 internal/js_ast/js_ast.go:1316-1319,
//! internal/js_parser/js_parser.go:10790-10808); the call graph lets code
//! before a constant run without losing it. Closure's `InferConsts` and
//! `InlineVariables` treat a constant as defined everywhere ("Constants are
//! allowed to be defined after their first use",
//! closure-compiler@0da58e1e ReferenceCollection.java:171-180,
//! InlineVariables.java:459-468): a read before initialization would stop
//! throwing, which D3.7 forbids. Their `isWellDefined` (a basic block that
//! provably runs before every reference, ReferenceCollection.java:54-76) is
//! the structured dominance used here within one unit.
use super::activation::StructuredDominance;
use super::call_graph::{CallGraph, EscapeAt, Seal};
use super::effects::Effects;
use super::views::Deps;
use super::*;
use ahash::AHashMap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A point of the root schedule, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootPoint(u32);

impl RootPoint {
    /// Every module's instantiation prefix: named functions exist, no code
    /// has run.
    pub const INSTANTIATION: Self = Self(0);
    /// The first point code can run at.
    pub const FIRST: Self = Self(1);
    /// After the last root statement: initialization has completed.
    pub const END: Self = Self(u32::MAX);

    pub fn ordinal(self) -> u32 {
        self.0
    }
}

/// What happens at a point of the schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    Instantiation,
    /// Host modules evaluate, before any root statement.
    HostModules,
    /// An entry-region operation of a module initializer.
    Statement {
        unit: UnitId,
        operation: OpId,
    },
}

/// Unit-local definite initialization: which operations of a unit run after
/// the initialization of a cell the unit owns. It needs neither contract nor
/// other units, so unit-local facts read it too.
#[derive(Debug)]
pub(super) struct UnitInitialization {
    dominance: StructuredDominance,
    /// The `Initialize` operations of each cell.
    initializers: AHashMap<CellId, Vec<OpId>>,
    /// A catch binding, for-in key or for-of item is initialized throughout
    /// the region its construct enters.
    bound: AHashMap<CellId, RegionId>,
}

impl UnitInitialization {
    pub(super) fn build(data: &UnitData) -> Self {
        let dominance = StructuredDominance::build(
            data,
            vec![None; data.regions.len()],
            vec![0; data.operations.len()],
            |_| Ok::<(), ()>(()),
        )
        .expect("an infallible work counter");
        let mut initializers: AHashMap<CellId, Vec<OpId>> = AHashMap::default();
        let mut bound = AHashMap::default();
        for (index, operation) in data.operations.iter().enumerate() {
            match operation.kind {
                OperationKind::Initialize(cell) => initializers
                    .entry(cell)
                    .or_default()
                    .push(OpId::from_index(index).unwrap()),
                OperationKind::Try {
                    catch: Some((Some(cell), region)),
                    ..
                }
                | OperationKind::ForIn {
                    key: cell,
                    body: region,
                }
                | OperationKind::ForOf {
                    item: cell,
                    body: region,
                } => {
                    bound.insert(cell, region);
                }
                _ => {}
            }
        }
        Self {
            dominance,
            initializers,
            bound,
        }
    }

    /// Whether `operation` of this unit runs only after `cell`, which the
    /// unit owns, is initialized: an `Initialize` of it dominates the
    /// operation, or the operation lies in the region its binding construct
    /// enters.
    pub(super) fn after(&self, data: &UnitData, cell: CellId, operation: OpId) -> bool {
        if let Some(sites) = self.initializers.get(&cell) {
            if sites.iter().any(|&initialize| {
                self.dominance
                    .after(data, initialize, operation, |_| Ok::<(), ()>(()))
                    .unwrap_or(false)
            }) {
                return true;
            }
        }
        match (
            self.bound.get(&cell),
            data.operations.get(operation.index()),
        ) {
            (Some(&region), Some(entry)) => self.within(data, entry.region, region),
            _ => false,
        }
    }

    /// Whether `region` is `ancestor` or nested inside it.
    fn within(&self, data: &UnitData, mut region: RegionId, ancestor: RegionId) -> bool {
        for _ in 0..=data.regions.len() {
            if region == ancestor {
                return true;
            }
            let Some(owner) = self.dominance.parent(region) else {
                return false;
            };
            region = data.operations[owner.index()].region;
        }
        false
    }

    /// The entry-region operation `operation` is nested in (itself when it
    /// is one).
    fn statement(&self, data: &UnitData, operation: OpId) -> Option<OpId> {
        let mut current = operation;
        for _ in 0..=data.regions.len() {
            let region = data.operations.get(current.index())?.region;
            if region == data.entry {
                return Some(current);
            }
            current = self.dominance.parent(region)?;
        }
        None
    }
}

/// Whether `unit`'s own access of `cell` at `operation` is past the cell's
/// initialization, from what the unit alone and the checker prove: the
/// unit-local answer (`facts.rs` reads it).
pub(super) fn local_access_initialized(
    program: &Program<'_>,
    unit: UnitId,
    data: &UnitData,
    local: &UnitInitialization,
    operation: OpId,
    cell: CellId,
) -> bool {
    let Some(storage) = program.cells.get(cell.index()) else {
        return false;
    };
    match storage.binding {
        CellBinding::Local => {
            checker_proved(storage) || (storage.owner == unit && local.after(data, cell, operation))
        }
        CellBinding::Parameter(_) => !program.is_reference_parameter(cell),
        CellBinding::Function(_) => program
            .unit(storage.owner)
            .is_some_and(|owner| owner.module == data.module),
        CellBinding::Foreign => false,
    }
}

/// The checker's seed (M4.3): no occurrence of the binding can run before
/// its initialization. Conversion-owned cells carry no checked proof.
fn checker_proved(storage: &Cell) -> bool {
    !storage.synthetic && !storage.observable_before_initialization
}

/// The initialization facts of one program under one sealing (see the module
/// comment).
#[derive(Debug)]
pub struct ProgramInitialization {
    deps: Deps,
    moments: Vec<Moment>,
    /// Per unit, for a statically ordered module initializer: the point of
    /// each operation's root statement.
    statements: Vec<Option<Vec<RootPoint>>>,
    /// Per cell: the point after which it is initialized in every run.
    settled: Vec<Option<RootPoint>>,
    locals: Vec<UnitInitialization>,
    /// Per unit: the one operation creating its closure, when there is one.
    creation: Vec<Option<(UnitId, OpId)>>,
    /// Per unit: the first point it may run at.
    first_run: Vec<RootPoint>,
    /// Per point: code the program does not see may run there.
    hazards: Vec<bool>,
}

impl ProgramInitialization {
    /// The answer that needs no effects: every body may run from the first
    /// point on.
    pub(super) fn structural(program: &Program<'_>, graph: &CallGraph) -> Self {
        let count = program.units.len();
        let locals: Vec<UnitInitialization> = program
            .units
            .iter()
            .map(|unit| UnitInitialization::build(unit.data()))
            .collect();
        let mut moments = vec![Moment::Instantiation];
        if program
            .modules
            .iter()
            .any(|module| !module.foreign_imports.is_empty())
        {
            moments.push(Moment::HostModules);
        }
        let mut statements: Vec<Option<Vec<RootPoint>>> = vec![None; count];
        for &initializer in program.initialization.iter() {
            let Some(data) = program.unit(initializer) else {
                continue;
            };
            if statements[initializer.index()].is_some() {
                continue;
            }
            let local = &locals[initializer.index()];
            let mut entry = vec![None; data.operations.len()];
            let prefix = data.instantiation_prefix as usize;
            for (position, &operation) in data.regions[data.entry.index()]
                .operations
                .iter()
                .enumerate()
            {
                entry[operation.index()] = Some(if position < prefix {
                    RootPoint::INSTANTIATION
                } else {
                    let point = RootPoint(moments.len() as u32);
                    moments.push(Moment::Statement {
                        unit: initializer,
                        operation,
                    });
                    point
                });
            }
            let points = (0..data.operations.len())
                .map(|index| {
                    local
                        .statement(data, OpId::from_index(index).unwrap())
                        .and_then(|statement| entry[statement.index()])
                        // Unverified structure: the earliest point a
                        // statement can hold, which proves nothing.
                        .unwrap_or(RootPoint::FIRST)
                })
                .collect();
            statements[initializer.index()] = Some(points);
        }
        let settled = program
            .cells
            .iter()
            .enumerate()
            .map(|(index, storage)| {
                let cell = CellId::from_index(index).unwrap();
                let points = statements.get(storage.owner.index())?.as_ref()?;
                match storage.binding {
                    CellBinding::Function(_) => Some(RootPoint::INSTANTIATION),
                    CellBinding::Local => {
                        let (unit, operation) = graph.initializer(cell)?;
                        let data = program.unit(unit)?;
                        (unit == storage.owner
                            && data.operations.get(operation.index())?.region == data.entry)
                            .then(|| points[operation.index()])
                            .filter(|&point| point != RootPoint::INSTANTIATION)
                    }
                    CellBinding::Parameter(_) | CellBinding::Foreign => None,
                }
            })
            .collect();
        let mut creation: Vec<Option<(UnitId, OpId)>> = vec![None; count];
        let mut created = vec![0u8; count];
        for (index, frozen) in program.units.iter().enumerate() {
            let unit = UnitId::from_index(index).unwrap();
            for (position, operation) in frozen.data().operations.iter().enumerate() {
                if let OperationKind::Closure(body) = operation.kind {
                    if let Some(slot) = created.get_mut(body.index()) {
                        *slot = slot.saturating_add(1);
                        creation[body.index()] = Some((unit, OpId::from_index(position).unwrap()));
                    }
                }
            }
        }
        for (slot, count) in creation.iter_mut().zip(&created) {
            if *count != 1 {
                *slot = None;
            }
        }
        let first_run = (0..count)
            .map(|index| {
                if statements[index].is_some() {
                    RootPoint::INSTANTIATION
                } else {
                    RootPoint::FIRST
                }
            })
            .collect();
        let hazards = vec![true; moments.len()];
        Self {
            deps: Deps::of_program(program),
            moments,
            statements,
            settled,
            locals,
            creation,
            first_run,
            hazards,
        }
    }

    /// Each body's first point, from the effects of the root statements
    /// (`statements`: per module initializer, each operation's effects as
    /// its summary joined them).
    pub(super) fn schedule(
        &mut self,
        program: &Program<'_>,
        graph: &CallGraph,
        statements: &[Vec<Effects>],
    ) {
        let count = program.units.len();
        let mut hazards = vec![false; self.moments.len()];
        for (point, moment) in self.moments.iter().enumerate() {
            if *moment == Moment::HostModules {
                hazards[point] = true;
            }
        }
        for (index, points) in self.statements.iter().enumerate() {
            let Some(points) = points else {
                continue;
            };
            let recorded = statements.get(index).map_or(&[][..], Vec::as_slice);
            if recorded.len() != points.len() {
                // No per-operation effects (an unknown summary): every
                // statement may run anything.
                for &point in points {
                    if point != RootPoint::INSTANTIATION {
                        hazards[point.0 as usize] = true;
                    }
                }
                continue;
            }
            for (effects, &point) in recorded.iter().zip(points) {
                if point != RootPoint::INSTANTIATION && hazard(effects) {
                    hazards[point.0 as usize] = true;
                }
            }
        }
        // The first hazard at or after each point.
        let mut next = vec![RootPoint::END; self.moments.len() + 1];
        for point in (0..self.moments.len()).rev() {
            next[point] = if hazards[point] {
                RootPoint(point as u32)
            } else {
                next[point + 1]
            };
        }
        let hazard_after = |point: RootPoint| -> RootPoint {
            next.get(point.0 as usize)
                .copied()
                .unwrap_or(RootPoint::END)
        };
        let mut first_run = vec![RootPoint::END; count];
        let mut escaped = vec![RootPoint::END; count];
        let mut heap: BinaryHeap<Reverse<(RootPoint, u32)>> = BinaryHeap::new();
        let statements = &self.statements;
        let locals = &self.locals;
        // Where an escape happens in a statically ordered initializer.
        let escape_point = |unit: UnitId, at: EscapeAt| -> RootPoint {
            let points = statements[unit.index()].as_ref().unwrap();
            let operation = match at {
                EscapeAt::Operation(operation) => Some(operation),
                EscapeAt::Region(region) => locals[unit.index()].dominance.parent(region),
            };
            operation
                .and_then(|operation| points.get(operation.index()).copied())
                .unwrap_or(RootPoint::FIRST)
        };
        let mut escape = |heap: &mut BinaryHeap<_>, body: UnitId, point: RootPoint| {
            if let Some(slot) = escaped.get_mut(body.index()) {
                if point < *slot {
                    *slot = point;
                    heap.push(Reverse((hazard_after(point), body.index() as u32)));
                }
            }
        };
        for (index, points) in statements.iter().enumerate() {
            let unit = UnitId::from_index(index).unwrap();
            match points {
                Some(points) => {
                    for edge in graph.calls_from(unit) {
                        let point = points
                            .get(edge.operation.index())
                            .copied()
                            .unwrap_or(RootPoint::FIRST);
                        heap.push(Reverse((point, edge.callee.index() as u32)));
                    }
                    for site in graph.escapes_in(unit) {
                        escape(&mut heap, site.body, escape_point(unit, site.at));
                    }
                }
                // A module initializer outside the static order belongs to a
                // module only `import()` loads: it may run whenever code
                // the program does not see runs.
                None if program
                    .unit(unit)
                    .is_some_and(|data| data.kind == UnitKind::ModuleInitialization) =>
                {
                    heap.push(Reverse((hazard_after(RootPoint::FIRST), index as u32)));
                }
                None => {}
            }
        }
        // The host-visible interface: the entry's exports and every
        // `import()` namespace. Under module sealing and without host
        // modules, the host reaches it once initialization has completed.
        // Other modules' exports stay inside the one delivered file; when
        // preserve-modules delivery (M3.3) makes each module a file, its
        // exports join this set.
        let host_modules = self.moments.contains(&Moment::HostModules);
        let exports_from = if graph.seal() == Seal::Module && !host_modules {
            RootPoint::END
        } else {
            RootPoint::INSTANTIATION
        };
        let interface = program
            .value_exports()
            .map(|(_, cell)| (cell, exports_from))
            .chain(program.modules.iter().flat_map(|module| {
                module
                    .namespace
                    .iter()
                    .map(|&(_, cell)| (cell, RootPoint::INSTANTIATION))
            }))
            .collect::<Vec<_>>();
        for (cell, point) in interface {
            if let Some(body) = graph.cell_body(cell) {
                escape(&mut heap, body, point);
            }
        }
        while let Some(Reverse((point, index))) = heap.pop() {
            let index = index as usize;
            if statements[index].is_some() || point >= first_run[index] {
                continue;
            }
            first_run[index] = point;
            let unit = UnitId::from_index(index).unwrap();
            for edge in graph.calls_from(unit) {
                if point < first_run[edge.callee.index()] {
                    heap.push(Reverse((point, edge.callee.index() as u32)));
                }
            }
            for site in graph.escapes_in(unit) {
                escape(&mut heap, site.body, point);
            }
        }
        for (index, points) in statements.iter().enumerate() {
            if points.is_some() {
                first_run[index] = RootPoint::INSTANTIATION;
            }
        }
        self.first_run = first_run;
        self.hazards = hazards;
    }

    pub fn deps(&self) -> &Deps {
        &self.deps
    }

    /// What happens at a point, `None` for `END`.
    pub fn moment(&self, point: RootPoint) -> Option<Moment> {
        self.moments.get(point.0 as usize).copied()
    }

    /// The root statement that settles a module-level cell: every read that
    /// runs after it finds the cell initialized. Named functions are settled
    /// at instantiation.
    pub fn settled(&self, cell: CellId) -> Option<RootPoint> {
        self.settled.get(cell.index()).copied().flatten()
    }

    /// The first point during which the unit may run: it is not invoked
    /// before that root statement. `END` for a body that runs only after
    /// initialization completes, or never. A module initializer's own
    /// operations are root statements; for it this is `INSTANTIATION`.
    pub fn first_run(&self, unit: UnitId) -> RootPoint {
        self.first_run
            .get(unit.index())
            .copied()
            .unwrap_or(RootPoint::FIRST)
    }

    /// Whether code the program does not see may run at a point.
    pub fn hazard(&self, point: RootPoint) -> bool {
        self.hazards.get(point.0 as usize).copied().unwrap_or(true)
    }

    /// The point `operation` of `unit` runs at, as early as it can.
    fn point(&self, unit: UnitId, operation: OpId) -> RootPoint {
        match self.statements.get(unit.index()) {
            Some(Some(points)) => points
                .get(operation.index())
                .copied()
                .unwrap_or(RootPoint::FIRST),
            _ => self.first_run(unit),
        }
    }

    /// Whether `unit`'s access of `cell` at `operation` is past the cell's
    /// initialization in every run: it cannot observe the temporal dead zone.
    pub fn initialized(
        &self,
        program: &Program<'_>,
        unit: UnitId,
        operation: OpId,
        cell: CellId,
    ) -> bool {
        let (Some(storage), Some(data)) = (program.cells.get(cell.index()), program.unit(unit))
        else {
            return false;
        };
        match storage.binding {
            CellBinding::Parameter(_) => !program.is_reference_parameter(cell),
            CellBinding::Foreign => false,
            CellBinding::Function(_) => {
                self.settled(cell).is_some()
                    || program
                        .unit(storage.owner)
                        .is_some_and(|owner| owner.module == data.module)
            }
            CellBinding::Local => {
                if checker_proved(storage) {
                    return true;
                }
                let owner = storage.owner;
                // Lexically: the access, or the creation of the closure that
                // leads to it, runs in the owner after the initialization.
                let (mut current, mut at) = (unit, operation);
                for _ in 0..=self.creation.len() {
                    if current == owner {
                        if let (Some(local), Some(owner_data)) =
                            (self.locals.get(owner.index()), program.unit(owner))
                        {
                            if local.after(owner_data, cell, at) {
                                return true;
                            }
                        }
                        break;
                    }
                    match self.creation.get(current.index()).copied().flatten() {
                        Some((creator, creating)) => {
                            current = creator;
                            at = creating;
                        }
                        None => break,
                    }
                }
                // By the schedule: the access cannot run before the root
                // statement settling the cell.
                self.settled(cell)
                    .is_some_and(|settled| self.point(unit, operation) > settled)
            }
        }
    }
}

/// Code the program does not see may run, or initialization may stop.
fn hazard(effects: &Effects) -> bool {
    effects.may_throw
        || effects.runs_user_code
        || effects.reenters
        || effects.suspends
        || effects.obligated()
}
