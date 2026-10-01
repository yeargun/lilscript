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
use super::analysis_storage as storage;
use super::call_graph::{CallGraph, EscapeAt, Seal};
use super::effects::Effects;
use super::views::Deps;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
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
    /// Another entry may have been consumed before this module starts.
    InterfaceHazard { module: ModuleId },
    /// One entry's closure is ready; its public functions may now be called.
    InterfaceReady { module: ModuleId },
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
    initializers: Vec<(CellId, OpId)>,
    /// A catch binding, for-in key or for-of item is initialized throughout
    /// the region its construct enters.
    bound: Vec<(CellId, usize, RegionId)>,
}

impl UnitInitialization {
    fn counts(data: &UnitData) -> (usize, usize) {
        let mut counts = (0, 0);
        for operation in &data.operations {
            match operation.kind {
                OperationKind::Initialize(_) | OperationKind::Declare(_) => counts.0 += 1,
                OperationKind::Try {
                    catch: Some((Some(_), _)),
                    ..
                }
                | OperationKind::ForIn { .. }
                | OperationKind::ForOf { .. } => counts.1 += 1,
                _ => {}
            }
        }
        counts
    }
    pub(super) fn build_in(
        data: &UnitData,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.retained_phase(|budget| {
            budget.work(WorkKind::Analysis, data.operations.len() as u64)?;
            let (initializers, bound) = Self::counts(data);
            let parent = budget.filled(Retained, data.regions.len(), None)?;
            let position = budget.filled(Retained, data.operations.len(), 0)?;
            let initializers = budget.vector(Retained, initializers)?;
            let bound = budget.vector(Retained, bound)?;
            Self::build_buffers(data, parent, position, initializers, bound, |amount| {
                budget.work(WorkKind::Analysis, amount as u64)
            })
        })
    }
    /// A facts session already reserves its complete result/scratch envelope.
    /// Reserve these exact-capacity arrays inside that envelope before creating
    /// them; use the same constructor and charge its actual traversal work.
    pub(super) fn build_bounded(data: &UnitData, work: &mut super::facts::Work) -> Option<Self> {
        if !work.charge(data.operations.len() as u64) {
            return None;
        }
        let (initializers, bound) = Self::counts(data);
        let bytes = data
            .regions
            .len()
            .checked_mul(std::mem::size_of::<Option<OpId>>())?
            .checked_add(
                data.operations
                    .len()
                    .checked_mul(std::mem::size_of::<usize>())?,
            )?
            .checked_add(initializers.checked_mul(std::mem::size_of::<(CellId, OpId)>())?)?
            .checked_add(bound.checked_mul(std::mem::size_of::<(CellId, usize, RegionId)>())?)?;
        if !work.reserve_storage(bytes as u64) {
            return None;
        }
        Self::build_buffers(
            data,
            vec![None; data.regions.len()],
            vec![0; data.operations.len()],
            Vec::with_capacity(initializers),
            Vec::with_capacity(bound),
            |amount| work.charge(amount as u64).then_some(()).ok_or(()),
        )
        .ok()
    }
    fn build_buffers<E>(
        data: &UnitData,
        parent: Vec<Option<OpId>>,
        position: Vec<usize>,
        mut initializers: Vec<(CellId, OpId)>,
        mut bound: Vec<(CellId, usize, RegionId)>,
        mut work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let dominance = StructuredDominance::build(data, parent, position, &mut work)?;
        for (index, operation) in data.operations.iter().enumerate() {
            work(1)?;
            match operation.kind {
                OperationKind::Initialize(cell) | OperationKind::Declare(cell) => {
                    initializers.push((cell, OpId::from_index(index).unwrap()))
                }
                OperationKind::Try {
                    catch: Some((Some(cell), region)),
                    ..
                }
                | OperationKind::ForIn {
                    key: cell,
                    body: region,
                    ..
                }
                | OperationKind::ForOf {
                    item: cell,
                    body: region,
                    ..
                } => bound.push((cell, index, region)),
                _ => {}
            }
        }
        let count = initializers.len().saturating_add(bound.len());
        work(count.saturating_mul((usize::BITS - count.max(1).leading_zeros()) as usize + 1))?;
        initializers.sort_unstable();
        bound.sort_unstable_by_key(|&(cell, index, _)| (cell, index));
        Ok(Self {
            dominance,
            initializers,
            bound,
        })
    }

