//! Formation from checked semantic units and selected physical recipes.
//!
//! Immutable values acquire storage only when their effective uses or evaluation
//! schedule require it. Source cells retain their lexical identity. The bounded
//! placement plan precedes binding allocation; numeric facts describe raw target
//! results separately from source integer normalization.
//! Prepared calls own argument schedules so a member getter runs before argument
//! effects without inventing a first-class JavaScript reference or a `.call`
//! lookup. Unsupported language/delivery contracts fail before printing.

#[cfg(test)]
#[path = "javascript_reprint_tests.rs"]
mod reprint_tests;

#[path = "javascript_shapes.rs"]
mod shapes;
#[path = "javascript_enums.rs"]
mod enums;
#[path = "javascript_variants.rs"]
mod variants;

use super::demand::{
    ContextId, ContextKind, DemandError, DemandMode, DemandPlan, HelperOperation, RecordOperation,
};
use super::implementations::ImplementationMap;
use super::record_family::RecordFamily;
use super::string_family::StringChoice;
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::value_placement::{self, PlacementDepth, ValueStorage};
use super::*;
use crate::compilation_contract::{
    JavaScriptAbiContract, JavaScriptCompilationContract, JavaScriptEffectPolicy,
    JavaScriptUnsafeAssumptions, JavaScriptWorld,
};
use crate::compilation_policy::{BudgetError, BudgetLedger, WorkDomain, WorkKind};
use crate::js;
use crate::js_syntax_target::{EcmaScriptEdition, JsSyntaxFeature};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::primitive::{Intrinsic, ResolvedIntrinsic};
use crate::scalar_transfer::NumberFacts;

#[path = "javascript_absence.rs"]
mod absence;
#[path = "javascript_checks.rs"]
mod checks;
#[path = "javascript_host.rs"]
mod host;
#[path = "javascript_methods.rs"]
mod methods;
#[path = "javascript_product_calls.rs"]
mod product_calls;
#[path = "javascript_products.rs"]
mod products;
#[path = "javascript_const.rs"]
mod const_data;
#[path = "javascript_const_graph.rs"]
mod const_graph;
#[path = "javascript_public_structs.rs"]
mod public_structs;
#[path = "javascript_references.rs"]
mod references;
#[path = "javascript_struct_boundaries.rs"]
mod struct_boundaries;
#[path = "javascript_structs.rs"]
mod structs;

pub(super) fn check_interfaces(
    program: &Program<'_>, contract: &JavaScriptCompilationContract,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), (ModuleId, FormationError)> {
    structs::validate_interfaces(program, contract, budget)
}

/// The operation's selected result recipe, shared by formation and domain
/// evidence. A typed result is its type's by R1 (trusted crossings): an
/// `int` result is an int32 with no code.
#[derive(Clone, Copy)]
enum CallResultRecipe {
    Raw,
    Void,
    IntrinsicInteger,
}
fn call_result_recipe(
    program: &Program<'_>,
    unit: &UnitData,
    operation: &Operation,
) -> CallResultRecipe {
    let OperationKind::Call(call) = operation.kind else {
        return CallResultRecipe::Raw;
    };
    let _ = program;
    match unit.calls[call.index()].target {
        CallTarget::Builtin(BuiltinCall::Print) => CallResultRecipe::Void,
        CallTarget::Intrinsic {
            operation: ResolvedIntrinsic::Method(method),
            ..
        } if js::integer_intrinsic(method) => CallResultRecipe::IntrinsicInteger,
        _ => CallResultRecipe::Raw,
    }
}

/// A load's result recipe. A typed load is its type's by R1, and an index
/// read is in range by R11's precondition, so an element read is its
/// JavaScript read: an `int` needs no `|0`, a `string` no `??""`. Absence
/// on a record's missing key and in a `T?` element reads as the language's
/// null until R2's second batch (M10.9). A `Uint32Array` element is a uint32,
/// which an `int` reads as int32.
#[derive(Clone, Copy)]
enum LoadResultRecipe {
    Raw,
    NullishNull,
    Uint32Element,
}
fn load_result_recipe(
    program: &Program<'_>,
    unit: &UnitData,
    place: PlaceId,
    ty: TypeId,
) -> LoadResultRecipe {
    let receiver = match unit.places[place.index()] {
        Place::Cell(_) | Place::Value(_) | Place::Field { .. } => return LoadResultRecipe::Raw,
        Place::Member { receiver, .. }
        | Place::ClassField { receiver, .. }
        | Place::Index { receiver, .. } => receiver,
    };
    let index = matches!(unit.places[place.index()], Place::Index { .. });
    let receiver_ty = &program.types[unit.values[receiver.index()].ty.index()];
    let result_ty = &program.types[ty.index()];
    if !program.source_contract.unified_absence() && (matches!(receiver_ty, Type::Record(_))
        || super::schema::is_shape(program, receiver_ty)
            && matches!(result_ty, Type::Nullable(_) | Type::Null)
        || index
            && matches!(receiver_ty, Type::Array(_))
            && matches!(result_ty, Type::Nullable(_) | Type::Null))
    {
        LoadResultRecipe::NullishNull
    } else if index && matches!(receiver_ty, Type::Uint32Array) && matches!(result_ty, Type::Int) {
        LoadResultRecipe::Uint32Element
    } else {
        LoadResultRecipe::Raw
    }
}

pub(super) struct JavaScriptRecipes;
impl super::facts::domains::Recipes for JavaScriptRecipes {
    fn result(
        &self,
        program: &Program<'_>,
        unit: UnitId,
        operation: OpId,
    ) -> super::facts::domains::ResultRecipe {
        use super::facts::domains::ResultRecipe;
        let data = program.unit(unit).unwrap();
        let operation = &data.operations[operation.index()];
        // An `int` result is an int32 by type (R1), with no code.
        let typed_int = operation.result.is_some_and(|value| {
            matches!(
                program.types[data.values[value.index()].ty.index()],
                Type::Int
            )
        });
        match operation.kind {
            OperationKind::Call(_) => match call_result_recipe(program, data, operation) {
                CallResultRecipe::Void => ResultRecipe::Undefined,
                CallResultRecipe::IntrinsicInteger => ResultRecipe::NormalizedI32,
                CallResultRecipe::Raw if typed_int => ResultRecipe::NormalizedI32,
                CallResultRecipe::Raw => ResultRecipe::Source,
            },
            OperationKind::Load(place)
                if typed_int
                    && !matches!(data.places[place.index()], Place::Cell(_) | Place::Value(_)) =>
            {
                ResultRecipe::NormalizedI32
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(operation))
                if js::integer_intrinsic(operation) =>
            {
                ResultRecipe::NormalizedI32
            }
            _ => ResultRecipe::Source,
        }
    }
}

/// Builtins spelled as a literal or operator over their operands: nothing
/// runs before the last operand is evaluated. Any other spelling first reads
/// a host path or method, which only pristine builtins make unobservable
/// where the forwarding function read it after its arguments.
fn operands_first(builtin: BuiltinCall) -> bool {
    use BuiltinCall as B;
    matches!(
        builtin,
        B::JsArray
            | B::JsObject
            | B::JsUndefined
            | B::JsAssume
            | B::JsNumber
            | B::JsString
            | B::JsTypeOf
            | B::JsIsNullish
            | B::JsIsFalse
            | B::JsIsUndefined
            | B::JsAdd
            | B::JsMod
            | B::JsLessThan
            | B::JsLessThanOrEqual
            | B::JsGreaterThan
            | B::JsGreaterThanOrEqual
            | B::JsStrictEqual
            | B::JsStrictNotEqual
            | B::JsConstruct
            | B::JsInstanceOf
            | B::JsSubtract
            | B::JsMultiply
            | B::JsDivide
            | B::JsNegate
    )
}

/// Builtins whose evaluation runs no user code once their operands are
/// values: no conversion hook, getter or call. In a sloppy script, user code
/// run from a forwarding function sees that frame as `arguments.callee.caller`;
/// only these keep it unobservable when the frame goes.
fn runs_no_user_code(builtin: BuiltinCall) -> bool {
    use BuiltinCall as B;
    matches!(
        builtin,
        B::JsArray
            | B::JsObject
            | B::JsUndefined
            | B::JsAssume
            | B::JsTypeOf
            | B::JsIsNullish
            | B::JsIsFalse
            | B::JsIsUndefined
            | B::JsStrictEqual
            | B::JsStrictNotEqual
    )
}

/// `JS.call`, `JS.apply` and `JS.construct` only invoke their first operand;
/// none reads its name.
fn invokes_argument(target: &CallTarget) -> bool {
    matches!(
        target,
        CallTarget::Builtin(BuiltinCall::JsCall | BuiltinCall::JsApply | BuiltinCall::JsConstruct)
    )
}

pub(super) fn lower(program: &Program<'_>) -> Result<js::Module, Unsupported> {
    // Explicit inspection defaults for Program::to_javascript. Project builds
    // enter through Compilation and always supply their resolved contract.
    let contract = JavaScriptCompilationContract {
        execution: crate::compilation_contract::JavaScriptExecution::Module,
        world: JavaScriptWorld::ReusableLibrary,
        ecmascript: EcmaScriptEdition::Es2022,
        abi: JavaScriptAbiContract {
            preserve_root_exports: true,
            keep_function_names: false,
            keep_published_function_names: true,
        },
        assumptions: JavaScriptUnsafeAssumptions {
            pristine_builtins: false,
            pure_property_reads: false,
            unconstructed_callbacks: false,
            private_underscore_properties: false,
            numeric_lengths: false,
        },
        effects: JavaScriptEffectPolicy {
            strip_debug: false,
            strip_console_calls: false,
        },
        checks: crate::compilation_contract::PreconditionChecks::Production,
    };
    // Inspection forms from the same use facts as an admitted build, so the
    // two agree on every use-directed shape; its ledger is never exhausted.
    let mut ledger = BudgetLedger::new(
        crate::compilation_policy::ResourceLimits::default(),
        crate::compilation_policy::BudgetPlan {
            baseline_work: u64::MAX / 2,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: u64::MAX,
        },
    )
    .expect("an unbounded inspection plan is valid");
    let unsupported = |feature| Unsupported {
        span: Span::default(),
        feature,
    };
    let uses =
        UseIndex::build(program, &mut ledger, WorkDomain::Baseline).map_err(
            |error| match error {
                super::uses::UseError::InvalidProgram(feature) => unsupported(feature),
                _ => unsupported("JavaScript inspection use index"),
            },
        )?;
    let module = form(
        program,
        Some(&uses),
        None,
        &contract,
        DemandMode::Prune,
        None,
    )
    .map_err(|error| match error {
        FormationError::Unsupported(error) => error,
        FormationError::Budget(_) => unreachable!("unaccounted inspection cannot exhaust a ledger"),
        FormationError::Allocation(_) => unsupported("JavaScript target allocation failed"),
    });
    uses.discard(&mut ledger)
        .expect("inspection releases exactly what it retained");
    module
}

#[derive(Debug)]
pub(super) enum FormationError {
    Unsupported(Unsupported),
    Budget(BudgetError),
    Allocation(AllocationError),
}
impl From<AllocationError> for FormationError {
    fn from(error: AllocationError) -> Self {
        Self::Allocation(error)
    }
}
impl From<js::rules::RuleError> for FormationError {
    fn from(error: js::rules::RuleError) -> Self {
        match error {
            js::rules::RuleError::Allocation(error) => Self::Allocation(error),
            // A compiler bug fails the build with its diagnostic
            // (architecture §8.2); the build ends, so the text may live on.
            js::rules::RuleError::Bug(message) => Self::Unsupported(Unsupported {
                span: Span::default(),
                feature: Box::leak(
                    format!("JavaScript target rules (a compiler bug): {message}").into_boxed_str(),
                ),
            }),
        }
    }
}
impl From<Unsupported> for FormationError {
    fn from(error: Unsupported) -> Self {
        Self::Unsupported(error)
    }
}
impl From<DemandError> for FormationError {
    fn from(error: DemandError) -> Self {
        match error {
            DemandError::Unsupported(error) => Self::Unsupported(error),
            DemandError::Budget(error) => Self::Budget(error),
            DemandError::Allocation(error) => Self::Allocation(error),
        }
    }
}

