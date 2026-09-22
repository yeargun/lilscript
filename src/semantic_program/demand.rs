//! Effective physical demand over checked source and selected implementations.
//!
//! Results and required evaluation are independent roots. The worklist never
//! changes source schedules: it retains their control/prepared-call envelopes.
//! Storage writers wait for observable storage instead of rooting themselves.
use super::activation::StructuredDominance;
use super::callable_inputs::{CallObservations, CallableInputs, InputOutcome};
use super::facts::{self, EvaluationBehavior, ObservationDemand};
use super::helper_family::HelperFamily;
use super::product_family::{ProductAccessRoot, ProductFamily, ProductOperationKind};
#[path = "demand_functions.rs"]
mod functions;
#[path = "demand_locations.rs"]
mod locations;
#[path = "demand_products.rs"]
mod products;
use super::function_layout::{FunctionLayout, ParameterLayout, ProductTransport};
use super::implementations::ImplementationMap;
use super::javascript_resource::ResourceView;
use super::raw_domains::{DomainInputs, DomainProof, Subject};
use super::record_family::{ReadOrWrite, RecordFamily};
use super::string_family::{StringChoice, StringFamily};
use super::uses::{UseIndex, ValueUse};
use super::*;
use crate::compilation_contract::JavaScriptCompilationContract;
use crate::compilation_policy::{BudgetError, BudgetLedger, WorkDomain, WorkKind};
use functions::FunctionCall;
pub(super) use functions::InlineActual;
pub(super) use functions::{shared_transport_support, SharedTransportSupport};
use locations::LocationUse;
pub(super) use locations::{LocationCheck, LocationDemand, PlaceLocation};
use products::{ProductBank, ProductSnapshot};
use std::mem::size_of;