    /// Whether `operation` of this unit runs only after `cell`, which the
    /// unit owns, is initialized: an `Initialize` of it dominates the
    /// operation, or the operation lies in the region its binding construct
    /// enters.
    pub(super) fn after(&self, data: &UnitData, cell: CellId, operation: OpId) -> bool {
        self.after_with(data, cell, operation, |_| Ok::<_, ()>(()))
            .unwrap_or(false)
    }
    fn after_with<E>(
        &self,
        data: &UnitData,
        cell: CellId,
        operation: OpId,
        mut work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        work((usize::BITS - self.initializers.len().max(1).leading_zeros()) as usize * 2 + 1)?;
        let start = self.initializers.partition_point(|&(key, _)| key < cell);
        let end = self.initializers.partition_point(|&(key, _)| key <= cell);
        for &(_, initialize) in &self.initializers[start..end] {
            if self
                .dominance
                .after(data, initialize, operation, &mut work)?
            {
                return Ok(true);
            }
        }
        work((usize::BITS - self.bound.len().max(1).leading_zeros()) as usize + 1)?;
        let end = self.bound.partition_point(|&(key, _, _)| key <= cell);
        if let (Some(&(key, _, ancestor)), Some(entry)) = (
            end.checked_sub(1).and_then(|index| self.bound.get(index)),
            data.operations.get(operation.index()),
        ) {
            if key == cell {
                let mut region = entry.region;
                for _ in 0..=data.regions.len() {
                    work(1)?;
                    if region == ancestor {
                        return Ok(true);
                    }
                    let Some(owner) = self.dominance.parent(region) else {
                        return Ok(false);
                    };
                    region = data.operations[owner.index()].region;
                }
            }
        }
        Ok(false)
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
    work: &mut super::facts::Work,
) -> bool {
    let Some(storage) = program.cells.get(cell.index()) else {
        return false;
    };
    match storage.binding {
        CellBinding::Local => {
            checker_proved(storage)
                || (storage.owner == unit
                    && local
                        .after_with(data, cell, operation, |amount| {
                            work.charge(amount as u64).then_some(()).ok_or(())
                        })
                        .unwrap_or(false))
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
        Self::structural_in(program, graph, &mut AllocationBudget::new(None))
            .expect("inspection initialization")
    }
    pub(super) fn structural_in(
        program: &Program<'_>,
        graph: &CallGraph,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.retained_phase(|budget| {
            let count = program.units.len();
            let mut locals = budget.vector(Retained, count)?;
            for unit in &program.units {
                let local = UnitInitialization::build_in(unit.data(), budget)?;
                budget.push(Retained, &mut locals, local)?;
            }
            let mut moments = budget.copy_slice(Retained, &[Moment::Instantiation])?;
            if program
                .modules
                .iter()
                .any(|module| !module.foreign_imports.is_empty())
            {
                budget.push(Retained, &mut moments, Moment::HostModules)?;
            }
            let mut statements = storage::collect((0..count).map(|_| None), Retained, budget)?;
            // Only the static DAG has one valid global order. Lazy modules
            // execute on demand; members of an import cycle can be entered
            // from different roots. Their local dominance facts remain usable,
            // but a canonical concatenation cannot prove cross-unit timing.
            let several = program.entries().len() > 1;
            let mut common = budget.filled(Scratch, program.modules.len(), true)?;
            let mut eager = budget.filled(Scratch, program.modules.len(), false)?;
            for entry in program.entries() {
                let mut scope = budget.scope();
                let order = crate::module::static_evaluation_order_from_admitted(
                    &[entry.module.index()], program.modules.len(),
                    |module| program.modules[module].dependencies.iter().map(|id| id.index()),
                    &mut scope,
                ).map_err(|error| match error {
                    crate::module::StaticOrderError::Resources(error) => error,
                    crate::module::StaticOrderError::Invalid(_) => AllocationError::Capacity,
                })?;
                let mut reached = scope.filled(Scratch, common.len(), false)?;
                for module in order { reached[module] = true; eager[module] = true; }
                for (common, reached) in common.iter_mut().zip(reached) { *common &= reached; }
            }
            let mut imports = budget.vector(Scratch, program.modules.len())?;
            for module in program.modules.iter() {
                let row = storage::collect(module.dependencies.iter().map(|id| id.index()), Scratch, budget)?;
                imports.push(row);
            }
            let cycles = crate::module::static_cycles_admitted(&imports, budget)?;
            for row in imports.iter_mut() { storage::release(std::mem::take(row), Scratch, budget)?; }
            storage::release(imports, Scratch, budget)?;
            for &initializer in program.initialization.iter() {
                let Some(data) = program.unit(initializer) else {
                    continue;
                };
                if statements[initializer.index()].is_some() || !eager[data.module.index()] || cycles[data.module.index()].is_some() {
                    continue;
                }
                if several && !common[data.module.index()] {
                    budget.push(Retained, &mut moments, Moment::InterfaceHazard { module: data.module })?;
                }
                let local = &locals[initializer.index()];
                let mut entry = budget.filled(Scratch, data.operations.len(), None)?;
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
                        budget.push(
                            Retained,
                            &mut moments,
                            Moment::Statement {
                                unit: initializer,
                                operation,
                            },
                        )?;
                        point
                    });
                }
                budget.work(WorkKind::Analysis, data.operations.len() as u64)?;
                let points = storage::collect(
                    (0..data.operations.len()).map(|index| {
                        local
                            .statement(data, OpId::from_index(index).unwrap())
                            .and_then(|statement| entry[statement.index()])
                            // Unverified structure: the earliest point a
                            // statement can hold, which proves nothing.
                            .unwrap_or(RootPoint::FIRST)
                    }),
                    Retained,
                    budget,
                )?;
                storage::release(entry, Scratch, budget)?;
                statements[initializer.index()] = Some(points);
                if several && program.entries().iter().any(|entry| entry.module == data.module) {
                    budget.push(Retained, &mut moments, Moment::InterfaceReady { module: data.module })?;
                }
            }
            storage::release(common, Scratch, budget)?;
            storage::release(eager, Scratch, budget)?;
            storage::release(cycles, Scratch, budget)?;
            let settled = storage::collect(
                program.cells.iter().enumerate().map(|(index, storage)| {
                    let cell = CellId::from_index(index).unwrap();
                    match storage.binding {
                        CellBinding::Function(_) => program.unit(storage.owner)
                            .filter(|owner| owner.kind == UnitKind::ModuleInitialization)
                            .map(|_| RootPoint::INSTANTIATION),
                        CellBinding::Local => {
                            let points = statements.get(storage.owner.index())?.as_ref()?;
                            let (unit, operation) = graph.initializer(cell)?;
                            let data = program.unit(unit)?;
                            (unit == storage.owner
                                && data.operations.get(operation.index())?.region == data.entry)
                                .then(|| points[operation.index()])
                                .filter(|&point| point != RootPoint::INSTANTIATION)
                        }
                        CellBinding::Parameter(_) | CellBinding::Foreign => None,
                    }
                }),
                Retained,
                budget,
            )?;
            let mut creation = budget.filled(Retained, count, None)?;
            let mut created = budget.filled(Scratch, count, 0u8)?;
            for (index, frozen) in program.units.iter().enumerate() {
                let unit = UnitId::from_index(index).unwrap();
                for (position, operation) in frozen.data().operations.iter().enumerate() {
                    budget.work(WorkKind::Analysis, 1)?;
                    if let OperationKind::Closure(body) = operation.kind {
                        if let Some(slot) = created.get_mut(body.index()) {
                            *slot = slot.saturating_add(1);
                            creation[body.index()] =
                                Some((unit, OpId::from_index(position).unwrap()));
                        }
                    }
                }
            }
            for (slot, count) in creation.iter_mut().zip(&created) {
                if *count != 1 {
                    *slot = None;
                }
            }
            let first_run = storage::collect(
                (0..count).map(|index| {
                    if statements[index].is_some() {
                        RootPoint::INSTANTIATION
                    } else {
                        RootPoint::FIRST
                    }
                }),
                Retained,
                budget,
            )?;
            let hazards = budget.filled(Retained, moments.len(), true)?;
            Ok(Self {
                deps: Deps::of_program_in(program, budget)?,
                moments,
                statements,
                settled,
                locals,
                creation,
                first_run,
                hazards,
            })
        })
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
        self.schedule_in(program, graph, statements, &mut AllocationBudget::new(None))
            .expect("inspection schedule")
    }
    pub(super) fn schedule_in(
        &mut self,
        program: &Program<'_>,
        graph: &CallGraph,
        statements: &[Vec<Effects>],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let count = program.units.len();
        let mut hazards = budget.filled(Retained, self.moments.len(), false)?;
        for (point, moment) in self.moments.iter().enumerate() {
            if matches!(moment, Moment::HostModules | Moment::InterfaceHazard { .. } | Moment::InterfaceReady { .. }) {
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
        let mut next = budget.filled(
            Scratch,
            self.moments
                .len()
                .checked_add(1)
                .ok_or(AllocationError::Capacity)?,
            RootPoint::END,
        )?;
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
        let mut first_run = budget.filled(Retained, count, RootPoint::END)?;
        let mut escaped = budget.filled(Scratch, count, RootPoint::END)?;
        // Every body's earliest heap pop settles it. Each edge and escape
        // can therefore enqueue at most once; public/lazy entries are seeds.
        let mut capacity = count
            .checked_add(program.exports().len())
            .ok_or(AllocationError::Capacity)?;
        for unit in &program.units {
            capacity = capacity
                .checked_add(graph.calls_from(unit.id()).len())
                .and_then(|n| n.checked_add(graph.escapes_in(unit.id()).len()))
                .ok_or(AllocationError::Capacity)?;
        }
        for module in program.modules.iter() {
            capacity = capacity
                .checked_add(module.namespace.len())
                .ok_or(AllocationError::Capacity)?;
        }
        let mut heap: BinaryHeap<Reverse<(RootPoint, u32)>> =
            BinaryHeap::from(budget.vector(Scratch, capacity)?);
        let push = |heap: &mut BinaryHeap<_>,
                    value,
                    budget: &mut AllocationBudget<'_>|
         -> Result<(), AllocationError> {
            budget.work(WorkKind::Analysis, 1)?;
            if heap.len() == heap.capacity() {
                return Err(AllocationError::Capacity);
            }
            heap.push(value);
            Ok(())
        };
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
        let mut escape = |heap: &mut BinaryHeap<_>,
                          body: UnitId,
                          point: RootPoint,
                          budget: &mut AllocationBudget<'_>|
         -> Result<(), AllocationError> {
            if let Some(slot) = escaped.get_mut(body.index()) {
                if point < *slot {
                    *slot = point;
                    push(
                        heap,
                        Reverse((hazard_after(point), body.index() as u32)),
                        budget,
                    )?;
                }
            }
            Ok(())
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
                        push(
                            &mut heap,
                            Reverse((point, edge.callee.index() as u32)),
                            budget,
                        )?;
                    }
                    for site in graph.escapes_in(unit) {
                        escape(&mut heap, site.body, escape_point(unit, site.at), budget)?;
                    }
                }
                // A module initializer outside the static order belongs to a
                // module only `import()` loads: it may run whenever code
                // the program does not see runs.
                None if program
                    .unit(unit)
                    .is_some_and(|data| data.kind == UnitKind::ModuleInitialization) =>
                {
                    push(
                        &mut heap,
                        Reverse((RootPoint::INSTANTIATION, index as u32)),
                        budget,
                    )?;
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
        let mut ready = budget.filled(Scratch, program.modules.len(), RootPoint::END)?;
        for (index, moment) in self.moments.iter().enumerate() {
            if let Moment::InterfaceReady { module } = moment {
                ready[module.index()] = RootPoint(index as u32);
            }
        }
        let interface = program.entries().iter().enumerate().flat_map(|(index, entry)| {
            let point = if graph.seal() == Seal::Module && !host_modules {
                ready[entry.module.index()]
            } else { RootPoint::INSTANTIATION };
            program.entry_exports(index).iter().filter_map(move |export| match export.target {
                InterfaceTarget::Value(cell) => Some((cell, point)),
                InterfaceTarget::Type(_) => None,
            })
        })
            .chain(program.modules.iter().flat_map(|module| {
                module
                    .namespace
                    .iter()
                    .map(|&(_, cell)| (cell, RootPoint::INSTANTIATION))
            }));
        for (cell, point) in interface {
            if let Some(body) = graph.cell_body(cell) {
                escape(&mut heap, body, point, budget)?;
            }
        }
        storage::release(ready, Scratch, budget)?;
        while let Some(Reverse((point, index))) = heap.pop() {
            budget.work(WorkKind::Analysis, 1)?;
            let index = index as usize;
            if statements[index].is_some() || point >= first_run[index] {
                continue;
            }
            first_run[index] = point;
            let unit = UnitId::from_index(index).unwrap();
            for edge in graph.calls_from(unit) {
                if point < first_run[edge.callee.index()] {
                    push(
                        &mut heap,
                        Reverse((point, edge.callee.index() as u32)),
                        budget,
                    )?;
                }
            }
            for site in graph.escapes_in(unit) {
                escape(&mut heap, site.body, point, budget)?;
            }
        }
        for (index, points) in statements.iter().enumerate() {
            if points.is_some() {
                first_run[index] = RootPoint::INSTANTIATION;
            }
        }
        storage::release(
            std::mem::replace(&mut self.first_run, first_run),
            Retained,
            budget,
        )?;
        storage::release(
            std::mem::replace(&mut self.hazards, hazards),
            Retained,
            budget,
        )?;
        storage::release(next, Scratch, budget)?;
        storage::release(escaped, Scratch, budget)?;
        storage::release(heap.into_vec(), Scratch, budget)?;
        Ok(())
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

    /// The root point of `operation` of `unit`, when the unit is a
    /// statically ordered module initializer.
    pub fn root_point(&self, unit: UnitId, operation: OpId) -> Option<RootPoint> {
        self.statements
            .get(unit.index())?
            .as_ref()?
            .get(operation.index())
            .copied()
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