/// The compilation checkpoint owns the complete, validated implementation.
/// Formation derives only transient target storage from its semantic IDs.
pub(super) fn lower_with_implementations(
    program: &Program<'_>,
    uses: &UseIndex,
    implementations: &ImplementationMap,
    contract: &JavaScriptCompilationContract,
    mode: DemandMode,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<js::Module, FormationError> {
    form(
        program,
        Some(uses),
        Some(implementations),
        contract,
        mode,
        Some((ledger, domain)),
    )
}

/// Compilation-owned formation. Demand, transient plans, and the target coexist
/// in one ledger/domain. Only a complete Module transfers retained allocations.
pub(super) fn lower_admitted(
    program: &Program<'_>,
    uses: &UseIndex,
    implementations: &ImplementationMap,
    contract: &JavaScriptCompilationContract,
    mode: DemandMode,
    compact: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<js::Module, FormationError> {
    lower_output_admitted(
        program,
        uses,
        implementations,
        contract,
        mode,
        compact,
        js::TargetRules::SEMANTIC,
        &[],
        // The canonical families: what a codec objective seeds.
        js::OutputFamilies::seed(js::selection::Objective::Brotli),
        &js::ChoiceMap::SEEDS,
        None,
        true,
        &[],
        budget,
    )
}

/// `lower_admitted` with the output's families and host modules.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_output_admitted(
    program: &Program<'_>,
    uses: &UseIndex,
    implementations: &ImplementationMap,
    contract: &JavaScriptCompilationContract,
    mode: DemandMode,
    compact: bool,
    rules: js::TargetRules,
    preserved_properties: &[String],
    families: js::OutputFamilies,
    choices: &js::ChoiceMap,
    hosts: Option<&crate::host_modules::HostDelivery>,
    reuse_normalization: bool,
    selected_entries: &[String],
    budget: &mut AllocationBudget<'_>,
) -> Result<js::Module, FormationError> {
    let mut phase = budget.scope();
    let demand = phase.with_ledger(|ledger| {
        DemandPlan::build_selected(
            program,
            Some(uses),
            Some(implementations),
            contract,
            mode,
            ledger,
            reuse_normalization,
            selected_entries,
        )
    })?;
    let result = form_with_demand(
        program,
        Some(uses),
        contract,
        &demand,
        compact,
        rules,
        preserved_properties,
        families,
        choices,
        hosts,
        reuse_normalization,
        &mut phase,
    );
    let discarded = phase.with_ledger(|ledger| demand.discard(ledger.map(|(ledger, _)| ledger)));
    match (result, discarded) {
        (Ok(module), Ok(())) => {
            phase.finish_retained()?;
            Ok(module)
        }
        (Ok(module), Err(error)) => {
            drop(module);
            Err(FormationError::Budget(error))
        }
        (Err(error), _) => Err(error),
    }
}

fn form(
    program: &Program<'_>,
    uses: Option<&UseIndex>,
    implementations: Option<&ImplementationMap>,
    contract: &JavaScriptCompilationContract,
    mode: DemandMode,
    mut budget: Option<(&mut BudgetLedger, WorkDomain)>,
) -> Result<js::Module, FormationError> {
    // A complete demand answer is required before target allocation. A failed
    // admission cannot turn unknown work into a successful partial artifact.
    let demand = DemandPlan::build(
        program,
        uses,
        implementations,
        contract,
        mode,
        budget
            .as_mut()
            .map(|(ledger, domain)| (&mut **ledger, *domain)),
    )?;
    let result = form_with_demand(
        program,
        uses,
        contract,
        &demand,
        !program.authored_choices.is_empty(),
        js::TargetRules {
            statement_spellings: !program.authored_choices.is_empty(),
            receiver_aliases: program.authored_choices.uses(crate::compilation_policy::TacticId::ReceiverAliases),
            ..js::TargetRules::NONE
        },
        &[],
        js::OutputFamilies::NONE,
        &js::ChoiceMap::SEEDS,
        None,
        true,
        &mut AllocationBudget::new(None),
    );
    demand
        .discard(budget.map(|(ledger, _)| ledger))
        .map_err(FormationError::Budget)?;
    result.map(|module| {
        debug_assert!(
            module.observed_literals.is_empty(),
            "inspection formation keeps exact literals"
        );
        module
    })
}

#[allow(clippy::too_many_arguments)]
fn form_with_demand(
    program: &Program<'_>,
    uses: Option<&UseIndex>,
    contract: &JavaScriptCompilationContract,
    demand: &DemandPlan<'_, '_>,
    compact: bool,
    rules: js::TargetRules,
    preserved_properties: &[String],
    families: js::OutputFamilies,
    choices: &js::ChoiceMap,
    hosts: Option<&crate::host_modules::HostDelivery>,
    reuse_normalization: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<js::Module, FormationError> {
    let _timing = crate::timing::JS_FORMATION.scope(0);
    let head = form_head(
        program,
        uses,
        contract,
        demand,
        compact,
        rules,
        preserved_properties,
        families.head(),
        hosts,
        reuse_normalization,
        budget,
    )?;
    form_tail(head, families, choices, budget)
}

/// A formed tree before its output families: everything formation does that
/// no family changes. The terminal stage forms it once per candidate and
/// runs `form_tail` on a copy for each challenger.
pub(super) struct FormedHead {
    module: js::Module,
    /// The contract facts the family tail reads; absent without target
    /// compaction, which runs no tail.
    tail: Option<TailContext>,
    /// The `int32_hints` family prints nothing in this program: no read or
    /// call result it would hint, and no integer intrinsic the printer would
    /// hint without pristine builtins. Its other head renders this one's
    /// bytes.
    hints_inert: bool,
    properties_inert: bool,
}

#[derive(Clone, Copy)]
struct TailContext {
    reuse_normalization: bool,
    rules: js::TargetRules,
    frames_hidden: bool,
    strict: bool,
    pristine: bool,
    prunes: bool,
    numeric_lengths: bool,
    year: u16,
}

impl FormedHead {
    /// An admitted copy, charged like the formation that built the original.
    pub(super) fn clone_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let module = self.module.clone_in(budget)?;
        Ok(Self {
            module,
            tail: self.tail,
            hints_inert: self.hints_inert,
            properties_inert: self.properties_inert,
        })
    }

    pub(super) fn hints_inert(&self) -> bool {
        self.hints_inert
    }

    pub(super) fn properties_inert(&self) -> bool {
        self.properties_inert
    }
}

/// Form the head from an existing demand plan: the terminal stage forms one
/// candidate under several families and builds its demand and head once.
#[allow(clippy::too_many_arguments)]
pub(super) fn form_head_admitted(
    program: &Program<'_>,
    uses: &UseIndex,
    contract: &JavaScriptCompilationContract,
    demand: &DemandPlan<'_, '_>,
    compact: bool,
    rules: js::TargetRules,
    preserved_properties: &[String],
    head: js::HeadChoices,
    hosts: Option<&crate::host_modules::HostDelivery>,
    reuse_normalization: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<FormedHead, FormationError> {
    let _timing = crate::timing::JS_FORMATION.scope(0);
    form_head(
        program,
        Some(uses),
        contract,
        demand,
        compact,
        rules,
        preserved_properties,
        head,
        hosts,
        reuse_normalization,
        budget,
    )
}

/// Apply the output families and choices to a formed head.
pub(super) fn form_tail_admitted(
    mut head: FormedHead,
    families: js::OutputFamilies,
    choices: &js::ChoiceMap,
    reuse_normalization: bool,
    data_estimator: Option<(crate::config::CompressionCostModel, crate::compression::CodecSettings)>,
    budget: &mut AllocationBudget<'_>,
) -> Result<js::Module, FormationError> {
    let _timing = crate::timing::JS_FORMATION.scope(0);
    if let Some(tail) = &mut head.tail { tail.reuse_normalization = reuse_normalization; }
    head.module.data_estimator = data_estimator;
    form_tail(head, families, choices, budget)
}

#[allow(clippy::too_many_arguments)]
fn form_head(
    program: &Program<'_>,
    uses: Option<&UseIndex>,
    contract: &JavaScriptCompilationContract,
    demand: &DemandPlan<'_, '_>,
    compact: bool,
    rules: js::TargetRules,
    preserved_properties: &[String],
    head: js::HeadChoices,
    hosts: Option<&crate::host_modules::HostDelivery>,
    reuse_normalization: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<FormedHead, FormationError> {
    use crate::compilation_policy::TacticId;
    let pins = program.authored_choices;
    if pins.uses(TacticId::TargetCompaction) && !compact
        || pins.uses(TacticId::StatementSpellings) && !rules.statement_spellings
        || pins.uses(TacticId::ReceiverAliases) && !rules.receiver_aliases {
        return Err(Unsupported { span: Span::default(), feature: "selected target rules conflict with source @choose" }.into());
    }
    let mut phase = budget.scope();
    let struct_plan = structs::plan(program, contract, demand, &mut phase)?;
    let reference_plan = references::Plan::new();
    let storage = if compact && rules.scalar_replacement && !program.structs.is_empty() {
        match uses {
            Some(uses) => super::physical_storage::StorageProofs::build(
                program,
                uses,
                super::call_graph::Seal::from_execution(contract.execution),
                super::physical_storage::StorageDemand::Products,
                &mut phase,
            )?,
            None => super::physical_storage::StorageProofs::default(),
        }
    } else {
        super::physical_storage::StorageProofs::default()
    };
    let private_fields = super::private_fields::Plan::new(
        program,
        preserved_properties,
        contract.assumptions.pristine_builtins,
        &mut phase,
    )?;
    let mut records = phase.vector(AllocationClass::Scratch, demand.records().len())?;
    for family in demand.records() {
        phase.work(WorkKind::Render, 1)?;
        let bindings = phase.filled(AllocationClass::Scratch, family.slots().len(), None)?;
        records.push(RecordStorage { family, bindings });
    }
    let mut contexts = phase.vector(AllocationClass::Scratch, demand.contexts().len())?;
    phase.work(WorkKind::Render, demand.contexts().len() as u64)?;
    contexts.resize_with(demand.contexts().len(), || None);
    let mut entry_depths = if compact {
        phase.filled(
            AllocationClass::Scratch,
            demand.contexts().len(),
            usize::MAX,
        )?
    } else {
        Vec::new()
    };
    if compact {
        for &root in demand.roots() {
            phase.work(WorkKind::Render, 1)?;
            entry_depths[root.index()] = 1;
        }
    }
    let mut module = js::Module::new_in(&mut phase)?;
    if program.authored_pooling {
        let mut seen = phase.filled(AllocationClass::Scratch, program.strings.len(), false)?;
        for source in program.modules.iter() {
            for string in &source.pooled_strings {
                phase.work(WorkKind::Render, 1)?;
                if std::mem::replace(&mut seen[string.index()], true) { continue; }
                let value = &program.strings[string.index()];
                let bytes = value.storage_bytes() as u64;
                phase.work(WorkKind::Render, bytes)?;
                phase.retain(AllocationClass::Retained, bytes)?;
                let value = value.try_clone().map_err(|_| AllocationError::AllocationFailed)?;
                phase.push(AllocationClass::Retained, &mut module.authored_pool, value)?;
            }
        }
        let levels = u64::from(usize::BITS - module.authored_pool.len().leading_zeros()) + 1;
        for value in &module.authored_pool {
            phase.work(WorkKind::Render, levels.saturating_mul(value.storage_bytes() as u64 + 1))?;
        }
        module.authored_pool.sort_unstable();
        let bytes = seen.capacity() as u64;
        drop(seen);
        phase.release(AllocationClass::Scratch, bytes)?;
    }
    module.pristine_builtins = contract.assumptions.pristine_builtins;
    module.pure_property_reads = contract.assumptions.pure_property_reads;
    module.unconstructed_callbacks = contract.assumptions.unconstructed_callbacks;
    if contract.assumptions.private_underscore_properties {
        module.private_names = Some(preserved_properties.iter().cloned().collect::<Vec<_>>().into());
    }
    module.int32_hints = head.int32_hints;
    // The program's value ranges (M6.4b), once per program under its seal.
    let ranges = std::sync::Arc::clone(demand.ranges());
    let mut formation = Formation {
        program,
        uses,
        contract: *contract,
        demand,
        compact,
        default_transport: compact && rules.constant_folding,
        ranges,
        module,
        string_sums: Vec::new(),
        contexts,
        entry_depths,
        records,
        struct_plan,
        storage,
        reference_plan,
        host_factories: Vec::new(),
        index_check: None,
        crossing_checks: Vec::new(),
        shape_helpers: Vec::new(),
        class_witnesses: Vec::new(),
        payload_cells: Vec::new(),
        int32_hints: head.int32_hints,
        property_mangling: head.property_mangling,
        preserved_properties,
        private_fields,
        property_sites: 0,
        hint_sites: 0,
        forming: None,
        unit_functions: Vec::new(),
        foreign_bindings: Vec::new(),
        stable_cells: Vec::new(),
        arguments_read: None,
        current_module: 0,
        anchor: js::Anchor::Anchored,
        point: None,
        classify_roots: false,
        this_cells: Vec::new(),
        arguments_cells: Vec::new(),
        activation_cells: Vec::new(),
        formal_lists: Vec::new(),
        method_calls: Vec::new(),
        prototype_forms: Vec::new(),
        class_methods: Vec::new(),
        unbound_cells: Vec::new(),
        budget: &mut phase,
    };
    let result = (|| {
        formation.prepare_class_witnesses()?;
        // One artifact lexical environment owns all module cells. Establish
        // every physical root's bindings before forming any callable body,
        // since its captures may refer to a later module in an import cycle.
        for &context in demand.roots() {
            formation.work(1)?;
            formation.current_module = formation.data(context).module.index() as u32;
            formation.plan_context(context, formation.module.root)?;
        }
        // The checked interface separates instantiation from evaluation.
        // Prefixes contain only complete named callable creation/initialization
        // pairs. Ordinary lexical cells are declared at their original suffix
        // operation, preserving cross-module TDZ and once-only side effects.
        // Instantiation creates named functions and runs nothing: every
        // statement it forms is a definition.
        formation.anchor = js::Anchor::Definition;
        for &context in demand.roots() {
            formation.work(1)?;
            let data = formation.data(context);
            formation.current_module = data.module.index() as u32;
            let prefix = data.instantiation_prefix as usize;
            formation.statement_operations(
                context,
                data.entry,
                &data.regions[data.entry.index()].operations[..prefix],
            )?;
        }
        formation.anchor = js::Anchor::Anchored;
        for &context in demand.roots() {
            formation.work(1)?;
            let data = formation.data(context);
            formation.current_module = data.module.index() as u32;
            let prefix = data.instantiation_prefix as usize;
            formation.classify_roots = true;
            formation.statement_operations(
                context,
                data.entry,
                &data.regions[data.entry.index()].operations[prefix..],
            )?;
            formation.classify_roots = false;
            formation.anchor = js::Anchor::Anchored;
        }
        for &context in demand.roots() {
            formation.work(1)?;
            formation.finish_unit(context)?;
        }
        formation.finish_reference_prefix()?;
        let context = demand.root();
        // D2 adapters and wrappers for exports only define (design §6).
        formation.anchor = js::Anchor::Definition;
        if contract.abi.preserve_root_exports {
            formation.work(program.exports().len())?;
            // Several entries (plan M3.3): one cell exported by two entries,
            // or under two names, is one identity with one adapter (DL6).
            let several = demand.entries().len() > 1;
            let mut identities: Vec<(CellId, js::BindingId)> = Vec::new();
            let mut positions: Vec<u32> = Vec::new();
            for export in demand.exports() {
                let InterfaceTarget::Value(cell) = export.target else {
                    positions.push(u32::MAX);
                    continue;
                };
                formation.current_module = program.units[program.cells[cell.index()].owner.index()].data().module.index() as u32;
                let export_name = export.name.as_str();
                formation.work(1)?;
                if several {
                    formation.work(identities.len() + formation.module.exports.len())?;
                    if let Some(&(_, binding)) =
                        identities.iter().find(|&&(known, _)| known == cell)
                    {
                        let position =
                            match formation.module.exports.iter().position(|known| {
                                known.binding == binding && known.name == export_name
                            }) {
                                Some(position) => position,
                                None => {
                                    let name = formation.text(export_name)?;
                                    formation.budget.push(
                                        AllocationClass::Retained,
                                        &mut formation.module.exports,
                                        js::Export { binding, name },
                                    )?;
                                    formation.module.exports.len() - 1
                                }
                            };
                        positions.push(position as u32);
                        continue;
                    }
                }
                if formation.addressed_cell(context, cell)? {
                    return Err(formation.error(
                        program.cells[cell.index()].declaration,
                        "public address-taken cell ABI adaptation",
                    ));
                }
                // JavaScript's `length` stops at the first default. The body
                // applies each default, so the printed parameter at that index
                // only needs default syntax (`p=void 0`), and the later ones
                // none.
                if let (Some(signature), CellBinding::Function(unit)) = (
                    program.types[program.cells[cell.index()].ty.index()].callable_signature(),
                    program.cells[cell.index()].binding,
                ) {
                    formation.work(signature.params.len())?;
                    // A class constructor's instance is `this`, not a
                    // parameter of the published constructor.
                    let receiver = usize::from(
                        program
                            .unit(unit)
                            .is_some_and(|data| data.constructor_of.is_some())
                            || signature.has_receiver(),
                    );
                    // A `JS.undefined()` default needs no syntax: absence is it.
                    let first = program
                        .unit(unit)
                        .and_then(|data| data.declared_length)
                        .map(|position| position as usize - receiver);
                    if let Some(first) = first.filter(|_| !formation.struct_plan.wrapped(unit)) {
                        formation.work(formation.unit_functions.len())?;
                        let mut formed = false;
                        for index in 0..formation.unit_functions.len() {
                            let (formed_unit, function) = formation.unit_functions[index];
                            if formed_unit != unit {
                                continue;
                            }
                            let function = &mut formation.module.functions[function.index()];
                            // A strict directive forbids non-simple parameters,
                            // and a D2 wrapper is the public face instead.
                            if function.strict || formation.struct_plan.wrapped(unit) {
                                formed = false;
                                break;
                            }
                            function.length = Some(first);
                            formed = true;
                        }
                        if !formed {
                            return Err(formation.error(
                                program.cells[cell.index()].declaration,
                                "public parameter default reflection",
                            ));
                        }
                    }
                }
                let mut binding = formation.cell_binding(context, cell)?;
                if let Some((_, public)) = formation.struct_plan.const_exports.iter().find(|(known, _)| *known == cell) {
                    binding = *public;
                }
                if !formation.struct_plan.abi_types.is_empty() && formation.struct_plan.abi_types[program.cells[cell.index()].ty.index()]
                    && matches!(program.cells[cell.index()].binding, CellBinding::Function(_))
                {
                    binding = formation.public_struct_export(cell, binding)?;
                }
                let name = formation.text(export_name)?;
                formation.budget.push(
                    AllocationClass::Retained,
                    &mut formation.module.exports,
                    js::Export { binding, name },
                )?;
                if several {
                    identities.push((cell, binding));
                    positions.push(formation.module.exports.len() as u32 - 1);
                }
            }
            // Each entry's exports, as positions in the one export list.
            if several {
                let mut start = 0;
                for &entry in demand.entries() {
                    let program_entry = &program.entries()[entry];
                    let count = program.entry_exports(entry).len();
                    let exports = positions[start..start + count]
                        .iter()
                        .copied()
                        .filter(|&position| position != u32::MAX)
                        .collect::<Vec<_>>();
                    start += count;
                    let name = formation.text(&program_entry.name)?;
                    formation.budget.push(
                        AllocationClass::Retained,
                        &mut formation.module.entries,
                        js::EntryPublic { name, exports },
                    )?;
                }
            }
        }
        Ok::<_, FormationError>(())
    })();
    if let Err(error) = result {
        drop(formation);
        return Err(error);
    }
    if formation.module.root_rows.len()
        != formation.module.regions[formation.module.root.index()]
            .statements
            .len()
    {
        let error = formation.error(Span::default(), "root statement without its source module");
        drop(formation);
        return Err(error);
    }
    // When each function formed from a unit may first run (M6.5): the
    // program's order, carried for the rules that ask which code runs after
    // a root binding holds its value.
    for index in 0..formation.unit_functions.len() {
        let (unit, function) = formation.unit_functions[index];
        let point = formation.demand.initialization().first_run(unit).ordinal();
        if let Err(error) = formation
            .module
            .first_run_in(function, point, formation.budget)
        {
            drop(formation);
            return Err(error.into());
        }
    }
    // One-use forwarding: a checked target edit on the finished tree, part
    // of target compaction.
    // Module loading and export-name validation survive an unused local binding.
    // The delivery graph owns these requests independently of value demand.
    for source in program.modules() {
        if formation.demand.module_context(source.initializer).is_none() { continue; }
        for import in &source.foreign_imports {
            let result = match import.cell {
                Some(cell) => formation.foreign_import(cell).map(|_| ()),
                None => formation.foreign_effect_import(&import.source),
            };
            if let Err(error) = result { drop(formation); return Err(error); }
        }
    }
    // Host syntax lowering is a delivery operation at every effort level.
    // Optimization permissions still govern the subsequent target rules.
    if let (true, Some(hosts)) = (formation.contract.execution.guarantees_strict_execution(), hosts) {
        if let Err(error) = formation.module.lower_hosts(hosts, formation.program.modules().len(), formation.budget) {
            drop(formation);
            return Err(error.into());
        }
    }
    let mut tail = None;
    if formation.compact {
        let pristine = formation.contract.assumptions.pristine_builtins;
        // Removing an unused declaration or a bare inert statement is dead-code
        // elimination, which its own permission governs.
        let prunes = formation.demand.prunes();
        let strict = formation.contract.execution.guarantees_strict_execution();
        let numeric_lengths = formation.contract.assumptions.numeric_lengths;
        let year = formation.contract.ecmascript.year();
        // Frames the rules may elide or move: strict code's, and every frame
        // in an application's world (Y5's frames amendment).
        let frames_hidden = formation.contract.frames_hidden();
        // The family-independent rules, to their fixed point (M5.3a).
        let context = js::rules::Context {
            reuse_normalization,
            rules,
            frames_hidden,
            strict,
            pristine,
            prunes,
            numeric_lengths,
            year,
            statements: js::StatementSpellings::NONE,
            choices: None,
            families: js::OutputFamilies::NONE,
        };
        if let Err(error) = formation
            .module
            .run_rules(js::rules::HEAD, &context, formation.budget)
        {
            drop(formation);
            return Err(error.into());
        }
        tail = Some(TailContext {
            reuse_normalization,
            rules,
            frames_hidden,
            strict,
            pristine,
            prunes,
            numeric_lengths,
            year,
        });
    }
    let Formation {
        module,
        contexts,
        entry_depths,
        records,
        struct_plan,
        storage,
        reference_plan,
        hint_sites,
        property_sites,
        private_fields,
        ..
    } = formation;
    drop(contexts);
    drop(entry_depths);
    drop(records);
    drop(struct_plan);
    drop(storage);
    drop(reference_plan);
    drop(private_fields);
    // Without pristine builtins the printer hints an integer intrinsic's
    // result under the family; the tail creates none.
    let hints_inert = hint_sites == 0
        && (module.pristine_builtins || {
            phase.work(WorkKind::Render, module.expressions.len() as u64)?;
            !module.expressions.iter().any(|expression| {
                matches!(expression, js::Expr::Intrinsic { operation, .. }
                    if crate::catalog::integer_intrinsic(*operation)
                        || crate::catalog::original_int32_intrinsic(*operation))
            })
        });
    let mut module = module;
    module.forming_choices = crate::representation::RegionalChoices::NONE;
    module.identify_spelling_sites(
        u8::from(head.int32_hints) | (u8::from(head.property_mangling) << 1),
        &mut phase,
    )?;
    phase.finish_retained()?;
    Ok(FormedHead {
        module,
        tail,
        hints_inert,
        properties_inert: property_sites == 0,
    })
}

/// The output families on a formed head: block inlining, flat blocks, the
/// statement rules and spellings, and the late passes that follow them, then
/// string pooling; and the print decisions, written onto the tree (live-16):
/// the printer renders what this artifact chose, and nothing else.
fn form_tail(
    head: FormedHead,
    families: js::OutputFamilies,
    choices: &js::ChoiceMap,
    budget: &mut AllocationBudget<'_>,
) -> Result<js::Module, FormationError> {
    let FormedHead {
        mut module, tail, ..
    } = head;
    if module.conflicts_with_authors(choices) {
        return Err(Unsupported { span: Span::default(), feature: "target choice conflicts with source @choose" }.into());
    }
    let Some(TailContext {
        reuse_normalization,
        rules,
        frames_hidden,
        strict,
        pristine,
        prunes,
        numeric_lengths,
        year,
    }) = tail
    else {
        module.loop_head_declarations = false;
        module.logical_statements = false;
        module.compound_assignments = false;
        module.quotes = false;
        if !module.authored_pool.is_empty() { module.pool_strings(false, budget)?; }
        return Ok(module);
    };
    module.loop_head_declarations = families.loop_heads;
    module.logical_statements = families.logical_statements;
    module.compound_assignments = families.compound_assignments;
    module.quotes = families.quotes;
    // The tail edits storage the head admitted, and can release it: it runs
    // in the scope that owns the head's charges. The artifact's families to
    // their fixed point, then repeated strings, once no other rule reads a
    // literal (M5.3a).
    let context = js::rules::Context {
            reuse_normalization,
        rules,
        frames_hidden,
        strict,
        pristine,
        prunes,
        numeric_lengths,
        year,
        statements: families.statements,
        choices: Some(choices),
        families,
    };
    // The tail's choice sites are this artifact's own.
    module.choice_sites.clear();
    let result = module
        .run_tail_rules(&js::rules::tail(&families, prunes), &context, budget)
        .and_then(|_| {
            if families.string_pooling || families.string_array_packing {
                module.run_rules(&js::rules::pooling(&families), &context, budget)
            } else {
                Ok(0)
            }
        });
    match result {
        Ok(_) => {
            if !families.string_pooling && !module.authored_pool.is_empty() {
                module.pool_strings(false, budget)?;
            }
            module.form_spelling_choices(families, rules, choices, frames_hidden, year, budget)?;
            Ok(module)
        }
        Err(error) => {
            drop(module);
            Err(error.into())
        }
    }
}

/// The value class a cell of this source type always holds, if any.
fn value_class(ty: &Type<'_>) -> Option<js::ValueClass> {
    let object = |ty: &Type<'_>| {
        matches!(
            ty,
            Type::Array(_)
                | Type::Record(_)
                | Type::Map(_, _)
                | Type::Set(_)
                | Type::Regex
                | Type::Struct(_)
                | Type::Class(_)
                | Type::StructInstance { .. }
                | Type::ClassInstance { .. }
                | Type::Function(_)
        )
    };
    match ty {
        Type::Int => Some(js::ValueClass::Int),
        Type::Float => Some(js::ValueClass::Number),
        Type::String => Some(js::ValueClass::String),
        Type::Bool => Some(js::ValueClass::Boolean),
        Type::Nullable(inner) if object(inner) => Some(js::ValueClass::NullableObject),
        other if object(other) => Some(js::ValueClass::Object),
        _ => None,
    }
}

struct FormationContext {
    plan: UnitPlan,
    // Dense only in this unit's owned cells; never a whole-program cell copy.
    cells: Vec<Option<js::BindingId>>,
    // Lookup-only cache. Hash iteration cannot affect target formation order.
    captures: Vec<(CellId, js::BindingId)>,
    products: products::Storage,
    product_parameters: product_calls::Parameters,
}

struct UnitPlan {
    regions: Vec<js::RegionId>,
    values: Vec<ValueStorage>,
    numbers: Vec<NumberFacts>,
    // Selected immutable data belongs to this physical activation. Inline
    // occurrences receive distinct bindings without cloning semantic units.
    shared_strings: Vec<(usize, js::BindingId)>,
    // Source closures inherit an activation even when their physical recipe
    // uses ordinary-function syntax. Declared functions start a new one.
    lexical_owner: ContextId,
    observes_activation: bool,
    /// Generator defaults execute in this call frame; the entry region is
    /// the separately suspended iterator body beneath it.
    generator_entry: Option<js::RegionId>,
    activation_aliases: [Option<js::BindingId>; 2],
    /// Printed strict (a classic script's struct-bearing frame).
    strict_frame: bool,
    reference_parameters: Vec<(CellId, js::BindingId)>,
    prepared_references: Vec<references::PreparedReference>,
}

use super::ambient::{self, Ambient};

struct RecordStorage<'a> {
    family: &'a RecordFamily,
    bindings: Vec<Option<js::BindingId>>,
}

struct Formation<'demand, 'program, 'src, 'budget, 'ledger> {
    budget: &'budget mut AllocationBudget<'ledger>,
    program: &'program Program<'src>,
    /// The program's value ranges. The explicit legacy normalization family
    /// retains its separate local spelling tier.
    ranges: std::sync::Arc<super::ranges::ProgramRanges>,
    uses: Option<&'program UseIndex>,
    contract: JavaScriptCompilationContract,
    demand: &'demand DemandPlan<'program, 'src>,
    compact: bool,
    default_transport: bool,
    module: js::Module,
    contexts: Vec<Option<FormationContext>>,
    // Expression insertion depths, not lexical scope depths: inline bodies
    // keep their caller's storage scope but execute inside its call schedule.
    entry_depths: Vec<usize>,
    records: Vec<RecordStorage<'program>>,
    // Target-boundary classification, allocated only for programs with structs.
    struct_plan: structs::Plan<'src>,
    storage: super::physical_storage::StorageProofs,
    reference_plan: references::Plan,
    /// One hoisted `JS.methodN` adapter factory per calling convention.
    host_factories: Vec<(u8, js::BindingId)>,
    /// The hoisted index-read check (`checks = "development"`), once formed.
    index_check: Option<js::BindingId>,
    /// The hoisted crossing checks, one per shape and absence.
    crossing_checks: Vec<((checks::Crossing, bool), js::BindingId)>,
    shape_helpers: Vec<(shapes::Helper<'src>, js::BindingId)>,
    class_witnesses: Vec<(NominalId, js::BindingId)>,
    payload_cells: Vec<CellId>,
    /// The `int32_hints` output family: the `|0` the compiler printed before
    /// R1 and R11 after an `int` field, member or element read and an `int`
    /// host call's result.
    int32_hints: bool,
    property_mangling: bool,
    preserved_properties: &'demand [String],
    private_fields: super::private_fields::Plan,
    property_sites: usize,
    /// The reads and call results the family hints, counted whatever its
    /// value.
    hint_sites: usize,
    /// The operation `ordinary_expression` is forming (its unit, its id, and
    /// the operation itself, by address), whose node takes the operation's
    /// behaviour when that operation's value is saved (M5.2).
    forming: Option<(UnitId, OpId, *const Operation)>,
    /// Each formed function unit's target function, for export reflection.
    unit_functions: Vec<(UnitId, js::FunctionId)>,
    /// One ES import binding per foreign cell with an `import extern` source.
    foreign_bindings: Vec<(CellId, js::BindingId)>,
    /// Per cell: 0 unknown, 1 nothing writes it after initialization, 2 written.
    stable_cells: Vec<u8>,
    /// Sums formed for `JS.add`, ascending: their literal operands may join
    /// the next literal added to them.
    string_sums: Vec<js::ExprId>,
    /// Whether any unit reads `arguments`, computed on first need.
    arguments_read: Option<bool>,
    /// The source module whose root statements are being formed.
    current_module: u32,
    /// The anchor of the root statements being formed (plan M3.3).
    anchor: js::Anchor,
    /// The program's root point of the operation being formed, while a
    /// module's evaluation forms root statements (M6.5).
    point: Option<u32>,
    /// Forming a module's evaluation: each root statement's anchor comes
    /// from the operations it is formed from.
    classify_roots: bool,
    /// Receiver cells of units formed as methods, read as `this`
    /// (`javascript_methods.rs`).
    this_cells: Vec<CellId>,
    /// Rest lists of units formed as methods, read as `arguments`.
    arguments_cells: Vec<CellId>,
    /// Both kinds: a nested callback reading one cannot be a function of
    /// its own.
    activation_cells: Vec<CellId>,
    /// Rest lists read only at constant indices, and each index's formal.
    formal_lists: Vec<(CellId, Vec<js::BindingId>)>,
    /// Adapter calls whose argument is formed as the method itself.
    method_calls: Vec<(UnitId, CallId)>,
    /// Each static body's prototype method form, once decided.
    prototype_forms: Vec<(UnitId, Option<methods::MethodForm>)>,
    /// Static bodies formed as their class's prototype methods.
    class_methods: Vec<(UnitId, js::FunctionId)>,
    /// Parameters a method form spells without a binding (its receiver as
    /// `this`, its list as `arguments` or formals, or either unread): they
    /// get no storage and no parameter.
    unbound_cells: Vec<CellId>,
}

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    fn error(&self, span: Span, feature: &'static str) -> FormationError {
        FormationError::Unsupported(Unsupported { span, feature })
    }

    /// The function cell a named function's closure initializes, when that
    /// function's `name` cannot be observed: nothing exports the cell and
    /// every read of it only calls the function. Such a function needs no
    /// exact-name carrier; its cell holds it directly.
    /// A load that can be read again at each use: a total read (demand keeps
    /// no evaluation at its site) of a local cell nothing writes after its
    /// initialization. Its uses stay inside the load's own region, where the
    /// binding is in scope, and see the same value there.
    fn rematerialized_cell(
        &mut self,
        context: ContextId,
        value: ValueId,
    ) -> Result<Option<CellId>, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(None);
        };
        if self.demand.context(context).kind.is_inline() {
            return Ok(None);
        }
        let semantic = self.semantic(context);
        let data = self.data(context);
        // CopyValue's default JavaScript recipe is the same persistent
        // value. A chain rooted at an immutable, initialized cell can read
        // that cell at each use without introducing a waiting temporary.
        // Selected product recipes keep their own physical snapshot owner.
        let mut source = value;
        let mut proof_work = 0;
        loop {
            let definition = data.values[source.index()].definition;
            if !matches!(
                data.operations[definition.index()].kind,
                OperationKind::CopyValue
            ) {
                break;
            }
            if self.demand.product_for_value(semantic, source).is_some() {
                return Ok(None);
            }
            source = data
                .operands(data.operations[definition.index()].operands)
                .unwrap()[0];
            proof_work += 1;
        }
        let definition = data.values[source.index()].definition;
        let OperationKind::Load(place) = data.operations[definition.index()].kind else {
            return Ok(None);
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return Ok(None);
        };
        self.work(proof_work)?;
        let program = self.program;
        // A parameter read never throws. Demand keeps a classic script's
        // parameter reads in place because mapped `arguments` can alias them;
        // with no `arguments` read anywhere, nothing but a store can change one.
        if matches!(
            program.cells[cell.index()].binding,
            CellBinding::Parameter(_)
        ) {
            if self.contract.execution == crate::compilation_contract::JavaScriptExecution::Script
                && self.reads_arguments()?
            {
                return Ok(None);
            }
        } else if self.demand.needs_execution(context, definition) {
            return Ok(None);
        }
        if program.cells[cell.index()].binding == CellBinding::Foreign
            || references::is_reference(program, cell)
        {
            return Ok(None);
        }
        self.product_lookup()?;
        if self.demand.product_for_cell(cell).is_some() || self.addressed_cell(context, cell)? {
            return Ok(None);
        }
        let Some(readers) = uses.unit(semantic).and_then(|uses| uses.value_uses(value)) else {
            return Ok(None);
        };
        self.work(readers.len())?;
        if readers
            .iter()
            .any(|reader| matches!(reader, ValueUse::RegionResult(_)))
        {
            return Ok(None);
        }
        if self.stable_cells.is_empty() {
            self.stable_cells =
                self.budget
                    .filled(AllocationClass::Scratch, program.cells.len(), 0u8)?;
        }
        if self.stable_cells[cell.index()] == 0 {
            let Some(users) = uses.cell(cell) else {
                return Ok(None);
            };
            self.work(users.sites().len())?;
            let written = users.reference_exposed()
                || users.sites().iter().any(|site| {
                    matches!(
                        site,
                        CellUseSite::Unit {
                            usage: CellUse::Write { .. } | CellUse::Reference { .. },
                            ..
                        }
                    )
                });
            self.stable_cells[cell.index()] = if written { 2 } else { 1 };
        }
        Ok((self.stable_cells[cell.index()] == 1).then_some(cell))
    }

    /// `object.length`, spelled either way.
    pub(super) fn length_member(&self, expression: js::ExprId) -> bool {
        let js::Expr::Member { property, .. } = &self.module.expressions[expression.index()] else {
            return false;
        };
        match property {
            js::Property::Named(name) => name == "length",
            js::Property::Computed(key) => matches!(
                &self.module.expressions[key.index()],
                js::Expr::Literal(js::Literal::String(value)) if value.as_unicode() == Some("length")
            ),
        }
    }

    /// Whether any unit reads the ambient `arguments` object.
    fn reads_arguments(&mut self) -> Result<bool, FormationError> {
        if let Some(reads) = self.arguments_read {
            return Ok(reads);
        }
        let Some(uses) = self.uses else {
            return Ok(true);
        };
        let program = self.program;
        self.work(program.cells.len())?;
        let reads = program.cells.iter().enumerate().any(|(index, cell)| {
            ambient::classify(cell) == Some(Ambient::Arguments)
                && CellId::from_index(index)
                    .and_then(|cell| uses.cell(cell))
                    .is_some_and(|users| !users.sites().is_empty())
        });
        self.arguments_read = Some(reads);
        Ok(reads)
    }

    fn private_function_cell(
        &mut self,
        unit: ContextId,
        value: ValueId,
    ) -> Result<Option<CellId>, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(None);
        };
        let semantic = self.semantic(unit);
        let data = self.data(unit);
        let OperationKind::Closure(child) =
            data.operations[data.values[value.index()].definition.index()].kind
        else {
            return Ok(None);
        };
        // A class constructor is a class value, whatever reads it.
        let body = self.program.unit(child).unwrap();
        if body.kind != UnitKind::Function
            || body.constructor_of.is_some()
            || self.struct_plan.wrapped(child)
            || self.payload_captures(child)?
        {
            return Ok(None);
        }
        let Some(
            &[ValueUse::Operand {
                operation: initialize,
                position: 0,
            }],
        ) = uses.unit(semantic).and_then(|uses| uses.value_uses(value))
        else {
            return Ok(None);
        };
        let OperationKind::Initialize(cell) = data.operations[initialize.index()].kind else {
            return Ok(None);
        };
        if self.program.cells[cell.index()].binding != CellBinding::Function(child) {
            return Ok(None);
        }
        Ok(self.callee_only_cell(cell)?.then_some(cell))
    }

    /// Whether every read of `cell` is only called, and nothing else writes,
    /// exports or references it: its value's identity and name are private.
    fn callee_only_cell(&mut self, cell: CellId) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        if self.namespace_member(cell)? {
            return Ok(false);
        }
        let Some(cell_uses) = uses.cell(cell) else {
            return Ok(false);
        };
        for site in cell_uses.sites() {
            self.work(1)?;
            match *site {
                CellUseSite::Unit {
                    usage: CellUse::Initialize(_) | CellUse::Capture,
                    ..
                } => {}
                CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Read { operation, .. },
                } => {
                    let Some(loaded) = self.program.units[reader.index()].data().operations
                        [operation.index()]
                    .result
                    else {
                        return Ok(false);
                    };
                    let Some(readers) = uses.unit(reader).and_then(|uses| uses.value_uses(loaded))
                    else {
                        return Ok(false);
                    };
                    self.work(readers.len())?;
                    if !readers
                        .iter()
                        .all(|reader| matches!(reader, ValueUse::CallCallee { .. }))
                    {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    /// A closure whose name nothing can read: each use invokes it, directly
    /// or through local cells it initializes or is stored into.
    fn unobserved_closure_name(
        &mut self,
        unit: ContextId,
        value: ValueId,
    ) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        let data = self.data(unit);
        let Some(readers) = uses
            .unit(self.semantic(unit))
            .and_then(|uses| uses.value_uses(value))
        else {
            return Ok(false);
        };
        self.work(readers.len())?;
        for reader in readers {
            match *reader {
                ValueUse::CallCallee { .. } => {}
                ValueUse::CallArgument {
                    call, position: 0, ..
                } if invokes_argument(&data.calls[call.index()].target) => {}
                ValueUse::Operand {
                    operation,
                    position: 0,
                } => {
                    let cell = match data.operations[operation.index()].kind {
                        OperationKind::Initialize(cell) => cell,
                        OperationKind::Store(place) => match data.places[place.index()] {
                            Place::Cell(cell) => cell,
                            _ => return Ok(false),
                        },
                        _ => return Ok(false),
                    };
                    if !self.name_unobserved_cell(cell)? {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(!readers.is_empty())
    }

    /// Whether no read of local `cell` can observe a function's name: nothing
    /// exports it or loads it through a module namespace, and each read only
    /// invokes the value. Other values written to the cell do not matter.
    fn name_unobserved_cell(&mut self, cell: CellId) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        if self.program.cells[cell.index()].binding != CellBinding::Local
            || self.namespace_member(cell)?
        {
            return Ok(false);
        }
        let Some(cell_uses) = uses.cell(cell) else {
            return Ok(false);
        };
        for site in cell_uses.sites() {
            self.work(1)?;
            match *site {
                CellUseSite::Unit {
                    usage: CellUse::Initialize(_) | CellUse::Capture | CellUse::Write { .. },
                    ..
                } => {}
                CellUseSite::Unit {
                    unit: reader,
                    usage: CellUse::Read { operation, place },
                } => {
                    if !self.live(reader, operation)? {
                        continue;
                    }
                    let data = self.program.units[reader.index()].data();
                    if !matches!(data.places[place.index()], Place::Cell(root) if root == cell) {
                        return Ok(false);
                    }
                    let operation = &data.operations[operation.index()];
                    match operation.kind {
                        OperationKind::Load(_) => {
                            let Some(loaded) = operation.result else {
                                return Ok(false);
                            };
                            if !self.invoked_only(reader, loaded)? {
                                return Ok(false);
                            }
                        }
                        OperationKind::Call(call)
                            if matches!(
                                data.calls[call.index()].target,
                                CallTarget::Reference { place: callee } if callee == place
                            ) => {}
                        _ => return Ok(false),
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Whether every use of `value` that runs invokes it.
    fn invoked_only(&mut self, unit: UnitId, value: ValueId) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        let Some(readers) = uses.unit(unit).and_then(|uses| uses.value_uses(value)) else {
            return Ok(false);
        };
        self.work(readers.len())?;
        let data = self.program.units[unit.index()].data();
        for reader in readers {
            match *reader {
                ValueUse::CallCallee { .. } => {}
                ValueUse::CallArgument {
                    call, position: 0, ..
                } if invokes_argument(&data.calls[call.index()].target) => {}
                ValueUse::Operand { operation, .. }
                | ValueUse::CallArgument { operation, .. }
                | ValueUse::PlaceReceiver { operation, .. }
                | ValueUse::PlaceKey { operation, .. } => {
                    if self.live(unit, operation)? {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Whether any demand context of `unit` runs `operation`.
    fn live(&mut self, unit: UnitId, operation: OpId) -> Result<bool, FormationError> {
        self.work(self.demand.contexts().len())?;
        Ok(self.demand.needs_operation_anywhere(unit, operation))
    }

    /// Whether a function value initializes or is stored into a cell that a
    /// declared public boundary publishes: a root export or a module
    /// namespace member.
    fn flows_into_exported_cell(
        &mut self,
        unit: ContextId,
        value: ValueId,
    ) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(true);
        };
        let data = self.data(unit);
        let Some(readers) = uses
            .unit(self.semantic(unit))
            .and_then(|uses| uses.value_uses(value))
        else {
            return Ok(true);
        };
        self.work(readers.len())?;
        for reader in readers {
            let ValueUse::Operand {
                operation,
                position: 0,
            } = *reader
            else {
                continue;
            };
            let cell = match data.operations[operation.index()].kind {
                OperationKind::Initialize(cell) => cell,
                OperationKind::Store(place) => match data.places[place.index()] {
                    Place::Cell(cell) => cell,
                    _ => continue,
                },
                _ => continue,
            };
            if self.namespace_member(cell)?
                || (self.contract.abi.preserve_root_exports
                    && self
                        .program
                        .value_exports()
                        .any(|(_, exported)| exported == cell))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A dynamic import's namespace hands the cell's value to its importer.
    fn namespace_member(&mut self, cell: CellId) -> Result<bool, FormationError> {
        let program = self.program;
        for module in program.modules.iter() {
            self.work(1 + module.namespace.len())?;
            if module.namespace.iter().any(|&(_, member)| member == cell) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn semantic(&self, context: ContextId) -> UnitId {
        self.demand.context(context).unit
    }
    fn data(&self, context: ContextId) -> &'program UnitData {
        self.program.units[self.semantic(context).index()].data()
    }
    fn plan(&self, context: ContextId) -> &UnitPlan {
        &self.contexts[context.index()].as_ref().unwrap().plan
    }
    fn work(&mut self, units: usize) -> Result<(), FormationError> {
        self.budget.work(
            WorkKind::Render,
            u64::try_from(units).map_err(|_| AllocationError::Capacity)?,
        )?;
        Ok(())
    }
    fn text(&mut self, value: &str) -> Result<String, FormationError> {
        Ok(self.budget.string(AllocationClass::Retained, value)?)
    }
    fn string(
        &mut self,
        value: &crate::literal::StringValue,
    ) -> Result<crate::literal::StringValue, FormationError> {
        Ok(self.budget.string_value(AllocationClass::Retained, value)?)
    }
    fn format(&mut self, value: std::fmt::Arguments<'_>) -> Result<String, FormationError> {
        Ok(self.budget.format(AllocationClass::Retained, value)?)
    }
    fn expression(&mut self, node: js::Expr) -> Result<js::ExprId, FormationError> {
        Ok(self.module.expression_in(node, None, self.budget)?)
    }
    fn literal(&mut self, literal: js::Literal) -> Result<js::ExprId, FormationError> {
        self.expression(js::Expr::Literal(literal))
    }

    fn authored_literal(&mut self, literal: js::Literal, choices: crate::representation::RegionalChoices) -> Result<js::ExprId, FormationError> {
        let prior = self.module.forming_choices;
        self.module.forming_choices = choices;
        let result = self.literal(literal);
        self.module.forming_choices = prior;
        result
    }
    fn family_literal(&mut self, family: usize) -> Result<js::ExprId, FormationError> {
        let value = self.string(self.demand.strings()[family].payload(self.program))?;
        self.authored_literal(js::Literal::String(value), self.demand.strings()[family].authored())
    }

    fn class_field_key(&mut self, field: FieldRef) -> Result<js::ExprId, FormationError> {
        let (key, _) = self
            .program
            .class_field(field)
            .ok_or_else(|| self.error(Span::default(), "unknown class field"))?;
        let declared = &self.program.strings[key.index()];
        let mut preserved = false;
        for name in self.preserved_properties {
            self.budget
                .work(WorkKind::Analysis, name.len() as u64 + 1)?;
            preserved |= declared.as_unicode() == Some(name.as_str());
        }
        let slot = self
            .private_fields
            .slot(field, self.budget)?
            .filter(|_| !preserved);
        self.property_sites += usize::from(slot.is_some());
        let text = if let Some(slot) = slot.filter(|_| self.property_mangling) {
            super::private_fields::Plan::spelling(slot, self.budget)?.into()
        } else {
            self.string(declared)?
        };
        self.literal(js::Literal::String(text))
    }

    fn reference(&mut self, binding: js::BindingId) -> Result<js::ExprId, FormationError> {
        self.expression(js::Expr::Binding(binding))
    }
    fn statement(
        &mut self,
        region: js::RegionId,
        statement: js::Statement,
    ) -> Result<(), FormationError> {
        // A function declaration only creates its function (design §6,
        // rule 1): a definition wherever it is formed.
        let anchor = match statement {
            js::Statement::Function { .. } => js::Anchor::Definition,
            js::Statement::Let { .. } => self.anchor,
            // A source operation's definition fact can survive the
            // lowering of its value to an evaluation. That statement
            // declares no binding: delivery must keep it at its module's
            // evaluation point, as its release-mode fallback already did.
            _ => js::Anchor::Anchored,
        };
        self.push_statement(
            region,
            statement,
            js::RootRow::new(self.current_module, anchor).at(self.point),
        )
    }
    /// A root statement formation creates for the whole program, not for the
    /// module being formed (a shared adapter, a D2 wrapper): a synthetic
    /// definition (design §6), beside the current module.
    pub(super) fn helper_statement(
        &mut self,
        region: js::RegionId,
        statement: js::Statement,
    ) -> Result<(), FormationError> {
        self.push_statement(
            region,
            statement,
            js::RootRow::synthetic(self.current_module),
        )
    }
    fn push_statement(
        &mut self,
        region: js::RegionId,
        statement: js::Statement,
        row: js::RootRow,
    ) -> Result<(), FormationError> {
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.regions[region.index()].statements,
            statement,
        )?;
        if region == self.module.root {
            self.budget
                .push(AllocationClass::Retained, &mut self.module.root_rows, row)?;
        }
        Ok(())
    }
    fn append<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), FormationError> {
        self.budget.push(AllocationClass::Retained, values, value)?;
        Ok(())
    }
    fn drop_scratch<T>(&mut self, values: Vec<T>) -> Result<(), FormationError> {
        let bytes = values
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(AllocationError::Capacity)?;
        drop(values);
        self.budget
            .release(AllocationClass::Scratch, bytes as u64)?;
        Ok(())
    }
    fn ambient_expression(&mut self, ambient: Ambient) -> Result<js::ExprId, FormationError> {
        let node = self.ambient_node(ambient)?;
        self.expression(node)
    }
    fn ambient_node(&mut self, ambient: Ambient) -> Result<js::Expr, FormationError> {
        Ok(match ambient {
            Ambient::This => js::Expr::This,
            Ambient::Arguments => js::Expr::Host(js::Host::new(self.text("arguments")?)),
        })
    }

    /// A call the effect contract drops (`DemandPlan::stripped_call`).
    fn stripped_log_call(&self, unit: UnitId, call: CallId) -> bool {
        self.demand.stripped_call(unit, call)
    }

    fn elided_log_lookup(&mut self, unit: UnitId, value: ValueId) -> Result<bool, FormationError> {
        if !self.demand.stripped_lookup_value(unit, value) {
            return Ok(false);
        }
        // Complete use coverage is required for removing the lookup. Every
        // inspected occurrence is charged even when it prevents this choice.
        let Some(uses) = self
            .uses
            .and_then(|uses| uses.unit(unit))
            .and_then(|uses| uses.value_uses(value))
        else {
            return Ok(false);
        };
        for usage in uses {
            self.work(1)?;
            if !self.demand.stripped_lookup_use(unit, usage) {
                return Ok(false);
            }
        }
        Ok(!uses.is_empty())
    }

    fn record_binding(&self, record: usize, slot: u32) -> Result<js::BindingId, FormationError> {
        self.records[record].bindings[slot as usize]
            .ok_or_else(|| self.error(Span::default(), "record slot outside its activation"))
    }

    fn initialize_record(
        &mut self,
        unit: ContextId,
        region: js::RegionId,
        record: usize,
    ) -> Result<(), FormationError> {
        for slot in 0..self.records[record].bindings.len() {
            self.work(1)?;
            let Some(binding) = self.records[record].bindings[slot] else {
                continue;
            };
            let initial = self.records[record].family.slots()[slot].initial_value;
            let value = match initial {
                Some(value) => self.value(unit, value)?,
                None => self.literal(js::Literal::Null)?,
            };
            self.statement(
                region,
                js::Statement::Let {
                    binding,
                    value: Some(value),
                },
            )?;
        }
        Ok(())
    }

    fn plan_context(
        &mut self,
        context: ContextId,
        body: js::RegionId,
    ) -> Result<(), FormationError> {
        self.work(1)?;
        let source_context = self.demand.context(context);
        let unit = source_context.unit;
        let parent = source_context.parent;
        let inline = matches!(source_context.kind, ContextKind::Inline { .. });
        if self.contexts[context.index()].is_some() {
            return Err(self.error(
                Span::default(),
                "multiply materialized semantic function unit",
            ));
        }
        let strict_frame = self.validate_struct_context(context)?;
        self.validate_reference_context(context)?;
        let data = self.program.units[unit.index()].data();
        if !inline { self.module.set_region_choices(body, data.regions[data.entry.index()].authored, self.budget)?; }

        let mut cells = self.budget.filled(
            AllocationClass::Scratch,
            self.demand.owned_cells(unit).len(),
            None,
        )?;
        let mut expression_regions =
            self.budget
                .filled(AllocationClass::Scratch, data.regions.len(), false)?;
        for operation in &data.operations {
            self.work(1)?;
            match operation.kind {
                OperationKind::ShortCircuit { right, .. } => {
                    expression_regions[right.index()] = true
                }
                OperationKind::Select { yes, no } => {
                    expression_regions[yes.index()] = true;
                    expression_regions[no.index()] = true;
                }
                OperationKind::Loop { test, update, .. } => {
                    expression_regions[test.index()] = true;
                    expression_regions[update.index()] = true;
                }
                _ => {}
            }
        }
        let generator_entry = (data.suspension == Suspension::Generator
            && data.parameter_region.is_some()).then_some(body);
        let execution_body = if generator_entry.is_some() {
            self.module.region_in(self.module.regions[body.index()].scope, self.budget)?
        } else { body };
        let mut regions = self
            .budget
            .filled(AllocationClass::Scratch, data.regions.len(), execution_body)?;
        let entry_depth = if self.compact {
            self.entry_depths[context.index()]
        } else {
            0
        };
        if self.compact && entry_depth == usize::MAX {
            return Err(self.error(Span::default(), "missing physical context depth"));
        }
        let mut enclosing = entry_depth;
        let mut pending = self
            .budget
            .copy_slice(AllocationClass::Scratch, &[(data.entry, entry_depth)])?;
        while let Some((parent, base_depth)) = pending.pop() {
            self.work(1)?;
            enclosing = enclosing.max(base_depth);
            let mut call_frames = 0usize;
            // Region ownership, not incidental arena numbering, establishes
            // the target lexical hierarchy after semantic edits.
            let data = self.program.units[unit.index()].data();
            for operation in &data.regions[parent.index()].operations {
                self.work(1)?;
                let kind = &data.operations[operation.index()].kind;
                if matches!(kind, OperationKind::Call(_)) {
                    call_frames = call_frames
                        .checked_sub(1)
                        .expect("checked balanced call schedule");
                }
                // A prepared-call ancestor adds at most Assign, ToInt32/Void,
                // Call and the argument Sequence. The placement planner owns
                // its own local frames; child regions/contexts inherit these.
                let site_depth = base_depth.saturating_add(call_frames.saturating_mul(4));
                if self.compact {
                    if let Some(child) = self.demand.child(context, *operation) {
                        let layers = match kind {
                            // A deferred closure lands inside a bounded
                            // consumer tree; a captured one under an Assign.
                            OperationKind::Closure(created) => value_placement::DEFERRED_CLOSURE_ENTRY
                                + if self.payload_captures(*created)? { 4 } else { 0 },
                            OperationKind::Call(_) => 2, // Assign→inline schedule Sequence→expression.
                            _ => {
                                return Err(self
                                    .error(Span::default(), "unsupported physical context owner"));
                            }
                        };
                        self.entry_depths[child.index()] = site_depth.saturating_add(layers);
                    }
                }
                for child in kind.child_regions() {
                    self.work(1)?;
                    if expression_regions[child.index()] { self.prepare_expression_bindings(unit, child)?; }
                    regions[child.index()] = if expression_regions[child.index()] {
                        regions[parent.index()]
                    } else {
                        let enclosing = if generator_entry.is_some() && data.parameter_region == Some(child) {
                            body
                        } else { regions[parent.index()] };
                        self.module.region_in(
                            self.module.regions[enclosing.index()].scope,
                            self.budget,
                        )?
                    };
                    if !expression_regions[child.index()] {
                        self.module.set_region_choices(regions[child.index()], data.regions[child.index()].authored, self.budget)?;
                    }
                    let layers = match kind {
                        // Captured owner + lazy operator + optional arm Sequence.
                        OperationKind::Select { .. } => 3,
                        OperationKind::ShortCircuit {
                            kind: ShortCircuit::Nullish,
                            ..
                        } => 4,
                        OperationKind::ShortCircuit { .. } => 3,
                        // A counted-copy block in an expression stays at its
                        // occurrence as Call → Function → body, rather than
                        // a statement block. Include that frame in placement.
                        OperationKind::Block(_)
                            if call_frames != 0 || expression_regions[parent.index()] =>
                        {
                            3
                        }
                        // Statement child or loop condition/update Sequence.
                        _ => 1,
                    };
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut pending,
                        (child, site_depth.saturating_add(layers)),
                    )?;
                }
                if matches!(kind, OperationKind::PrepareCall(_)) {
                    call_frames = call_frames
                        .checked_add(1)
                        .ok_or(AllocationError::Capacity)?;
                }
            }
            debug_assert_eq!(call_frames, 0);
        }
        for &cell_id in self.demand.owned_cells(unit) {
            self.work(1)?;
            if self.unbound_cells.contains(&cell_id) {
                continue;
            }
            // A static body its class's prototype method is has no cell of
            // its own: nothing reads it.
            if let CellBinding::Function(method) = self.program.cells[cell_id.index()].binding {
                if self.prototype_method_form(method)?.is_some() {
                    continue;
                }
            }
            let index = cell_id.index();
            let cell = &self.program.cells[index];
            if cell.binding != CellBinding::Foreign {
                self.product_lookup()?;
                if self.demand.helper_for_cell(cell_id).is_some()
                    || self.demand.product_for_cell(cell_id).is_some()
                    || (self.demand.context(context).kind.is_inline()
                        && references::is_reference(self.program, cell_id))
                {
                    continue;
                }
                let region = if generator_entry.is_some() && matches!(cell.binding, CellBinding::Parameter(_)) {
                    body
                } else { regions[cell.region.index()] };
                if let Some(record) = self.demand.record_for_cell(cell_id) {
                    for slot in 0..self.records[record].bindings.len() {
                        self.work(1)?;
                        self.work(1)?;
                        if !self.demand.needs_slot(context, record, slot as u32) {
                            continue;
                        }
                        let binding = {
                            let binding = js::Binding {
                                source_symbol: None,
                                scope: self.module.regions[region.index()].scope,
                                spelling: self
                                    .format(format_args!("record_{index}_slot_{slot}"))?,
                                pinned: false,
                                class: None,
                                defined: false,
                            };
                            self.module.binding_in(binding, self.budget)?
                        };
                        self.records[record].bindings[slot] = Some(binding);
                    }
                    continue;
                }
                // The parameter list is a callable ABI obligation even when
                // its contents are unused. Inline parameters have no such ABI.
                if !self.demand.needs_binding(context, cell_id) {
                    continue;
                }
                let binding = {
                    let binding = js::Binding {
                        source_symbol: cell.source_symbol,
                        scope: self.module.regions[region.index()].scope,
                        // Stable legal spelling also handles reserved words in the
                        // source language; observable function names live on values.
                        spelling: if {
                            self.work(cell.name.len())?;
                            js::identifier(&cell.name)
                        } {
                            self.text(&cell.name)?
                        } else {
                            self.format(format_args!("cell_{index}"))?
                        },
                        pinned: false,
                        class: None,
                        defined: false,
                    };
                    self.module.binding_in(binding, self.budget)?
                };
                if cell.declared_const && rules::const_data::aggregate(&self.program.types[cell.ty.index()]) {
                    self.budget.push(AllocationClass::Retained, &mut self.module.immutable_data, binding)?;
                }
                // What the cell holds, for the tree's type-directed edits.
                self.module.bindings[binding.index()].class =
                    if self.payload_cell(cell_id)? { None } else { value_class(&self.program.types[cell.ty.index()]) };
                // When the program settles a module cell (M6.5): reads in
                // code that cannot run before then find its value.
                if !inline {
                    if let Some(point) = self.demand.initialization().settled(cell_id) {
                        self.module
                            .settle_in(binding, point.ordinal(), self.budget)?;
                    }
                }
                cells[self.demand.cell_ordinal(cell_id)] = Some(binding);
                if inline {
                    // Each occurrence has private scalar cells in the caller
                    // activation. Initialization still executes at the call.
                    self.statement(
                        region,
                        js::Statement::Let {
                            binding,
                            value: None,
                        },
                    )?;
                }
            }
        }
        let mut values = self.budget.filled(
            AllocationClass::Scratch,
            data.values.len(),
            ValueStorage::Absent,
        )?;
        for (index, slot) in values.iter_mut().enumerate() {
            self.work(1)?;
            let value = ValueId::from_index(index).unwrap();
            let definition = self.program.units[unit.index()].data().values[index].definition;
            let operation = &self.program.units[unit.index()].data().operations[definition.index()];
            self.product_lookup()?;
            if !self.demand.needs_value(context, value)
                || self.demand.product_for_value(unit, value).is_some()
                || (matches!(operation.kind, OperationKind::Closure(_))
                    && self.private_function_cell(context, value)?.is_some())
                || matches!(operation.kind, OperationKind::Closure(created)
                    if self.prototype_method_form(created)?.is_some())
                || self.formal_list_value(context, value).is_some()
                || matches!(operation.kind, OperationKind::Constant(_))
                || self.elided_log_lookup(unit, value)?
                || self.demand.string_value(unit, value).is_some_and(|family| {
                    matches!(
                        self.demand.strings()[family].choice(),
                        StringChoice::SharedLiteral { .. }
                    )
                })
                || matches!(
                    self.demand.record_operation(unit, definition),
                    Some(RecordOperation::ElidedHandle)
                )
                || matches!(
                    self.demand.helper_operation(unit, definition),
                    Some(HelperOperation::Elided)
                )
            {
                continue;
            }
            if self.compact {
                if let Some(cell) = self.rematerialized_cell(context, value)? {
                    *slot = ValueStorage::Rematerialized(cell);
                    continue;
                }
            }
            *slot = if self.compact
                && !matches!(
                    self.demand.helper_operation(unit, definition),
                    Some(HelperOperation::Call(_))
                )
                && !(self.demand.string_operation(unit, definition).is_some()
                    && self.demand.needs_execution(context, definition))
            {
                ValueStorage::Candidate
            } else {
                ValueStorage::Required
            };
        }
        if self.compact {
            let mut closure_entries =
                self.budget
                    .filled(AllocationClass::Scratch, data.values.len(), 0usize)?;
            value_placement::plan_with_closures(
                data,
                self.demand,
                context,
                &mut values,
                Some(&mut closure_entries),
                PlacementDepth {
                    enclosing,
                    limit: js::MAX_NESTING,
                },
                self.budget,
            )?;
            // A deferred closure's body starts below the tree that holds it;
            // a captured one's under its assignment.
            for (index, operation) in data.operations.iter().enumerate() {
                self.work(1)?;
                let (OperationKind::Closure(created), Some(result)) = (&operation.kind, operation.result)
                else {
                    continue;
                };
                let Some(child) = self.demand.child(context, OpId::from_index(index).unwrap())
                else {
                    continue;
                };
                let factory_layers = if self.payload_captures(*created)? { 4 } else { 0 };
                let depth = &mut self.entry_depths[child.index()];
                if !matches!(values[result.index()], ValueStorage::Deferred(_)) {
                    *depth = depth
                        .saturating_sub(value_placement::DEFERRED_CLOSURE_ENTRY)
                        .saturating_add(value_placement::CAPTURED_CLOSURE_ENTRY);
                } else if closure_entries[result.index()] != 0 {
                    *depth = closure_entries[result.index()].saturating_add(factory_layers);
                }
            }
            self.drop_scratch(closure_entries)?;
        }
        for (index, slot) in values.iter_mut().enumerate() {
            self.work(1)?;
            if !matches!(slot, ValueStorage::Required) {
                continue;
            }
            let definition = data.values[index].definition;
            let operation = &data.operations[definition.index()];
            let region = regions[operation.region.index()];
            let binding = {
                let binding = js::Binding {
                    source_symbol: None,
                    scope: self.module.regions[region.index()].scope,
                    spelling: self.format(format_args!("value_{}_{}", context.index(), index))?,
                    pinned: false,
                    class: None,
                    defined: false,
                };
                self.module.binding_in(binding, self.budget)?
            };
            self.statement(
                region,
                js::Statement::Let {
                    binding,
                    value: None,
                },
            )?;
            *slot = ValueStorage::Captured(binding);
        }
        let numbers = if self.compact && self.int32_hints {
            self.budget.filled(
                AllocationClass::Scratch,
                data.values.len(),
                NumberFacts::UNKNOWN,
            )?
        } else {
            Vec::new()
        };
        let mut shared_strings = Vec::new();
        for family in self.demand.shared_strings(context) {
            self.work(1)?;
            let binding = {
                let binding = js::Binding {
                    source_symbol: None,
                    scope: self.module.regions[body.index()].scope,
                    spelling: self.format(format_args!("string_{}_{}", context.index(), family))?,
                    pinned: false,
                    class: None,
                    defined: false,
                };
                self.module.binding_in(binding, self.budget)?
            };
            // Ordinary activations initialize at entry. An inline occurrence
            // initializes inside its original call schedule, including loops
            // and lazy arms; merely declaring its storage performs no work.
            let value = if inline {
                None
            } else {
                Some({
                    self.family_literal(family)
                }?)
            };
            self.statement(body, js::Statement::Let { binding, value })?;
            self.budget.push(
                AllocationClass::Scratch,
                &mut shared_strings,
                (family, binding),
            )?;
        }
        let lexical_owner =
            if ambient::inherits(self.program, self.program.units[unit.index()].data()) {
                self.plan(parent.ok_or_else(|| {
                    self.error(
                        Span::default(),
                        "source closure without a lexical activation",
                    )
                })?)
                .lexical_owner
            } else {
                context
            };
        self.drop_scratch(expression_regions)?;
        self.drop_scratch(pending)?;
        self.contexts[context.index()] = Some(FormationContext {
            cells,
            products: products::Storage::default(),
            product_parameters: product_calls::Parameters::default(),
            captures: self
                .budget
                .vector(AllocationClass::Scratch, data.captures.len())?,
            plan: UnitPlan {
                regions,
                values,
                numbers,
                shared_strings,
                lexical_owner,
                observes_activation: false,
                generator_entry,
                activation_aliases: [None; 2],
                strict_frame,
                reference_parameters: Vec::new(),
                prepared_references: Vec::new(),
            },
        });
        self.plan_product_parameters(context)?;
        self.plan_product_context(context)?;
        self.initialize_reference_context(context)?;
        Ok(())
    }

    fn number(&self, unit: ContextId, value: ValueId) -> NumberFacts {
        if !self.compact {
            return NumberFacts::UNKNOWN;
        }
        // Semantic ranges have one owner. The legacy local tier remains
        // only for the explicit int32_hints spelling alternative.
        if !self.int32_hints {
            return self.ranges.number(self.semantic(unit), value);
        }
        let data = self.data(unit);
        let local = match data.operations[data.values[value.index()].definition.index()].kind {
            OperationKind::Constant(Constant::Integer(value)) => {
                NumberFacts::literal(f64::from(value))
            }
            OperationKind::Constant(Constant::Number(bits)) => {
                NumberFacts::literal(f64::from_bits(bits))
            }
            _ => self.plan(unit).numbers[value.index()],
        };
        // Explicit legacy spelling family: codec repetition can prefer the
        // older normalizations, so it competes alongside the shared ranges.
        local
    }

    /// One admitted fixed-cost transfer per emitted numeric operation. Facts
    /// describe normal completion, without authorizing movement or deletion.
    /// Raw result knowledge stays distinct from the source's ToInt32 obligation.
    fn transfer_number(
        &mut self,
        unit: ContextId,
        operation: &Operation,
        normalize: bool,
        transfer: impl FnOnce(&Self) -> NumberFacts,
    ) -> Result<NumberFacts, FormationError> {
        if !self.compact || (!self.int32_hints && !normalize) {
            return Ok(NumberFacts::UNKNOWN);
        }
        self.budget.work(WorkKind::Analysis, 1)?;
        let raw = transfer(self);
        if let Some(result) = operation.result.filter(|_| self.int32_hints) {
            self.contexts[unit.index()].as_mut().unwrap().plan.numbers[result.index()] =
                if normalize { raw.to_int32() } else { raw };
        }
        Ok(raw)
    }

    fn value(&mut self, unit: ContextId, value: ValueId) -> Result<js::ExprId, FormationError> {
        let formed = self.value_representation(unit, value)?;
        if self.encoded(unit, value)? {
            // Its only use is a `JsValue` position: the D2 public shape.
            let program = self.program;
            let id = self.data(unit).values[value.index()].ty;
            if program.types[id.index()].callable_signature().is_some() {
                return self.public_callable(id, formed);
            }
            return self.public_value(&program.types[id.index()], formed, false);
        }
        Ok(formed)
    }

    fn value_representation(
        &mut self,
        unit: ContextId,
        value: ValueId,
    ) -> Result<js::ExprId, FormationError> {
        self.work(1)?;
        self.product_lookup()?;
        if self
            .demand
            .product_for_value(self.semantic(unit), value)
            .is_some()
        {
            return self.packed_product_snapshot(unit, value);
        }
        if let Some(family) = self.demand.string_value(self.semantic(unit), value) {
            if matches!(
                self.demand.strings()[family].choice(),
                StringChoice::SharedLiteral { .. }
            ) {
                self.work(
                    (usize::BITS - self.plan(unit).shared_strings.len().leading_zeros()) as usize,
                )?;
                let strings = &self.plan(unit).shared_strings;
                let position = strings
                    .binary_search_by_key(&family, |(family, _)| *family)
                    .map_err(|_| self.error(Span::default(), "missing shared string storage"))?;
                return Ok(self.reference(strings[position].1)?);
            }
        }
        let operation =
            &self.data(unit).operations[self.data(unit).values[value.index()].definition.index()];
        let literal = match &operation.kind {
            OperationKind::Constant(Constant::Integer(value)) => {
                js::Literal::Number(f64::from(*value))
            }
            OperationKind::Constant(Constant::Number(bits)) => {
                let value = f64::from_bits(*bits);
                if !value.is_finite() {
                    // These are semantic numbers, not JavaScript identifiers:
                    // host bindings named NaN/Infinity must not affect them.
                    let left = self.literal(js::Literal::Number(if value.is_nan() {
                        0.0
                    } else if value.is_sign_negative() {
                        -1.0
                    } else {
                        1.0
                    }))?;
                    let right = self.literal(js::Literal::Number(0.0))?;
                    return self.expression(js::Expr::Binary {
                        op: js::Binary::Divide,
                        left,
                        right,
                    });
                }
                js::Literal::Number(value)
            }
            OperationKind::Constant(Constant::String(id)) => {
                let text = self.string(&self.program.strings[id.index()])?;
                let expression = self.authored_literal(js::Literal::String(text), operation.authored)?;
                if self.compact {
                    let observation = self.demand.observation(unit, value, |n| {
                        self.budget.work(WorkKind::Render, n as u64)
                    })?;
                    let weak = match observation {
                        facts::ObservationDemand::Truthy => {
                            Some(js::WeakLiteralObservation::Truthy)
                        }
                        facts::ObservationDemand::Nullish => {
                            Some(js::WeakLiteralObservation::Nullish)
                        }
                        facts::ObservationDemand::Discarded | facts::ObservationDemand::Exact => {
                            None
                        }
                    };
                    if let Some(weak) = weak {
                        self.module.observe_in(
                            js::LiteralAlternative::new(expression, weak),
                            self.budget,
                        )?;
                    }
                }
                return Ok(expression);
            }
            OperationKind::Constant(Constant::Boolean(value)) => js::Literal::Bool(*value),
            OperationKind::Constant(Constant::Null) => js::Literal::Null,
            OperationKind::Constant(Constant::Undefined) => js::Literal::Undefined,
            _ => {
                let storage =
                    &mut self.contexts[unit.index()].as_mut().unwrap().plan.values[value.index()];
                return match storage {
                    ValueStorage::Captured(binding) => {
                        let binding = *binding;
                        self.reference(binding)
                    }
                    ValueStorage::Deferred(expression) => expression.take().ok_or_else(|| {
                        self.error(
                            operation.span,
                            "deferred value is missing or consumed more than once",
                        )
                    }),
                    ValueStorage::Rematerialized(cell) => {
                        let cell = *cell;
                        self.cell(unit, cell)
                    }
                    _ => Err(self.error(
                        operation.span,
                        "semantic value has no selected JavaScript storage",
                    )),
                };
            }
        };
        Ok(self.authored_literal(literal, operation.authored)?)
    }

    fn ambient(
        &mut self,
        unit: ContextId,
        ambient: Ambient,
        span: Span,
    ) -> Result<js::ExprId, FormationError> {
        let owner = self.plan(unit).lexical_owner;
        if ambient == Ambient::Arguments && !self.struct_plan.boundary_types.is_empty() {
            // Own/lexically captured arguments is an untyped view of actual
            // parameters, even in strict modules. Use the existing activation
            // owner; it cannot expose private immutable product backing.
            for &parameter in &self.data(owner).parameters {
                self.work(1)?;
                if self.struct_plan.boundary_types[self.program.cells[parameter.index()].ty.index()]
                {
                    return Err(self.error(span, "value-struct arguments ABI adaptation"));
                }
            }
        }
        if ambient == Ambient::Arguments {
            for &cell in &self.data(owner).parameters {
                self.work(1)?;
                if references::is_reference(self.program, cell) {
                    return Err(self.error(span, "reference callable arguments ABI adaptation"));
                }
            }
        }
        self.contexts[owner.index()]
            .as_mut()
            .unwrap()
            .plan
            .observes_activation = true;
        if let Some(entry) = self.plan(owner).generator_entry {
            let index = usize::from(ambient == Ambient::Arguments);
            let binding = if let Some(binding) = self.plan(owner).activation_aliases[index] {
                binding
            } else {
                let name = if index == 0 { "generator_this" } else { "generator_arguments" };
                let binding = self.generated_binding(entry, name)?;
                let value = self.ambient_expression(ambient)?;
                self.statement(entry, js::Statement::Let { binding, value: Some(value) })?;
                self.contexts[owner.index()].as_mut().unwrap().plan.activation_aliases[index] = Some(binding);
                binding
            };
            return self.reference(binding);
        }
        Ok(self.ambient_expression(ambient)?)
    }

    fn call_body(&self, unit: ContextId) -> js::RegionId {
        self.plan(unit).generator_entry.unwrap_or(self.plan(unit).regions[self.data(unit).entry.index()])
    }

    fn storage_region(&self, unit: ContextId, cell: CellId) -> js::RegionId {
        let storage = &self.program.cells[cell.index()];
        if matches!(storage.binding, CellBinding::Parameter(_)) {
            if let Some(entry) = self.plan(unit).generator_entry { return entry; }
        }
        self.plan(unit).regions[storage.region.index()]
    }

    fn finish_unit(&mut self, unit: ContextId) -> Result<(), FormationError> {
        self.work(1)?;
        let body = self.call_body(unit);
        let mut reference_prefix = self.product_parameter_prefix(unit)?;
        for &cell in &self.data(unit).parameters {
            self.work(1)?;
            self.product_lookup()?;
            if self.demand.product_for_cell(cell).is_none() && self.addressed_cell(unit, cell)? {
                let binding = self.cell_binding(unit, cell)?;
                let raw = self.reference(binding)?;
                let value = self.carrier_value(unit, cell, raw)?;
                let target = self.reference(binding)?;
                let assignment = self.expression(js::Expr::Assign { target, value })?;
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut reference_prefix,
                    js::Statement::Evaluate(assignment),
                )?;
            }
        }
        if !reference_prefix.is_empty() {
            let count = reference_prefix.len();
            self.work(self.module.regions[body.index()].statements.len())?;
            let statements = &mut self.module.regions[body.index()].statements;
            self.budget
                .reserve_vec(AllocationClass::Retained, statements, count)?;
            statements.extend(reference_prefix.drain(..));
            statements.rotate_right(count);
            let module = self.data(unit).module.index() as u32;
            self.prepend_root_owners(body, count, js::RootRow::new(module, js::Anchor::Anchored))?;
        }
        self.drop_scratch(reference_prefix)?;
        if self.plan(unit).generator_entry.is_some() {
            let iterator = js::FunctionId::try_new(self.module.functions.len())
                .ok_or(AllocationError::Capacity)?;
            let iterator_body = self.plan(unit).regions[self.data(unit).entry.index()];
            let strict = self.plan(unit).strict_frame;
            self.budget.push(AllocationClass::Retained, &mut self.module.functions, js::Function {
                parameters: Vec::new(), rest: false,
                body: iterator_body,
                arrow: false, name: js::FunctionName::Unobserved,
                strict, length: None,
                suspension: js::Suspension::Generator,
            })?;
            let callee = self.expression(js::Expr::Function(iterator))?;
            let value = self.expression(js::Expr::Call {
                callee, arguments: Vec::new(), invocation: Invocation::Value,
            })?;
            self.statement(body, js::Statement::Return(Some(value)))?;
        }
        Ok(())
    }

    /// Keep the root rows aligned when statements are prepended to a region
    /// that is the artifact root.
    pub(super) fn prepend_root_owners(
        &mut self,
        region: js::RegionId,
        count: usize,
        row: js::RootRow,
    ) -> Result<(), FormationError> {
        if region != self.module.root {
            return Ok(());
        }
        self.work(self.module.root_rows.len())?;
        let rows = &mut self.module.root_rows;
        self.budget
            .reserve_vec(AllocationClass::Retained, rows, count)?;
        rows.extend(std::iter::repeat_n(row, count));
        rows.rotate_right(count);
        Ok(())
    }

    fn cell_binding(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<js::BindingId, FormationError> {
        // A re-export can be the first use of an extern binding. It has an
        // import owner, never lexical storage in its source module.
        if self.program.cells[cell.index()].binding == CellBinding::Foreign {
            if let Some(binding) = self.foreign_import(cell)? { return Ok(binding); }
        }
        let captures = &self.contexts[context.index()].as_ref().unwrap().captures;
        self.budget.work(
            WorkKind::Render,
            (usize::BITS - captures.len().leading_zeros()) as u64,
        )?;
        let position = match captures.binary_search_by_key(&cell, |(cell, _)| *cell) {
            Ok(position) => return Ok(captures[position].1),
            Err(position) => position,
        };
        let owner = self.program.cells[cell.index()].owner;
        // Module cells have one artifact-wide storage owner. Local captures
        // still follow the physical creator chain (including inline contexts).
        // Reuse Demand's canonical root lookup rather than scanning modules.
        let mut cursor = self.demand.module_context(owner).or(Some(context));
        while let Some(candidate) = cursor {
            self.work(1)?;
            let source = self.demand.context(candidate);
            let scope = self.contexts[candidate.index()].as_ref().unwrap();
            let captured = scope.captures.binary_search_by_key(&cell, |(cell,_)| *cell).ok()
                .map(|index| scope.captures[index].1);
            if source.unit == owner || captured.is_some() {
                let binding = captured.or_else(|| scope.cells[self.demand.cell_ordinal(cell)]).ok_or_else(|| {
                    self.error(
                        self.program.cells[cell.index()].declaration,
                        "source cell has no selected JavaScript storage",
                    )
                })?;
                if candidate != context {
                    let captures = &mut self.contexts[context.index()].as_mut().unwrap().captures;
                    self.budget
                        .work(WorkKind::Render, (captures.len() - position) as u64)?;
                    self.budget
                        .reserve_vec(AllocationClass::Scratch, captures, 1)?;
                    captures.insert(position, (cell, binding));
                }
                return Ok(binding);
            }
            cursor = source.parent;
        }
        Err(self.error(
            self.program.cells[cell.index()].declaration,
            "source capture outside its lexical activation",
        ))
    }

    fn cell(&mut self, unit: ContextId, cell: CellId) -> Result<js::ExprId, FormationError> {
        self.work(1)?;
        // A method's receiver is `this`, its rest list `arguments`.
        if let Some(read) = self.activation_read(unit, cell)? {
            return Ok(read);
        }
        if self.program.cells[cell.index()].binding == CellBinding::Foreign {
            if let Some(binding) = self.foreign_import(cell)? {
                return self.reference(binding);
            }
            let cell = &self.program.cells[cell.index()];
            if let Some(ambient) = ambient::classify(cell) {
                return self.ambient(unit, ambient, cell.declaration);
            }
            self.host_binding(&cell.name)
        } else if references::is_reference(self.program, cell) {
            self.read_reference(unit, cell)
        } else {
            self.product_lookup()?;
            if self.demand.product_for_cell(cell).is_some() {
                return self.packed_product_cell(unit, cell);
            }
            let binding = self.cell_binding(unit, cell)?;
            let value = self.reference(binding)?;
            if self.addressed_cell(unit, cell)? {
                self.slot(value, 0)
            } else {
                Ok(value)
            }
        }
    }

    /// The ES import binding of a foreign cell whose runtime source is an
    /// `import extern` edge, created with its import on first use. A classic
    /// script has no module syntax to spell one.
    fn foreign_effect_import(&mut self, source: &str) -> Result<(), FormationError> {
        self.work(self.module.imports.len() + 1)?;
        if self.module.imports.iter().any(|import| import.source.as_unicode() == Some(source)) { return Ok(()); }
        let scope = self.module.regions[self.module.root.index()].scope;
        // An unused target binding retains the request's graph identity. Its
        // empty imported name prints only the request, never a named export.
        let binding = self.fresh_binding(scope, "effect")?;
        self.module.import_in(source, "", binding, self.budget)?;
        Ok(())
    }

    fn foreign_import(&mut self, cell: CellId) -> Result<Option<js::BindingId>, FormationError> {
        self.work(self.foreign_bindings.len() + 1)?;
        if let Some(&(_, binding)) = self
            .foreign_bindings
            .iter()
            .find(|(known, _)| *known == cell)
        {
            return Ok(Some(binding));
        }
        let program = self.program;
        let mut found = None;
        for module in program.modules.iter() {
            self.work(module.foreign_imports.len() + 1)?;
            if let Some(import) = module
                .foreign_imports
                .iter()
                .find(|import| import.cell == Some(cell))
            {
                found = Some(import);
                break;
            }
        }
        let Some(import) = found else {
            return Ok(None);
        };
        // A classic script can only use a foreign module its output carries;
        // output preparation refuses any other import there.
        //
        // An import's identity is what it imports, `(source, imported)`
        // (ECMA-262 ImportEntry records): every module importing it shares one
        // binding, whatever each calls it, and two externs of one name from
        // two sources are two imports. The local name is the naming
        // allocator's, like any other root binding's (Rolldown and esbuild
        // key imports by symbol, never by local spelling).
        self.work(self.module.imports.len())?;
        for index in 0..self.module.imports.len() {
            let existing = &self.module.imports[index];
            if existing.imported != import.imported
                || existing.source.as_unicode() != Some(import.source.as_str())
            {
                continue;
            }
            let binding = existing.binding;
            self.budget.push(
                AllocationClass::Scratch,
                &mut self.foreign_bindings,
                (cell, binding),
            )?;
            return Ok(Some(binding));
        }
        let scope = self.module.regions[self.module.root.index()].scope;
        let spelling = self.text(&program.cells[cell.index()].name)?;
        let binding = self.module.binding_in(
            js::Binding {
                source_symbol: None,
                scope,
                spelling,
                pinned: false,
                class: None,
                defined: false,
            },
            self.budget,
        )?;
        self.module
            .import_in(&import.source, &import.imported, binding, self.budget)?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.foreign_bindings,
            (cell, binding),
        )?;
        Ok(Some(binding))
    }

    fn place(&mut self, unit: ContextId, place: PlaceId) -> Result<js::ExprId, FormationError> {
        self.work(1)?;
        let (receiver, property) = match self.data(unit).places[place.index()].clone() {
            Place::Cell(cell) => return self.cell(unit, cell),
            Place::Value(value) => return self.value(unit, value),
            Place::Field { .. } => return self.value_field(unit, place),
            Place::Member { receiver, key } => {
                // A literal key stays a string occurrence, so the string
                // family can still pool it; the printer spells `o.name`.
                let key = {
                    let literal =
                        js::Literal::String(self.string(&self.program.strings[key.index()])?);
                    self.literal(literal)
                }?;
                (receiver, js::Property::Computed(key))
            }
            // A class field is the instance's own data property of the
            // field's spelling.
            Place::ClassField { receiver, field } => {
                let key = self.class_field_key(field)?;
                (receiver, js::Property::Computed(key))
            }
            Place::Index { receiver, key } => {
                // `list[k]` of a rest list spelled as formals is formal `k`.
                if let Some(formal) = self.formal_read(unit, receiver, place)? {
                    return Ok(formal);
                }
                let key_ty = &self.program.types[self.data(unit).values[key.index()].ty.index()];
                // Converting an object key runs host code (`toString`,
                // `valueOf`, `Symbol.toPrimitive`). JavaScript converts once
                // per access, so a place accessed once converts exactly as
                // its source does; a place both checked or read and then
                // written would convert again.
                if !self.pure_property_key(key_ty)? && !self.single_access_place(unit, place)? {
                    return Err(self.error(
                        Span::default(),
                        "effectful property-key conversion representation",
                    ));
                }
                let key = self.value(unit, key)?;
                (receiver, js::Property::Computed(key))
            }
        };
        let object = self.value(unit, receiver)?;
        Ok(self.expression(js::Expr::Member { object, property })?)
    }

    /// Whether converting a key of this type to a property key is free of
    /// host code.
    fn pure_property_key(&mut self, ty: &Type<'_>) -> Result<bool, FormationError> {
        self.work(1)?;
        Ok(match ty {
            Type::Int
            | Type::Float
            | Type::String
            | Type::Bool
            | Type::Null
            | Type::Enum(_)
            | Type::Symbol => true,
            Type::Nullable(inner) => self.pure_property_key(inner)?,
            Type::Union(members) => {
                let mut pure = true;
                for member in members {
                    pure &= self.pure_property_key(member)?;
                }
                pure
            }
            _ => false,
        })
    }

    /// Whether exactly one operation of this context accesses `place`.
    fn single_access_place(
        &mut self,
        unit: ContextId,
        place: PlaceId,
    ) -> Result<bool, FormationError> {
        let data = self.data(unit);
        self.work(data.operations.len())?;
        let mut accesses = 0;
        for operation in &data.operations {
            accesses += match operation.kind {
                OperationKind::Load(used)
                | OperationKind::Store(used)
                | OperationKind::CheckPlace(used) => usize::from(used == place),
                // A prepared call and its call are one access.
                OperationKind::Call(call) => usize::from(matches!(
                    data.calls[call.index()].target,
                    CallTarget::Reference { place: used } if used == place
                )),
                _ => 0,
            };
        }
        Ok(accesses <= 1)
    }

    fn load(
        &mut self,
        unit: ContextId,
        operation: &Operation,
        place: PlaceId,
        ty: TypeId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if let Place::Cell(cell) = self.data(unit).places[place.index()] {
            if references::is_reference(self.program, cell)
                && matches!(self.program.types[ty.index()], Type::Int)
            {
                let value = self.reference_integer_load(unit, cell)?;
                return self.save(unit, operation, value);
            }
        }
        // A development build checks an index read's precondition (R11); the
        // checked read is the element, so absence recipes still apply.
        let mut raw = if self.checked_index_read(unit, place) {
            self.index_read_check(unit, place)?
        } else {
            self.place(unit, place)?
        };
        if self.compact {
            if let Some(projected) = js::literal_array_projection(&self.module, raw, self.budget)? {
                raw = projected;
            }
        }
        // A typed host binding's value crosses in at each read (R1): a
        // development build checks it. An extern function crosses where its
        // calls return.
        if let Place::Cell(cell) = self.data(unit).places[place.index()] {
            if self.program.cells[cell.index()].binding == CellBinding::Foreign
                && !matches!(self.program.types[ty.index()], Type::Function(_))
            {
                raw = self.crossing_check(ty, raw)?;
            }
        }
        if let Place::ClassField { field, .. } = self.data(unit).places[place.index()] {
            if self.program.class(field.nominal).is_some_and(|class| class.shape
                && class.accessors.get(field.slot as usize) == Some(&true)) {
                raw = self.crossing_check(ty, raw)?;
            }
        }
        raw = self.storage_decode(unit, place, raw)?;
        // A load is its JavaScript read (R1, R11), but for absence until
        // R2's second batch and a `Uint32Array` element read as an `int`.
        match load_result_recipe(self.program, self.data(unit), place, ty) {
            LoadResultRecipe::NullishNull => {
                let right = self.literal(js::Literal::Null)?;
                Ok(self.save_nullish(unit, operation, raw, right)?)
            }
            LoadResultRecipe::Uint32Element => {
                let value = self.expression(js::Expr::ToInt32(raw))?;
                Ok(self.save(unit, operation, value)?)
            }
            LoadResultRecipe::Raw
                if matches!(self.program.types[ty.index()], Type::Int)
                    && !matches!(
                        self.data(unit).places[place.index()],
                        Place::Cell(_) | Place::Value(_)
                    ) =>
            {
                self.hint_sites += 1;
                let value = if self.int32_hints {
                    self.expression(js::Expr::ToInt32(raw))?
                } else {
                    raw
                };
                Ok(self.save(unit, operation, value)?)
            }
            LoadResultRecipe::Raw => Ok(self.save(unit, operation, raw)?),
        }
    }

    /// The result binding owns the evaluated left value. Older targets test
    /// that stored value and conditionally replace it; neither a getter nor
    /// the fallback schedule is duplicated. Strict tests also preserve the
    /// special HTMLDDA host value, unlike `left == null`.
    fn save_nullish(
        &mut self,
        unit: ContextId,
        operation: &Operation,
        left: js::ExprId,
        right: js::ExprId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if self
            .contract
            .ecmascript
            .allows(JsSyntaxFeature::NullishCoalescing)
        {
            // `(x??null)??y` is `x??y`: both evaluate `y` exactly when `x` is
            // null or undefined. The left occurrence is owned by this node.
            let mut left = left;
            if let js::Expr::Binary {
                op: js::Binary::Nullish,
                left: inner,
                right: absent,
            } = self.module.expressions[left.index()]
            {
                if matches!(
                    self.module.expressions[absent.index()],
                    js::Expr::Literal(js::Literal::Null)
                ) {
                    left = inner;
                }
            }
            let value = self.expression(js::Expr::Binary {
                op: js::Binary::Nullish,
                left,
                right,
            })?;
            return self.save(unit, operation, value);
        }
        // Older targets need a physical scratch value to gate the right arm,
        // even when the semantic result itself is discarded.
        let result = operation.result.unwrap();
        let existing = self.plan(unit).values[result.index()].binding();
        let binding = if let Some(binding) = existing {
            binding
        } else {
            let region = self.plan(unit).regions[operation.region.index()];
            let binding = js::Binding {
                source_symbol: None,
                scope: self.module.regions[region.index()].scope,
                spelling: self.format(format_args!(
                    "nullish_{}_{}",
                    unit.index(),
                    result.index()
                ))?,
                pinned: false,
                class: None,
                defined: false,
            };
            let binding = self.module.binding_in(binding, self.budget)?;
            self.statement(
                region,
                js::Statement::Let {
                    binding,
                    value: None,
                },
            )?;
            binding
        };
        // The scratch owns this recipe's repeated read. A deferred semantic
        // result still owns the complete sequence and is consumed only once.
        let capture = self.assign(binding, left, operation.origin)?;
        let value = self.reference(binding)?;
        let null = self.literal(js::Literal::Null)?;
        let is_null = self.expression(js::Expr::Binary {
            op: js::Binary::StrictEqual,
            left: value,
            right: null,
        })?;
        let value = self.reference(binding)?;
        let undefined = self.literal(js::Literal::Undefined)?;
        let is_undefined = self.expression(js::Expr::Binary {
            op: js::Binary::StrictEqual,
            left: value,
            right: undefined,
        })?;
        let condition = self.expression(js::Expr::Binary {
            op: js::Binary::Or,
            left: is_null,
            right: is_undefined,
        })?;
        let yes = self.assign(binding, right, operation.origin)?;
        let no = self.reference(binding)?;
        let result = self.expression(js::Expr::Conditional { condition, yes, no })?;
        let values = self
            .budget
            .copy_slice(AllocationClass::Retained, &[capture, result])?;
        let expression = self.expression(js::Expr::Sequence(values))?;
        if existing.is_some() {
            Ok(Some(expression))
        } else {
            self.save(unit, operation, expression)
        }
    }

    fn assign(
        &mut self,
        binding: js::BindingId,
        value: js::ExprId,
        origin: Option<SourceOriginId>,
    ) -> Result<js::ExprId, FormationError> {
        let target = self.reference(binding)?;
        Ok(self
            .module
            .expression_in(js::Expr::Assign { target, value }, origin, self.budget)?)
    }

    fn sequence(
        &mut self,
        mut values: Vec<js::ExprId>,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if values.len() > 1 {
            return Ok(Some(self.expression(js::Expr::Sequence(values))?));
        }
        let value = values.pop();
        let bytes = values
            .capacity()
            .checked_mul(std::mem::size_of::<js::ExprId>())
            .ok_or(AllocationError::Capacity)?;
        drop(values);
        self.budget
            .release(AllocationClass::Retained, bytes as u64)?;
        Ok(value)
    }

    fn save(
        &mut self,
        unit: ContextId,
        operation: &Operation,
        value: js::ExprId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if self.module.origins[value.index()].is_none() { self.module.origins[value.index()] = operation.origin; }
        self.record_behaviour(operation, value)?;
        if operation.result.is_some_and(|result| {
            self.demand
                .string_value(self.semantic(unit), result)
                .is_some()
        }) {
            return Ok(Some(value));
        }
        if self.compact && matches!(self.module.expressions[value.index()], js::Expr::ToInt32(_)) {
            self.transfer_number(unit, operation, false, |_| NumberFacts::I32)?;
        }
        self.save_result(unit, operation, value)
    }

    fn save_result(
        &mut self,
        unit: ContextId,
        operation: &Operation,
        value: js::ExprId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        if let Some(result) = operation.result {
            let storage =
                &mut self.contexts[unit.index()].as_mut().unwrap().plan.values[result.index()];
            match storage {
                ValueStorage::Captured(binding) => {
                    let binding = *binding;
                    return Ok(Some(self.assign(binding, value, operation.origin)?));
                }
                ValueStorage::Deferred(expression) => {
                    if expression.replace(value).is_some() {
                        return Err(
                            self.error(operation.span, "deferred value produced more than once")
                        );
                    }
                    return Ok(None);
                }
                ValueStorage::Absent => {}
                ValueStorage::Rematerialized(_) => {
                    return Err(self.error(operation.span, "rematerialized load was formed"));
                }
                _ => return Err(self.error(operation.span, "value placement was not completed")),
            }
        }
        Ok(Some(value))
    }

    fn expression_region(
        &mut self,
        unit: ContextId,
        region: RegionId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        let operations = &self.data(unit).regions[region.index()].operations;
        let mut cursor = 0;
        let mut expressions = Vec::new();
        while cursor < operations.len() {
            self.work(1)?;
            if let Some(value) = self.scheduled_expression(unit, &operations, &mut cursor)? {
                self.append(&mut expressions, value)?;
            }
        }
        if self.demand.needs_region_result(unit, region) {
            if let Some(result) = self.data(unit).regions[region.index()].result {
                {
                    let appended = self.value(unit, result)?;
                    self.append(&mut expressions, appended)
                }?;
            }
        }
        Ok(self.sequence(expressions)?)
    }

    /// Consume a complete prepared call as one target evaluation occurrence.
    /// Its callee reference is resolved before the first argument's schedule.
    fn prepared_call(
        &mut self,
        unit: ContextId,
        call: CallId,
        operations: &[OpId],
        cursor: &mut usize,
    ) -> Result<Option<js::ExprId>, FormationError> {
        let mut before = Vec::new();
        while *cursor < operations.len() {
            self.work(1)?;
            let operation = &self.data(unit).operations[operations[*cursor].index()];
            if let OperationKind::Call(found) = operation.kind {
                if found != call {
                    return Err(self.error(operation.span, "unbalanced prepared call schedule"));
                }
                let call_operation = operations[*cursor];
                *cursor += 1;
                if !self.demand.needs_operation(unit, call_operation) {
                    return Ok(self.sequence(before)?);
                }
                let values = self
                    .data(unit)
                    .arguments(self.data(unit).calls[call.index()].arguments)
                    .unwrap();
                if let Some(HelperOperation::Call(helper)) = self
                    .demand
                    .helper_operation(self.semantic(unit), call_operation)
                {
                    let expression = self.inline_call(
                        unit,
                        call_operation,
                        helper,
                        &values,
                        before,
                        operation.span,
                    )?;
                    return match expression {
                        Some(value) => self.save(unit, operation, value),
                        None => Ok(None),
                    };
                }
                let mut arguments = self
                    .budget
                    .vector(AllocationClass::Retained, values.len())?;
                let mut expanded_products = false;
                for (index, argument) in values.iter().copied().enumerate() {
                    self.work(1)?;
                    match argument {
                        CallArgument::Value(value) => {
                            expanded_products |= self.append_product_argument(
                                unit,
                                call,
                                index as u32,
                                value,
                                &mut before,
                                &mut arguments,
                            )?;
                        }
                        // `...value` (R7): one array or iterable, never a
                        // product, printed as a spread.
                        CallArgument::Spread(value) => {
                            let start = arguments.len();
                            expanded_products |= self.append_product_argument(
                                unit,
                                call,
                                index as u32,
                                value,
                                &mut before,
                                &mut arguments,
                            )?;
                            if arguments.len() != start + 1 {
                                return Err(self.error(operation.span, "spread argument expanded"));
                            }
                            let value = arguments.pop().expect("appended the spread value");
                            let spread = self.expression(js::Expr::Spread(value))?;
                            self.append(&mut arguments, spread)?;
                        }
                        CallArgument::Reference(_) => {
                            let (carrier, path) =
                                self.prepared_reference(unit, call, index as u32)?;
                            let carrier = self.reference(carrier)?;
                            self.append_prepared_argument(&mut before, &mut arguments, carrier)?;
                            let path = self.reference(path)?;
                            self.append_prepared_argument(&mut before, &mut arguments, path)?;
                        }
                    }
                }
                if !before.is_empty() {
                    // Zero-width product expansion has no first physical
                    // argument to own the prefix. The common placement owner
                    // must have frozen the callee before source argument work.
                    let CallTarget::Value {
                        callee,
                        invocation: Invocation::Value,
                    } = self.data(unit).calls[call.index()].target
                    else {
                        return Err(self.error(
                            operation.span,
                            "argument prefix without captured value callee",
                        ));
                    };
                    if !matches!(
                        self.plan(unit).values[callee.index()],
                        ValueStorage::Captured(_) | ValueStorage::Rematerialized(_)
                    ) {
                        return Err(self
                            .error(operation.span, "zero-width product callee was not captured"));
                    }
                }
                if self.stripped_log_call(self.semantic(unit), call) {
                    if operation.result.is_some_and(|result| {
                        !matches!(
                            self.program.types[self.data(unit).values[result.index()].ty.index()],
                            Type::Void
                        )
                    }) {
                        return Err(self.error(operation.span, "non-void dropped logging call"));
                    }
                    // Preparing a dropped call never touches the logger. Its
                    // argument schedules have already been retained in
                    // `arguments`. Omit the host lookup/invocation, preserving
                    // all argument evaluations and abrupt completion at their
                    // source site.
                    {
                        let appended = self.literal(js::Literal::Undefined)?;
                        self.append(&mut arguments, appended)
                    }?;
                    let value = self.sequence(arguments)?.unwrap();
                    return self.save(unit, &operation, value);
                }
                // A call to a declared function that only forwards its
                // arguments to a host builtin is that builtin: the same
                // evaluations, one frame fewer. Measured never larger in
                // Brotli on the reference ports, often smaller: the builtin
                // spellings repeat where the wrapper names did not.
                if self.compact && before.is_empty() && !expanded_products {
                    if let Some(builtin) = operation
                        .result
                        .and_then(|result| self.forwarding_builtin(unit, result))
                        .filter(|&builtin| {
                            crate::catalog::host_builtin(builtin)
                                && (self.contract.assumptions.pristine_builtins
                                    || operands_first(builtin))
                                && (self.contract.frames_hidden()
                                    || runs_no_user_code(builtin))
                                && (builtin != BuiltinCall::JsObject
                                    || arguments.len() % 2 == 0
                                        && arguments
                                            .chunks_exact(2)
                                            .all(|pair| self.string_key(pair[0])))
                                // A view of a host value as a struct decodes
                                // it inside the wrapper (`assumed_product`).
                                && builtin != BuiltinCall::JsAssume
                        })
                    {
                        let expression = self.host_builtin(builtin, arguments, operation.span)?;
                        let sum = builtin == BuiltinCall::JsAdd
                            && matches!(
                                expression,
                                js::Expr::Binary {
                                    op: js::Binary::Add,
                                    ..
                                }
                            );
                        let expression = self.expression(expression)?;
                        if sum {
                            self.budget.push(
                                AllocationClass::Scratch,
                                &mut self.string_sums,
                                expression,
                            )?;
                        }
                        return self.save(unit, &operation, expression);
                    }
                }
                // A call to a function that only returns undefined is that
                // value: its callee is a declared function, and it has no
                // arguments to evaluate.
                if self.compact
                    && before.is_empty()
                    && operation
                        .result
                        .is_some_and(|result| self.undefined_call(unit, result))
                {
                    let value = self.literal(js::Literal::Undefined)?;
                    return self.save(unit, &operation, value);
                }
                let expression =
                    self.call(unit, call, arguments, operation.span, expanded_products)?;
                self.module.origins[expression.index()] = operation.origin;
                let mut expression = self.assumed_product(unit, call, &operation, expression)?;
                if self.host_call_boundary(unit, call) {
                    if let Some(result) = operation.result {
                        let ty = &self.program.types[self.data(unit).values[result.index()].ty.index()];
                        if public_structs::carries_product(ty, self.budget)? && self.foreign_product_call(unit, call)?
                            || ty.callable_signature().is_some() && public_structs::carries_absence(self.program, ty, self.budget)? {
                            expression = self.public_value(ty, expression, true)?;
                        }
                    }
                }
                // Host calls retain their evaluation/throwing behavior, but
                // their returned JS value still owes the source operation's
                // result contract. A replaced logger cannot give Print a
                // value, and a replaced integer member must be normalized.
                let mut expression =
                    match call_result_recipe(self.program, self.data(unit), operation) {
                        CallResultRecipe::Void => self.expression(js::Expr::Unary {
                            op: js::Unary::Void,
                            value: expression,
                        })?,
                        CallResultRecipe::Raw
                            if operation.result.is_some_and(|result| {
                                matches!(
                                    self.program.types
                                        [self.data(unit).values[result.index()].ty.index()],
                                    Type::Int
                                )
                            }) && matches!(
                                self.data(unit).calls[call.index()].target,
                                CallTarget::Reference { .. } | CallTarget::Builtin(_)
                            ) =>
                        {
                            self.hint_sites += 1;
                            if self.int32_hints {
                                self.expression(js::Expr::ToInt32(expression))?
                            } else {
                                expression
                            }
                        }
                        CallResultRecipe::Raw | CallResultRecipe::IntrinsicInteger => expression,
                    };
                // A development build checks what crosses in (R1).
                if self.crossing_call(unit, call) {
                    if let Some(result) = operation.result {
                        let ty = self.data(unit).values[result.index()].ty;
                        expression = self.crossing_check(ty, expression)?;
                    }
                }
                if !before.is_empty() {
                    self.append(&mut before, expression)?;
                    expression = self.sequence(before)?.unwrap();
                }
                return self.save(unit, &operation, expression);
            }
            if let Some(expression) = self.scheduled_expression(unit, operations, cursor)? {
                self.append(&mut before, expression)?;
            }
        }
        Err(self.error(Span::default(), "unterminated prepared call schedule"))
    }

    fn inline_call(
        &mut self,
        caller: ContextId,
        call: OpId,
        helper: usize,
        arguments: &[CallArgument],
        mut schedule: Vec<js::ExprId>,
        span: Span,
    ) -> Result<Option<js::ExprId>, FormationError> {
        let tail = self.demand.helpers()[helper].tail_return();
        // Declarations live in the caller activation, but all initialization,
        // argument effects and body operations stay in the original call's
        // expression region (including lazy branches and loop tests).
        let body = self.plan(caller).regions[self.data(caller).entry.index()];
        let context = self
            .demand
            .child(caller, call)
            .ok_or_else(|| self.error(span, "missing inline demand context"))?;
        self.plan_context(context, body)?;
        let parameters = &self.data(context).parameters;
        if parameters.len() != arguments.len() {
            return Err(self.error(span, "inline helper arity disagrees with its proof"));
        }
        // Retained argument work precedes these assignments; deferred arguments
        // execute on their assignment RHS in source order. The helper proof
        // keeps fresh private parameter cells unobservable until body entry.
        // Reentry uses another caller activation, not this occurrence's cells.
        for (parameter, argument) in parameters.iter().copied().zip(arguments) {
            self.work(1)?;
            let CallArgument::Value(argument) = *argument else {
                if !references::is_reference(self.program, parameter) {
                    return Err(self.error(span, "inline reference/value parameter mismatch"));
                }
                // The formal borrows its original prepared location. No value
                // copy or per-invocation binding can replace this alias.
                continue;
            };
            if self.initialize_inline_product_parameter(
                context,
                caller,
                parameter,
                argument,
                &mut schedule,
            )? {
                continue;
            }
            if !self.demand.needs_cell(context, parameter) {
                continue;
            }
            let binding = self.cell_binding(context, parameter)?;
            let target = self.reference(binding)?;
            let value = self.value(caller, argument)?;
            {
                let appended = self.expression(js::Expr::Assign { target, value })?;
                self.append(&mut schedule, appended)
            }?;
        }
        for index in 0..self.plan(context).shared_strings.len() {
            self.work(1)?;
            let (family, binding) = self.plan(context).shared_strings[index];
            let target = self.reference(binding)?;
            let value = {
                self.family_literal(family)
            }?;
            {
                let appended = self.expression(js::Expr::Assign { target, value })?;
                self.append(&mut schedule, appended)
            }?;
        }
        let operations = &self.data(context).regions[self.data(context).entry.index()].operations;
        let mut cursor = 0;
        while cursor < operations.len() {
            self.work(1)?;
            if Some(operations[cursor]) == tail {
                if cursor + 1 != operations.len() {
                    return Err(self.error(span, "inline helper return is not the proved tail"));
                }
                if !self.demand.needs_return(context) {
                    return Ok(self.sequence(schedule)?);
                }
                let operation = &self.data(context).operations[operations[cursor].index()];
                let returned = self
                    .data(context)
                    .operands(operation.operands)
                    .unwrap()
                    .first()
                    .copied();
                let value = match returned {
                    Some(value) => self.value(context, value)?,
                    None => self.literal(js::Literal::Undefined)?,
                };
                self.append(&mut schedule, value)?;
                return Ok(self.sequence(schedule)?);
            }
            if let Some(expression) =
                self.scheduled_expression(context, &operations, &mut cursor)?
            {
                self.append(&mut schedule, expression)?;
            }
        }
        if tail.is_some() {
            return Err(self.error(span, "inline helper is missing its proved return"));
        }
        // Only a completed exact-Void helper certificate admits fallthrough.
        // Keep the original effects and supply undefined when its value is used.
        if self.demand.needs_return(context) {
            let value = self.literal(js::Literal::Undefined)?;
            self.append(&mut schedule, value)?;
        }
        self.sequence(schedule)
    }

    /// Whether a `JS.call`'s receiver argument is a call to a function whose
    /// body only returns undefined.
    fn undefined_receiver(&self, unit: ContextId, call: CallId) -> bool {
        let data = self.data(unit);
        let Some([_, CallArgument::Value(receiver), ..]) =
            data.arguments(data.calls[call.index()].arguments)
        else {
            return false;
        };
        self.undefined_call(unit, *receiver)
    }

    /// The host builtin that a call's declared callee only forwards to: its
    /// body loads each by-value parameter once, in order, passes them to one
    /// builtin and returns that result. The call is that builtin applied to
    /// the same arguments, one frame fewer. `value` is the call's result.
    /// `JS.assume(v)` of a host value to a type holding a value struct: the
    /// struct's storage is private, so the view decodes the value's public
    /// shape, its fields by name, as an exported function's input is decoded
    /// (the D2 decoder). A struct is a value, so the view is a copy made at
    /// the assumption; a host object read and written in place is an
    /// `extern class`. A type the decoder cannot reach (for example an aliased
    /// mutable record of structs) is refused rather than read as private storage.
    fn assumed_product(
        &mut self,
        unit: ContextId,
        call: CallId,
        operation: &Operation,
        expression: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let program = self.program;
        let data = self.data(unit);
        if !matches!(
            data.calls[call.index()].target,
            CallTarget::Builtin(BuiltinCall::JsAssume)
        ) {
            return Ok(expression);
        }
        let Some(result) = operation.result else {
            return Ok(expression);
        };
        let result = &program.types[data.values[result.index()].ty.index()];
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
        let [CallArgument::Value(argument)] = arguments[..] else {
            return Ok(expression);
        };
        let argument = &program.types[data.values[argument.index()].ty.index()];
        if !public_structs::carries_product(result, self.budget)?
            || public_structs::carries_product(argument, self.budget)?
        {
            return Ok(expression);
        }
        let decoded = match result {
            Type::Array(element) => element.as_ref(),
            result => result,
        };
        if !public_structs::adaptable(program, decoded, 0, self.budget)? {
            return Err(self.error(
                operation.span,
                "JS.assume to a type holding a value struct needs its public decoder",
            ));
        }
        self.public_value(result, expression, true)
    }

    fn forwarding_builtin(&self, unit: ContextId, value: ValueId) -> Option<BuiltinCall> {
        let program = self.program;
        let data = self.data(unit);
        let OperationKind::Call(outer) =
            data.operations[data.values[value.index()].definition.index()].kind
        else {
            return None;
        };
        let site = &data.calls[outer.index()];
        let CallTarget::Value {
            callee,
            invocation: Invocation::Value,
        } = site.target
        else {
            return None;
        };
        let OperationKind::Load(place) =
            data.operations[data.values[callee.index()].definition.index()].kind
        else {
            return None;
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return None;
        };
        let CellBinding::Function(body) = program.cells[cell.index()].binding else {
            return None;
        };
        let function = program.unit(body)?;
        let arguments = data.arguments(site.arguments)?;
        if function.suspension != Suspension::None
            || arguments.len() != function.parameters.len()
            || arguments
                .iter()
                .any(|argument| !matches!(argument, CallArgument::Value(_)))
            || function
                .parameters
                .iter()
                .any(|&parameter| program.is_reference_parameter(parameter))
        {
            return None;
        }
        // The body: its parameters' loads, in order, then one operation over
        // them whose result it returns. A builtin call is that builtin. A
        // natural form is the builtin it is operand for operand: a reference
        // call on `p0[p1]` with the rest as arguments is `JS.invoke`, a load of
        // `p0[p1]` is `JS.get`, an empty object literal is `JS.object()` and an
        // array literal of the parameters is `JS.array` (R12: syntax keeps its
        // natural IR, so a wrapper written in syntax forwards as its spelling
        // did).
        let mut loads: Vec<ValueId> = Vec::new();
        let mut forwarded: Option<(&Operation, ValueId)> = None;
        let mut returned = false;
        for &operation in &function.regions[function.entry.index()].operations {
            let operation = &function.operations[operation.index()];
            match operation.kind {
                OperationKind::PrepareCall(call)
                    if matches!(
                        function.calls[call.index()].target,
                        CallTarget::Builtin(_) | CallTarget::Reference { .. }
                    ) => {}
                OperationKind::Load(place)
                    if forwarded.is_none() && loads.len() < function.parameters.len() =>
                {
                    let Place::Cell(cell) = function.places[place.index()] else {
                        return None;
                    };
                    if function.parameters.get(loads.len()) != Some(&cell) {
                        return None;
                    }
                    loads.push(operation.result?);
                }
                OperationKind::Call(_)
                | OperationKind::Load(_)
                | OperationKind::Allocate { .. }
                    if forwarded.is_none() && loads.len() == function.parameters.len() =>
                {
                    forwarded = Some((operation, operation.result?));
                }
                OperationKind::Return if !returned => {
                    let (_, result) = forwarded.as_ref()?;
                    if function.operands(operation.operands)? != [*result] {
                        return None;
                    }
                    returned = true;
                }
                _ => return None,
            }
        }
        let (operation, result) = forwarded.filter(|_| returned)?;
        let loaded = |values: &[CallArgument]| {
            values.len() == loads.len()
                && values
                    .iter()
                    .zip(&loads)
                    .all(|(value, load)| *value == CallArgument::Value(*load))
        };
        let builtin = match &operation.kind {
            &OperationKind::Call(call) => {
                let site = &function.calls[call.index()];
                let operands = function.arguments(site.arguments)?;
                match site.target {
                    // Each builtin operand is its parameter's load, in order.
                    CallTarget::Builtin(builtin) if loaded(operands) => builtin,
                    CallTarget::Reference { place } => {
                        let Place::Index { receiver, key } = function.places[place.index()] else {
                            return None;
                        };
                        let [first, second, rest @ ..] = loads.as_slice() else {
                            return None;
                        };
                        if (receiver, key) != (*first, *second)
                            || operands.len() != rest.len()
                            || operands
                                .iter()
                                .zip(rest)
                                .any(|(operand, load)| *operand != CallArgument::Value(*load))
                        {
                            return None;
                        }
                        BuiltinCall::JsInvoke
                    }
                    _ => return None,
                }
            }
            &OperationKind::Load(place) => {
                let Place::Index { receiver, key } = function.places[place.index()] else {
                    return None;
                };
                if loads.as_slice() != [receiver, key] {
                    return None;
                }
                BuiltinCall::JsGet
            }
            OperationKind::Allocate {
                kind: AllocationKind::Object(keys),
                ..
            } if keys.is_empty() && loads.is_empty() => BuiltinCall::JsObject,
            OperationKind::Allocate {
                kind: AllocationKind::Array,
                ..
            } => {
                if function.operands(operation.operands)? != loads.as_slice() {
                    return None;
                }
                BuiltinCall::JsArray
            }
            _ => return None,
        };
        // An integer result would owe the builtin's own normalization.
        (!matches!(
            program.types[function.values[result.index()].ty.index()],
            Type::Int
        ))
        .then_some(builtin)
    }

    /// Whether `value` is a zero-argument call to a declared function whose
    /// body only returns undefined: no parameters, no suspension, no effect.
    fn undefined_call(&self, unit: ContextId, value: ValueId) -> bool {
        let program = self.program;
        let data = self.data(unit);
        let OperationKind::Call(inner) =
            data.operations[data.values[value.index()].definition.index()].kind
        else {
            return false;
        };
        let CallTarget::Value { callee, .. } = data.calls[inner.index()].target else {
            return false;
        };
        let OperationKind::Load(place) =
            data.operations[data.values[callee.index()].definition.index()].kind
        else {
            return false;
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return false;
        };
        let CellBinding::Function(body) = program.cells[cell.index()].binding else {
            return false;
        };
        let Some(function) = program.unit(body) else {
            return false;
        };
        if !function.parameters.is_empty()
            || function.suspension != Suspension::None
            || data
                .arguments(data.calls[inner.index()].arguments)
                .is_some_and(|arguments| !arguments.is_empty())
        {
            return false;
        }
        // A call's preparation fixes its callee before its arguments; here
        // it prepares a builtin and observes nothing.
        let mut operations = function.regions[function.entry.index()]
            .operations
            .iter()
            .copied()
            .filter(|operation| {
                !matches!(
                    function.operations[operation.index()].kind,
                    OperationKind::PrepareCall(prepared)
                        if matches!(function.calls[prepared.index()].target, CallTarget::Builtin(BuiltinCall::JsUndefined))
                )
            });
        let returned = |operation: OpId| {
            let operation = &function.operations[operation.index()];
            matches!(operation.kind, OperationKind::Return).then(|| {
                function
                    .operands(operation.operands)
                    .unwrap_or(&[])
                    .first()
                    .copied()
            })
        };
        match (operations.next(), operations.next(), operations.next()) {
            (Some(only), None, _) => returned(only) == Some(None),
            (Some(first), Some(second), None) => {
                let produced = &function.operations[first.index()];
                let undefined = match produced.kind {
                    OperationKind::Constant(Constant::Undefined) => true,
                    OperationKind::Call(value) => matches!(
                        function.calls[value.index()].target,
                        CallTarget::Builtin(BuiltinCall::JsUndefined)
                    ),
                    _ => false,
                };
                undefined && produced.result.is_some() && returned(second) == Some(produced.result)
            }
            _ => false,
        }
    }

    /// A strict directive cannot occur in a rest-parameter function. A
    /// zero-argument lexical factory supplies strictness without changing
    /// the callable's receiver, parameter array, name or reflected length.
    fn rest_strict_frame(
        &mut self,
        function: js::FunctionId,
        needed: bool,
        factory: Option<js::RegionId>,
    ) -> Result<js::Expr, FormationError> {
        if !needed {
            if let Some(factory) = factory {
                let inner = self.module.functions[function.index()].body;
                let scope = self.module.regions[inner.index()].scope;
                self.module.scopes[scope.index()] =
                    self.module.scopes[self.module.regions[factory.index()].scope.index()];
            }
            return Ok(js::Expr::Function(function));
        }
        let body = factory.ok_or_else(|| {
            self.error(
                Span::default(),
                "strict rest factory scope was not reserved",
            )
        })?;
        let value = self.expression(js::Expr::Function(function))?;
        self.statement(body, js::Statement::Return(Some(value)))?;
        let factory = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters: Vec::new(),
                rest: false,
                body,
                arrow: true,
                name: js::FunctionName::Unobserved,
                strict: true,
                length: None,
                suspension: js::Suspension::None,
            },
        )?;
        let callee = self.expression(js::Expr::Function(factory))?;
        Ok(js::Expr::Call {
            callee,
            arguments: Vec::new(),
            invocation: Invocation::Value,
        })
    }

    fn call(
        &mut self,
        unit: ContextId,
        call: CallId,
        mut arguments: Vec<js::ExprId>,
        span: Span,
        expanded_products: bool,
    ) -> Result<js::ExprId, FormationError> {
        let contract = self.data(unit).calls[call.index()].contract;
        if contract.defaults == DefaultConvention::PreserveOmission
            && arguments.len() != contract.supplied as usize
            && !expanded_products
        {
            return Err(self.error(span, "JavaScript call changed preserved source arity"));
        }
        // Default transport is proved on the checked program. Product ABI
        // expansion changes argument positions, so it retains the full list.
        if self.default_transport && !expanded_products {
            let omitted = self.data(unit).calls[call.index()].omit_trailing as usize;
            // Formation can attach preceding argument effects to the final
            // literal as a sequence. Such an expression must still execute.
            let inert = arguments
                .iter()
                .rev()
                .take(omitted)
                .take_while(|value| {
                    matches!(self.module.expressions[value.index()], js::Expr::Literal(_))
                })
                .count();
            arguments.truncate(arguments.len().saturating_sub(inert));
        }
        // Source IR transports one fresh rest array; the public JavaScript
        // convention accepts trailing arguments. Native keeps the packed ABI.
        let signature = contract
            .instantiation
            .map(|id| self.data(unit).call_instantiations[id.index()].signature)
            .or(contract.signature);
        let receiver = signature.is_some_and(|id| matches!(&self.program.types[id.index()],Type::Function(signature) if signature.has_receiver()));
        let rest = signature.is_some_and(|id| match &self.program.types[id.index()] {
            Type::Function(signature) => signature.has_rest(),
            Type::GenericFunction(function) => function.signature.has_rest(),
            _ => false,
        });
        let complete = signature.is_some_and(|id| match &self.program.types[id.index()] {
            Type::Function(signature) => arguments.len() == signature.params.len(),
            Type::GenericFunction(function) => arguments.len() == function.signature.params.len(),
            _ => false,
        });
        if rest && complete && contract.defaults != DefaultConvention::PreserveOmission {
            let packed = arguments
                .pop()
                .ok_or_else(|| self.error(span, "packed rest argument is missing"))?;
            // The shared ABI's fresh array is only argument transport here.
            // Its literal elements can be passed directly: the JavaScript
            // callee creates the one observable rest array, in the same order.
            if let js::Expr::Array(elements) = &self.module.expressions[packed.index()] {
                let elements = self.budget.copy_slice(AllocationClass::Scratch, elements)?;
                for element in &elements {
                    self.append(&mut arguments, *element)?;
                }
                self.drop_scratch(elements)?;
            } else {
                let spread = self.expression(js::Expr::Spread(packed))?;
                self.append(&mut arguments, spread)?;
            }
        }
        let node = match self.data(unit).calls[call.index()].target.clone() {
            CallTarget::Value { callee, invocation } => {
                if invocation == Invocation::DirectEval {
                    return Err(self.error(span, "semantic JavaScript direct eval contract"));
                }
                let mut callee = self.value(unit, callee)?;
                if receiver {
                    let property = js::Property::Named(self.text("call")?);
                    callee = self.expression(js::Expr::Member {
                        object: callee,
                        property,
                    })?;
                }
                js::Expr::Call {
                    callee,
                    arguments,
                    invocation: if receiver {
                        Invocation::Reference
                    } else {
                        invocation
                    },
                }
            }
            CallTarget::Reference { place } => {
                let callee = self.place(unit, place)?;
                js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Reference,
                }
            }
            CallTarget::Builtin(builtin @ (BuiltinCall::Print | BuiltinCall::MathImul)) => {
                let (host, method) = if builtin == BuiltinCall::Print {
                    ("console", "log")
                } else {
                    ("Math", "imul")
                };
                let object = {
                    let node = js::Expr::Host(js::Host::new(self.text(host)?));
                    self.expression(node)
                }?;
                let property = js::Property::Named(self.text(method)?);
                let callee = self.expression(js::Expr::Member { object, property })?;
                js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Reference,
                }
            }
            // An adapter whose callback was formed as the method itself is
            // that function (`javascript_methods.rs`).
            CallTarget::Builtin(_)
                if arguments.len() == 1 && self.method_call(self.semantic(unit), call) =>
            {
                return Ok(arguments[0]);
            }
            CallTarget::Builtin(builtin) if crate::catalog::host_builtin(builtin) => {
                let mut arguments = arguments;
                // `JS.call(f, t, ...)` whose `t` is a call to a function that
                // only returns undefined is `f(...)`: the dropped call has no
                // effect, and a plain call's receiver is undefined too.
                if builtin == BuiltinCall::JsCall
                    && self.compact
                    && arguments.len() >= 2
                    && self.undefined_receiver(unit, call)
                {
                    arguments[1] = self.literal(js::Literal::Undefined)?;
                }
                self.host_builtin(builtin, arguments, span)?
            }
            // `value.truthy()`, `value.isArray()`, `value.isObject()` on a
            // host value, spelled as the old route's emitter spelled them.
            CallTarget::Intrinsic {
                operation:
                    ResolvedIntrinsic::Method(
                        operation @ (Intrinsic::JsTruthy
                        | Intrinsic::JsIsArray
                        | Intrinsic::JsIsObject),
                    ),
                receiver: Some(receiver),
            } if arguments.is_empty() => {
                let value = self.value(unit, receiver)?;
                match operation {
                    Intrinsic::JsTruthy => {
                        let once = self.expression(js::Expr::Unary {
                            op: js::Unary::Not,
                            value,
                        })?;
                        js::Expr::Unary {
                            op: js::Unary::Not,
                            value: once,
                        }
                    }
                    Intrinsic::JsIsArray => {
                        let callee = self.host_path(&["Array", "isArray"])?;
                        let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
                        self.append(&mut arguments, value)?;
                        js::Expr::Call {
                            callee,
                            arguments,
                            invocation: Invocation::Reference,
                        }
                    }
                    _ => {
                        let object = self.string(&crate::literal::StringValue::from("object"))?;
                        let left = self.literal(js::Literal::String(object))?;
                        let right = self.expression(js::Expr::Unary {
                            op: js::Unary::TypeOf,
                            value,
                        })?;
                        js::Expr::Binary {
                            op: js::Binary::Equal,
                            left,
                            right,
                        }
                    }
                }
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Method(Intrinsic::FloatToInt),
                receiver: Some(receiver),
            } if arguments.is_empty() => {
                let mut value = self.value(unit, receiver)?;
                if self.compact {
                    // ToInt32 applies ToNumber itself: `+x|0` is `x|0`,
                    // with the same single coercion and the same throws.
                    if let js::Expr::Unary {
                        op: js::Unary::Plus,
                        value: inner,
                    } = self.module.expressions[value.index()]
                    {
                        value = inner;
                    }
                    // Under the numeric-lengths assumption a length is
                    // already an int32 Number.
                    if self.contract.assumptions.numeric_lengths && self.length_member(value) {
                        return Ok(value);
                    }
                }
                js::Expr::ToInt32(value)
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Method(Intrinsic::StringCodePointLength),
                receiver: Some(receiver),
            } if arguments.is_empty() => {
                // The string iterator yields one element per code point, as
                // in the old route's emitter; the host iterator may be patched, so
                // the count is normalized like every integer intrinsic.
                let string = self.value(unit, receiver)?;
                let spread = self.expression(js::Expr::Spread(string))?;
                let mut elements = self.budget.vector(AllocationClass::Retained, 1)?;
                self.append(&mut elements, spread)?;
                let object = self.expression(js::Expr::Array(elements))?;
                let property = js::Property::Named(self.text("length")?);
                let length = self.expression(js::Expr::Member { object, property })?;
                js::Expr::ToInt32(length)
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Method(Intrinsic::ArrayPop),
                receiver: Some(receiver),
            } if arguments.is_empty() => {
                // An empty array pops `undefined`. The element type's absent
                // value applies exactly as for an indexed read.
                let object = self.value(unit, receiver)?;
                let property = js::Property::Named(self.text("pop")?);
                let callee = self.expression(js::Expr::Member { object, property })?;
                let popped = js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Reference,
                };
                let result = contract.signature.and_then(|signature| {
                    match &self.program.types[signature.index()] {
                        Type::Function(signature) => Some(signature.return_type.as_ref()),
                        _ => None,
                    }
                });
                match result {
                    Some(Type::Int) => js::Expr::ToInt32(self.expression(popped)?),
                    Some(Type::String)
                        if self
                            .contract
                            .ecmascript
                            .allows(JsSyntaxFeature::NullishCoalescing) =>
                    {
                        let left = self.expression(popped)?;
                        let empty = self.string(&crate::literal::StringValue::default())?;
                        let right = self.literal(js::Literal::String(empty))?;
                        js::Expr::Binary {
                            op: js::Binary::Nullish,
                            left,
                            right,
                        }
                    }
                    Some(Type::String) => {
                        return Err(self.error(span, "string Array.pop before ES2020"));
                    }
                    _ => popped,
                }
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Method(operation),
                receiver,
            } if js::intrinsic_host_function(operation, self.contract.ecmascript).is_some() => {
                // The legacy spelling `Math.abs(x)`: the receiver becomes the
                // first argument of a host namespace function.
                let path = js::intrinsic_host_function(operation, self.contract.ecmascript)
                    .expect("guarded host function");
                let mut all = self
                    .budget
                    .vector(AllocationClass::Retained, arguments.len() + 1)?;
                if let Some(receiver) = receiver {
                    let receiver = self.value(unit, receiver)?;
                    self.append(&mut all, receiver)?;
                }
                for argument in arguments {
                    self.append(&mut all, argument)?;
                }
                let callee = self.host_path(path)?;
                js::Expr::Call {
                    callee,
                    arguments: all,
                    invocation: Invocation::Reference,
                }
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Method(operation),
                receiver: Some(receiver),
            } if js::supports_intrinsic_method(operation) => {
                let mut receiver = self.value(unit, receiver)?;
                let mut arguments = arguments;
                // A development build checks the code unit's index (R11):
                // `index_checked(s,i)` is the one-unit string at `i`.
                if operation == Intrinsic::StringCodeUnitAt
                    && self.contract.checks
                        == crate::compilation_contract::PreconditionChecks::Development
                {
                    (receiver, arguments) = self.code_unit_check(receiver, arguments)?;
                }
                if operation == Intrinsic::IntToUnsignedString {
                    let zero = self.literal(js::Literal::Number(0.0))?;
                    receiver = self.expression(js::Expr::Binary {
                        op: js::Binary::UnsignedShiftRight,
                        left: receiver,
                        right: zero,
                    })?;
                }
                let call = js::Expr::Intrinsic {
                    operation,
                    receiver,
                    arguments,
                };
                if operation != Intrinsic::MapGet || self.program.source_contract.unified_absence() {
                    call
                } else if self
                    .contract
                    .ecmascript
                    .allows(JsSyntaxFeature::NullishCoalescing)
                {
                    // An absent key is `undefined` in the host and `null` in
                    // the language until R2's second batch (M10.9).
                    let left = self.expression(call)?;
                    let right = self.literal(js::Literal::Null)?;
                    js::Expr::Binary {
                        op: js::Binary::Nullish,
                        left,
                        right,
                    }
                } else {
                    return Err(self.error(span, "Map.get absent value before ES2020"));
                }
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Constructor(operation),
                receiver: None,
            } if js::supports_intrinsic_construction(operation, arguments.len()) => {
                js::Expr::ConstructIntrinsic {
                    operation,
                    arguments,
                }
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Constructor(Intrinsic::SymbolNew),
                receiver: None,
            } => {
                // `Symbol` is called, never constructed.
                let host = self.text("Symbol")?;
                let callee = self.expression(js::Expr::Host(js::Host::new(host)))?;
                js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Value,
                }
            }
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Constructor(Intrinsic::RegexNew),
                receiver: None,
            } => {
                // A typed construction means the original `RegExp` (R10): of
                // literal strings in the proven subset it is the literal, a
                // fresh object at each evaluation. Otherwise the lookup stays
                // before every argument effect, without a temporary or .call.
                let text = |id: &js::ExprId| match &self.module.expressions[id.index()] {
                    js::Expr::Literal(js::Literal::String(value)) => value.as_unicode(),
                    _ => None,
                };
                let literal = match arguments.as_slice() {
                    [pattern] => text(pattern).map(|pattern| (pattern, "")),
                    [pattern, flags] => text(pattern).zip(text(flags)),
                    _ => None,
                }
                .and_then(|(pattern, flags)| {
                    crate::js_regex::literal_from_decoded_checked(
                        pattern,
                        flags,
                        self.contract.ecmascript.year() >= 2018,
                    )
                });
                if let Some(regex) = literal {
                    js::Expr::Regex(regex)
                } else {
                    let host = self.text("RegExp")?;
                    let callee = self.expression(js::Expr::Host(js::Host::new(host)))?;
                    js::Expr::Construct { callee, arguments }
                }
            }
            _ => return Err(self.error(span, "semantic JavaScript call implementation")),
        };
        let sum = matches!(
            node,
            js::Expr::Binary {
                op: js::Binary::Add,
                ..
            }
        ) && matches!(
            self.data(unit).calls[call.index()].target,
            CallTarget::Builtin(BuiltinCall::JsAdd)
        );
        let id = self.expression(node)?;
        if sum {
            self.budget
                .push(AllocationClass::Scratch, &mut self.string_sums, id)?;
        }
        Ok(id)
    }

    /// `Math.abs`, `Object.prototype.hasOwnProperty.call`: one host lookup
    /// followed by ordinary member reads, each observable in order.
    fn host_path(&mut self, path: &[&str]) -> Result<js::ExprId, FormationError> {
        let root = self.text(path[0])?;
        let mut callee = self.expression(js::Expr::Host(js::Host::new(root)))?;
        for member in &path[1..] {
            self.work(1)?;
            let property = js::Property::Named(self.text(member)?);
            callee = self.expression(js::Expr::Member {
                object: callee,
                property,
            })?;
        }
        Ok(callee)
    }

    fn host_binding(&mut self, path: &str) -> Result<js::ExprId, FormationError> {
        self.work(path.len())?;
        let mut parts = path.split('.');
        let name = self.text(parts.next().unwrap())?;
        let mut value = self.expression(js::Expr::Host(js::Host::new(name)))?;
        for part in parts {
            let property = js::Property::Named(self.text(part)?);
            value = self.expression(js::Expr::Member { object: value, property })?;
        }
        Ok(value)
    }

    fn scheduled_expression(
        &mut self,
        unit: ContextId,
        operations: &[OpId],
        cursor: &mut usize,
    ) -> Result<Option<js::ExprId>, FormationError> {
        let prior = self.module.forming_choices;
        self.module.forming_choices = self.data(unit).operations[operations[*cursor].index()].authored;
        let result = self.scheduled_expression_inner(unit, operations, cursor);
        self.module.forming_choices = prior;
        result
    }
    fn scheduled_expression_inner(
        &mut self,
        unit: ContextId,
        operations: &[OpId],
        cursor: &mut usize,
    ) -> Result<Option<js::ExprId>, FormationError> {
        self.work(1)?;
        let operation_id = operations[*cursor];
        let operation = &self.data(unit).operations[operation_id.index()];
        *cursor += 1;
        if let OperationKind::PrepareCall(call) = operation.kind {
            // Consume the original scheduling envelope even if the selected
            // invocation is inert: argument evaluations may still be required.
            return self.prepared_call(unit, call, operations, cursor);
        }
        if !self.demand.needs_operation(unit, operation_id) {
            return Ok(None);
        }
        // Each use of a rematerialized load reads its binding again.
        if operation.result.is_some_and(|result| {
            matches!(
                self.plan(unit).values[result.index()],
                ValueStorage::Rematerialized(_)
            )
        }) {
            return Ok(None);
        }
        if matches!(
            self.demand
                .helper_operation(self.semantic(unit), operation_id),
            Some(HelperOperation::Elided)
        ) {
            return Ok(None);
        }
        self.product_lookup()?;
        if let Some(choice) = self
            .demand
            .product_operation(self.semantic(unit), operation_id)
            .copied()
        {
            if !matches!(
                choice,
                super::product_family::ProductOperationKind::Access(_)
            ) {
                return self.product_expression(unit, operation, choice);
            }
        }
        if let Some(choice) = self
            .demand
            .record_operation(self.semantic(unit), operation_id)
        {
            let value = match choice {
                RecordOperation::ElidedHandle => return Ok(None),
                RecordOperation::Initialize(_) => {
                    return Err(
                        self.error(operation.span, "record initialization in expression region")
                    );
                }
                RecordOperation::Read { record, slot } => {
                    let binding = self.record_binding(record, slot)?;
                    let left = self.reference(binding)?;
                    let right = self.literal(js::Literal::Null)?;
                    return self.save_nullish(unit, &operation, left, right);
                }
                RecordOperation::Write { record, slot } => {
                    let binding = self.record_binding(record, slot)?;
                    let target = self.reference(binding)?;
                    let value = self.data(unit).operands(operation.operands).unwrap()[0];
                    let value = self.value(unit, value)?;
                    self.expression(js::Expr::Assign { target, value })?
                }
            };
            return self.save(unit, &operation, value);
        }
        if let Some(family) = self
            .demand
            .string_operation(self.semantic(unit), operation_id)
        {
            let mut schedule = Vec::new();
            if self.demand.needs_execution(unit, operation_id) {
                if let Some(evaluation) =
                    self.ordinary_expression(unit, operation_id, &operation)?
                {
                    self.append(&mut schedule, evaluation)?;
                }
            }
            if matches!(
                self.demand.strings()[family].choice(),
                StringChoice::LiteralAtDefinition
            ) && operation
                .result
                .is_some_and(|result| self.demand.needs_value(unit, result))
                && !matches!(operation.kind, OperationKind::Constant(_))
            {
                let literal = {
                    self.family_literal(family)
                }?;
                {
                    if let Some(appended) = self.save_result(unit, &operation, literal)? {
                        self.append(&mut schedule, appended)?;
                    }
                    Ok::<_, FormationError>(())
                }?;
            }
            return Ok(self.sequence(schedule)?);
        }
        self.ordinary_expression(unit, operation_id, &operation)
    }

    fn ordinary_expression(
        &mut self,
        unit: ContextId,
        operation_id: OpId,
        operation: &Operation,
    ) -> Result<Option<js::ExprId>, FormationError> {
        let previous = self.forming.replace((
            self.semantic(unit),
            operation_id,
            std::ptr::from_ref(operation),
        ));
        let prior_choices = self.module.forming_choices;
        self.module.forming_choices = operation.authored;
        let formed = self.ordinary_expression_of(unit, operation_id, operation);
        self.module.forming_choices = prior_choices;
        self.forming = previous;
        formed
    }

    /// The operation node `value` holds, with its recipe's wrappers (`void`,
    /// `|0`), takes the behaviour of the operation being formed (M5.2): a
    /// call, a construction, an operator or an integer operation.
    fn record_behaviour(
        &mut self,
        saved: &Operation,
        value: js::ExprId,
    ) -> Result<(), FormationError> {
        // Only the operation being formed: a nested path saves its own.
        let Some((unit, operation, formed)) = self.forming else {
            return Ok(());
        };
        if !std::ptr::eq(saved, formed) {
            return Ok(());
        }
        let operation_node = |expression: &js::Expr| {
            matches!(
                expression,
                js::Expr::Call { .. }
                    | js::Expr::Construct { .. }
                    | js::Expr::Binary { .. }
                    | js::Expr::IntBinary { .. }
                    | js::Expr::IntNegate(_)
                    | js::Expr::ToInt32(_)
            )
        };
        let mut nodes = Vec::new();
        let mut node = value;
        loop {
            match &self.module.expressions[node.index()] {
                js::Expr::Unary {
                    op: js::Unary::Void,
                    value,
                } => {
                    nodes.push(node);
                    node = *value;
                }
                js::Expr::ToInt32(inner)
                    if operation_node(&self.module.expressions[inner.index()]) =>
                {
                    nodes.push(node);
                    node = *inner;
                }
                expression => {
                    if operation_node(expression) {
                        nodes.push(node);
                    } else {
                        nodes.clear();
                    }
                    break;
                }
            }
        }
        if nodes.is_empty() {
            return Ok(());
        }
        let Some(behavior) = self.demand.operation_behavior(unit, operation) else {
            return Ok(());
        };
        let behaviour = js::Behaviour {
            reads: behavior.reads != super::facts::MemoryAccess::None,
            writes: behavior.writes != super::facts::MemoryAccess::None,
            throws: behavior.may_throw,
            diverges: behavior.may_diverge,
            reenters: behavior.may_reenter,
            suspends: behavior.may_suspend,
        };
        for node in nodes {
            self.module
                .record_behaviour_in(node, behaviour, self.budget)?;
        }
        Ok(())
    }

    fn ordinary_expression_of(
        &mut self,
        unit: ContextId,
        operation_id: OpId,
        operation: &Operation,
    ) -> Result<Option<js::ExprId>, FormationError> {
        self.work(1)?;
        let operands = self.data(unit).operands(operation.operands).unwrap();
        let node = match operation.kind {
            OperationKind::Enum { declaration, operation: kind } => {
                let mut arguments = self.budget.vector(AllocationClass::Scratch, operands.len())?;
                for &operand in operands {
                    let value = self.value(unit, operand)?;
                    self.budget.push(AllocationClass::Scratch, &mut arguments, value)?;
                }
                let value = self.enum_operation(declaration, kind, &arguments)?;
                self.drop_scratch(arguments)?;
                return self.save(unit, operation, value);
            }
            OperationKind::Constant(_) => return Ok(None),
            OperationKind::Block(body) => self.statement_expression(unit, body, operation.span)?,
            OperationKind::PrepareReference { call, position } => {
                return Ok(Some(self.prepare_reference(unit, call, position)?));
            }
            OperationKind::CheckPlace(place) => {
                // Access checks never perform the leaf value's result recipe.
                // In particular checking an int field must not call valueOf.
                if self.reference_check_proved(
                    unit,
                    place,
                    super::demand::LocationCheck::CheckPlace {
                        operation: operation_id,
                    },
                )? {
                    return Ok(None);
                }
                return Ok(Some(self.check_reference_place(unit, place)?));
            }
            OperationKind::Load(place) => {
                if let Some(result) = operation.result {
                    if self.elided_log_lookup(self.semantic(unit), result)? {
                        return Ok(None);
                    }
                    // A rest list spelled as formals is read only through
                    // its indices (`place`).
                    if self.formal_list_value(unit, result).is_some() {
                        return Ok(None);
                    }
                }
                let ty = self.data(unit).values[operation.result.unwrap().index()].ty;
                return self.load(unit, &operation, place, ty);
            }
            OperationKind::Store(place) => {
                if matches!(self.data(unit).places[place.index()], Place::Field { .. }) {
                    let value = self.store_value_field(unit, place, operands[0], operation.span)?;
                    if operation.result.is_some() {
                        let result = self.value(unit, operands[0])?;
                        let saved = self.save(unit, operation, result)?;
                        let mut sequence = self.budget.vector(AllocationClass::Retained, 2)?;
                        self.append(&mut sequence, value)?;
                        if let Some(saved) = saved {
                            self.append(&mut sequence, saved)?;
                        }
                        return self.sequence(sequence);
                    }
                    return Ok(Some(value));
                }
                let value = self.value(unit, operands[0])?;
                if let Place::Cell(cell) = self.data(unit).places[place.index()] {
                    let expected = &self.program.types[self.program.cells[cell.index()].ty.index()];
                    let actual = &self.program.types[self.data(unit).values[operands[0].index()].ty.index()];
                    let value = if matches!(expected, Type::Dynamic | Type::Unknown) { self.absence_erasure(actual, value)? } else { value };
                    let stored = self.store_cell(unit, cell, value)?;
                    if references::is_reference(self.program, cell) && operation.result.is_some() {
                        let mut sequence = self.budget.vector(AllocationClass::Retained, 2)?;
                        self.append(&mut sequence, stored)?;
                        let result = self.value(unit, operands[0])?;
                        if let Some(saved) = self.save(unit, operation, result)? {
                            self.append(&mut sequence, saved)?;
                        }
                        return self.sequence(sequence);
                    }
                    return self.save(unit, operation, stored);
                }
                let value = if let Place::Index { receiver, .. } = self.data(unit).places[place.index()] {
                    match &self.program.types[self.data(unit).values[receiver.index()].ty.index()] {
                        Type::Array(element) => self.stored_absence(element, value)?,
                        _ => value,
                    }
                } else { value };
                // A legacy construction seed is not a value of a required
                // product field. Actual optional slots still owe their public
                // absence pin, including when the source writes literal null.
                let public_type = self.public_storage_type(unit, place)?;
                let public_type = public_type.filter(|ty| {
                    !matches!(self.program.types[self.data(unit).values[operands[0].index()].ty.index()], Type::Null)
                        || matches!(ty.as_ref(), Type::Nullable(_))
                });
                let mut sequence = Vec::new();
                let result_binding = if public_type.is_some() && operation.result.is_some() {
                    let region = self.plan(unit).regions[operation.region.index()];
                    let scope = self.module.regions[region.index()].scope;
                    let binding = self.fresh_binding(scope, "stored")?;
                    self.statement(region, js::Statement::Let { binding, value: None })?;
                    let captured = self.assign(binding, value, None)?;
                    self.append(&mut sequence, captured)?;
                    Some(binding)
                } else { None };
                let value = match result_binding { Some(binding) => self.reference(binding)?, None => value };
                let value = if let Some(ty) = public_type { self.public_value(&ty, value, false)? } else { value };
                let stored = if let Some(stored) = self.shape_store(unit, place, value)? { stored } else {
                    let target = self.place(unit, place)?;
                    self.expression(js::Expr::Assign { target, value })?
                };
                let Some(binding) = result_binding else { return self.save(unit, operation, stored); };
                // An encoded store returns the original private value. Capture
                // it once even when placement would inline its producer.
                self.append(&mut sequence, stored)?;
                let result = self.reference(binding)?;
                if let Some(saved) = self.save(unit, operation, result)? { self.append(&mut sequence, saved)?; }
                return self.sequence(sequence);
            }
            OperationKind::Initialize(cell) => {
                let binding = self.cell_binding(unit, cell)?;
                if !self.demand.context(unit).kind.is_inline() {
                    let region = self.plan(unit).regions[operation.region.index()];
                    self.statement(region, js::Statement::Let { binding, value: None })?;
                }
                let target = self.reference(binding)?;
                let value = self.value(unit, operands[0])?;
                let value = self.boundary_cell(cell, value)?;
                let value = self.carrier_value(unit, cell, value)?;
                js::Expr::Assign { target, value }
            }
            OperationKind::Declare(cell) => {
                if !self.demand.context(unit).kind.is_inline() {
                    let region = self.plan(unit).regions[operation.region.index()];
                    let binding = self.cell_binding(unit, cell)?;
                    self.statement(region, js::Statement::Let { binding, value: None })?;
                }
                if self.payload_cell(cell)? {
                    let absent = self.literal(js::Literal::Undefined)?;
                    let value = self.carrier_value(unit, cell, absent)?;
                    let binding = self.cell_binding(unit, cell)?;
                    let target = self.reference(binding)?;
                    return Ok(Some(self.expression(js::Expr::Assign { target, value })?));
                }
                return Ok(None);
            }
            OperationKind::IsUndefined { nullish, .. } => {
                let left = self.value(unit, operands[0])?;
                let right = self.literal(if nullish { js::Literal::Null } else { js::Literal::Undefined })?;
                js::Expr::Binary {
                    op: if nullish { js::Binary::Equal } else { js::Binary::StrictEqual },
                    left,
                    right,
                }
            }
            OperationKind::Await => js::Expr::Await(self.value(unit, operands[0])?),
            OperationKind::LoadModule { module, specifier } => {
                let promise = self.host_path(&["Promise"])?;
                let string = self.host_path(&["String"])?;
                let program = self.program;
                let members_len = program.modules[module.index()].namespace.len();
                let mut members = self.budget.vector(AllocationClass::Retained, members_len)?;
                for index in 0..members_len {
                    let (name, cell) = &program.modules[module.index()].namespace[index];
                    let binding = self.cell_binding(unit, *cell)?;
                    let value = self.expression(js::Expr::Binding(binding))?;
                    let name = self.text(name)?;
                    self.budget
                        .push(AllocationClass::Retained, &mut members, (name, value))?;
                }
                let specifier =
                    program.strings[specifier.index()]
                        .as_unicode()
                        .ok_or_else(|| {
                            self.error(operation.span, "dynamic import specifier is not text")
                        })?;
                let specifier = self.text(specifier)?;
                js::Expr::LoadModule {
                    module: module.index() as u32,
                    specifier,
                    members,
                    promise,
                    string,
                }
            }
            // `new C(...)`: the first operand is C's constructor, the class value.
            OperationKind::ConstructClass => {
                let callee = self.value(unit, operands[0])?;
                let signature = self.program.types
                    [self.data(unit).values[operands[0].index()].ty.index()]
                    .callable_signature();
                let mut arguments = self
                    .budget
                    .vector(AllocationClass::Retained, operands.len() - 1)?;
                for (index, &argument) in operands[1..].iter().enumerate() {
                    let parameter = signature.and_then(|signature| signature.params.get(index + 1));
                    let argument = self.public_constructor_argument(unit, argument, parameter)?;
                    self.append(&mut arguments, argument)?;
                }
                if let Some(signature) = self.program.types
                    [self.data(unit).values[operands[0].index()].ty.index()]
                .callable_signature()
                {
                    if signature.has_rest() && arguments.len() + 1 == signature.params.len() {
                        let last = arguments.last_mut().unwrap();
                        *last = self.expression(js::Expr::Spread(*last))?;
                    }
                }
                js::Expr::Construct { callee, arguments }
            }
            OperationKind::ClosedClassTest(class) => {
                let value = self.value(unit, operands[0])?;
                self.closed_class_test(class, value)?
            }
            OperationKind::TypeTest(target) if super::schema::is_shape(self.program, &self.program.types[target.index()]) => {
                let value = self.value(unit, operands[0])?;
                let result = self.shape_test(target, value)?;
                return self.save(unit, operation, result);
            }
            OperationKind::TypeTest(target)
                if matches!(
                    self.program.types[target.index()],
                    crate::check::Type::Class(_)
                ) =>
            {
                // An identity test on a class (R13): `instanceof`, over the
                // kept class's constructor, or the host class by its name.
                let crate::check::Type::Class(declaration) = &self.program.types[target.index()]
                else {
                    unreachable!("matched a class target")
                };
                let class = declaration.identity;
                let left = self.value(unit, operands[0])?;
                let right = match operands.get(1) {
                    Some(&constructor) => self.value(unit, constructor)?,
                    None => {
                        let definition = self.program.class(class).ok_or_else(|| {
                            self.error(operation.span, "identity test on an undeclared class")
                        })?;
                        self.host_binding(&definition.name)?
                    }
                };
                js::Expr::Binary {
                    op: js::Binary::InstanceOf,
                    left,
                    right,
                }
            }
            OperationKind::TypeTest(target) => {
                let value = self.value(unit, operands[0])?;
                match crate::primitive::runtime_type_test(&self.program.types[target.index()]) {
                    Some(crate::primitive::RuntimeTypeTest::TypeOf(name)) => {
                        let left = self.expression(js::Expr::Unary {
                            op: js::Unary::TypeOf,
                            value,
                        })?;
                        let name = self.string(&crate::literal::StringValue::from(name))?;
                        let right = self.literal(js::Literal::String(name))?;
                        js::Expr::Binary {
                            op: js::Binary::StrictEqual,
                            left,
                            right,
                        }
                    }
                    Some(crate::primitive::RuntimeTypeTest::IsArray) => {
                        let callee = self.host_path(&["Array", "isArray"])?;
                        let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
                        self.append(&mut arguments, value)?;
                        js::Expr::Call {
                            callee,
                            arguments,
                            invocation: Invocation::Reference,
                        }
                    }
                    None => {
                        return Err(self.error(operation.span, "type test without a runtime test"));
                    }
                }
            }
            OperationKind::Template => {
                // Literal chunks become template text; a flushed prefix that
                // stayed inline is spliced so one literal carries the whole.
                let mut parts = self
                    .budget
                    .vector(AllocationClass::Retained, operands.len())?;
                for &operand in operands.iter() {
                    self.work(1)?;
                    let value = self.value(unit, operand)?;
                    let spliced = match &self.module.expressions[value.index()] {
                        js::Expr::Literal(js::Literal::String(text)) => {
                            usize::from(!self.module.pinned_string(text, self.budget)?)
                        }
                        js::Expr::Template(inner) => inner.len(),
                        _ => 0,
                    };
                    if spliced == 0 {
                        self.append(&mut parts, js::TemplatePart::Expression(value))?;
                        continue;
                    }
                    for index in 0..spliced {
                        self.work(1)?;
                        let part = match &self.module.expressions[value.index()] {
                            js::Expr::Literal(js::Literal::String(text)) => {
                                js::TemplatePart::String(
                                    self.budget.string_value(AllocationClass::Retained, text)?,
                                )
                            }
                            js::Expr::Template(inner) => match &inner[index] {
                                js::TemplatePart::String(text) => js::TemplatePart::String(
                                    self.budget.string_value(AllocationClass::Retained, text)?,
                                ),
                                js::TemplatePart::Expression(id) => {
                                    js::TemplatePart::Expression(*id)
                                }
                            },
                            _ => unreachable!("counted splice"),
                        };
                        self.append(&mut parts, part)?;
                    }
                }
                js::Expr::Template(parts)
            }
            OperationKind::CopyValue => {
                self.transfer_number(unit, operation, false, |this| {
                    this.number(unit, operands[0])
                })?;
                let value = self.value(unit, operands[0])?;
                return self.save(unit, &operation, value);
            }
            OperationKind::IntBinary(op) => {
                let raw = self.transfer_number(unit, operation, true, |this| {
                    this.number(unit, operands[0])
                        .binary(op.javascript(), this.number(unit, operands[1]))
                })?;
                let left = self.value(unit, operands[0])?;
                let right = self.value(unit, operands[1])?;
                if self.compact && raw.normalization_redundant() {
                    js::Expr::Binary {
                        op: op.javascript(),
                        left,
                        right,
                    }
                } else {
                    js::Expr::IntBinary { op, left, right }
                }
            }
            OperationKind::Binary(op) => {
                self.transfer_number(unit, operation, false, |this| {
                    this.number(unit, operands[0])
                        .binary(binary(op), this.number(unit, operands[1]))
                })?;
                let left = self.value(unit, operands[0])?;
                let right = self.value(unit, operands[1])?;
                if op == BinaryOp::Nullish {
                    return self.save_nullish(unit, &operation, left, right);
                }
                // Equality on a `JsValue` is JavaScript's `==`: a dynamic
                // equality conversion (the language contract; `JS.strictEqual`
                // is the strict form). A typed nullable value is `T` or `null`,
                // but a host may hand one over as `undefined` (a missing
                // element, say): the loose test treats both as absent.
                let program = self.program;
                let ty = |value: ValueId| {
                    &program.types[self.data(unit).values[value.index()].ty.index()]
                };
                let dynamic = matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && operands[..2]
                        .iter()
                        .any(|&value| matches!(ty(value), Type::Dynamic));
                let absent_test = self.program.source_contract.unified_absence()
                    && matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && operands[..2].iter().any(|&value| matches!(ty(value), Type::Nullable(_)));
                let null_test = matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && operands[..2]
                        .iter()
                        .any(|&value| matches!(ty(value), Type::Null));
                // Two numbers, two strings or two booleans compare the same
                // loosely: `==` converts nothing when the types already agree.
                let primitive = |ty: &Type<'_>| match ty {
                    Type::Int | Type::Float => Some(0),
                    Type::Enum(declaration) => Some(if declaration.is_string() { 1 } else { 0 }),
                    Type::String => Some(1),
                    Type::Bool => Some(2),
                    _ => None,
                };
                let same_primitive = self.compact
                    && matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && primitive(ty(operands[0])).is_some()
                    && primitive(ty(operands[0])) == primitive(ty(operands[1]));
                js::Expr::Binary {
                    op: match (dynamic || null_test || absent_test || same_primitive, op) {
                        (true, BinaryOp::Eq) => js::Binary::Equal,
                        (true, _) => js::Binary::NotEqual,
                        _ => binary(op),
                    },
                    left,
                    right,
                }
            }
            OperationKind::Unary { op, integer } => {
                let raw =
                    self.transfer_number(unit, operation, integer && op == UnaryOp::Neg, |this| {
                        this.number(unit, operands[0]).unary(match op {
                            UnaryOp::Neg => js::Unary::Negate,
                            UnaryOp::Not => js::Unary::Not,
                        })
                    })?;
                let value = self.value(unit, operands[0])?;
                match op {
                    UnaryOp::Neg if integer && !(self.compact && raw.normalization_redundant()) => {
                        js::Expr::IntNegate(value)
                    }
                    UnaryOp::Neg => js::Expr::Unary {
                        op: js::Unary::Negate,
                        value,
                    },
                    UnaryOp::Not => js::Expr::Unary {
                        op: js::Unary::Not,
                        value,
                    },
                }
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(property))
                if js::supports_intrinsic_property(property) =>
            {
                // A typed length is the original's (R10), at most 2^30 (the
                // bound counting loops already use), so arithmetic near it
                // needs no int32 normalization.
                if matches!(property, Intrinsic::StringLength | Intrinsic::ArrayLength) {
                    self.transfer_number(unit, operation, false, |_| {
                        NumberFacts::integer_range(0, 1 << 30, false)
                            .unwrap_or(NumberFacts::UNKNOWN)
                    })?;
                }
                let receiver = self.value(unit, operands[0])?;
                js::Expr::Intrinsic {
                    operation: property,
                    receiver,
                    arguments: Vec::new(),
                }
            }
            OperationKind::ShortCircuit { kind, right } => {
                let right_result = self.data(unit).regions[right.index()].result;
                let left = self.value(unit, operands[0])?;
                let right = match self.expression_region(unit, right)? {
                    Some(value) => value,
                    None => self.literal(js::Literal::Undefined)?,
                };
                let op = match kind {
                    ShortCircuit::BooleanAnd | ShortCircuit::JavaScriptAnd => js::Binary::And,
                    ShortCircuit::BooleanOr | ShortCircuit::JavaScriptOr => js::Binary::Or,
                    ShortCircuit::Nullish => js::Binary::Nullish,
                };
                self.transfer_number(unit, operation, false, |this| {
                    this.number(unit, operands[0]).binary(
                        op,
                        right_result.map_or(NumberFacts::UNKNOWN, |value| this.number(unit, value)),
                    )
                })?;
                if kind == ShortCircuit::Nullish {
                    return self.save_nullish(unit, &operation, left, right);
                }
                js::Expr::Binary { op, left, right }
            }
            OperationKind::Select { yes, no } => {
                let yes_result = self.data(unit).regions[yes.index()].result;
                let no_result = self.data(unit).regions[no.index()].result;
                let condition = self.value(unit, operands[0])?;
                let yes = match self.expression_region(unit, yes)? {
                    Some(value) => value,
                    None => self.literal(js::Literal::Undefined)?,
                };
                let no = match self.expression_region(unit, no)? {
                    Some(value) => value,
                    None => self.literal(js::Literal::Undefined)?,
                };
                self.transfer_number(unit, operation, false, |this| {
                    yes_result
                        .map_or(NumberFacts::UNKNOWN, |value| this.number(unit, value))
                        .join(
                            no_result
                                .map_or(NumberFacts::UNKNOWN, |value| this.number(unit, value)),
                        )
                })?;
                js::Expr::Conditional { condition, yes, no }
            }
            OperationKind::Allocate { ref kind, .. } => match kind {
                AllocationKind::SpreadArray(spread) => {
                    let mut values = self
                        .budget
                        .vector(AllocationClass::Retained, operands.len())?;
                    for (&operand, &spreads) in operands.iter().zip(spread) {
                        self.work(1)?;
                        let mut value = self.value(unit, operand)?;
                        if spreads {
                            value = self.array_spread(unit, operation.result.unwrap(), operand, value)?;
                            value = self.expression(js::Expr::Spread(value))?;
                        } else {
                            value = self.array_element(unit, operation.result.unwrap(), value)?;
                        }
                        self.append(&mut values, value)?;
                    }
                    js::Expr::Array(values)
                }
                AllocationKind::Array => {
                    let mut values = self
                        .budget
                        .vector(AllocationClass::Retained, operands.len())?;
                    for &operand in operands {
                        self.work(1)?;
                        let value = self.value(unit, operand)?;
                        let value = self.array_element(unit, operation.result.unwrap(), value)?;
                        self.append(&mut values, value)?;
                    }
                    js::Expr::Array(values)
                }
                AllocationKind::Record(keys)
                | AllocationKind::Object(keys)
                | AllocationKind::Instance { keys, .. } => {
                    let count = keys
                        .len()
                        .checked_add(usize::from(matches!(kind, AllocationKind::Record(_))))
                        .ok_or(AllocationError::Capacity)?;
                    let mut entries = self.budget.vector(AllocationClass::Retained, count)?;
                    if matches!(kind, AllocationKind::Record(_)) {
                        let null = self.literal(js::Literal::Null)?;
                        let property = js::Property::Named(self.text("__proto__")?);
                        self.append(&mut entries, (property, null))?;
                    }
                    for (slot, (&key, &operand)) in keys.iter().zip(operands).enumerate() {
                        self.work(1)?;
                        let mut value = self.value(unit, operand)?;
                        let ty = self.data(unit).values[operation.result.unwrap().index()].ty;
                        if let AllocationKind::Instance { class, .. } = kind {
                            if self.program.class(*class).is_some_and(|class| class.reflected || class.external) {
                                let ty = super::schema::class_field_type(self.program, &self.program.types[ty.index()],
                                    FieldRef { nominal: *class, slot: slot as u32 },
                                    &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget))?
                                    .ok_or_else(|| self.error(operation.span, "missing construction field"))?;
                                if !matches!(self.program.types[self.data(unit).values[operand.index()].ty.index()], Type::Null)
                                    || matches!(ty.as_ref(), Type::Nullable(_)) {
                                    value = self.public_value(&ty, value, false)?;
                                }
                            }
                        } else if matches!(self.program.types[ty.index()], Type::Intersection(_)) {
                            let fields = super::schema::shape_fields(self.program, &self.program.types[ty.index()],
                                &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget))?
                                .ok_or_else(|| self.error(operation.span, "missing joined construction fields"))?;
                            value = self.public_value(&fields[slot].1, value, false)?;
                        }
                        // Kept a string occurrence; the printer spells an
                        // identifier key `name:` (never `__proto__:`).
                        let key = if let AllocationKind::Instance { class, .. } = kind {
                            self.class_field_key(FieldRef {
                                nominal: *class,
                                slot: slot as u32,
                            })?
                        } else {
                            let payload = self.string(&self.program.strings[key.index()])?;
                            self.literal(js::Literal::String(payload))?
                        };
                        self.append(&mut entries, (js::Property::Computed(key), value))?;
                    }
                    let object = self.expression(js::Expr::Object(entries))?;
                    let ty = self.data(unit).values[operation.result.unwrap().index()].ty;
                    let object = self.shape_cleanup(ty, object)?;
                    return self.save(unit, operation, object);
                }
                AllocationKind::Struct(identity) => {
                    self.validate_struct_schema(*identity, operation.span)?;
                    let mut values = self
                        .budget
                        .vector(AllocationClass::Retained, operands.len())?;
                    for &operand in operands {
                        self.work(1)?;
                        let value = self.value(unit, operand)?;
                        self.append(&mut values, value)?;
                    }
                    return {
                        let value = self.product(values)?;
                        self.save(unit, operation, value)
                    };
                }
            },
            OperationKind::Closure(created) => {
                // A static body only its class's prototype reaches is formed
                // inside the class, as the method itself (`class_expression`).
                if self.prototype_method_form(created)?.is_some() {
                    self.prototype_method_function(unit, created, operation.span)?;
                    return Ok(None);
                }
                let region = self.plan(unit).regions[operation.region.index()];
                let rest_scope = self
                    .program
                    .unit(created)
                    .filter(|data| data.constructor_of.is_none())
                    .and_then(|data| data.callable_type)
                    .is_some_and(|ty| match &self.program.types[ty.index()] {
                        Type::Function(signature) => signature.has_rest(),
                        Type::GenericFunction(generic) => generic.signature.has_rest(),
                        _ => false,
                    });
                let parent = self.module.regions[region.index()].scope;
                let capture_factory = self.payload_factory(unit, created, parent)?;
                let parent = capture_factory.as_ref().map_or(parent, |factory| self.module.regions[factory.body.index()].scope);
                let declared_defaults = self.program.unit(created)
                    .is_some_and(|data| data.constructor_of.is_none() && data.declared_length.is_some());
                let rest_factory = if rest_scope || declared_defaults {
                    Some(self.module.region_in(parent, self.budget)?)
                } else {
                    None
                };
                let parent = rest_factory
                    .map_or(parent, |factory| self.module.regions[factory.index()].scope);
                let body = self.module.region_in(parent, self.budget)?;
                let child = self
                    .demand
                    .child(unit, operation_id)
                    .ok_or_else(|| self.error(operation.span, "missing callable demand context"))?;
                // An adapter's private callback is the adapter's result: a
                // method, its own function (law P1).
                let declared_method = self.declared_method_form(created)?;
                let method = match declared_method {
                    Some(form) => Some(form),
                    None => self.adapter_method_form(unit, operation_id)?,
                };
                let formals = match &method {
                    Some(form) => self.begin_method(body, form)?,
                    None => Vec::new(),
                };
                // A kept class's constructor holds its instance: `this`,
                // bound from entry in a root class and where `super(...)`
                // returns in a derived one. Its reads are `this` where they
                // see the constructor's own `this`; otherwise a root class
                // aliases it at entry, and a derived one after `super(...)`.
                let instance_this = match self.data(child).constructor_of {
                    Some(_) => {
                        let instance = self.data(child).parameters[0];
                        let this = self.activation_spelling(created, instance)? == Some(true);
                        if this {
                            for list in [
                                &mut self.this_cells,
                                &mut self.activation_cells,
                                &mut self.unbound_cells,
                            ] {
                                self.budget.push(AllocationClass::Scratch, list, instance)?;
                            }
                        }
                        this
                    }
                    None => false,
                };
                self.plan_context(child, body)?;
                if let Some(factory) = &capture_factory { self.payload_capture_bindings(child, factory)?; }
                if let Some(form) = &method {
                    self.method_aliases(child, body, form)?;
                }
                if let Some(class) = self.data(child).constructor_of {
                    let instance = self.data(child).parameters[0];
                    if !instance_this
                        && self
                            .program
                            .class(class)
                            .is_some_and(|definition| definition.base.is_none())
                    {
                        let this = self.expression(js::Expr::This)?;
                        let binding = self.cell_binding(child, instance)?;
                        self.statement(
                            body,
                            js::Statement::Let {
                                binding,
                                value: Some(this),
                            },
                        )?;
                    }
                }
                // A development build checks what an exported function's
                // caller passes (R1).
                if let (Some(result), None) = (operation.result, &method) {
                    self.export_parameter_checks(unit, result, child, body)?;
                }
                self.public_constructor_parameters(child, body)?;
                self.statement_region(child, self.data(child).entry)?;
                self.finish_unit(child)?;
                let parameters = match &method {
                    Some(form) => self.method_parameters(child, form, formals)?,
                    None => self.physical_parameters(child)?,
                };
                let name = self.data(child).function_name.ok_or_else(|| {
                    self.error(operation.span, "missing semantic function-name contract")
                })?;
                let function = js::FunctionId::try_new(self.module.functions.len())
                    .ok_or(AllocationError::Capacity)?;
                // A function published through a D2 wrapper, or a class
                // method (only ever called directly), has no observable name.
                let body_unit = self.semantic(child);
                let mut private = self.struct_plan.wrapped(body_unit);
                // Synthetic cells follow every checked cell.
                let program = self.program;
                for cell in program.cells.iter().rev() {
                    self.work(1)?;
                    if private || !cell.synthetic {
                        break;
                    }
                    private = cell.binding == CellBinding::Function(body_unit);
                }
                // A private function's cell holds it directly (below).
                let private_cell = match operation.result {
                    Some(result) => self.private_function_cell(unit, result)?,
                    None => None,
                };
                // Only direct calls reach a private function: nothing can
                // construct it or read its prototype, so an arrow is the same
                // callable unless it observes its own activation (below).
                let requested_arrow = self.data(child).kind == UnitKind::Closure
                    || self.compact && (private || private_cell.is_some());
                // A declaration supplies its own receiver/arguments, including
                // observations in lexical descendants. That language contract
                // takes precedence over arrow spelling, as with the legacy
                // arguments rule. Closures keep their original lexical owner.
                let suspension = match self.data(child).suspension {
                    Suspension::None => js::Suspension::None,
                    Suspension::Async => js::Suspension::Async,
                    Suspension::Generator if self.plan(child).generator_entry.is_some() => js::Suspension::None,
                    Suspension::Generator => js::Suspension::Generator,
                };
                // A generator has no arrow form.
                let arrow = requested_arrow
                    && suspension != js::Suspension::Generator
                    && !(self.data(child).kind == UnitKind::Function
                        && self.plan(child).observes_activation);
                let unobserved = match operation.result {
                    Some(result) if self.compact && !private && private_cell.is_none() => {
                        self.unobserved_closure_name(unit, result)?
                    }
                    _ => false,
                };
                // D2 keeps a published function's source name unless the
                // contract publishes names only. Any other function's name is
                // its binding's unless the contract keeps every name some
                // code could read.
                let internal = self.compact
                    && !self.contract.abi.keep_function_names
                    && !(self.contract.abi.keep_published_function_names
                        && match operation.result {
                            Some(result) => self.flows_into_exported_cell(unit, result)?,
                            None => true,
                        });
                // The exact name is allocated only when it is kept.
                let mut name = if private || private_cell.is_some() || unobserved || internal {
                    js::FunctionName::Unobserved
                } else {
                    js::FunctionName::Exact(self.string(&self.program.strings[name.index()])?)
                };
                let mut arrow = arrow;
                let mut length = None;
                if let Some(form) = &method {
                    // The adapter's result: an anonymous function where its
                    // name is observed; a rest adapter's `length` is 0.
                    arrow = form.arrow;
                    name = if form.exact_empty_name {
                        js::FunctionName::Exact(crate::literal::StringValue::default())
                    } else {
                        js::FunctionName::Unobserved
                    };
                    if matches!(form.list, Some((_, methods::List::Formals(count))) if count > 0) {
                        length = Some(0);
                    }
                }
                let strict = self.plan(child).strict_frame || declared_method.is_some();
                if method.is_none()
                    && (self.plan(child).generator_entry.is_some()
                        || !private && private_cell.is_none())
                {
                    // Observable arity belongs to the source declaration,
                    // independently of optional default-argument transport.
                    let receiver = usize::from(self.data(child).constructor_of.is_some());
                    length = self.data(child).declared_length
                        .map(|length| (length as usize).saturating_sub(receiver));
                }
                if self.default_transport
                    && length.is_none()
                    && method.is_none()
                    && !strict
                    && !self.plan(child).observes_activation
                    && self.data(child).constructor_of.is_none()
                    && parameters.len() == self.data(child).parameters.len()
                {
                    length = self
                        .data(child)
                        .native_default_length
                        .map(|value| value as usize);
                }
                if strict && self.plan(child).observes_activation && declared_method.is_none() {
                    // Strictness would change this frame's `this`/`arguments`.
                    return Err(self.error(
                        operation.span,
                        "value-struct script callable frame adaptation",
                    ));
                }
                // A host-derived class's constructor: its instance parameter
                // is `this`, bound where `super(...)` returns.
                let constructor_of = self.data(child).constructor_of;
                let mut parameters = parameters;
                if constructor_of.is_some() && !instance_this {
                    parameters.remove(0);
                }
                if declared_method.is_some() {
                    length = self
                        .data(child)
                        .declared_length
                        .map(|length| length.saturating_sub(1) as usize);
                }
                let rest = self.data(child).callable_type.is_some_and(|id| {
                    match &self.program.types[id.index()] {
                        Type::Function(signature) => signature.has_rest(),
                        Type::GenericFunction(function) => function.signature.has_rest(),
                        _ => false,
                    }
                });
                let non_simple_frame = rest || declared_defaults && length.is_some();
                self.budget.push(
                    AllocationClass::Retained,
                    &mut self.module.functions,
                    js::Function {
                        rest,
                        parameters,
                        body,
                        arrow: arrow && constructor_of.is_none(),
                        name: if constructor_of.is_some() {
                            js::FunctionName::Unobserved
                        } else {
                            name
                        },
                        strict: strict && constructor_of.is_none() && !non_simple_frame,
                        length,
                        suspension,
                    },
                )?;
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut self.unit_functions,
                    (body_unit, function),
                )?;
                if self.contract.checks == crate::compilation_contract::PreconditionChecks::Production
                    && self.demand.discardable_body(body_unit) {
                    self.budget.push(AllocationClass::Retained, &mut self.module.discardable_functions, function)?;
                }
                // A private function goes straight into its cell's binding;
                // its name is not observable, so any spelling will do.
                if let Some(result) = operation.result.filter(|_| private_cell.is_some()) {
                    if let Some(cell) = private_cell {
                        let _ = result;
                        let node =
                            self.rest_strict_frame(function, non_simple_frame && strict, rest_factory)?;
                        let value = self.expression(node)?;
                        let binding = self.cell_binding(unit, cell)?;
                        let region = self.plan(unit).regions[operation.region.index()];
                        self.statement(
                            region,
                            js::Statement::Let {
                                binding,
                                value: Some(value),
                            },
                        )?;
                        return Ok(None);
                    }
                }
                let node = match constructor_of {
                    Some(class) => self.class_expression(
                        unit,
                        operation.region,
                        class,
                        function,
                        operation.span,
                    )?,
                    None => self.rest_strict_frame(function, non_simple_frame && strict, rest_factory)?,
                };
                self.finish_payload_factory(capture_factory, node)?
            }
            _ => {
                return Err(self.error(
                    operation.span,
                    "semantic statement in expression schedule or unsupported implementation",
                ));
            }
        };
        let value = self
            .module
            .expression_in(node, operation.origin, self.budget)?;
        self.save(unit, &operation, value)
    }

    /// Compiler-created counted copies can occur inside a call argument,
    /// conditional arm or loop test. Keep their complete schedule at that
    /// occurrence. A private strict arrow preserves lexical this/arguments
    /// and hides the added frame from sloppy callers. Only typed storage and
    /// primitive arithmetic enter this frame: no host call, dynamic access,
    /// suspension or transfer out of the surrounding function is moved here.
    fn statement_expression(
        &mut self,
        unit: ContextId,
        body: RegionId,
        span: Span,
    ) -> Result<js::Expr, FormationError> {
        let mut pending = self.budget.vector(AllocationClass::Scratch, 1)?;
        self.budget
            .push(AllocationClass::Scratch, &mut pending, body)?;
        while let Some(region) = pending.pop() {
            for &id in &self.data(unit).regions[region.index()].operations {
                self.work(1)?;
                let operation = &self.data(unit).operations[id.index()];
                let data = self.data(unit);
                let allowed = match operation.kind {
                    OperationKind::Constant(_)
                    | OperationKind::CopyValue
                    | OperationKind::IntBinary(_)
                    | OperationKind::Loop { .. }
                    | OperationKind::Block(_) => true,
                    OperationKind::Initialize(cell) | OperationKind::Declare(cell) => {
                        self.program.cells[cell.index()].synthetic
                            && self.program.cells[cell.index()].source_symbol.is_none()
                    }
                    OperationKind::Load(place) | OperationKind::Store(place) => {
                        match data.places[place.index()] {
                            Place::Cell(cell) => {
                                self.program.cells[cell.index()].synthetic
                                    && self.program.cells[cell.index()].source_symbol.is_none()
                            }
                            Place::Index { receiver, .. } => matches!(
                                self.program.types[data.values[receiver.index()].ty.index()],
                                Type::Array(_) | Type::Record(_)
                            ),
                            _ => false,
                        }
                    }
                    OperationKind::Binary(_) => data
                        .operands(operation.operands)
                        .unwrap()
                        .iter()
                        .all(|value| {
                            matches!(
                                self.program.types[data.values[value.index()].ty.index()],
                                Type::Int | Type::Float | Type::Bool | Type::String
                            )
                        }),
                    OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                        crate::primitive::Intrinsic::ArrayLength,
                    )) => true,
                    _ => false,
                };
                if !allowed {
                    self.drop_scratch(pending)?;
                    return Err(
                        self.error(span, "statement expression requires an unobservable frame")
                    );
                }
                for child in operation.kind.child_regions() {
                    self.budget
                        .push(AllocationClass::Scratch, &mut pending, child)?;
                }
            }
        }
        self.drop_scratch(pending)?;
        self.statement_region(unit, body)?;
        let body = self.plan(unit).regions[body.index()];
        let function = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                rest: false,
                parameters: Vec::new(),
                body,
                arrow: true,
                name: js::FunctionName::Unobserved,
                strict: true,
                length: None,
                suspension: js::Suspension::None,
            },
        )?;
        let callee = self.expression(js::Expr::Function(function))?;
        Ok(js::Expr::Call {
            callee,
            arguments: Vec::new(),
            invocation: Invocation::Value,
        })
    }

    /// A kept class as a JavaScript class value: its name, its base (a host
    /// base by its extern declaration's name, which is the host's; an internal
    /// base by its constructor's binding), its constructor, and for a
    /// published chain its prototype methods.
    fn class_expression(
        &mut self,
        unit: ContextId,
        region: RegionId,
        class: crate::check::NominalId,
        constructor: js::FunctionId,
        span: Span,
    ) -> Result<js::Expr, FormationError> {
        let program = self.program;
        let definition = program
            .class(class)
            .ok_or_else(|| self.error(span, "kept class without a definition"))?;
        let base = match definition.base {
            None => None,
            Some(base) => {
                let base = program
                    .class(base)
                    .ok_or_else(|| self.error(span, "kept class with an undeclared base"))?;
                Some(if base.external {
                    self.host_binding(&base.name)?
                } else {
                    let cell = base
                        .value
                        .ok_or_else(|| self.error(span, "kept class with a dissolved base"))?;
                    self.cell(unit, cell)?
                })
            }
        };
        let mut methods = self
            .budget
            .vector(AllocationClass::Retained, definition.prototype.len())?;
        for &(key, cell) in &definition.prototype {
            self.work(1)?;
            let name = program.strings[key.index()]
                .as_unicode()
                .filter(|name| *name != "constructor")
                .ok_or_else(|| self.error(span, "prototype method name"))?;
            let name = self.text(name)?;
            let method = self.prototype_method(unit, region, cell, span)?;
            self.append(&mut methods, (name, method))?;
        }
        // A root class's constructor that takes nothing and does nothing
        // is JavaScript's implicit one (a derived class's implicit
        // constructor forwards any arguments, which no formed constructor
        // does).
        let formed = &self.module.functions[constructor.index()];
        let implicit = base.is_none()
            && formed.parameters.is_empty()
            && !formed.strict
            && formed.length.is_none()
            && self.module.regions[formed.body.index()]
                .statements
                .is_empty();
        let expression = js::Expr::Class {
            name: definition.name.clone(),
            base,
            constructor: (!implicit).then_some(constructor),
            methods,
        };
        self.class_witness_registration(class, expression)
    }

    /// A published class's prototype method, `name(p…){return m(this,p…)}`:
    /// the static method unit `m` called with the receiver. Its `length`
    /// stops at the first default, as the source method's would.
    fn prototype_method(
        &mut self,
        unit: ContextId,
        region: RegionId,
        cell: CellId,
        span: Span,
    ) -> Result<js::FunctionId, FormationError> {
        let program = self.program;
        let CellBinding::Function(method) = program.cells[cell.index()].binding else {
            return Err(self.error(span, "prototype method without a function"));
        };
        // A body nothing else calls is the method itself.
        if self.prototype_method_form(method)?.is_some() {
            return self.prototype_method_function(unit, method, span);
        }
        let signature = program
            .unit(method)
            .and_then(|data| data.callable_type)
            .and_then(|ty| program.types[ty.index()].callable_signature())
            .cloned()
            .ok_or_else(|| self.error(span, "prototype method signature"))?;
        if !public_structs::adaptable_export(program, cell, self.budget)? {
            return Err(self.error(span, "prototype method value-struct ABI adaptation"));
        }
        let parent = self.plan(unit).regions[region.index()];
        let scope = self.module.regions[parent.index()].scope;
        let body = self.module.region_in(scope, self.budget)?;
        let inner = self.module.regions[body.index()].scope;
        let count = signature.params.len().saturating_sub(1);
        let mut parameters = self.budget.vector(AllocationClass::Retained, count)?;
        let mut arguments = self.budget.vector(AllocationClass::Retained, count + 1)?;
        let this = self.expression(js::Expr::This)?;
        self.append(&mut arguments, this)?;
        for formal in signature.params.iter().skip(1) {
            self.work(1)?;
            let parameter = self.fresh_binding(inner, "argument")?;
            self.append(&mut parameters, parameter)?;
            let value = self.public_parameter(formal, parameter)?;
            self.append(&mut arguments, value)?;
        }
        if signature.has_rest() {
            let last = arguments.last_mut().unwrap();
            *last = self.expression(js::Expr::Spread(*last))?;
        }
        let callee = self.cell(unit, cell)?;
        let call = self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })?;
        let returned = self.public_value(&signature.return_type, call, false)?;
        self.statement(body, js::Statement::Return(Some(returned)))?;
        let length = program
            .unit(method)
            .and_then(|data| data.declared_length)
            .map(|p| p as usize - 1);
        let function = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                rest: signature.has_rest(),
                parameters,
                body,
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length,
                suspension: js::Suspension::None,
            },
        )?;
        Ok(function)
    }

    fn statement_region(
        &mut self,
        unit: ContextId,
        region: RegionId,
    ) -> Result<(), FormationError> {
        let operations = &self.data(unit).regions[region.index()].operations;
        self.statement_operations(unit, region, operations)
    }

    /// A borrowed slice of the checked schedule. Module instantiation and
    /// evaluation use this same statement owner, with no copied operation list
    /// or second lowering path.
    /// The anchor of root statements formed from `operations` (plan M3.3,
    /// design §6), from the demand plan's per-operation effects: a
    /// definition when no operation formed can throw, diverge, re-enter,
    /// suspend or leave, writes anything but the cell it initializes, reads
    /// a cell written after its initialization or state the program does
    /// not own, or loads a module. Anything else is anchored.
    fn root_anchor(
        &mut self,
        unit: ContextId,
        operations: &[OpId],
    ) -> Result<js::Anchor, FormationError> {
        let semantic = self.semantic(unit);
        let data = self.data(unit);
        for &operation in operations {
            self.work(1)?;
            let kind = &data.operations[operation.index()].kind;
            if !matches!(kind, OperationKind::PrepareCall(_))
                && !self.demand.needs_operation(unit, operation)
            {
                continue;
            }
            if matches!(kind, OperationKind::LoadModule { .. }) {
                return Ok(js::Anchor::Anchored);
            }
            let Some(behavior) = self.demand.operation_behavior(semantic, operation) else {
                return Ok(js::Anchor::Anchored);
            };
            if behavior.may_throw
                || behavior.may_diverge
                || behavior.may_reenter
                || behavior.may_suspend
                || behavior.transfers_control
            {
                return Ok(js::Anchor::Anchored);
            }
            let writes = match behavior.writes {
                super::facts::MemoryAccess::None => true,
                super::facts::MemoryAccess::Cell(cell) => {
                    matches!(kind, OperationKind::Initialize(initialized) if *initialized == cell)
                }
                super::facts::MemoryAccess::Unknown => false,
            };
            let reads = match behavior.reads {
                super::facts::MemoryAccess::None => true,
                super::facts::MemoryAccess::Cell(cell) => self
                    .program
                    .cells
                    .get(cell.index())
                    .is_some_and(|read| !read.reassigned),
                super::facts::MemoryAccess::Unknown => false,
            };
            if !writes || !reads {
                return Ok(js::Anchor::Anchored);
            }
        }
        Ok(js::Anchor::Definition)
    }

    fn statement_operations(
        &mut self,
        unit: ContextId,
        region: RegionId,
        operations: &'program [OpId],
    ) -> Result<(), FormationError> {
        let prior = self.module.forming_choices;
        let result = self.statement_operations_inner(unit, region, operations);
        self.module.forming_choices = prior;
        result
    }
    fn statement_operations_inner(
        &mut self,
        unit: ContextId,
        region: RegionId,
        operations: &'program [OpId],
    ) -> Result<(), FormationError> {
        let target_region = self.plan(unit).regions[region.index()];
        // A root statement is formed from the operations since the previous
        // one: its anchor is theirs (plan M3.3, design §6). One operation
        // that is no definition anchors the whole window, so the anchor is a
        // running fold, each operation judged once (not the window again at
        // every operation, which is quadratic in a long window).
        let classify = self.classify_roots && target_region == self.module.root;
        let mut anchored = false;
        let mut formed = self.module.root_rows.len();
        let mut cursor = 0;
        while cursor < operations.len() {
            self.work(1)?;
            if classify {
                if self.module.root_rows.len() != formed {
                    formed = self.module.root_rows.len();
                    anchored = false;
                }
                anchored = anchored
                    || self.root_anchor(unit, &operations[cursor..=cursor])?
                        == js::Anchor::Anchored;
                self.anchor = if anchored {
                    js::Anchor::Anchored
                } else {
                    js::Anchor::Definition
                };
            }
            let operation_id = operations[cursor];
            self.module.forming_choices = self.data(unit).operations[operation_id.index()].authored;
            if classify {
                self.point = self.root_point(unit, operation_id);
            }
            let is_prepare = matches!(
                self.data(unit).operations[operation_id.index()].kind,
                OperationKind::PrepareCall(_)
            );
            if !is_prepare && !self.demand.needs_operation(unit, operation_id) {
                cursor += 1;
                continue;
            }
            if matches!(
                self.demand
                    .helper_operation(self.semantic(unit), operation_id),
                Some(HelperOperation::Elided)
            ) {
                cursor += 1;
                continue;
            }
            self.product_lookup()?;
            if let Some(super::product_family::ProductOperationKind::Initialize { cell, input }) =
                self.demand
                    .product_operation(self.semantic(unit), operation_id)
                    .copied()
            {
                self.initialize_product(unit, target_region, cell, input)?;
                cursor += 1;
                continue;
            }
            if let Some(RecordOperation::Initialize(record)) = self
                .demand
                .record_operation(self.semantic(unit), operation_id)
            {
                self.initialize_record(unit, target_region, record)?;
                cursor += 1;
                continue;
            }
            let operation = &self.data(unit).operations[operations[cursor].index()];
            let operands = self.data(unit).operands(operation.operands).unwrap();
            let statement = match operation.kind {
                OperationKind::Initialize(cell)
                    if self.private_function_cell(unit, operands[0])? == Some(cell) =>
                {
                    // The closure itself declared this cell with its function.
                    cursor += 1;
                    continue;
                }
                OperationKind::Initialize(cell)
                    if matches!(self.program.cells[cell.index()].binding,
                        CellBinding::Function(method)
                            if self.prototype_method_form(method)?.is_some()) =>
                {
                    // A static body only its class reaches is that class's
                    // prototype method; nothing reads the cell.
                    cursor += 1;
                    continue;
                }
                OperationKind::Initialize(cell) => {
                    let value = self.value(unit, operands[0])?;
                    let expected = &self.program.types[self.program.cells[cell.index()].ty.index()];
                    let actual = &self.program.types[self.data(unit).values[operands[0].index()].ty.index()];
                    let value = if matches!(expected, Type::Dynamic | Type::Unknown) { self.absence_erasure(actual, value)? } else { value };
                    let value = self.boundary_cell(cell, value)?;
                    let value = self.carrier_value(unit, cell, value)?;
                    js::Statement::Let {
                        binding: self.cell_binding(unit, cell)?,
                        value: Some(value),
                    }
                }
                // `let x;` (R3), in an inlined body as in its own: the context
                // binds the cell as `Initialize`'s `let` does.
                OperationKind::Declare(cell) => {
                    let value = if self.payload_cell(cell)? {
                        let absent = self.literal(js::Literal::Undefined)?;
                        Some(self.carrier_value(unit, cell, absent)?)
                    } else { None };
                    js::Statement::Let { binding: self.cell_binding(unit, cell)?, value }
                },
                OperationKind::Return => {
                    let returned = if let Some(&source) = operands.first() {
                        let value = self.value(unit, source)?;
                        let data = self.data(unit);
                        let expected = data.callable_type.and_then(|id| self.program.types[id.index()].callable_signature())
                            .map(|s| s.return_type.as_ref());
                        let actual = &self.program.types[data.values[source.index()].ty.index()];
                        Some(if expected.is_some_and(|ty| matches!(ty, Type::Dynamic | Type::Unknown)) {
                            self.absence_erasure(actual, value)?
                        } else { value })
                    } else { None };
                    js::Statement::Return(returned)
                },
                OperationKind::Throw => js::Statement::Throw(self.value(unit, operands[0])?),
                // `super(...)`, then the instance parameter names `this`.
                OperationKind::SuperConstruct => {
                    let base = self
                        .data(unit)
                        .constructor_of
                        .and_then(|class| self.program.class(class))
                        .and_then(|class| class.base)
                        .and_then(|base| self.program.class(base));
                    let signature = base.and_then(|base| {
                        base.constructor
                            .or_else(|| base.value.map(|cell| self.program.cells[cell.index()].ty))
                    }).and_then(|ty| self.program.types[ty.index()].callable_signature());
                    let receiver = usize::from(base.is_some_and(|base| !base.external));
                    let mut arguments = self
                        .budget
                        .vector(AllocationClass::Retained, operands.len())?;
                    for (index, &argument) in operands.iter().enumerate() {
                        let parameter = signature.and_then(|signature| signature.params.get(index + receiver));
                        let argument = self.public_constructor_argument(unit, argument, parameter)?;
                        self.append(&mut arguments, argument)?;
                    }
                    if let Some(signature) = signature {
                        if signature.has_rest()
                            && arguments.len() + receiver == signature.params.len()
                        {
                            let last = arguments.last_mut().unwrap();
                            *last = self.expression(js::Expr::Spread(*last))?;
                        }
                    }
                    let call = self.expression(js::Expr::SuperCall { arguments })?;
                    let instance = self.data(unit).parameters[0];
                    if self.this_cells.contains(&instance) {
                        // Every read of the instance is `this`.
                        js::Statement::Evaluate(call)
                    } else {
                        self.statement(target_region, js::Statement::Evaluate(call))?;
                        let this = self.expression(js::Expr::This)?;
                        js::Statement::Let {
                            binding: self.cell_binding(unit, instance)?,
                            value: Some(this),
                        }
                    }
                }
                OperationKind::Yield { delegate } => {
                    let value = self.value(unit, operands[0])?;
                    js::Statement::Evaluate(self.expression(js::Expr::Yield { value, delegate })?)
                }
                OperationKind::Break => js::Statement::Break,
                OperationKind::Continue => js::Statement::Continue,
                OperationKind::Block(body) => {
                    self.statement_region(unit, body)?;
                    if self.data(unit).parameter_region == Some(body) {
                        if let Some(entry) = self.plan(unit).generator_entry {
                            self.statement(entry, js::Statement::Block(self.plan(unit).regions[body.index()]))?;
                            cursor += 1;
                            continue;
                        }
                    }
                    js::Statement::Block(self.plan(unit).regions[body.index()])
                }
                OperationKind::If { yes, no } => {
                    let condition = self.value(unit, operands[0])?;
                    self.statement_region(unit, yes)?;
                    if let Some(no) = no {
                        self.statement_region(unit, no)?;
                    }
                    js::Statement::If {
                        condition,
                        yes: self.plan(unit).regions[yes.index()],
                        no: no.map(|no| self.plan(unit).regions[no.index()]),
                    }
                }
                OperationKind::Loop { test, body, update } => {
                    let condition = self.expression_region(unit, test)?;
                    let update = self.expression_region(unit, update)?;
                    self.statement_region(unit, body)?;
                    js::Statement::Loop {
                        condition,
                        update,
                        body: self.plan(unit).regions[body.index()],
                    }
                }
                OperationKind::ForIn { key, body } => {
                    let object = self.value(unit, operands[0])?;
                    if self.addressed_cell(unit, key)? {
                        return Err(self.error(operation.span, "address-taken for-in key"));
                    }
                    let binding = self.cell_binding(unit, key)?;
                    self.statement_region(unit, body)?;
                    js::Statement::ForIn {
                        binding,
                        object,
                        body: self.plan(unit).regions[body.index()],
                    }
                }
                OperationKind::ForOf { item, body } => {
                    let iterable = self.value(unit, operands[0])?;
                    if self.addressed_cell(unit, item)? {
                        return Err(self.error(operation.span, "address-taken for-of item"));
                    }
                    let binding = self.cell_binding(unit, item)?;
                    self.statement_region(unit, body)?;
                    js::Statement::ForOf {
                        binding,
                        iterable,
                        body: self.plan(unit).regions[body.index()],
                    }
                }
                OperationKind::Try {
                    body,
                    catch,
                    finally,
                } => {
                    self.statement_region(unit, body)?;
                    let catch = if let Some((binding, region)) = catch {
                        self.statement_region(unit, region)?;
                        let body = self.plan(unit).regions[region.index()];
                        let binding = match binding {
                            Some(cell) => {
                                let binding = self.cell_binding(unit, cell)?;
                                if self.addressed_cell(unit, cell)? {
                                    // A catch parameter receives a raw host value;
                                    // establish its carrier at catch entry, before
                                    // the original body can expose the location.
                                    let value = self.reference(binding)?;
                                    let value = self.carrier_value(unit, cell, value)?;
                                    let target = self.reference(binding)?;
                                    let boxed =
                                        self.expression(js::Expr::Assign { target, value })?;
                                    self.work(self.module.regions[body.index()].statements.len())?;
                                    let statements =
                                        &mut self.module.regions[body.index()].statements;
                                    self.budget.reserve_vec(
                                        AllocationClass::Retained,
                                        statements,
                                        1,
                                    )?;
                                    statements.push(js::Statement::Evaluate(boxed));
                                    statements.rotate_right(1);
                                    Some(binding)
                                } else if self
                                    .contract
                                    .ecmascript
                                    .allows(JsSyntaxFeature::OptionalCatchBinding)
                                    && !self.module.mentions(&[body], &[], binding, false)
                                {
                                    // `catch{…}`: nothing reads the exception.
                                    None
                                } else {
                                    Some(binding)
                                }
                            }
                            None if !self
                                .contract
                                .ecmascript
                                .allows(JsSyntaxFeature::OptionalCatchBinding) =>
                            {
                                Some({
                                    let binding = js::Binding {
                                        source_symbol: None,
                                        scope: self.module.regions[body.index()].scope,
                                        spelling: self.format(format_args!(
                                            "caught_{}_{}",
                                            unit.index(),
                                            region.index()
                                        ))?,
                                        pinned: false,
                                        class: None,
                                        defined: false,
                                    };
                                    self.module.binding_in(binding, self.budget)?
                                })
                            }
                            None => None,
                        };
                        Some(js::Catch { binding, body })
                    } else {
                        None
                    };
                    if let Some(finally) = finally {
                        self.statement_region(unit, finally)?;
                    }
                    js::Statement::Try {
                        body: self.plan(unit).regions[body.index()],
                        catch,
                        finally: finally.map(|region| self.plan(unit).regions[region.index()]),
                    }
                }
                _ => {
                    let start = cursor;
                    let first_row = self.module.root_rows.len();
                    let expression = self.scheduled_expression(unit, &operations, &mut cursor)?;
                    if classify {
                        // A prepared call consumes its argument schedule and
                        // invocation together. Those operations do not pass
                        // through this loop again: fold their effects into the
                        // same window, including calls deferred into the next
                        // declaration's initializer.
                        anchored |= self.root_anchor(unit, &operations[start + 1..cursor])?
                            == js::Anchor::Anchored;
                        self.anchor = if anchored { js::Anchor::Anchored } else { js::Anchor::Definition };
                        if anchored {
                            for row in &mut self.module.root_rows[first_row..] {
                                if row.origin == js::RowOrigin::Source {
                                    row.anchor = js::Anchor::Anchored;
                                }
                            }
                        }
                    }
                    if let Some(expression) = expression {
                        // The statement completes with the last operation
                        // it took.
                        if classify {
                            self.point = self.root_point(unit, operations[cursor - 1]);
                        }
                        self.statement(target_region, js::Statement::Evaluate(expression))?;
                    }
                    continue;
                }
            };
            let initialized = match operation.kind { OperationKind::Initialize(cell) => Some(cell), _ => None };
            cursor += 1;
            self.statement(target_region, statement)?;
            if let Some(cell) = initialized {
                if target_region == self.module.root {
                    self.publish_const_graph(unit, cell)?;
                }
            }
        }
        if classify {
            self.point = None;
        }
        Ok(())
    }

    /// The program's root point of `operation` of `unit`'s evaluation.
    fn root_point(&self, unit: ContextId, operation: OpId) -> Option<u32> {
        self.demand
            .initialization()
            .root_point(self.semantic(unit), operation)
            .map(|point| point.ordinal())
    }
}