#[derive(Debug)]
pub(super) enum DemandError {
    Budget(BudgetError),
    Unsupported(Unsupported),
}
impl From<BudgetError> for DemandError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
fn unsupported(feature: &'static str) -> DemandError {
    DemandError::Unsupported(Unsupported {
        span: Span::default(),
        feature,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct ContextId(usize);
impl ContextId {
    pub(super) fn index(self) -> usize {
        self.0
    }
}

/// One evaluation position, not merely an unordered semantic dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EffectiveUseSite {
    Operation(OpId),
    RegionResult(RegionId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EffectiveUseRole {
    Operand(u32),
    PlaceReceiver(PlaceId),
    PlaceKey(PlaceId),
    CallCallee(CallId),
    CapturedCallCallee(CallId),
    ProductArgument {
        call: CallId,
        position: u32,
        slot: u32,
    },
    CallReceiver(CallId),
    InlineParameter {
        context: ContextId,
        position: u32,
    },
    RecordSlot {
        record: usize,
        slot: u32,
    },
    Return,
    RegionResult,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EffectiveValueUse {
    pub value: ValueId,
    pub site: EffectiveUseSite,
    pub role: EffectiveUseRole,
    pub observation: ObservationDemand,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DemandMode {
    Prune,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContextKind {
    Named,
    Inline { helper: usize, call: OpId },
}
impl ContextKind {
    pub(super) fn is_inline(self) -> bool {
        matches!(self, Self::Inline { .. })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecordOperation {
    ElidedHandle,
    Initialize(usize),
    Read { record: usize, slot: u32 },
    Write { record: usize, slot: u32 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HelperOperation {
    Elided,
    Call(usize),
}

#[derive(Debug, Clone, Copy)]
struct StringOperation {
    family: usize,
    /// Index in the shared-family owner table, never a target binding.
    shared: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct DemandWork {
    pub steps: u64,
    pub contexts: usize,
    pub peak_bytes: u64,
    pub retained_bytes: u64,
}
#[derive(Debug, Clone, Copy, Default)]
struct Storage {
    // Only Context.cells owns location demand. Product/record slot rows leave
    // this None; observation/writer/rewrite fields remain separate.
    location: LocationDemand,
    observation: ObservationDemand,
    declaration: bool,
    // Product slots only: a monotone summary, independent of writer waiters.
    // An already-needed slot still records every possible later replacement.
    rewritten: bool,
    writers: Option<usize>,
}
#[derive(Debug)]
pub(super) struct Context {
    pub unit: UnitId,
    pub parent: Option<ContextId>,
    pub kind: ContextKind,
    creation: Option<OpId>,
    values: Vec<ObservationDemand>,
    operations: Vec<bool>,
    execution: Vec<bool>,
    production: Vec<bool>,
    dependencies: Vec<Option<ObservationDemand>>,
    regions: Vec<u8>,
    region_results: Vec<ObservationDemand>,
    cells: Vec<Storage>,
    children: Vec<Option<ContextId>>,
    return_observation: ObservationDemand,
    /// Dense only over this source unit's selected shared families. Every
    /// inline occurrence owns separate bits and eventual target storage.
    shared_strings: Vec<bool>,
    product_banks: Vec<ProductBank>,
    product_snapshots: Vec<ProductSnapshot>,
}
#[derive(Debug, Clone, Copy)]
enum Initialization {
    Missing,
    Parameter(u32),
    Catch(RegionId),
    Operation(OpId),
    Multiple,
}
#[derive(Debug)]
struct UnitSummary {
    // A demanded original generic call carries a concrete nominal interface.
    // True records completed Body-scoped input and returned-origin qualification
    // under this Demand's immutable source and execution contract. False is
    // never a primitive-domain fact. One original body serves every context.
    generic_product_transport: bool,
    named_context: Option<ContextId>,
    shared_strings: std::ops::Range<usize>,
    effects: Vec<EvaluationBehavior>,
    dominance: StructuredDominance,
    region_owners: Vec<Option<OpId>>,
    call_envelopes: Vec<Option<OpId>>,
    prepares: Vec<Option<OpId>>,
    invocations: Vec<Option<OpId>>,
    initializers: Vec<Initialization>,
}
#[derive(Debug, Clone, Copy)]
enum StorageId {
    Cell(ContextId, usize),
    Slot(usize, u32),
    Product(ContextId, usize, u32),
}
#[derive(Debug, Clone, Copy)]
struct Writer {
    context: ContextId,
    operation: OpId,
    next: Option<usize>,
}
#[derive(Debug, Clone, Copy)]
enum Requirement {
    Execute,
    Produce,
}
impl Requirement {
    fn bit(self) -> u8 {
        match self {
            Self::Execute => 1,
            Self::Produce => 2,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Pending {
    Value(ContextId, ValueId),
    Operation(ContextId, OpId, Requirement),
    Storage(StorageId),
    Snapshot(ContextId, usize, u32),
}

#[derive(Clone, Copy)]
enum InputRecipe {
    Empty,
    RecordInitialize(usize),
    RecordRead { record: usize, slot: u32 },
    RecordWrite { record: usize, slot: u32 },
    InlineCall { child: ContextId, prepare: OpId },
    Ordinary,
    Product(ProductOperationKind),
}
/// The same allocation-free cursor drives liveness and retained occurrences.
/// A step can skip an undemanded slot/parameter without an uncharged scan.
struct InputCursor {
    context: ContextId,
    operation: OpId,
    recipe: InputRecipe,
    stage: usize,
    index: usize,
    place: Option<PlaceId>,
    product_snapshot: Option<usize>,
    component: usize,
    observation: ObservationDemand,
}
enum InputDependency {
    Value(EffectiveValueUse),
    Cell(CellId, ObservationDemand),
    Slot(usize, u32),
    Prepare(OpId),
    ProductCell(CellId, u32),
    ProductSnapshot(ValueId, u32),
    Location(PlaceLocation, LocationUse),
}
enum InputStep {
    Dependency(InputDependency),
    Skip,
    Done,
}

#[must_use = "discard demand storage through its original compilation ledger after formation"]
pub(super) struct DemandPlan<'program, 'src> {
    program: &'program Program<'src>,
    uses: Option<&'program UseIndex>,
    /// Inspection can omit the index; without it, any reference argument
    /// conservatively disables isolated-cell refinements for that whole output.
    unindexed_references: bool,
    // A demanded write still reaches a named incoming reference after the
    // original inline-actual cursor resolves its location. This physical
    // requirement is monotone; it is not a source alias or per-helper index.
    writes_incoming_references: bool,
    contract: JavaScriptCompilationContract,
    resource: ResourceView<'program>,
    mode: DemandMode,
    contexts: Vec<Context>,
    /// Physical roots in the verified module evaluation order. Every root is
    /// allocated before any initializer is seeded, so module cells have one
    /// canonical storage owner even in cycles.
    roots: Vec<ContextId>,
    owned_cells: Vec<Vec<CellId>>,
    cell_ordinals: Vec<usize>,
    summaries: Vec<Option<UnitSummary>>,
    records: Vec<&'program RecordFamily>,
    helpers: Vec<&'program HelperFamily>,
    strings: Vec<&'program StringFamily>,
    products: Vec<&'program ProductFamily>,
    functions: Vec<&'program FunctionLayout>,
    function_calls: Vec<((UnitId, CallId), FunctionCall)>,
    max_function_parameters: usize,
    product_cell_index: Vec<((UnitId, CellId), usize)>,
    product_value_index: Vec<((UnitId, ValueId), usize)>,
    product_operations: Vec<((UnitId, OpId), ProductOperationKind)>,
    string_operations: Vec<((UnitId, OpId), StringOperation)>,
    /// Sorted by semantic activation and family. Contexts use a contiguous
    /// local range, so neither captures nor inline expansion copy global bits.
    shared_strings: Vec<(UnitId, usize)>,
    record_operations: Vec<((UnitId, OpId), RecordOperation)>,
    helper_operations: Vec<((UnitId, OpId), HelperOperation)>,
    record_cells: Vec<(CellId, usize)>,
    helper_cells: Vec<(CellId, usize)>,
    slots: Vec<Vec<Storage>>,
    writers: Vec<Writer>,
    pending: Vec<Pending>,
    work: DemandWork,
    charge: Option<(WorkDomain, u64)>,
}

impl<'program, 'src> DemandPlan<'program, 'src> {
    pub(super) fn build(
        program: &'program Program<'src>,
        uses: Option<&'program UseIndex>,
        implementations: Option<&'program ImplementationMap>,
        contract: &JavaScriptCompilationContract,
        mode: DemandMode,
        budget: Option<(&mut BudgetLedger, WorkDomain)>,
    ) -> Result<Self, DemandError> {
        Self::build_resource(
            program,
            uses,
            implementations,
            contract,
            mode,
            ResourceView::Whole,
            budget,
        )
    }
    pub(super) fn build_resource(
        program: &'program Program<'src>,
        uses: Option<&'program UseIndex>,
        implementations: Option<&'program ImplementationMap>,
        contract: &JavaScriptCompilationContract,
        mode: DemandMode,
        resource: ResourceView<'program>,
        budget: Option<(&mut BudgetLedger, WorkDomain)>,
    ) -> Result<Self, DemandError> {
        let _timing = crate::timing::JS_DEMAND.scope(0);
        let mut budget = Budget::new(budget);
        if let Some(implementations) = implementations {
            budget.work(1)?;
            if contract.execution != crate::compilation_contract::JavaScriptExecution::Module {
                if implementations.functions().len() != 0 {
                    return Err(unsupported(
                        "private product transport requires module execution",
                    ));
                }
                for family in implementations.products() {
                    budget.work(1)?;
                    if family.requires_module() {
                        return Err(unsupported(
                            "private product inputs require module execution",
                        ));
                    }
                }
            }
            for helper in implementations.helpers() {
                budget.work(1)?;
                if !helper.frame_elision_allowed(contract.execution) {
                    return Err(unsupported("observable helper frame in script output"));
                }
            }
        }
        budget.retain(size_of::<Self>() as u64)?;
        let mut owned_cells = budget.vector(program.units.len())?;
        owned_cells.resize_with(program.units.len(), Vec::new);
        let mut counts = budget.filled(program.units.len(), 0usize)?;
        budget.work(program.cells.len())?;
        for cell in program.cells.iter() {
            counts[cell.owner.index()] += 1;
        }
        for (unit, &count) in counts.iter().enumerate() {
            owned_cells[unit] = budget.vector(count)?;
        }
        let mut cell_ordinals = budget.filled(program.cells.len(), 0usize)?;
        for (index, cell) in program.cells.iter().enumerate() {
            budget.work(1)?;
            cell_ordinals[index] = owned_cells[cell.owner.index()].len();
            owned_cells[cell.owner.index()].push(CellId::from_index(index).unwrap());
        }
        let mut summaries = budget.vector(program.units.len())?;
        summaries.resize_with(program.units.len(), || None);
        let mut unindexed_references = false;
        if uses.is_none() {
            for unit in program.units.iter() {
                budget.work(1 + unit.data().call_arguments.len())?;
                unindexed_references |= unit
                    .data()
                    .call_arguments
                    .iter()
                    .any(|argument| matches!(argument, CallArgument::Reference(_)));
            }
        }
        let mut plan = Self {
            program,
            uses,
            unindexed_references,
            writes_incoming_references: false,
            contract: *contract,
            resource,
            mode,
            contexts: Vec::new(),
            roots: budget.vector(program.initialization.len())?,
            owned_cells,
            cell_ordinals,
            summaries,
            records: Vec::new(),
            helpers: Vec::new(),
            strings: Vec::new(),
            products: Vec::new(),
            functions: Vec::new(),
            function_calls: Vec::new(),
            max_function_parameters: 0,
            product_cell_index: Vec::new(),
            product_value_index: Vec::new(),
            product_operations: Vec::new(),
            string_operations: Vec::new(),
            shared_strings: Vec::new(),
            record_operations: Vec::new(),
            helper_operations: Vec::new(),
            record_cells: Vec::new(),
            helper_cells: Vec::new(),
            slots: Vec::new(),
            writers: Vec::new(),
            pending: Vec::new(),
            work: DemandWork::default(),
            charge: None,
        };
        plan.index_implementations(implementations, &mut budget)?;
        for &unit in program.initialization.iter() {
            budget.work(1)?;
            if !resource.includes_initializer(unit) {
                continue;
            }
            let context =
                plan.allocate_context(unit, None, None, ContextKind::Named, &mut budget)?;
            plan.roots.push(context);
        }
        for index in 0..plan.roots.len() {
            budget.work(1)?;
            plan.seed_context(plan.roots[index], &mut budget)?;
        }
        let root = plan.root();
        if let Some(export) = resource.producer_export() {
            budget.work(1)?;
            plan.need_cell(root, export.cell(), &mut budget)?;
        } else if contract.abi.preserve_root_exports {
            // The borrowed runtime filter visits type exports too.
            budget.work(program.exports().len())?;
            for (_, cell) in program.value_exports() {
                plan.need_cell(root, cell, &mut budget)?;
            }
        }
        let mut cursor = 0;
        plan.drain_pending(&mut cursor, &mut budget)?;
        // Temporary counts share the same reservation until here; release
        // their exact payload rather than claiming retained demand owns them.
        let bytes = counts.capacity() as u64 * size_of::<usize>() as u64;
        drop(counts);
        budget.release(bytes)?;
        plan.work = DemandWork {
            steps: budget.steps,
            contexts: plan.contexts.len(),
            peak_bytes: budget.peak,
            retained_bytes: budget.retained,
        };
        plan.charge = budget.finish();
        Ok(plan)
    }
    fn drain_pending(
        &mut self,
        cursor: &mut usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        while *cursor < self.pending.len() {
            budget.work(1)?;
            let pending = self.pending[*cursor];
            *cursor += 1;
            match pending {
                Pending::Value(context, value) => self.visit_value(context, value, budget)?,
                Pending::Operation(context, operation, Requirement::Execute) => {
                    self.visit_execution(context, operation, budget)?
                }
                Pending::Operation(context, operation, Requirement::Produce) => {
                    self.visit_production(context, operation, budget)?
                }
                Pending::Storage(storage) => self.visit_storage(storage, budget)?,
                Pending::Snapshot(context, snapshot, slot) => {
                    self.visit_snapshot(context, snapshot, slot, budget)?
                }
            }
        }
        Ok(())
    }
    pub(super) fn root(&self) -> ContextId {
        self.module_context(self.resource.entry_initializer(self.program))
            .expect("verified resource entry has an initializer context")
    }
    pub(super) fn resource(&self) -> ResourceView<'program> {
        self.resource
    }
    pub(super) fn roots(&self) -> &[ContextId] {
        &self.roots
    }
    /// Initializer storage is shared by the artifact, independent of the
    /// lexical creation chain used by ordinary and inlined callable contexts.
    pub(super) fn module_context(&self, unit: UnitId) -> Option<ContextId> {
        if self.program.units[unit.index()].data().kind != UnitKind::ModuleInitialization {
            return None;
        }
        self.summaries[unit.index()]
            .as_ref()
            .and_then(|summary| summary.named_context)
    }
    /// Paid by the caller's ordinary context lookup. True is the completed
    /// original-body qualification held by this immutable Demand; no copied
    /// call index or independently reusable proof is retained.
    pub(super) fn generic_product_transport(&self, context: ContextId) -> bool {
        self.summaries[self.context(context).unit.index()]
            .as_ref()
            .is_some_and(|summary| summary.generic_product_transport)
    }
    pub(super) fn contexts(&self) -> &[Context] {
        &self.contexts
    }
    pub(super) fn context(&self, id: ContextId) -> &Context {
        &self.contexts[id.index()]
    }
    pub(super) fn child(&self, id: ContextId, operation: OpId) -> Option<ContextId> {
        self.context(id).children[operation.index()]
    }
    pub(super) fn needs_operation(&self, id: ContextId, op: OpId) -> bool {
        self.context(id).operations[op.index()]
    }
    /// Whether some context of `unit` keeps `op`. An operation no context
    /// keeps never runs, so nothing it would read is observed.
    pub(super) fn needs_operation_anywhere(&self, unit: UnitId, op: OpId) -> bool {
        self.contexts
            .iter()
            .any(|context| context.unit == unit && context.operations[op.index()])
    }
    pub(super) fn needs_execution(&self, id: ContextId, op: OpId) -> bool {
        self.context(id).execution[op.index()]
    }
    pub(super) fn needs_production(&self, id: ContextId, op: OpId) -> bool {
        self.context(id).production[op.index()]
    }
    /// Final immutable answer; the caller admits the borrowed lookup through
    /// its existing Formation/diagnostic allocation owner.
    pub(super) fn observation<E>(
        &self,
        context: ContextId,
        value: ValueId,
        mut work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<ObservationDemand, E> {
        work(1)?;
        Ok(self.context(context).values[value.index()])
    }
    fn operation_observation<E>(
        &self,
        context: ContextId,
        operation: OpId,
        mut work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<ObservationDemand, E> {
        work(1)?;
        if self.mode == DemandMode::Preserve {
            return Ok(ObservationDemand::Exact);
        }
        let data = self.program.units[self.context(context).unit.index()].data();
        let op = &data.operations[operation.index()];
        let cell = match op.kind {
            OperationKind::Initialize(cell) => Some(cell),
            OperationKind::Store(place) => match data.places[place.index()] {
                Place::Cell(cell) => Some(cell),
                _ => return Ok(ObservationDemand::Exact),
            },
            _ => None,
        };
        if let Some(cell) = cell {
            if !self.isolated_cell(cell) {
                return Ok(ObservationDemand::Exact);
            }
            let Some(owner) = self.cell_owner_visited(context, cell, || work(1))? else {
                return Ok(ObservationDemand::Exact);
            };
            let observation = self.context(owner).cells[self.cell_ordinal(cell)].observation;
            return Ok(if observation.is_observed() {
                observation
            } else {
                ObservationDemand::Exact
            });
        }
        if matches!(op.kind, OperationKind::Return) {
            return Ok(self.context(context).return_observation);
        }
        Ok(op.result.map_or(ObservationDemand::Discarded, |value| {
            self.context(context).values[value.index()]
        }))
    }
    fn operand_observation(
        &self,
        operation: &Operation,
        result: ObservationDemand,
    ) -> ObservationDemand {
        if self.mode == DemandMode::Preserve {
            return ObservationDemand::Exact;
        }
        match operation.kind {
            OperationKind::CopyValue
            | OperationKind::Initialize(_)
            | OperationKind::Store(_)
            | OperationKind::Return => result,
            OperationKind::Unary {
                op: UnaryOp::Not, ..
            }
            | OperationKind::If { .. }
            | OperationKind::Select { .. } => ObservationDemand::Truthy,
            OperationKind::ShortCircuit { kind, .. } => {
                let gate = if kind == ShortCircuit::Nullish {
                    ObservationDemand::Nullish
                } else {
                    ObservationDemand::Truthy
                };
                gate.join(result)
            }
            _ => ObservationDemand::Exact,
        }
    }
    pub(super) fn needs_value(&self, id: ContextId, value: ValueId) -> bool {
        self.context(id).values[value.index()].is_observed()
    }
    pub(super) fn needs_region_result(&self, id: ContextId, region: RegionId) -> bool {
        self.context(id).region_results[region.index()].is_observed()
    }
    pub(super) fn needs_return(&self, id: ContextId) -> bool {
        self.context(id).return_observation.is_observed()
    }
    /// All retained semantic value occurrences, including repeated operands.
    /// This is an inventory, not an execution-order walk across regions. Use
    /// `visit_effective_site_uses` while walking the checked region schedule.
    /// Generated target scratch/storage uses have their own materializer owner.
    pub(super) fn visit_effective_value_uses<E>(
        &self,
        context: ContextId,
        mut charge: impl FnMut(usize) -> Result<(), E>,
        mut visit: impl FnMut(EffectiveValueUse) -> Result<(), E>,
    ) -> Result<(), E> {
        let data = self.program.units[self.context(context).unit.index()].data();
        for index in 0..data.operations.len() {
            self.visit_effective_site_uses(
                context,
                EffectiveUseSite::Operation(OpId::from_index(index).unwrap()),
                &mut charge,
                &mut visit,
            )?;
        }
        for index in 0..data.regions.len() {
            self.visit_effective_site_uses(
                context,
                EffectiveUseSite::RegionResult(RegionId::from_index(index).unwrap()),
                &mut charge,
                &mut visit,
            )?;
        }
        Ok(())
    }
    /// Inputs at one source evaluation site, in receiver/key/operand order.
    /// A surviving call's callee belongs to PrepareCall; arguments belong to
    /// Call. This does not grant permission to move either across that envelope.
    pub(super) fn visit_effective_site_uses<E>(
        &self,
        context: ContextId,
        site: EffectiveUseSite,
        mut charge: impl FnMut(usize) -> Result<(), E>,
        mut visit: impl FnMut(EffectiveValueUse) -> Result<(), E>,
    ) -> Result<(), E> {
        charge(1)?;
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        let operation = match site {
            EffectiveUseSite::RegionResult(region) => {
                if self.needs_region_result(context, region) {
                    if let Some(input) = self.region_input(context, region) {
                        let lookup = self.effective_input_work(input);
                        if lookup != 0 {
                            charge(lookup)?;
                        }
                        visit(input)?;
                    }
                }
                return Ok(());
            }
            EffectiveUseSite::Operation(operation) => {
                if let OperationKind::PrepareCall(call) = data.operations[operation.index()].kind {
                    let invocation = self.summaries[unit.index()].as_ref().unwrap().invocations
                        [call.index()]
                    .unwrap();
                    if !self.needs_operation(context, invocation) {
                        return Ok(());
                    }
                    invocation
                } else {
                    let inline_return = self.context(context).kind.is_inline()
                        && self.needs_return(context)
                        && matches!(
                            data.operations[operation.index()].kind,
                            OperationKind::Return
                        );
                    if !self.needs_operation(context, operation) && !inline_return {
                        return Ok(());
                    }
                    operation
                }
            }
        };
        charge(self.input_lookup_work())?;
        // Source execution and ordinary production share one physical recipe.
        // A selected literal's production has no source-value input edges.
        if self.string_recipe(unit, operation).is_some()
            && !self.needs_execution(context, operation)
        {
            return Ok(());
        }
        let observation = self.operation_observation(context, operation, &mut charge)?;
        let mut cursor = self.input_cursor(context, operation, observation);
        loop {
            charge(self.input_step_work(&cursor))?;
            match self.next_input(&mut cursor) {
                InputStep::Dependency(InputDependency::Value(input)) if input.site == site => {
                    let lookup = self.effective_input_work(input);
                    if lookup != 0 {
                        charge(lookup)?;
                    }
                    visit(input)?;
                }
                InputStep::Done => break,
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn call_prepare(&self, context: ContextId, call: CallId) -> OpId {
        self.summaries[self.context(context).unit.index()]
            .as_ref()
            .unwrap()
            .prepares[call.index()]
        .unwrap()
    }
    pub(super) fn call_invocation(&self, context: ContextId, call: CallId) -> OpId {
        self.summaries[self.context(context).unit.index()]
            .as_ref()
            .unwrap()
            .invocations[call.index()]
        .unwrap()
    }
    pub(super) fn call_envelope(&self, context: ContextId, operation: OpId) -> Option<OpId> {
        self.summaries[self.context(context).unit.index()]
            .as_ref()
            .unwrap()
            .call_envelopes[operation.index()]
    }
    pub(super) fn needs_cell(&self, id: ContextId, cell: CellId) -> bool {
        self.cell_owner(id, cell).is_some_and(|owner| {
            self.context(owner).cells[self.cell_ordinal(cell)]
                .observation
                .is_observed()
        })
    }
    pub(super) fn needs_binding(&self, id: ContextId, cell: CellId) -> bool {
        self.cell_owner(id, cell).is_some_and(|owner| {
            let storage = self.context(owner).cells[self.cell_ordinal(cell)];
            storage.observation.is_observed() || storage.declaration
        })
    }
    pub(super) fn needs_slot(&self, _id: ContextId, record: usize, slot: u32) -> bool {
        self.slots[record][slot as usize].observation.is_observed()
    }
    pub(super) fn records(&self) -> &[&'program RecordFamily] {
        &self.records
    }
    pub(super) fn helpers(&self) -> &[&'program HelperFamily] {
        &self.helpers
    }
    pub(super) fn strings(&self) -> &[&'program StringFamily] {
        &self.strings
    }
    pub(super) fn string_operation(&self, unit: UnitId, op: OpId) -> Option<usize> {
        self.string_recipe(unit, op).map(|recipe| recipe.family)
    }
    pub(super) fn string_value(&self, unit: UnitId, value: ValueId) -> Option<usize> {
        let definition = self.program.units[unit.index()].data().values[value.index()].definition;
        self.string_operation(unit, definition)
    }
    /// Needed shared data, ordered by family index, within this physical
    /// activation only. No source definition is scanned during binding setup.
    pub(super) fn shared_strings(&self, id: ContextId) -> impl Iterator<Item = usize> + '_ {
        let context = self.context(id);
        let range = self.summaries[context.unit.index()]
            .as_ref()
            .unwrap()
            .shared_strings
            .clone();
        self.shared_strings[range]
            .iter()
            .zip(&context.shared_strings)
            .filter_map(|(&(_, family), &needed)| needed.then_some(family))
    }
    pub(super) fn needs_shared_string(&self, id: ContextId, family: usize) -> bool {
        let context = self.context(id);
        self.shared_strings
            .binary_search(&(context.unit, family))
            .ok()
            .is_some_and(|index| {
                let start = self.summaries[context.unit.index()]
                    .as_ref()
                    .unwrap()
                    .shared_strings
                    .start;
                context.shared_strings[index - start]
            })
    }
    pub(super) fn owned_cells(&self, unit: UnitId) -> &[CellId] {
        &self.owned_cells[unit.index()]
    }
    pub(super) fn cell_ordinal(&self, cell: CellId) -> usize {
        self.cell_ordinals[cell.index()]
    }
    pub(super) fn record_for_cell(&self, cell: CellId) -> Option<usize> {
        self.record_cells
            .binary_search_by_key(&cell, |entry| entry.0)
            .ok()
            .map(|i| self.record_cells[i].1)
    }
    pub(super) fn helper_for_cell(&self, cell: CellId) -> Option<usize> {
        self.helper_cells
            .binary_search_by_key(&cell, |entry| entry.0)
            .ok()
            .map(|i| self.helper_cells[i].1)
    }
    pub(super) fn record_operation(&self, unit: UnitId, op: OpId) -> Option<RecordOperation> {
        self.record_operations
            .binary_search_by_key(&(unit, op), |entry| entry.0)
            .ok()
            .map(|i| self.record_operations[i].1)
    }
    pub(super) fn helper_operation(&self, unit: UnitId, op: OpId) -> Option<HelperOperation> {
        self.helper_operations
            .binary_search_by_key(&(unit, op), |entry| entry.0)
            .ok()
            .map(|i| self.helper_operations[i].1)
    }
    pub(super) fn work(&self) -> DemandWork {
        self.work
    }
    pub(super) fn discard(self, ledger: Option<&mut BudgetLedger>) -> Result<(), BudgetError> {
        let charge = self.charge;
        drop(self);
        if let Some((domain, bytes)) = charge {
            ledger
                .expect("budgeted demand requires its original ledger")
                .release(domain, bytes)?;
        }
        Ok(())
    }

    fn string_recipe(&self, unit: UnitId, op: OpId) -> Option<StringOperation> {
        self.string_operations
            .binary_search_by_key(&(unit, op), |entry| entry.0)
            .ok()
            .map(|index| self.string_operations[index].1)
    }

    fn checked_string_recipe(
        &self,
        unit: UnitId,
        op: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<StringOperation>, DemandError> {
        if self.string_operations.is_empty() {
            return Ok(None);
        }
        budget.work((usize::BITS - self.string_operations.len().leading_zeros()) as usize)?;
        Ok(self.string_recipe(unit, op))
    }

    fn index_implementations(
        &mut self,
        implementations: Option<&'program ImplementationMap>,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let Some(implementations) = implementations else {
            return Ok(());
        };
        self.index_functions(implementations, budget)?;
        self.index_products(implementations, budget)?;
        for family in implementations.records() {
            let record = self.records.len();
            budget.push(&mut self.records, family)?;
            budget.push(&mut self.record_cells, (family.root().state, record))?;
            let slots = budget.filled(family.slots().len(), Storage::default())?;
            budget.push(&mut self.slots, slots)?;
            for projection in family.projections() {
                let kind = match projection.kind {
                    ReadOrWrite::Read => RecordOperation::Read {
                        record,
                        slot: projection.slot,
                    },
                    ReadOrWrite::Write => RecordOperation::Write {
                        record,
                        slot: projection.slot,
                    },
                };
                budget.push(
                    &mut self.record_operations,
                    (
                        (projection.operation.unit, projection.operation.operation),
                        kind,
                    ),
                )?;
            }
            for reference in family
                .handle_loads()
                .iter()
                .copied()
                .chain([family.allocation()])
            {
                budget.push(
                    &mut self.record_operations,
                    (
                        (reference.unit, reference.operation),
                        RecordOperation::ElidedHandle,
                    ),
                )?;
            }
            let initial = family.initialize();
            budget.push(
                &mut self.record_operations,
                (
                    (initial.unit, initial.operation),
                    RecordOperation::Initialize(record),
                ),
            )?;
        }
        for family in implementations.helpers() {
            let helper = self.helpers.len();
            budget.push(&mut self.helpers, family)?;
            budget.push(&mut self.helper_cells, (family.root().cell, helper))?;
            for reference in family
                .handle_loads()
                .iter()
                .copied()
                .chain([family.closure(), family.initialize()])
            {
                budget.push(
                    &mut self.helper_operations,
                    (
                        (reference.unit, reference.operation),
                        HelperOperation::Elided,
                    ),
                )?;
            }
            for call in family.calls() {
                budget.push(
                    &mut self.helper_operations,
                    ((call.caller, call.call), HelperOperation::Call(helper)),
                )?;
            }
        }
        for family in implementations.strings() {
            let string = self.strings.len();
            budget.push(&mut self.strings, family)?;
            let shared = match family.choice() {
                StringChoice::LiteralAtDefinition => false,
                StringChoice::SharedLiteral { activation } => {
                    budget.push(&mut self.shared_strings, (activation, string))?;
                    true
                }
            };
            for definition in family.definitions() {
                budget.work(1)?;
                let data = self
                    .program
                    .unit(definition.unit)
                    .ok_or_else(|| unsupported("string recipe unit"))?;
                let value = data
                    .values
                    .get(definition.value.index())
                    .ok_or_else(|| unsupported("string recipe value"))?;
                if let StringChoice::SharedLiteral { activation } = family.choice() {
                    if activation != definition.unit {
                        return Err(unsupported("string sharing across unproved activation"));
                    }
                }
                budget.push(
                    &mut self.string_operations,
                    (
                        (definition.unit, value.definition),
                        StringOperation {
                            family: string,
                            shared: shared.then_some(0),
                        },
                    ),
                )?;
            }
        }
        for len in [
            self.string_operations.len(),
            self.shared_strings.len(),
            self.record_operations.len(),
            self.helper_operations.len(),
            self.record_cells.len(),
            self.helper_cells.len(),
        ] {
            budget.work(sort_work(len)?)?;
        }
        self.string_operations.sort_unstable_by_key(|entry| entry.0);
        self.shared_strings.sort_unstable();
        for ((unit, _), operation) in &mut self.string_operations {
            if operation.shared.is_some() {
                budget.work(
                    (usize::BITS - self.shared_strings.len().max(1).leading_zeros()) as usize,
                )?;
                operation.shared = Some(
                    self.shared_strings
                        .binary_search(&(*unit, operation.family))
                        .expect("validated local string activation"),
                );
            }
        }
        self.record_operations.sort_unstable_by_key(|entry| entry.0);
        self.helper_operations.sort_unstable_by_key(|entry| entry.0);
        self.record_cells.sort_unstable_by_key(|entry| entry.0);
        self.helper_cells.sort_unstable_by_key(|entry| entry.0);
        if self.string_operations.windows(2).any(|p| p[0].0 == p[1].0)
            || self.record_operations.windows(2).any(|p| p[0].0 == p[1].0)
            || self.helper_operations.windows(2).any(|p| p[0].0 == p[1].0)
            || self.record_cells.windows(2).any(|p| p[0].0 == p[1].0)
            || self.helper_cells.windows(2).any(|p| p[0].0 == p[1].0)
        {
            return Err(unsupported("overlapping demand implementation coverage"));
        }
        Ok(())
    }

    fn summarize(&mut self, unit: UnitId, budget: &mut Budget<'_>) -> Result<(), DemandError> {
        if self.summaries[unit.index()].is_some() {
            return Ok(());
        }
        let data = self.program.units[unit.index()].data();
        let mut domains = budget.filled(data.values.len(), false)?;
        let mut effects = budget.filled(data.operations.len(), EvaluationBehavior::UNKNOWN)?;
        let mut region_owners = budget.filled(data.regions.len(), None)?;
        let mut call_envelopes = budget.filled(data.operations.len(), None)?;
        let mut prepares = budget.filled(data.calls.len(), None)?;
        let mut invocations = budget.filled(data.calls.len(), None)?;
        let mut initializers = budget.filled(
            self.owned_cells[unit.index()].len(),
            Initialization::Missing,
        )?;
        for (index, &cell) in data.parameters.iter().enumerate() {
            budget.work(1)?;
            initializers[self.cell_ordinal(cell)] = Initialization::Parameter(index as u32);
        }
        // Shared producer evidence restores domains only where the selected
        // target recipe and every storage writer justify them. This summary
        // applies to every materialization of the unit: one private callable's
        // arguments cannot qualify another closure of the same body.
        let mut roots = Vec::new();
        let raw_domains = if let Some(uses) = self.uses {
            for operation in &data.operations {
                budget.work(1)?;
                if matches!(
                    operation.kind,
                    OperationKind::Load(_)
                        | OperationKind::Call(_)
                        | OperationKind::Select { .. }
                        | OperationKind::ShortCircuit { .. }
                ) {
                    if let Some(value) = operation.result {
                        budget.push(&mut roots, Subject::Value { unit, value })?;
                    }
                }
            }
            if roots.is_empty() {
                None
            } else {
                Some(DomainProof::build(
                    self.program,
                    uses,
                    &roots,
                    DomainInputs::empty()
                        .with_records(&self.records)
                        .with_execution(self.contract.execution),
                    &super::javascript::JavaScriptRecipes,
                    budget,
                )?)
            }
        } else {
            None
        };
        for (index, operation) in data.operations.iter().enumerate() {
            budget.work(
                1usize
                    .checked_add(data.operands(operation.operands).unwrap().len())
                    .and_then(|count| {
                        count.checked_add(match operation.kind {
                            OperationKind::Call(call) => {
                                data.calls[call.index()].arguments.len as usize
                            }
                            _ => 0,
                        })
                    })
                    .ok_or_else(|| unsupported("demand work capacity"))?,
            )?;
            effects[index] =
                facts::operation_evaluation_behavior(self.program, data, operation, &domains);
            let id = OpId::from_index(index).unwrap();
            if let Some(value) = operation.result {
                domains[value.index()] =
                    facts::primitive_result_domain(self.program, data, operation, &domains);
                if !domains[value.index()] {
                    if let Some(proof) = &raw_domains {
                        domains[value.index()] =
                            proof.primitive(Subject::Value { unit, value }, budget)?;
                    }
                }
                if self.checked_string_recipe(unit, id, budget)?.is_some() {
                    domains[value.index()] = true;
                } else if let Some(HelperOperation::Call(helper)) = self.helper_operation(unit, id)
                {
                    domains[value.index()] = self.helpers[helper].return_primitive();
                }
            }
            if let OperationKind::Initialize(cell) = operation.kind {
                let initial = &mut initializers[self.cell_ordinal(cell)];
                *initial = if matches!(initial, Initialization::Missing) {
                    Initialization::Operation(OpId::from_index(index).unwrap())
                } else {
                    Initialization::Multiple
                };
            }
            if let OperationKind::Try {
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
            } = operation.kind
            {
                initializers[self.cell_ordinal(cell)] = Initialization::Catch(region);
            }
        }
        if let Some(proof) = raw_domains {
            proof.discard(budget)?;
        }
        super::raw_domains::Admission::release(budget, roots)?;
        let mut stack = budget.vector(data.calls.len())?;
        for region in &data.regions {
            stack.clear();
            for &id in &region.operations {
                budget.work(1)?;
                let op = &data.operations[id.index()];
                call_envelopes[id.index()] = stack.last().copied();
                match op.kind {
                    OperationKind::PrepareCall(call) => {
                        prepares[call.index()] = Some(id);
                        stack.push(id);
                    }
                    OperationKind::Call(call) => {
                        if invocations[call.index()].replace(id).is_some() {
                            return Err(unsupported("demand duplicate call invocation"));
                        }
                        let prepare = prepares[call.index()]
                            .ok_or_else(|| unsupported("demand call without prepare"))?;
                        if stack.pop() != Some(prepare) {
                            return Err(unsupported("demand unbalanced call schedule"));
                        }
                    }
                    _ => {}
                }
                for child in op.kind.child_regions() {
                    budget.work(1)?;
                    region_owners[child.index()] = Some(id);
                }
            }
            if !stack.is_empty() {
                return Err(unsupported("demand unfinished call schedule"));
            }
        }
        let stack_bytes = stack.capacity() as u64 * size_of::<OpId>() as u64;
        drop(stack);
        budget.release(stack_bytes)?;
        let parents = budget.filled(data.regions.len(), None)?;
        let positions = budget.filled(data.operations.len(), 0usize)?;
        let dominance = StructuredDominance::build(data, parents, positions, |n| budget.work(n))?;
        let domain_bytes = domains.capacity() as u64 * size_of::<bool>() as u64;
        drop(domains);
        budget.release(domain_bytes)?;
        if !self.shared_strings.is_empty() {
            budget.work(2 * (usize::BITS - self.shared_strings.len().leading_zeros()) as usize)?;
        }
        let shared_start = self
            .shared_strings
            .partition_point(|&(owner, _)| owner < unit);
        let shared_end = self
            .shared_strings
            .partition_point(|&(owner, _)| owner <= unit);
        self.summaries[unit.index()] = Some(UnitSummary {
            generic_product_transport: false,
            named_context: None,
            shared_strings: shared_start..shared_end,
            effects,
            dominance,
            region_owners,
            call_envelopes,
            prepares,
            invocations,
            initializers,
        });
        Ok(())
    }

    fn add_context(
        &mut self,
        unit: UnitId,
        parent: Option<ContextId>,
        creation: Option<OpId>,
        kind: ContextKind,
        budget: &mut Budget<'_>,
    ) -> Result<ContextId, DemandError> {
        let id = self.allocate_context(unit, parent, creation, kind, budget)?;
        self.seed_context(id, budget)?;
        Ok(id)
    }

    fn allocate_context(
        &mut self,
        unit: UnitId,
        parent: Option<ContextId>,
        creation: Option<OpId>,
        kind: ContextKind,
        budget: &mut Budget<'_>,
    ) -> Result<ContextId, DemandError> {
        self.summarize(unit, budget)?;
        if !kind.is_inline()
            && self.summaries[unit.index()]
                .as_ref()
                .unwrap()
                .named_context
                .is_some()
        {
            return Err(unsupported("multiply materialized semantic function unit"));
        }
        let data = self.program.units[unit.index()].data();
        let (product_banks, product_snapshots) = self.allocate_product_context(unit, budget)?;
        let context = Context {
            unit,
            parent,
            creation,
            product_banks,
            product_snapshots,
            kind,
            values: budget.filled(data.values.len(), ObservationDemand::Discarded)?,
            operations: budget.filled(data.operations.len(), false)?,
            execution: budget.filled(data.operations.len(), false)?,
            production: budget.filled(data.operations.len(), false)?,
            dependencies: budget.filled(data.operations.len(), None)?,
            regions: budget.filled(data.regions.len(), 0u8)?,
            region_results: budget.filled(data.regions.len(), ObservationDemand::Discarded)?,
            cells: budget.filled(self.owned_cells[unit.index()].len(), Storage::default())?,
            children: budget.filled(data.operations.len(), None)?,
            return_observation: if kind.is_inline() {
                ObservationDemand::Discarded
            } else {
                ObservationDemand::Exact
            },
            shared_strings: budget.filled(
                self.summaries[unit.index()]
                    .as_ref()
                    .unwrap()
                    .shared_strings
                    .len(),
                false,
            )?,
        };
        let id = ContextId(self.contexts.len());
        budget.push(&mut self.contexts, context)?;
        if !kind.is_inline() {
            self.summaries[unit.index()].as_mut().unwrap().named_context = Some(id);
        }
        if let (Some(parent), Some(creation)) = (parent, creation) {
            self.contexts[parent.index()].children[creation.index()] = Some(id);
        }
        Ok(id)
    }

    fn seed_context(&mut self, id: ContextId, budget: &mut Budget<'_>) -> Result<(), DemandError> {
        let unit = self.context(id).unit;
        let kind = self.context(id).kind;
        let data = self.program.units[unit.index()].data();
        // Retain direct callable signature/catch binding structure separately
        // from content demand. Inline parameters have no surviving callable ABI.
        if !kind.is_inline() {
            for &cell in &data.parameters {
                budget.work(1)?;
                let ordinal = self.cell_ordinal(cell);
                self.contexts[id.index()].cells[ordinal].declaration = true;
            }
        }
        for (index, op) in data.operations.iter().enumerate() {
            budget.work(1)?;
            if let OperationKind::Try {
                catch: Some((Some(cell), _)),
                ..
            }
            | OperationKind::ForIn { key: cell, .. }
            | OperationKind::ForOf { item: cell, .. } = op.kind
            {
                let ordinal = self.cell_ordinal(cell);
                self.contexts[id.index()].cells[ordinal].declaration = true;
            }
            let operation = OpId::from_index(index).unwrap();
            if matches!(
                self.helper_operation(unit, operation),
                Some(HelperOperation::Elided)
            ) || matches!(
                self.record_operation(unit, operation),
                Some(RecordOperation::ElidedHandle)
            ) || self.elided_log_lookup(unit, operation, budget)?
            {
                continue;
            }
            if let Some(HelperOperation::Call(helper)) = self.helper_operation(unit, operation) {
                let body = self.helpers[helper].root().body;
                self.add_context(
                    body,
                    Some(id),
                    Some(operation),
                    ContextKind::Inline {
                        helper,
                        call: operation,
                    },
                    budget,
                )?;
                continue;
            }
            if let Some(choice) = self.record_operation(unit, operation) {
                match choice {
                    RecordOperation::Initialize(record) => {
                        for slot in 0..self.slots[record].len() {
                            self.wait_for(
                                StorageId::Slot(record, slot as u32),
                                id,
                                operation,
                                budget,
                            )?;
                        }
                    }
                    RecordOperation::Write { record, slot } => {
                        self.wait_for(StorageId::Slot(record, slot), id, operation, budget)?
                    }
                    RecordOperation::Read { .. } | RecordOperation::ElidedHandle => {}
                }
                continue;
            }
            if self.seed_product_operation(id, operation, budget)? {
                continue;
            }
            match op.kind {
                // Address validation is ordered execution, independent of the
                // final field value or any following store being observed.
                OperationKind::CheckPlace(_) | OperationKind::PrepareReference { .. } => {
                    self.need_operation(id, operation, budget)?;
                }
                OperationKind::Initialize(cell) => {
                    let storage = self.cell_storage(id, cell, budget)?;
                    self.wait_for(storage, id, operation, budget)?;
                }
                OperationKind::Store(place) => {
                    if self.seed_location_store(id, operation, place, budget)? {
                        continue;
                    }
                    if let Place::Cell(cell) = data.places[place.index()] {
                        if self.isolated_cell(cell)
                            && self.initialized(id, cell, operation, budget)?
                        {
                            let storage = self.cell_storage(id, cell, budget)?;
                            self.wait_for(storage, id, operation, budget)?;
                        } else {
                            self.need_operation(id, operation, budget)?;
                        }
                    } else {
                        // A nominal source annotation does not prove that a
                        // projected read/update of incoming storage is inert.
                        // Preserve evaluation until a product-domain proof is
                        // available; the common cursor retains the mutable root.
                        self.need_operation(id, operation, budget)?;
                    }
                }
                OperationKind::ShortCircuit { .. }
                | OperationKind::Select { .. }
                | OperationKind::If { .. }
                | OperationKind::Block(_) => {}
                OperationKind::Return if kind.is_inline() => {}
                _ => {
                    let mut behavior = match kind {
                        ContextKind::Inline { helper, .. } => {
                            self.helpers[helper]
                                .operation_facts(operation)
                                .expect("complete inline family operation evidence")
                                .behavior
                        }
                        ContextKind::Named => {
                            self.summaries[unit.index()].as_ref().unwrap().effects[index]
                        }
                    };
                    if let OperationKind::Load(place) = op.kind {
                        if let Place::Cell(cell) = data.places[place.index()] {
                            if self.isolated_cell(cell)
                                && self.initialized(id, cell, operation, budget)?
                            {
                                behavior.may_throw = false;
                            }
                        }
                    }
                    // Inlining preserves the certified operation behavior.
                    // Private storage proves access safety; a conversion of
                    // its opaque contents can still throw or invoke user code.
                    if self.stripped_log_call(unit, &op.kind)
                        && op.result.is_none_or(|result| {
                            matches!(
                                self.program.types[data.values[result.index()].ty.index()],
                                Type::Void
                            )
                        })
                    {
                        behavior = EvaluationBehavior::TOTAL;
                    }
                    if required(behavior) {
                        self.need_operation(id, operation, budget)?;
                    }
                }
            }
        }
        if self.mode == DemandMode::Preserve {
            self.preserve_context(id, budget)?;
        }
        Ok(())
    }

    fn preserve_context(
        &mut self,
        context: ContextId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        self.contexts[context.index()].return_observation = ObservationDemand::Exact;
        for index in 0..self.owned_cells[unit.index()].len() {
            budget.work(1)?;
            let cell = self.owned_cells[unit.index()][index];
            if self.helper_for_cell(cell).is_none()
                && self.record_for_cell(cell).is_none()
                && self.product_for_cell(cell).is_none()
            {
                self.need_cell(context, cell, budget)?;
            }
        }
        self.preserve_products(context, budget)?;
        for record in 0..self.records.len() {
            budget.work(1)?;
            if self.records[record].initialize().unit == unit {
                for slot in 0..self.slots[record].len() {
                    self.need_storage(StorageId::Slot(record, slot as u32), budget)?;
                }
            }
        }
        for (index, op) in data.operations.iter().enumerate() {
            budget.work(1)?;
            let id = OpId::from_index(index).unwrap();
            if matches!(
                self.helper_operation(unit, id),
                Some(HelperOperation::Elided)
            ) || matches!(
                self.record_operation(unit, id),
                Some(RecordOperation::ElidedHandle)
            ) || self.elided_log_lookup(unit, id, budget)?
            {
                continue;
            }
            if !(self.context(context).kind.is_inline() && matches!(op.kind, OperationKind::Return))
            {
                self.need_operation(context, id, budget)?;
            }
            if let Some(value) = op.result {
                self.need_value(context, value, ObservationDemand::Exact, budget)?;
            }
            if self.context(context).kind.is_inline() && matches!(op.kind, OperationKind::Return) {
                for &value in data.operands(op.operands).unwrap() {
                    self.need_value(context, value, ObservationDemand::Exact, budget)?;
                }
            }
        }
        for index in 0..data.regions.len() {
            self.need_region_result(context, RegionId::from_index(index).unwrap(), budget)?;
        }
        Ok(())
    }

    pub(super) fn cell_owner(&self, context: ContextId, cell: CellId) -> Option<ContextId> {
        self.cell_owner_visited(context, cell, || Ok::<_, std::convert::Infallible>(()))
            .unwrap()
    }
    pub(super) fn cell_owner_visited<E>(
        &self,
        mut context: ContextId,
        cell: CellId,
        mut visit: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<ContextId>, E> {
        visit()?;
        let owner = self.program.cells[cell.index()].owner;
        if let Some(root) = self.module_context(owner) {
            return Ok(Some(root));
        }
        loop {
            visit()?;
            let data = self.context(context);
            if data.unit == owner {
                return Ok(Some(context));
            }
            let Some(parent) = data.parent else {
                return Ok(None);
            };
            context = parent;
        }
    }
    fn cell_storage(
        &self,
        mut context: ContextId,
        cell: CellId,
        budget: &mut Budget<'_>,
    ) -> Result<StorageId, DemandError> {
        let owner = self.program.cells[cell.index()].owner;
        budget.work(1)?;
        if let Some(root) = self.module_context(owner) {
            return Ok(StorageId::Cell(root, self.cell_ordinal(cell)));
        }
        loop {
            budget.work(1)?;
            let data = self.context(context);
            if data.unit == owner {
                return Ok(StorageId::Cell(context, self.cell_ordinal(cell)));
            }
            context = data
                .parent
                .ok_or_else(|| unsupported("demand capture outside activation"))?;
        }
    }
    pub(super) fn is_reference_parameter(&self, cell: CellId) -> bool {
        self.program.is_reference_parameter(cell)
    }
    fn isolated_cell(&self, cell: CellId) -> bool {
        !self.resource.imported(cell)
            && self.program.cells[cell.index()].binding != CellBinding::Foreign
            // Sloppy functions can expose parameter writes through mapped
            // arguments, including lexical descendants and escaped views. A
            // lexical source cell is not proof of exclusive physical storage.
            // Keep Script parameters Exact until an unmapped-activation proof
            // is available. Module functions execute strictly and do not alias.
            && !(self.contract.execution == crate::compilation_contract::JavaScriptExecution::Script
                && matches!(self.program.cells[cell.index()].binding, CellBinding::Parameter(_)))
            && !self.program.is_reference_parameter(cell)
            && self.uses.map_or(!self.unindexed_references, |uses| {
                uses.cell(cell)
                    .is_some_and(|users| !users.reference_exposed())
            })
    }
    fn initialized(
        &self,
        context: ContextId,
        cell: CellId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<bool, DemandError> {
        if self.resource.imported(cell) {
            // Import linkage owns this storage. A source annotation alone does
            // not prove that its physical read is initialized or unobservable.
            return Ok(false);
        }
        if self.context(context).kind.is_inline() {
            return Ok(true);
        }
        let owner = self.program.cells[cell.index()].owner;
        let mut current = context;
        let mut at = operation;
        while self.context(current).unit != owner {
            budget.work(1)?;
            let source = self.context(current);
            // An imported module cell is valid shared storage, but another
            // module's lexical schedule does not prove it initialized. Keep
            // the possible TDZ observation until a cross-module activation
            // proof establishes more than source ownership alone.
            if source.parent.is_none() && self.module_context(owner).is_some() {
                return Ok(false);
            }
            at = source
                .creation
                .ok_or_else(|| unsupported("demand capture creator"))?;
            current = source
                .parent
                .ok_or_else(|| unsupported("demand capture owner"))?;
        }
        let data = self.program.units[owner.index()].data();
        let summary = self.summaries[owner.index()].as_ref().unwrap();
        match summary.initializers[self.cell_ordinal(cell)] {
            Initialization::Parameter(_) => Ok(true),
            Initialization::Operation(initial) => summary
                .dominance
                .after(data, initial, at, |n| budget.work(n)),
            Initialization::Catch(region) => {
                let mut cursor = Some(data.operations[at.index()].region);
                while let Some(current) = cursor {
                    budget.work(1)?;
                    if current == region {
                        return Ok(true);
                    }
                    cursor = data.regions[current.index()].parent;
                }
                Ok(false)
            }
            Initialization::Missing | Initialization::Multiple => Ok(false),
        }
    }
    fn storage(&self, id: StorageId) -> &Storage {
        match id {
            StorageId::Cell(context, ordinal) => &self.context(context).cells[ordinal],
            StorageId::Slot(record, slot) => &self.slots[record][slot as usize],
            StorageId::Product(context, bank, slot) => {
                &self.context(context).product_banks[bank].slots[slot as usize]
            }
        }
    }
    fn storage_mut(&mut self, id: StorageId) -> &mut Storage {
        match id {
            StorageId::Cell(context, ordinal) => &mut self.contexts[context.index()].cells[ordinal],
            StorageId::Slot(record, slot) => &mut self.slots[record][slot as usize],
            StorageId::Product(context, bank, slot) => {
                &mut self.contexts[context.index()].product_banks[bank].slots[slot as usize]
            }
        }
    }
    fn wait_for(
        &mut self,
        storage: StorageId,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(1)?;
        let observation = self.storage(storage).observation;
        // Exact cannot upgrade. Keep its old allocation-free fast path.
        if observation != ObservationDemand::Exact {
            let next = self.storage(storage).writers;
            let index = self.writers.len();
            budget.push(
                &mut self.writers,
                Writer {
                    context,
                    operation,
                    next,
                },
            )?;
            self.storage_mut(storage).writers = Some(index);
        }
        if observation.is_observed() {
            self.product_writer_input(storage, context, operation, budget)?;
            self.need_operation(context, operation, budget)?;
            self.visit_shared_dependencies(context, operation, budget)?;
        }
        Ok(())
    }
    fn need_storage(&mut self, id: StorageId, budget: &mut Budget<'_>) -> Result<(), DemandError> {
        self.observe_storage(id, ObservationDemand::Exact, budget)
    }
    fn observe_storage(
        &mut self,
        id: StorageId,
        observation: ObservationDemand,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(1)?;
        let old = self.storage(id).observation;
        let joined = old.join(observation);
        if old != joined {
            self.storage_mut(id).observation = joined;
            budget.push(&mut self.pending, Pending::Storage(id))?;
        }
        Ok(())
    }
    fn need_cell(
        &mut self,
        context: ContextId,
        cell: CellId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        self.observe_cell(context, cell, ObservationDemand::Exact, budget)
    }
    fn observe_cell(
        &mut self,
        context: ContextId,
        cell: CellId,
        observation: ObservationDemand,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        if self.program.cells[cell.index()].binding == CellBinding::Foreign
            || self.resource.imported(cell)
        {
            return Ok(());
        }
        let observation = if self.isolated_cell(cell) {
            observation
        } else {
            ObservationDemand::Exact
        };
        let storage = self.cell_storage(context, cell, budget)?;
        self.observe_storage(storage, observation, budget)
    }
    fn need_value(
        &mut self,
        context: ContextId,
        value: ValueId,
        observation: ObservationDemand,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(1)?;
        if observation == ObservationDemand::Discarded {
            return Ok(());
        }
        budget.work((usize::BITS - self.product_value_index.len().leading_zeros()) as usize)?;
        if self
            .product_for_value(self.context(context).unit, value)
            .is_some()
        {
            return self.need_whole_snapshot(context, value, budget);
        }
        let old = self.context(context).values[value.index()];
        let joined = old.join(observation);
        if old != joined {
            self.contexts[context.index()].values[value.index()] = joined;
            budget.push(&mut self.pending, Pending::Value(context, value))?;
        }
        Ok(())
    }
    fn need_operation(
        &mut self,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        self.need_requirement(context, operation, Requirement::Execute, budget)
    }
    fn need_production(
        &mut self,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        self.need_requirement(context, operation, Requirement::Produce, budget)
    }
    fn need_requirement(
        &mut self,
        context: ContextId,
        operation: OpId,
        requirement: Requirement,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(1)?;
        let flag = match requirement {
            Requirement::Execute => {
                &mut self.contexts[context.index()].execution[operation.index()]
            }
            Requirement::Produce => {
                &mut self.contexts[context.index()].production[operation.index()]
            }
        };
        if !*flag {
            *flag = true;
            self.contexts[context.index()].operations[operation.index()] = true;
            budget.push(
                &mut self.pending,
                Pending::Operation(context, operation, requirement),
            )?;
        }
        Ok(())
    }
    fn need_region_result(
        &mut self,
        context: ContextId,
        region: RegionId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        self.observe_region(context, region, ObservationDemand::Exact, budget)
    }
    fn observe_region(
        &mut self,
        context: ContextId,
        region: RegionId,
        observation: ObservationDemand,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(1)?;
        let old = self.context(context).region_results[region.index()];
        let joined = old.join(observation);
        if old != joined {
            self.contexts[context.index()].region_results[region.index()] = joined;
            if let Some(input) = self.region_input(context, region) {
                self.need_value(context, input.value, input.observation, budget)?;
            }
        }
        Ok(())
    }
    fn visit_value(
        &mut self,
        context: ContextId,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        let definition = data.values[value.index()].definition;
        self.need_production(context, definition, budget)?;
        if self
            .checked_string_recipe(unit, definition, budget)?
            .is_some()
        {
            // Only production edges are replaced. Independently queued source
            // execution keeps its own region/results/argument dependencies.
            return Ok(());
        }
        let observation = self.context(context).values[value.index()];
        // Production may have run under an earlier weaker observation.
        self.visit_shared_dependencies(context, definition, budget)?;
        match data.operations[definition.index()].kind {
            OperationKind::ShortCircuit { right, .. } => {
                self.observe_region(context, right, observation, budget)?
            }
            OperationKind::Select { yes, no } => {
                self.observe_region(context, yes, observation, budget)?;
                self.observe_region(context, no, observation, budget)?;
            }
            _ => {}
        }
        if let Some(HelperOperation::Call(helper)) = self.helper_operation(unit, definition) {
            let child = self
                .child(context, definition)
                .ok_or_else(|| unsupported("demand inline context missing"))?;
            self.contexts[child.index()].return_observation =
                self.context(child).return_observation.join(observation);
            let Some(tail) = self.helpers[helper].tail_return() else {
                // Proved Void fallthrough has no semantic return operand. Its
                // body effects still belong to the existing context worklist.
                return Ok(());
            };
            budget.work(self.input_lookup_work())?;
            let mut cursor = self.input_cursor(child, tail, self.context(child).return_observation);
            loop {
                budget.work(self.input_step_work(&cursor))?;
                match self.next_input(&mut cursor) {
                    InputStep::Dependency(InputDependency::Value(input)) => {
                        self.need_value(child, input.value, input.observation, budget)?;
                    }
                    InputStep::Done => break,
                    _ => {}
                }
            }
        }
        Ok(())
    }
    fn visit_storage(&mut self, id: StorageId, budget: &mut Budget<'_>) -> Result<(), DemandError> {
        let mut writer = self.storage(id).writers;
        while let Some(index) = writer {
            budget.work(1)?;
            let pending = self.writers[index];
            writer = pending.next;
            if let StorageId::Slot(record, slot) = id {
                let unit = self.context(pending.context).unit;
                if matches!(self.record_operation(unit,pending.operation),Some(RecordOperation::Initialize(found)) if found==record)
                {
                    if let Some(input) =
                        self.record_slot_input(pending.operation, record, slot as usize)
                    {
                        self.need_value(
                            pending.context,
                            input.value,
                            ObservationDemand::Exact,
                            budget,
                        )?;
                    }
                }
            }
            self.product_writer_input(id, pending.context, pending.operation, budget)?;
            self.need_operation(pending.context, pending.operation, budget)?;
            self.visit_shared_dependencies(pending.context, pending.operation, budget)?;
        }
        if let StorageId::Product(context, bank, slot) = id {
            self.product_parameter_input(context, bank, slot, budget)?;
        }
        if let StorageId::Cell(context, ordinal) = id {
            let unit = self.context(context).unit;
            if self.context(context).kind.is_inline() {
                budget.work(1)?;
                if let Initialization::Parameter(position) =
                    self.summaries[unit.index()].as_ref().unwrap().initializers[ordinal]
                {
                    let actual = self.inline_actual(context, position);
                    match *actual.argument {
                        CallArgument::Value(value) => {
                            self.need_value(
                                actual.caller,
                                value,
                                self.storage(id).observation,
                                budget,
                            )?;
                        }
                        CallArgument::Reference(place) => {
                            self.need_place_location(
                                PlaceLocation {
                                    context: actual.caller,
                                    place,
                                },
                                LocationUse::Preparation {
                                    call: actual.call,
                                    position: actual.position,
                                },
                                budget,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn visit_execution(
        &mut self,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        self.retain_envelope(context, operation, Requirement::Execute, budget)?;
        self.visit_shared_dependencies(context, operation, budget)
    }
    fn visit_production(
        &mut self,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        self.retain_envelope(context, operation, Requirement::Produce, budget)?;
        let unit = self.context(context).unit;
        if let Some(recipe) = self.checked_string_recipe(unit, operation, budget)? {
            if let Some(shared) = recipe.shared {
                let start = self.summaries[unit.index()]
                    .as_ref()
                    .unwrap()
                    .shared_strings
                    .start;
                self.contexts[context.index()].shared_strings[shared - start] = true;
            }
            // Knowing the exact value changes the selected production recipe,
            // never the independently rooted source evaluation. In Preserve
            // mode Execute still visits original operands even for this op.
            return Ok(());
        }
        self.visit_shared_dependencies(context, operation, budget)
    }
    fn retain_envelope(
        &mut self,
        context: ContextId,
        operation: OpId,
        requirement: Requirement,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        let op = &data.operations[operation.index()];
        // Each retained child execution retains the source gate, never hoists
        // the child to the parent's unconditional schedule.
        let mut region = op.region;
        while self.context(context).regions[region.index()] & requirement.bit() == 0 {
            self.contexts[context.index()].regions[region.index()] |= requirement.bit();
            let parent =
                self.summaries[unit.index()].as_ref().unwrap().region_owners[region.index()];
            let Some(parent) = parent else {
                break;
            };
            self.need_requirement(context, parent, requirement, budget)?;
            region = data.operations[parent.index()].region;
        }
        if let Some(prepare) = self.summaries[unit.index()]
            .as_ref()
            .unwrap()
            .call_envelopes[operation.index()]
        {
            self.need_requirement(context, prepare, requirement, budget)?;
        }
        if let ContextKind::Inline { call, .. } = self.context(context).kind {
            self.need_requirement(
                self.context(context).parent.unwrap(),
                call,
                requirement,
                budget,
            )?;
        }
        Ok(())
    }
    fn input_lookup_work(&self) -> usize {
        1 + [
            self.record_operations.len(),
            self.helper_operations.len(),
            self.string_operations.len(),
            self.product_operations.len(),
            self.product_cell_index.len(),
            self.product_value_index.len(),
            self.function_calls.len(),
        ]
        .into_iter()
        .map(|len| (usize::BITS - len.leading_zeros()) as usize)
        .sum::<usize>()
    }
    fn input_cursor(
        &self,
        context: ContextId,
        operation: OpId,
        observation: ObservationDemand,
    ) -> InputCursor {
        let unit = self.context(context).unit;
        let recipe = if let Some(product) = self.product_operation(unit, operation) {
            InputRecipe::Product(*product)
        } else if let Some(record) = self.record_operation(unit, operation) {
            match record {
                RecordOperation::ElidedHandle => InputRecipe::Empty,
                RecordOperation::Initialize(record) => InputRecipe::RecordInitialize(record),
                RecordOperation::Read { record, slot } => InputRecipe::RecordRead { record, slot },
                RecordOperation::Write { record, slot } => {
                    InputRecipe::RecordWrite { record, slot }
                }
            }
        } else if let Some(helper) = self.helper_operation(unit, operation) {
            match helper {
                HelperOperation::Elided => InputRecipe::Empty,
                HelperOperation::Call(_) => {
                    let OperationKind::Call(call) =
                        self.program.units[unit.index()].data().operations[operation.index()].kind
                    else {
                        unreachable!("checked helper call")
                    };
                    InputRecipe::InlineCall {
                        child: self
                            .child(context, operation)
                            .expect("demand inline context"),
                        prepare: self.call_prepare(context, call),
                    }
                }
            }
        } else {
            InputRecipe::Ordinary
        };
        let product_snapshot = match recipe {
            InputRecipe::Product(ProductOperationKind::Construct { value }) => self
                .context(context)
                .product_snapshots
                .binary_search_by_key(&value, |snapshot| snapshot.value)
                .ok(),
            _ => None,
        };
        InputCursor {
            observation,
            product_snapshot,
            component: 0,
            context,
            operation,
            recipe,
            stage: 0,
            index: 0,
            place: None,
        }
    }
    fn region_input(&self, context: ContextId, region: RegionId) -> Option<EffectiveValueUse> {
        self.program.units[self.context(context).unit.index()]
            .data()
            .regions[region.index()]
        .result
        .map(|value| EffectiveValueUse {
            value,
            site: EffectiveUseSite::RegionResult(region),
            role: EffectiveUseRole::RegionResult,
            observation: self.context(context).region_results[region.index()],
        })
    }
    fn record_slot_input(
        &self,
        operation: OpId,
        record: usize,
        slot: usize,
    ) -> Option<EffectiveValueUse> {
        self.records[record].slots()[slot]
            .initial_value
            .map(|value| EffectiveValueUse {
                value,
                site: EffectiveUseSite::Operation(operation),
                observation: ObservationDemand::Exact,
                role: EffectiveUseRole::RecordSlot {
                    record,
                    slot: slot as u32,
                },
            })
    }
    fn inline_parameter_input(&self, context: ContextId, position: u32) -> EffectiveValueUse {
        let (_, value) = self.inline_parameter_source(context, position);
        let ContextKind::Inline { call, .. } = self.context(context).kind else {
            unreachable!("inline parameter")
        };
        EffectiveValueUse {
            value,
            site: EffectiveUseSite::Operation(call),
            role: EffectiveUseRole::InlineParameter { context, position },
            observation: self.context(context).cells[self.cell_ordinal(
                self.program.units[self.context(context).unit.index()]
                    .data()
                    .parameters[position as usize],
            )]
            .observation,
        }
    }
    fn place_input(
        &self,
        site: EffectiveUseSite,
        place: PlaceId,
        cursor: &mut InputCursor,
    ) -> InputStep {
        let data = self.program.units[self.context(cursor.context).unit.index()].data();
        let root = *cursor.place.get_or_insert(place);
        let selected = &data.places[root.index()];
        if let Place::Field { base, .. } = selected {
            // One cursor step follows one admitted projection edge. The same
            // walk drives demand and placement occurrences without path copies.
            debug_assert!(base.index() < root.index());
            cursor.place = Some(*base);
            return InputStep::Skip;
        }
        let (value, role) = match (selected, cursor.index) {
            (Place::Cell(cell), 0) => {
                cursor.index += 1;
                if self.is_reference_parameter(*cell)
                    || self.product_for_cell(*cell).is_some()
                    || matches!(
                        data.operations[cursor.operation.index()].kind,
                        OperationKind::PrepareReference { .. }
                    )
                {
                    let usage = match data.operations[cursor.operation.index()].kind {
                        OperationKind::CheckPlace(_) => LocationUse::CheckPlace {
                            operation: cursor.operation,
                        },
                        OperationKind::PrepareReference { call, position } => {
                            LocationUse::Preparation { call, position }
                        }
                        OperationKind::Store(_) => LocationUse::Write,
                        _ => LocationUse::Read,
                    };
                    return InputStep::Dependency(InputDependency::Location(
                        PlaceLocation {
                            context: cursor.context,
                            place,
                        },
                        usage,
                    ));
                }
                let operation = &data.operations[cursor.operation.index()];
                let observation = if root == place
                    && cursor.observation.is_observed()
                    && matches!(
                        operation.kind,
                        OperationKind::Load(_) | OperationKind::Store(_)
                    ) {
                    cursor.observation
                } else {
                    ObservationDemand::Exact
                };
                return InputStep::Dependency(InputDependency::Cell(*cell, observation));
            }
            (
                Place::Value(receiver)
                | Place::Member { receiver, .. }
                | Place::Index { receiver, .. },
                0,
            ) => (*receiver, EffectiveUseRole::PlaceReceiver(place)),
            (Place::Index { key, .. }, 1) => (*key, EffectiveUseRole::PlaceKey(place)),
            _ => return InputStep::Done,
        };
        cursor.index += 1;
        InputStep::Dependency(InputDependency::Value(EffectiveValueUse {
            value,
            site,
            role,
            observation: ObservationDemand::Exact,
        }))
    }
    fn next_input(&self, cursor: &mut InputCursor) -> InputStep {
        let context = cursor.context;
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        let operation = &data.operations[cursor.operation.index()];
        let site = EffectiveUseSite::Operation(cursor.operation);
        let input_observation = if matches!(cursor.recipe, InputRecipe::Ordinary) {
            self.operand_observation(operation, cursor.observation)
        } else {
            ObservationDemand::Exact
        };
        let input = |value, role| {
            InputStep::Dependency(InputDependency::Value(EffectiveValueUse {
                value,
                site,
                role,
                observation: input_observation,
            }))
        };
        match cursor.recipe {
            InputRecipe::Empty => InputStep::Done,
            InputRecipe::Product(product) => self.next_product_input(cursor, product),
            InputRecipe::RecordInitialize(record) => {
                let slot = cursor.index;
                if slot == self.slots[record].len() {
                    return InputStep::Done;
                }
                cursor.index += 1;
                if self.slots[record][slot].observation.is_observed() {
                    if let Some(input) = self.record_slot_input(cursor.operation, record, slot) {
                        return InputStep::Dependency(InputDependency::Value(input));
                    }
                }
                InputStep::Skip
            }
            InputRecipe::RecordRead { record, slot }
            | InputRecipe::RecordWrite { record, slot } => {
                if cursor.stage == 0 {
                    cursor.stage = 1;
                    return InputStep::Dependency(InputDependency::Slot(record, slot));
                }
                if matches!(cursor.recipe, InputRecipe::RecordRead { .. }) {
                    return InputStep::Done;
                }
                let operands = data.operands(operation.operands).unwrap();
                let Some(&value) = operands.get(cursor.index) else {
                    return InputStep::Done;
                };
                let position = cursor.index;
                cursor.index += 1;
                input(value, EffectiveUseRole::Operand(position as u32))
            }
            InputRecipe::InlineCall { child, prepare } => {
                if cursor.stage == 0 {
                    cursor.stage = 1;
                    return InputStep::Dependency(InputDependency::Prepare(prepare));
                }
                self.next_inline_argument(cursor, child)
            }
            InputRecipe::Ordinary => match cursor.stage {
                0 => {
                    let dependency = match operation.kind {
                        OperationKind::Load(place)
                        | OperationKind::Store(place)
                        | OperationKind::CheckPlace(place) => {
                            match self.place_input(site, place, cursor) {
                                InputStep::Done => None,
                                step => return step,
                            }
                        }
                        OperationKind::PrepareReference { call, position } => {
                            let CallArgument::Reference(place) = data
                                .arguments(data.calls[call.index()].arguments)
                                .unwrap()[position as usize]
                            else {
                                unreachable!("verified reference preparation")
                            };
                            match self.place_input(site, place, cursor) {
                                InputStep::Done => None,
                                step => return step,
                            }
                        }
                        OperationKind::Initialize(cell) if cursor.index == 0 => {
                            Some(InputDependency::Cell(cell, cursor.observation))
                        }
                        _ => None,
                    };
                    if let Some(dependency) = dependency {
                        cursor.index += 1;
                        return InputStep::Dependency(dependency);
                    }
                    cursor.stage = 1;
                    cursor.index = 0;
                    cursor.place = None;
                    InputStep::Skip
                }
                1 => {
                    if let OperationKind::Call(call) = operation.kind {
                        if !self.stripped_log_call(unit, &operation.kind) {
                            let prepare_site =
                                EffectiveUseSite::Operation(self.call_prepare(context, call));
                            let dependency = match data.calls[call.index()].target {
                                CallTarget::Value { callee, .. } if cursor.index == 0 => {
                                    Some(InputDependency::Value(EffectiveValueUse {
                                        value: callee,
                                        site: prepare_site,
                                        observation: ObservationDemand::Exact,
                                        role: if self.call_has_empty_transport(context, call) {
                                            EffectiveUseRole::CapturedCallCallee(call)
                                        } else {
                                            EffectiveUseRole::CallCallee(call)
                                        },
                                    }))
                                }
                                CallTarget::Reference { place } => {
                                    match self.place_input(prepare_site, place, cursor) {
                                        InputStep::Done => None,
                                        step => return step,
                                    }
                                }
                                CallTarget::Intrinsic {
                                    receiver: Some(value),
                                    ..
                                } if cursor.index == 0 => {
                                    Some(InputDependency::Value(EffectiveValueUse {
                                        value,
                                        site: prepare_site,
                                        role: EffectiveUseRole::CallReceiver(call),
                                        observation: ObservationDemand::Exact,
                                    }))
                                }
                                _ => None,
                            };
                            if let Some(dependency) = dependency {
                                cursor.index += 1;
                                return InputStep::Dependency(dependency);
                            }
                        }
                    }
                    cursor.stage = 2;
                    cursor.index = 0;
                    cursor.place = None;
                    InputStep::Skip
                }
                2 => {
                    if let OperationKind::Call(call) = operation.kind {
                        return self.next_shared_argument(cursor, call);
                    }
                    let operands = data.operands(operation.operands).unwrap();
                    if let Some(&value) = operands.get(cursor.index) {
                        let position = cursor.index;
                        cursor.index += 1;
                        let role = if matches!(operation.kind, OperationKind::Return) {
                            EffectiveUseRole::Return
                        } else {
                            EffectiveUseRole::Operand(position as u32)
                        };
                        return input(value, role);
                    }
                    cursor.stage = 3;
                    InputStep::Skip
                }
                3 => {
                    cursor.stage = 4;
                    if let OperationKind::Call(call) = operation.kind {
                        InputStep::Dependency(InputDependency::Prepare(
                            self.call_prepare(context, call),
                        ))
                    } else {
                        InputStep::Skip
                    }
                }
                _ => InputStep::Done,
            },
        }
    }
    fn demand_generic_product_transport(
        &mut self,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        let OperationKind::Call(call) = data.operations[operation.index()].kind else {
            return Ok(());
        };
        let site = &data.calls[call.index()];
        if site.contract.instantiation.is_none() {
            return Ok(());
        }
        budget.work(1)?;
        let signature = data
            .call_signature(site)
            .ok_or_else(|| unsupported("generic call effective signature"))?;
        let mut pending = Vec::new();
        let required = facts::contains_nominal_product(
            &self.program.types[signature.index()],
            &mut pending,
            budget,
        );
        let released = super::raw_domains::Admission::release(budget, pending);
        let required = required?;
        released?;
        if !required {
            return Ok(());
        }
        let uses = self
            .uses
            .ok_or_else(|| unsupported("generic product transport requires complete use index"))?;
        let body = super::callable_inputs::body_for_call(self.program, uses, unit, call, budget)?
            .ok_or_else(|| {
            unsupported("generic product call requires an original owned body")
        })?;
        // The locator grants identity only. A completed body-global result
        // can serve every named/inline physical context in this same immutable
        // Demand. Do not repeat the complete original call-set proof per call.
        self.summarize(body, budget)?;
        if self.summaries[body.index()]
            .as_ref()
            .unwrap()
            .generic_product_transport
        {
            return Ok(());
        }
        budget.work(1)?;
        if !self.program.units[body.index()]
            .data()
            .callable_type
            .is_some_and(|ty| matches!(self.program.types[ty.index()], Type::GenericFunction(_)))
        {
            return Err(unsupported("generic product callable declaration mismatch"));
        }
        let outcome = CallableInputs::for_body_published(
            self.program,
            uses,
            body,
            CallObservations::from_execution(self.contract.execution),
            budget,
        )?;
        let InputOutcome::Complete(inputs) = outcome else {
            return Err(unsupported(
                "generic product transport requires a complete private interface",
            ));
        };
        let result = (|| {
            if !inputs.runtime_inputs_sealed() {
                return Err(unsupported(
                    "generic product transport requires strict module execution",
                ));
            }
            match facts::returned_value_origin(self.program, uses, body, budget, |_, _, _| Ok(()))?
            {
                facts::ReturnedValueOrigin::Parameter { .. } => Ok(()),
                facts::ReturnedValueOrigin::Unknown(_) => Err(unsupported(
                    "generic product body requires closed value forwarding",
                )),
            }
        })();
        // The Program/UseIndex and execution contract cannot change during this
        // Demand's lifetime. Temporary proof backing is dropped before its
        // charge is released; refusal never publishes the completion bit.
        let released = inputs.discard(budget);
        result?;
        released?;
        self.summaries[body.index()]
            .as_mut()
            .unwrap()
            .generic_product_transport = true;
        Ok(())
    }
    fn visit_shared_dependencies(
        &mut self,
        context: ContextId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        // Qualify transport before dependency replay shortcuts. A later demand
        // cannot turn an actual nominal boundary into primitive-only evidence.
        self.demand_generic_product_transport(context, operation, budget)?;
        let observation = self.operation_observation(context, operation, |n| budget.work(n))?;
        let old = self.context(context).dependencies[operation.index()];
        if old.is_some_and(|old| old.join(observation) == old) {
            return Ok(());
        }
        self.contexts[context.index()].dependencies[operation.index()] =
            Some(old.map_or(observation, |old| old.join(observation)));
        let unit = self.context(context).unit;
        let data = self.program.units[unit.index()].data();
        let op = &data.operations[operation.index()];
        budget.work(self.input_lookup_work())?;
        let mut cursor = self.input_cursor(context, operation, observation);
        loop {
            budget.work(self.input_step_work(&cursor))?;
            match self.next_input(&mut cursor) {
                InputStep::Dependency(InputDependency::Value(input)) => {
                    self.need_value(context, input.value, input.observation, budget)?;
                }
                InputStep::Dependency(InputDependency::Cell(cell, observation)) => {
                    self.observe_cell(context, cell, observation, budget)?
                }
                InputStep::Dependency(InputDependency::Location(location, usage)) => {
                    self.need_place_location(location, usage, budget)?;
                }
                InputStep::Dependency(InputDependency::Slot(record, slot)) => {
                    self.need_storage(StorageId::Slot(record, slot), budget)?
                }
                InputStep::Dependency(InputDependency::ProductCell(cell, slot)) => {
                    let storage = self.product_storage(context, cell, slot, budget)?;
                    self.need_storage(storage, budget)?;
                }
                InputStep::Dependency(InputDependency::ProductSnapshot(value, slot)) => {
                    self.need_snapshot(context, value, slot, budget)?;
                }
                InputStep::Dependency(InputDependency::Prepare(prepare)) => {
                    self.need_operation(context, prepare, budget)?
                }
                InputStep::Skip => {}
                InputStep::Done => break,
            }
        }
        match op.kind {
            OperationKind::Closure(child) if matches!(cursor.recipe, InputRecipe::Ordinary) => {
                if self.child(context, operation).is_none() {
                    self.add_context(
                        child,
                        Some(context),
                        Some(operation),
                        ContextKind::Named,
                        budget,
                    )?;
                }
            }
            OperationKind::Loop { test, update, .. } => {
                self.observe_region(context, test, ObservationDemand::Truthy, budget)?;
                let _ = update;
            }
            // A namespace's members are read once its task settles, like an
            // export's.
            OperationKind::LoadModule { module, .. } => {
                let root = self.root();
                let members = &self.program.modules[module.index()].namespace;
                budget.work(members.len())?;
                for index in 0..members.len() {
                    let cell = self.program.modules[module.index()].namespace[index].1;
                    self.need_cell(root, cell, budget)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn stripped_log_call(&self, unit: UnitId, kind: &OperationKind) -> bool {
        if !self.contract.effects.strip_console {
            return false;
        }
        let OperationKind::Call(call) = kind else {
            return false;
        };
        let data = self.program.units[unit.index()].data();
        match data.calls[call.index()].target {
            CallTarget::Builtin(BuiltinCall::Print) => true,
            CallTarget::Value {
                callee,
                invocation: Invocation::Value,
            } => self.debug_log_value(unit, callee),
            _ => false,
        }
    }
    fn debug_log_value(&self, unit: UnitId, value: ValueId) -> bool {
        let data = self.program.units[unit.index()].data();
        let OperationKind::Load(place) =
            data.operations[data.values[value.index()].definition.index()].kind
        else {
            return false;
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return false;
        };
        let cell = &self.program.cells[cell.index()];
        cell.binding == CellBinding::Foreign && cell.name == "debugLog"
    }
    fn elided_log_lookup(
        &self,
        unit: UnitId,
        operation: OpId,
        budget: &mut Budget<'_>,
    ) -> Result<bool, DemandError> {
        if !self.contract.effects.strip_console {
            return Ok(false);
        }
        let data = self.program.units[unit.index()].data();
        let Some(value) = data.operations[operation.index()].result else {
            return Ok(false);
        };
        if !self.debug_log_value(unit, value) {
            return Ok(false);
        }
        let Some(usages) = self
            .uses
            .and_then(|uses| uses.unit(unit))
            .and_then(|uses| uses.value_uses(value))
        else {
            return Ok(false);
        };
        budget.work(usages.len())?;
        Ok(!usages.is_empty()&&usages.iter().all(|usage|matches!(usage,ValueUse::CallCallee{call,..} if self.stripped_log_call(unit,&OperationKind::Call(*call)))))
    }
}

fn required(behavior: EvaluationBehavior) -> bool {
    behavior.requires_evaluation()
}
fn sort_work(len: usize) -> Result<usize, DemandError> {
    len.checked_mul((usize::BITS - len.max(1).leading_zeros()) as usize)
        .ok_or_else(|| unsupported("demand sorting capacity"))
}

/// Every allocation and queue growth is admitted by the same optional ledger.
/// Inspection uses this algorithm with no ledger, never a hidden second owner.
struct Budget<'a> {
    ledger: Option<&'a mut BudgetLedger>,
    domain: WorkDomain,
    retained: u64,
    peak: u64,
    steps: u64,
    armed: bool,
}
impl<'a> Budget<'a> {
    fn new(budget: Option<(&'a mut BudgetLedger, WorkDomain)>) -> Self {
        let (ledger, domain) = match budget {
            Some((ledger, domain)) => (Some(ledger), domain),
            None => (None, WorkDomain::Baseline),
        };
        Self {
            ledger,
            domain,
            retained: 0,
            peak: 0,
            steps: 0,
            armed: true,
        }
    }
    fn work(&mut self, amount: usize) -> Result<(), DemandError> {
        let amount = u64::try_from(amount).map_err(|_| unsupported("demand work capacity"))?;
        if let Some(ledger) = self.ledger.as_deref_mut() {
            ledger.charge(self.domain, WorkKind::Analysis, amount)?;
        }
        self.steps = self
            .steps
            .checked_add(amount)
            .ok_or_else(|| unsupported("demand work capacity"))?;
        Ok(())
    }
    fn retain(&mut self, bytes: u64) -> Result<(), DemandError> {
        let next = self
            .retained
            .checked_add(bytes)
            .ok_or_else(|| unsupported("demand memory capacity"))?;
        if let Some(ledger) = self.ledger.as_deref_mut() {
            ledger.retain(self.domain, bytes)?;
        }
        self.retained = next;
        self.peak = self.peak.max(next);
        Ok(())
    }
    fn release(&mut self, bytes: u64) -> Result<(), DemandError> {
        if let Some(ledger) = self.ledger.as_deref_mut() {
            ledger.release(self.domain, bytes)?;
        }
        self.retained = self
            .retained
            .checked_sub(bytes)
            .expect("demand owns released reservation");
        Ok(())
    }
    fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>, DemandError> {
        self.work(capacity)?;
        let layout = crate::output_budget::VectorLayout::<T>::new(capacity)
            .map_err(|_| unsupported("demand vector capacity"))?;
        self.retain(layout.bytes())?;
        layout
            .allocate()
            .map_err(|_| unsupported("demand allocation failed"))
    }
    fn filled<T: Clone>(&mut self, len: usize, value: T) -> Result<Vec<T>, DemandError> {
        let mut vector = self.vector(len)?;
        vector.resize(len, value);
        Ok(vector)
    }
    fn push<T>(&mut self, vector: &mut Vec<T>, value: T) -> Result<(), DemandError> {
        self.work(1)?;
        if vector.len() == vector.capacity() {
            let old = vector.capacity();
            let extra = old.max(4);
            let capacity = old
                .checked_add(extra)
                .ok_or_else(|| unsupported("demand growth capacity"))?;
            // Reallocation may briefly own both buffers. Admit the complete
            // destination and its copy work before asking Vec to relocate.
            self.work(old)?;
            let mut next = self.vector::<T>(capacity)?;
            next.append(vector);
            drop(std::mem::replace(vector, next));
            self.release((old * size_of::<T>()) as u64)?;
        }
        vector.push(value);
        Ok(())
    }
    fn finish(mut self) -> Option<(WorkDomain, u64)> {
        self.armed = false;
        self.ledger
            .is_some()
            .then_some((self.domain, self.retained))
    }
}
impl Drop for Budget<'_> {
    fn drop(&mut self) {
        if self.armed {
            if let Some(ledger) = self.ledger.as_deref_mut() {
                ledger
                    .release(self.domain, self.retained)
                    .expect("demand owns failure reservation");
            }
        }
    }
}

impl super::raw_domains::Admission for Budget<'_> {
    type Error = DemandError;
    fn work(&mut self, amount: usize) -> Result<(), Self::Error> {
        Budget::work(self, amount)
    }
    fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>, Self::Error> {
        Budget::vector(self, capacity)
    }
    fn push<T>(&mut self, vector: &mut Vec<T>, value: T) -> Result<(), Self::Error> {
        Budget::push(self, vector, value)
    }
    fn release<T>(&mut self, vector: Vec<T>) -> Result<(), Self::Error> {
        let bytes = vector
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| unsupported("domain release capacity"))?;
        drop(vector);
        Budget::release(self, bytes as u64)
    }
    fn invalid(&self, message: &'static str) -> Self::Error {
        unsupported(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetPlan, CompilationRequest, ResourceLimits};

    fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        inspect(&from_checked_source(&syntax, &semantics).unwrap());
    }
    fn contract() -> JavaScriptCompilationContract {
        let mut config = crate::config::ProjectConfig::default();
        config.javascript.strip_console = false;
        *config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap()
            .javascript_contract()
            .unwrap()
    }
    fn ledger(memory: u64, optional_work: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 1_000_000,
                optional_work,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap()
    }

    #[test]
    fn execution_roots_and_value_production_have_independent_flags_and_diagnostics() {
        checked("extern int effect();effect();print(2+3);", |program| {
            let mut ledger = ledger(1_000_000, 1_000_000);
            let plan = DemandPlan::build(
                program,
                None,
                None,
                &contract(),
                DemandMode::Prune,
                Some((&mut ledger, WorkDomain::Optional)),
            )
            .unwrap();
            let root = plan.root();
            let unit = program.unit(plan.context(root).unit).unwrap();
            let arithmetic = OpId::from_index(
                unit.operations
                    .iter()
                    .position(|op| matches!(op.kind, OperationKind::IntBinary(_)))
                    .unwrap(),
            )
            .unwrap();
            let call = OpId::from_index(unit.operations.iter().position(|op| matches!(op.kind, OperationKind::Call(call) if matches!(unit.calls[call.index()].target, CallTarget::Value{..}))).unwrap()).unwrap();
            assert!(plan.needs_production(root, arithmetic));
            assert!(!plan.needs_execution(root, arithmetic));
            assert!(plan.needs_execution(root, call));
            assert!(!plan.needs_production(root, call));
            let work = plan.work();
            assert_eq!(work.contexts, plan.contexts().len());
            assert_eq!(work.contexts, 1);
            assert_eq!(work.steps, ledger.work_used(WorkDomain::Optional));
            assert_eq!(ledger.work_used(WorkDomain::Baseline), 0);
            assert_eq!(work.retained_bytes, ledger.retained_bytes());
            assert!(work.peak_bytes >= work.retained_bytes);
            assert!(work.steps > unit.operations.len() as u64);
            plan.discard(Some(&mut ledger)).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }

    #[test]
    fn preserve_mode_keeps_unused_computation_through_the_same_kernel() {
        checked("int unused=2+3;print(9);", |program| {
            let mut work = Vec::new();
            for mode in [DemandMode::Prune, DemandMode::Preserve] {
                let mut ledger = ledger(1_000_000, 1_000_000);
                let plan = DemandPlan::build(
                    program,
                    None,
                    None,
                    &contract(),
                    mode,
                    Some((&mut ledger, WorkDomain::Optional)),
                )
                .unwrap();
                let root = plan.root();
                let unit = program.unit(plan.context(root).unit).unwrap();
                let arithmetic = OpId::from_index(
                    unit.operations
                        .iter()
                        .position(|op| matches!(op.kind, OperationKind::IntBinary(_)))
                        .unwrap(),
                )
                .unwrap();
                let cell = CellId::from_index(
                    program
                        .cells
                        .iter()
                        .position(|cell| cell.name == "unused")
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    plan.needs_operation(root, arithmetic),
                    mode == DemandMode::Preserve
                );
                assert_eq!(plan.needs_binding(root, cell), mode == DemandMode::Preserve);
                work.push(plan.work().steps);
                plan.discard(Some(&mut ledger)).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
            assert!(
                work[1] > work[0],
                "preservation work remains explicitly charged"
            );
        });
    }

    #[test]
    fn resource_only_unused_closure_does_not_enter_its_effectful_body() {
        checked(
            "extern int host();int unused(){return host();}print(1);",
            |program| {
                let plan =
                    DemandPlan::build(program, None, None, &contract(), DemandMode::Prune, None)
                        .unwrap();
                assert_eq!(plan.work().contexts, 1);
                let root = plan.root();
                let unit = program.unit(plan.context(root).unit).unwrap();
                let closure = OpId::from_index(
                    unit.operations
                        .iter()
                        .position(|op| matches!(op.kind, OperationKind::Closure(_)))
                        .unwrap(),
                )
                .unwrap();
                assert!(!plan.needs_operation(root, closure));
                assert!(plan.child(root, closure).is_none());
                plan.discard(None).unwrap();
            },
        );
    }

    #[test]
    fn failed_admission_releases_memory_and_keeps_work_in_original_domain() {
        checked(
            "extern int effect();int value=effect();print(value+1);",
            |program| {
                let mut no_work = ledger(1_000_000, 0);
                assert!(matches!(
                    DemandPlan::build(
                        program,
                        None,
                        None,
                        &contract(),
                        DemandMode::Prune,
                        Some((&mut no_work, WorkDomain::Optional))
                    ),
                    Err(DemandError::Budget(BudgetError::WorkExhausted(
                        WorkDomain::Optional
                    )))
                ));
                assert_eq!(no_work.retained_bytes(), 0);
                assert_eq!(no_work.work_used(WorkDomain::Baseline), 0);

                let mut no_memory = ledger(size_of::<DemandPlan<'_, '_>>() as u64 + 256, 1_000_000);
                assert!(matches!(
                    DemandPlan::build(
                        program,
                        None,
                        None,
                        &contract(),
                        DemandMode::Prune,
                        Some((&mut no_memory, WorkDomain::Optional))
                    ),
                    Err(DemandError::Budget(BudgetError::MemoryExhausted(
                        WorkDomain::Optional
                    )))
                ));
                assert_eq!(no_memory.retained_bytes(), 0);
                assert!(no_memory.work_used(WorkDomain::Optional) > 0);
                assert_eq!(no_memory.work_used(WorkDomain::Baseline), 0);
            },
        );
    }

    #[test]
    fn growing_context_and_pending_buffers_admit_transient_peak_and_roll_back_failures() {
        let calls = format!("extern int effect();{}print(1);", "effect();".repeat(80));
        let mut contexts = String::new();
        for index in 0..40 {
            contexts.push_str(&format!(
                "int f{index}(){{return {index};}}print(f{index}());"
            ));
        }
        for source in [&calls, &contexts] {
            checked(source, |program| {
                let mut full = ledger(10_000_000, 1_000_000);
                let plan = DemandPlan::build(
                    program,
                    None,
                    None,
                    &contract(),
                    DemandMode::Prune,
                    Some((&mut full, WorkDomain::Optional)),
                )
                .unwrap();
                let work = plan.work();
                assert!(
                    work.peak_bytes > work.retained_bytes,
                    "relocation and summary scratch must be visible"
                );
                assert_eq!(work.retained_bytes, full.retained_bytes());
                plan.discard(Some(&mut full)).unwrap();
                assert_eq!(full.retained_bytes(), 0);

                let mut tight = ledger(work.peak_bytes - 1, 1_000_000);
                assert!(matches!(
                    DemandPlan::build(
                        program,
                        None,
                        None,
                        &contract(),
                        DemandMode::Prune,
                        Some((&mut tight, WorkDomain::Optional))
                    ),
                    Err(DemandError::Budget(BudgetError::MemoryExhausted(
                        WorkDomain::Optional
                    )))
                ));
                assert_eq!(tight.retained_bytes(), 0);
                assert!(tight.work_used(WorkDomain::Optional) > 0);

                let mut short = ledger(10_000_000, work.steps - 1);
                assert!(matches!(
                    DemandPlan::build(
                        program,
                        None,
                        None,
                        &contract(),
                        DemandMode::Prune,
                        Some((&mut short, WorkDomain::Optional))
                    ),
                    Err(DemandError::Budget(BudgetError::WorkExhausted(
                        WorkDomain::Optional
                    )))
                ));
                assert_eq!(short.retained_bytes(), 0);
                assert_eq!(short.work_used(WorkDomain::Baseline), 0);
            });
        }
    }

    #[test]
    fn checked_recursive_closure_producer_rejects_in_inspection_and_budgeted_routes() {
        checked(
            "int recur(){auto child=()=>1;return child();}print(recur());",
            |program| {
                let CellBinding::Function(unit) = program
                    .cells
                    .iter()
                    .find(|cell| cell.name == "recur")
                    .unwrap()
                    .binding
                else {
                    panic!("declared function");
                };
                let mut changed = program.clone();
                let mut working = changed.units[unit.index()].clone().into_working();
                let closure = working
                    .get_mut()
                    .operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Closure(_)))
                    .unwrap();
                closure.kind = OperationKind::Closure(unit);
                changed.units[unit.index()] = working.freeze();
                changed.verify().unwrap();
                assert!(matches!(
                    DemandPlan::build(&changed, None, None, &contract(), DemandMode::Prune, None),
                    Err(DemandError::Unsupported(Unsupported {
                        feature: "multiply materialized semantic function unit",
                        ..
                    }))
                ));
                let mut ledger = ledger(1_000_000, 1_000_000);
                assert!(matches!(
                    DemandPlan::build(
                        &changed,
                        None,
                        None,
                        &contract(),
                        DemandMode::Prune,
                        Some((&mut ledger, WorkDomain::Optional))
                    ),
                    Err(DemandError::Unsupported(Unsupported {
                        feature: "multiply materialized semantic function unit",
                        ..
                    }))
                ));
                assert_eq!(ledger.retained_bytes(), 0);
            },
        );
    }
}

#[cfg(test)]
#[path = "demand_observation_tests.rs"]
mod observation_tests;