fn binary(op: BinaryOp) -> js::Binary {
    match op {
        BinaryOp::Add => js::Binary::Add,
        BinaryOp::Sub => js::Binary::Subtract,
        BinaryOp::Mul => js::Binary::Multiply,
        BinaryOp::Div => js::Binary::Divide,
        BinaryOp::Mod => js::Binary::Remainder,
        BinaryOp::BitAnd => js::Binary::BitAnd,
        BinaryOp::BitOr => js::Binary::BitOr,
        BinaryOp::Xor => js::Binary::BitXor,
        BinaryOp::ShiftLeft => js::Binary::ShiftLeft,
        BinaryOp::ShiftRight => js::Binary::ShiftRight,
        BinaryOp::UnsignedShiftRight => js::Binary::UnsignedShiftRight,
        BinaryOp::Eq => js::Binary::StrictEqual,
        BinaryOp::NotEq => js::Binary::StrictNotEqual,
        BinaryOp::Less => js::Binary::Less,
        BinaryOp::LessEq => js::Binary::LessEqual,
        BinaryOp::Greater => js::Binary::Greater,
        BinaryOp::GreaterEq => js::Binary::GreaterEqual,
        BinaryOp::And => js::Binary::And,
        BinaryOp::Or => js::Binary::Or,
        BinaryOp::Nullish => js::Binary::Nullish,
    }
}
