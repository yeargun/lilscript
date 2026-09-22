//! Formation from checked semantic units and selected physical recipes.
//!
//! Immutable values acquire storage only when their effective uses or evaluation
//! schedule require it. Source cells retain their lexical identity. The bounded
//! placement plan precedes binding allocation; numeric facts describe raw target
//! results separately from source integer normalization.
//! Prepared calls own argument schedules so a member getter runs before argument
//! effects without inventing a first-class JavaScript reference or a `.call`
//! lookup. Unsupported language/delivery contracts fail before printing.

use super::demand::{
    ContextId, ContextKind, DemandError, DemandMode, DemandPlan, HelperOperation, RecordOperation,
};
use super::implementations::ImplementationMap;
use super::javascript_resource::ResourceView;
use super::record_family::RecordFamily;
use super::string_family::StringChoice;
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::value_placement::{self, PlacementDepth, ValueStorage};
use super::*;
use crate::codegen_ir_js::FunctionSpelling;
use crate::compilation_contract::{
    JavaScriptAbiContract, JavaScriptCompilationContract, JavaScriptEffectPolicy,
    JavaScriptUnsafeAssumptions, JavaScriptWorld,
};
use crate::compilation_policy::{BudgetError, BudgetLedger, WorkDomain, WorkKind};
use crate::js_syntax_target::{EcmaScriptEdition, JsSyntaxFeature};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::primitive::{Intrinsic, ResolvedIntrinsic};
use crate::scalar_transfer::NumberFacts;
use crate::structured_js as js;

#[path = "javascript_resource_formation.rs"]
mod resource_formation;

#[path = "javascript_product_calls.rs"]
mod product_calls;
#[path = "javascript_products.rs"]
mod products;
#[path = "javascript_references.rs"]
mod references;
#[path = "javascript_struct_boundaries.rs"]
mod struct_boundaries;
#[path = "javascript_structs.rs"]
mod structs;
#[path = "javascript_public_structs.rs"]
mod public_structs;
#[path = "javascript_host.rs"]
mod host;
#[path = "javascript_int32.rs"]
mod int32;

/// The operation's selected result recipe, shared by formation and domain
/// evidence. The source signature alone never supplies a runtime domain.
#[derive(Clone, Copy)]
enum CallResultRecipe {
    Raw,
    Void,
    NormalizeInteger,
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
    let integer = operation.result.is_some_and(|result| {
        matches!(
            program.types[unit.values[result.index()].ty.index()],
            Type::Int
        )
    });
    match unit.calls[call.index()].target {
        CallTarget::Builtin(BuiltinCall::Print) => CallResultRecipe::Void,
        CallTarget::Reference { .. } | CallTarget::Builtin(_) if integer => {
            CallResultRecipe::NormalizeInteger
        }
        CallTarget::Intrinsic {
            operation: ResolvedIntrinsic::Method(method),
            ..
        } if js::integer_intrinsic(method) => CallResultRecipe::IntrinsicInteger,
        _ => CallResultRecipe::Raw,
    }
}

#[derive(Clone, Copy)]
enum LoadResultRecipe {
    Raw,
    NullishNull,
    NullishEmptyString,
    NormalizeInteger,
}
fn load_result_recipe(
    program: &Program<'_>,
    unit: &UnitData,
    place: PlaceId,
    ty: TypeId,
) -> LoadResultRecipe {
    let receiver = match unit.places[place.index()] {
        Place::Cell(_) | Place::Value(_) => return LoadResultRecipe::Raw,
        Place::Field { .. } => {
            // A private layout is not evidence for a primitive host payload.
            // Preserve the nominal integer-load contract until producer facts
            // establish that this normalization is redundant.
            return if matches!(program.types[ty.index()], Type::Int) {
                LoadResultRecipe::NormalizeInteger
            } else {
                LoadResultRecipe::Raw
            };
        }
        Place::Member { receiver, .. } | Place::Index { receiver, .. } => receiver,
    };
    let receiver_ty = &program.types[unit.values[receiver.index()].ty.index()];
    let result_ty = &program.types[ty.index()];
    if matches!(receiver_ty, Type::Record(_)) {
        LoadResultRecipe::NullishNull
    } else if matches!(unit.places[place.index()], Place::Index { .. })
        && matches!(receiver_ty, Type::Array(_))
        && matches!(result_ty, Type::Nullable(_) | Type::Null)
    {
        // A position past the end is `undefined`; the language reads null.
        LoadResultRecipe::NullishNull
    } else if matches!(unit.places[place.index()], Place::Index { .. })
        && matches!(result_ty, Type::String)
    {
        LoadResultRecipe::NullishEmptyString
    } else if matches!(result_ty, Type::Int) {
        LoadResultRecipe::NormalizeInteger
    } else {
        LoadResultRecipe::Raw
    }
}

pub(super) struct JavaScriptRecipes;
impl super::raw_domains::Recipes for JavaScriptRecipes {
    fn result(
        &self,
        program: &Program<'_>,
        unit: UnitId,
        operation: OpId,
    ) -> super::raw_domains::ResultRecipe {
        use super::raw_domains::ResultRecipe;
        let data = program.unit(unit).unwrap();
        let operation = &data.operations[operation.index()];
        match operation.kind {
            OperationKind::Call(_) => match call_result_recipe(program, data, operation) {
                CallResultRecipe::Void => ResultRecipe::Undefined,
                CallResultRecipe::NormalizeInteger | CallResultRecipe::IntrinsicInteger => {
                    ResultRecipe::NormalizedI32
                }
                CallResultRecipe::Raw => ResultRecipe::Source,
            },
            OperationKind::Load(place)
                if operation.result.is_some_and(|value| {
                    matches!(
                        load_result_recipe(program, data, place, data.values[value.index()].ty),
                        LoadResultRecipe::NormalizeInteger
                    )
                }) =>
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
            public_aggregate_abi: crate::config::PublicAggregateAbi::Named,
            preserve_extern_fields: true,
            internal_export_bindings_may_mangle: true,
            public_function_spelling: None,
        },
        assumptions: JavaScriptUnsafeAssumptions {
            pristine_builtins: false,
            pure_property_reads: false,
            numeric_lengths: false,
        },
        effects: JavaScriptEffectPolicy {
            strip_console: false,
        },
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
    let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).map_err(|error| {
        match error {
            super::uses::UseError::InvalidProgram(feature) => unsupported(feature),
            _ => unsupported("JavaScript inspection use index"),
        }
    })?;
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
) -> Result<(js::Module, Vec<js::LiteralAlternative>), FormationError> {
    lower_resource_admitted(
        program,
        uses,
        implementations,
        contract,
        mode,
        compact,
        ResourceView::Whole,
        budget,
    )
}

pub(super) fn lower_resource_admitted(
    program: &Program<'_>,
    uses: &UseIndex,
    implementations: &ImplementationMap,
    contract: &JavaScriptCompilationContract,
    mode: DemandMode,
    compact: bool,
    resource: ResourceView<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(js::Module, Vec<js::LiteralAlternative>), FormationError> {
    let mut phase = budget.scope();
    let demand = phase.with_ledger(|ledger| {
        DemandPlan::build_resource(
            program,
            Some(uses),
            Some(implementations),
            contract,
            mode,
            resource,
            ledger,
        )
    })?;
    let result = form_with_demand(program, Some(uses), contract, &demand, compact, &mut phase);
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
        false,
        &mut AllocationBudget::new(None),
    );
    demand
        .discard(budget.map(|(ledger, _)| ledger))
        .map_err(FormationError::Budget)?;
    result.map(|(module, literals)| {
        debug_assert!(
            literals.is_empty(),
            "inspection formation keeps exact literals"
        );
        module
    })
}

fn form_with_demand(
    program: &Program<'_>,
    uses: Option<&UseIndex>,
    contract: &JavaScriptCompilationContract,
    demand: &DemandPlan<'_, '_>,
    compact: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<(js::Module, Vec<js::LiteralAlternative>), FormationError> {
    let _timing = crate::timing::JS_FORMATION.scope(0);
    let mut phase = budget.scope();
    let struct_plan = structs::plan(program, contract, &mut phase)?;
    let reference_plan = references::Plan::new();
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
    module.pristine_builtins = contract.assumptions.pristine_builtins;
    let mut formation = Formation {
        program,
        uses,
        contract: *contract,
        demand,
        compact,
        module,
        literal_alternatives: Vec::new(),
        contexts,
        entry_depths,
        records,
        struct_plan,
        reference_plan,
        imported_binding: None,
        host_factories: Vec::new(),
        unit_functions: Vec::new(),
        foreign_bindings: Vec::new(),
        stable_cells: Vec::new(),
        arguments_read: None,
        int32_cells: Vec::new(),
        current_module: 0,
        budget: &mut phase,
    };
    let result = (|| {
        formation.plan_resource_import()?;
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
        for &context in demand.roots() {
            formation.work(1)?;
            let data = formation.data(context);
            formation.current_module = data.module.index() as u32;
            let prefix = data.instantiation_prefix as usize;
            formation.statement_operations(
                context,
                data.entry,
                &data.regions[data.entry.index()].operations[prefix..],
            )?;
        }
        for &context in demand.roots() {
            formation.work(1)?;
            formation.finish_unit(context)?;
        }
        formation.finish_reference_prefix()?;
        let context = demand.root();
        if let Some(export) = demand.resource().producer_export() {
            formation.work(1)?;
            let binding = formation.cell_binding(context, export.cell())?;
            let name = formation.text(export.export_name())?;
            formation.budget.push(
                AllocationClass::Retained,
                &mut formation.module.exports,
                js::Export { binding, name },
            )?;
        } else if contract.abi.preserve_root_exports {
            formation.work(program.exports().len())?;
            for (export_name, cell) in program.value_exports() {
                formation.work(1)?;
                if formation.addressed_cell(context, cell)? {
                    return Err(formation.error(
                        program.cells[cell.index()].declaration,
                        "public address-taken cell ABI adaptation",
                    ));
                }
                // JavaScript's `length` stops at the first default. The body
                // applies each default, so the printed parameters from that one
                // on only need default syntax: `p=void 0`.
                if let (Type::Function(signature), CellBinding::Function(unit)) = (
                    &program.types[program.cells[cell.index()].ty.index()],
                    program.cells[cell.index()].binding,
                ) {
                    formation.work(signature.params.len())?;
                    // A `JS.undefined()` default needs no syntax: absence is it.
                    let first = signature.params.iter().position(|parameter| {
                        parameter.default.as_ref().is_some_and(|default| {
                            !matches!(default, crate::semantic::DefaultValue::Undefined)
                        })
                    });
                    if let Some(first) = first {
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
                if !formation.struct_plan.boundary_types.is_empty()
                    && formation.struct_plan.boundary_types
                        [program.cells[cell.index()].ty.index()]
                {
                    binding = formation.public_struct_export(cell, binding)?;
                }
                let name = formation.text(export_name)?;
                formation.budget.push(
                    AllocationClass::Retained,
                    &mut formation.module.exports,
                    js::Export { binding, name },
                )?;
            }
        }
        Ok::<_, FormationError>(())
    })();
    if let Err(error) = result {
        drop(formation);
        return Err(error);
    }
    if formation.module.root_modules.len()
        != formation.module.regions[formation.module.root.index()].statements.len()
    {
        let error = formation.error(Span::default(), "root statement without its source module");
        drop(formation);
        return Err(error);
    }
    // One-use forwarding: a checked target edit on the finished tree, part
    // of target compaction.
    if formation.compact {
        let pristine = formation.contract.assumptions.pristine_builtins;
        let edited = formation
            .module
            .forward_single_uses(formation.budget)
            .and_then(|_| {
                if pristine {
                    formation.module.fold_object_stores(formation.budget)?;
                }
                formation.module.elide_undefined(formation.budget)?;
                formation.module.drop_unreferenced_functions(formation.budget)
            });
        if let Err(error) = edited {
            drop(formation);
            return Err(error.into());
        }
    }
    let Formation {
        module,
        literal_alternatives,
        contexts,
        entry_depths,
        records,
        struct_plan,
        reference_plan,
        ..
    } = formation;
    drop(contexts);
    drop(entry_depths);
    drop(records);
    drop(struct_plan);
    drop(reference_plan);
    phase.finish_retained()?;
    Ok((module, literal_alternatives))
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
    /// Printed strict (a classic script's struct-bearing frame).
    strict_frame: bool,
    // At most one physical capture per ambient binding and owning activation.
    // Created on demand during formation, with no extra source graph scan.
    ambient_captures: [Option<js::BindingId>; 2],
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
    uses: Option<&'program UseIndex>,
    contract: JavaScriptCompilationContract,
    demand: &'demand DemandPlan<'program, 'src>,
    compact: bool,
    module: js::Module,
    // Formation owns these target occurrences with the Module. Failed growth
    // drops both before its phase releases their allocation admission.
    literal_alternatives: Vec<js::LiteralAlternative>,
    contexts: Vec<Option<FormationContext>>,
    // Expression insertion depths, not lexical scope depths: inline bodies
    // keep their caller's storage scope but execute inside its call schedule.
    entry_depths: Vec<usize>,
    records: Vec<RecordStorage<'program>>,
    // Target-boundary classification, allocated only for programs with structs.
    struct_plan: structs::Plan,
    reference_plan: references::Plan,
    // Only a resource consumer owns this physical endpoint. It is not a
    // second semantic cell or alias table; the sealed view owns its CellId.
    imported_binding: Option<js::BindingId>,
    /// One hoisted `JS.methodN` adapter factory per calling convention.
    host_factories: Vec<(u8, js::BindingId)>,
    /// Each formed function unit's target function, for export reflection.
    unit_functions: Vec<(UnitId, js::FunctionId)>,
    /// One ES import binding per foreign cell with an `import extern` source.
    foreign_bindings: Vec<(CellId, js::BindingId)>,
    /// Per cell: 0 unknown, 1 nothing writes it after initialization, 2 written.
    stable_cells: Vec<u8>,
    /// Whether any unit reads `arguments`, computed on first need.
    arguments_read: Option<bool>,
    /// Per cell, whether every write is an int32 Number; built on first need.
    int32_cells: Vec<u8>,
    /// The source module whose root statements are being formed.
    current_module: u32,
}

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    fn error(&self, span: Span, feature: &'static str) -> FormationError {
        FormationError::Unsupported(Unsupported { span, feature })
    }

    /// Spans are local to their module. Until refusals render a source
    /// diagnostic (011), `LILSCRIPT_DEBUG_VERIFY` names the refused unit.
    fn debug_refusal(&self, unit: UnitId, error: FormationError) -> FormationError {
        if std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some() {
            let data = self.program.units[unit.index()].data();
            eprintln!(
                "formation refusal in module {} unit {:?}: {error:?}",
                data.module.index(),
                data.function_name.map(|name| &self.program.strings[name.index()]),
            );
        }
        error
    }

    /// The function cell a named function's closure initializes, when that
    /// function's `name` cannot be observed: nothing exports the cell and
    /// every read of it only calls the function. Such a function needs no
    /// exact-name carrier; its cell holds it directly.
    /// A load that can be read again at each use: a total read (demand keeps
    /// no evaluation at its site) of a local cell nothing writes after its
    /// initialization. Its uses stay inside the load's own region, where the
    /// binding is in scope, and see the same value there.
    fn rematerialized_load(
        &mut self,
        context: ContextId,
        value: ValueId,
    ) -> Result<bool, FormationError> {
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        if self.demand.context(context).kind.is_inline() {
            return Ok(false);
        }
        let semantic = self.semantic(context);
        let data = self.data(context);
        let definition = data.values[value.index()].definition;
        let OperationKind::Load(place) = data.operations[definition.index()].kind else {
            return Ok(false);
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return Ok(false);
        };
        let program = self.program;
        // A parameter read never throws. Demand keeps a classic script's
        // parameter reads in place because mapped `arguments` can alias them;
        // with no `arguments` read anywhere, nothing but a store can change one.
        if matches!(program.cells[cell.index()].binding, CellBinding::Parameter(_)) {
            if self.contract.execution == crate::compilation_contract::JavaScriptExecution::Script
                && self.reads_arguments()?
            {
                return Ok(false);
            }
        } else if self.demand.needs_execution(context, definition) {
            return Ok(false);
        }
        if program.cells[cell.index()].binding == CellBinding::Foreign
            || references::is_reference(program, cell)
        {
            return Ok(false);
        }
        self.product_lookup()?;
        if self.demand.product_for_cell(cell).is_some() || self.addressed_cell(context, cell)? {
            return Ok(false);
        }
        let Some(readers) = uses.unit(semantic).and_then(|uses| uses.value_uses(value)) else {
            return Ok(false);
        };
        self.work(readers.len())?;
        if readers
            .iter()
            .any(|reader| matches!(reader, ValueUse::RegionResult(_)))
        {
            return Ok(false);
        }
        if self.stable_cells.is_empty() {
            self.stable_cells =
                self.budget
                    .filled(AllocationClass::Scratch, program.cells.len(), 0u8)?;
        }
        if self.stable_cells[cell.index()] == 0 {
            let Some(users) = uses.cell(cell) else {
                return Ok(false);
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
        Ok(self.stable_cells[cell.index()] == 1)
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
        if self.program.unit(child).unwrap().kind != UnitKind::Function
            || self.struct_plan.wrapped(child)
        {
            return Ok(None);
        }
        let Some(&[ValueUse::Operand {
            operation: initialize,
            position: 0,
        }]) = uses.unit(semantic).and_then(|uses| uses.value_uses(value))
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
    fn reference(&mut self, binding: js::BindingId) -> Result<js::ExprId, FormationError> {
        self.expression(js::Expr::Binding(binding))
    }
    fn statement(
        &mut self,
        region: js::RegionId,
        statement: js::Statement,
    ) -> Result<(), FormationError> {
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.regions[region.index()].statements,
            statement,
        )?;
        if region == self.module.root {
            let module = self.current_module;
            self.budget
                .push(AllocationClass::Retained, &mut self.module.root_modules, module)?;
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
            Ambient::Arguments => js::Expr::Host(self.text("arguments")?),
        })
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

    fn stripped_log_call(&self, unit: UnitId, call: CallId) -> bool {
        self.contract.effects.strip_console
            && match self.program.units[unit.index()].data().calls[call.index()].target {
                CallTarget::Builtin(BuiltinCall::Print) => true,
                CallTarget::Value {
                    callee,
                    invocation: Invocation::Value,
                } => self.debug_log_value(unit, callee),
                _ => false,
            }
    }

    fn elided_log_lookup(&mut self, unit: UnitId, value: ValueId) -> Result<bool, FormationError> {
        if !self.contract.effects.strip_console || !self.debug_log_value(unit, value) {
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
            if !matches!(usage, ValueUse::CallCallee { call, .. } if self.stripped_log_call(unit, *call))
            {
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
        let strict_frame = self
            .validate_struct_context(context)
            .map_err(|error| self.debug_refusal(unit, error))?;
        self.validate_reference_context(context)
            .map_err(|error| self.debug_refusal(unit, error))?;
        let data = self.program.units[unit.index()].data();
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
        let mut regions = self
            .budget
            .filled(AllocationClass::Scratch, data.regions.len(), body)?;
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
                            OperationKind::Closure(_) => value_placement::DEFERRED_CLOSURE_ENTRY,
                            OperationKind::Call(_) => 2, // Assign→inline schedule Sequence→expression.
                            _ => {
                                return Err(self
                                    .error(Span::default(), "unsupported physical context owner"))
                            }
                        };
                        self.entry_depths[child.index()] = site_depth.saturating_add(layers);
                    }
                }
                for child in kind.child_regions() {
                    self.work(1)?;
                    regions[child.index()] = if expression_regions[child.index()] {
                        regions[parent.index()]
                    } else {
                        self.module.region_in(
                            self.module.regions[regions[parent.index()].index()].scope,
                            self.budget,
                        )?
                    };
                    let layers = match kind {
                        // Captured owner + lazy operator + optional arm Sequence.
                        OperationKind::Select { .. } => 3,
                        OperationKind::ShortCircuit {
                            kind: ShortCircuit::Nullish,
                            ..
                        } => 4,
                        OperationKind::ShortCircuit { .. } => 3,
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
                let region = regions[cell.region.index()];
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
                        source_symbol: Some(cell.source_symbol),
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
                    };
                    self.module.binding_in(binding, self.budget)?
                };
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
            if self.compact && self.rematerialized_load(context, value)? {
                *slot = ValueStorage::Rematerialized;
                continue;
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
            value_placement::plan(
                data,
                self.demand,
                context,
                &mut values,
                PlacementDepth {
                    enclosing,
                    limit: js::MAX_NESTING,
                },
                self.budget,
            )?;
            // Only a deferred closure needs its consumer tree's allowance.
            for (index, operation) in data.operations.iter().enumerate() {
                self.work(1)?;
                let (OperationKind::Closure(_), Some(result)) = (&operation.kind, operation.result)
                else {
                    continue;
                };
                let Some(child) = self
                    .demand
                    .child(context, OpId::from_index(index).unwrap())
                else {
                    continue;
                };
                if !matches!(values[result.index()], ValueStorage::Deferred(_)) {
                    let depth = &mut self.entry_depths[child.index()];
                    *depth = depth
                        .saturating_sub(value_placement::DEFERRED_CLOSURE_ENTRY)
                        .saturating_add(value_placement::CAPTURED_CLOSURE_ENTRY);
                }
            }
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
        let numbers = if self.compact {
            let mut numbers = self.budget.filled(
                AllocationClass::Scratch,
                data.values.len(),
                NumberFacts::UNKNOWN,
            )?;
            self.counter_facts(context, &mut numbers)?;
            numbers
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
                    let literal = js::Literal::String(
                        self.string(self.demand.strings()[family].payload(self.program))?,
                    );
                    self.literal(literal)
                }?)
            };
            self.statement(body, js::Statement::Let { binding, value })?;
            self.budget.push(
                AllocationClass::Scratch,
                &mut shared_strings,
                (family, binding),
            )?;
        }
        let lexical_owner = if ambient::inherits(self.program.units[unit.index()].data().kind) {
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
                strict_frame,
                ambient_captures: [None; 2],
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
        let data = self.data(unit);
        match data.operations[data.values[value.index()].definition.index()].kind {
            OperationKind::Constant(Constant::Integer(value)) => {
                NumberFacts::literal(f64::from(value))
            }
            OperationKind::Constant(Constant::Number(bits)) => {
                NumberFacts::literal(f64::from_bits(bits))
            }
            _ => self.plan(unit).numbers[value.index()],
        }
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
        if !self.compact {
            return Ok(NumberFacts::UNKNOWN);
        }
        self.budget.work(WorkKind::Analysis, 1)?;
        let raw = transfer(self);
        if let Some(result) = operation.result {
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
            if matches!(program.types[id.index()], Type::Function(_)) {
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
                    return Err(self.error(operation.span, "non-finite semantic literal"));
                }
                js::Literal::Number(value)
            }
            OperationKind::Constant(Constant::String(id)) => {
                let text = self.string(&self.program.strings[id.index()])?;
                let expression = self.literal(js::Literal::String(text))?;
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
                        self.budget.push(
                            AllocationClass::Retained,
                            &mut self.literal_alternatives,
                            js::LiteralAlternative::new(expression, weak),
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
                    ValueStorage::Rematerialized => {
                        let OperationKind::Load(place) = operation.kind else {
                            return Err(self.error(operation.span, "rematerialized non-load"));
                        };
                        let Place::Cell(cell) = self.data(unit).places[place.index()] else {
                            return Err(self.error(operation.span, "rematerialized non-cell load"));
                        };
                        self.cell(unit, cell)
                    }
                    _ => Err(self.error(
                        operation.span,
                        "semantic value has no selected JavaScript storage",
                    )),
                };
            }
        };
        Ok(self.literal(literal)?)
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
        if owner == unit
            || self.contract.abi.public_function_spelling != Some(FunctionSpelling::Function)
        {
            return Ok(self.ambient_expression(ambient)?);
        }
        // An ordinary spelling of a source closure must still use its lexical
        // activation. Reading an ordinary function's own immutable arguments
        // binding or this on entry is safe; module arguments is a potentially
        // missing, effectful global lookup and must remain at the source read.
        if ambient == Ambient::Arguments && self.data(owner).kind == UnitKind::ModuleInitialization
        {
            return Err(self.error(span, "ordinary closure with module-lexical arguments"));
        }
        if let Some(binding) = self.plan(owner).ambient_captures[ambient as usize] {
            return Ok(self.reference(binding)?);
        }
        let body = self.plan(owner).regions[self.data(owner).entry.index()];
        let binding = {
            let binding = js::Binding {
                source_symbol: None,
                scope: self.module.regions[body.index()].scope,
                spelling: self.format(format_args!(
                    "activation_{}_{}",
                    owner.index(),
                    ambient as usize
                ))?,
                pinned: false,
            };
            self.module.binding_in(binding, self.budget)?
        };
        self.contexts[owner.index()]
            .as_mut()
            .unwrap()
            .plan
            .ambient_captures[ambient as usize] = Some(binding);
        Ok(self.reference(binding)?)
    }

    fn finish_unit(&mut self, unit: ContextId) -> Result<(), FormationError> {
        self.work(1)?;
        let captures = self.plan(unit).ambient_captures;
        let body = self.plan(unit).regions[self.data(unit).entry.index()];
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
            self.prepend_root_owners(body, count, module)?;
        }
        self.drop_scratch(reference_prefix)?;
        let mut prefix = [None, None];
        let mut count = 0;
        for ambient in [Ambient::This, Ambient::Arguments] {
            if let Some(binding) = captures[ambient as usize] {
                prefix[count] = Some(js::Statement::Let {
                    binding,
                    value: Some(self.ambient_expression(ambient)?),
                });
                count += 1;
            }
        }
        if count != 0 {
            self.work(self.module.regions[body.index()].statements.len())?;
            let statements = &mut self.module.regions[body.index()].statements;
            self.budget
                .reserve_vec(AllocationClass::Retained, statements, count)?;
            // Existing statements move once; no splice iterator buffer or new
            // source scan. Prefix contains at most two admitted expressions.
            for statement in prefix.into_iter().flatten() {
                statements.push(statement);
            }
            statements.rotate_right(count);
            let module = self.data(unit).module.index() as u32;
            self.prepend_root_owners(body, count, module)?;
        }
        Ok(())
    }

    /// Keep `root_modules` aligned when statements are prepended to a region
    /// that is the artifact root.
    pub(super) fn prepend_root_owners(
        &mut self,
        region: js::RegionId,
        count: usize,
        module: u32,
    ) -> Result<(), FormationError> {
        if region != self.module.root {
            return Ok(());
        }
        self.work(self.module.root_modules.len())?;
        let owners = &mut self.module.root_modules;
        self.budget
            .reserve_vec(AllocationClass::Retained, owners, count)?;
        owners.extend(std::iter::repeat(module).take(count));
        owners.rotate_right(count);
        Ok(())
    }

    fn cell_binding(
        &mut self,
        context: ContextId,
        cell: CellId,
    ) -> Result<js::BindingId, FormationError> {
        if self.demand.resource().imported(cell) {
            self.work(1)?;
            return self
                .imported_binding
                .ok_or_else(|| self.error(Span::default(), "unformed fixed resource import"));
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
            if source.unit == owner {
                let binding = scope.cells[self.demand.cell_ordinal(cell)].ok_or_else(|| {
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
        if self.program.cells[cell.index()].binding == CellBinding::Foreign {
            if let Some(binding) = self.foreign_import(cell)? {
                return self.reference(binding);
            }
            let cell = &self.program.cells[cell.index()];
            if let Some(ambient) = ambient::classify(cell) {
                return self.ambient(unit, ambient, cell.declaration);
            }
            Ok({
                let node = js::Expr::Host(self.text(&cell.name)?);
                self.expression(node)
            }?)
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
    fn foreign_import(&mut self, cell: CellId) -> Result<Option<js::BindingId>, FormationError> {
        self.work(self.foreign_bindings.len() + 1)?;
        if let Some(&(_, binding)) = self.foreign_bindings.iter().find(|(known, _)| *known == cell) {
            return Ok(Some(binding));
        }
        let program = self.program;
        let mut found = None;
        for module in program.modules.iter() {
            self.work(module.foreign_imports.len() + 1)?;
            if let Some(import) = module.foreign_imports.iter().find(|import| import.cell == cell) {
                found = Some(import);
                break;
            }
        }
        let Some(import) = found else {
            return Ok(None);
        };
        // A classic script can only use a foreign module its output carries;
        // output preparation refuses any other import there.
        let declaration = program.cells[cell.index()].declaration;
        // Pinned local names are module-wide: modules importing the same
        // binding share one import, and one name cannot name two sources.
        let name = &program.cells[cell.index()].name;
        self.work(self.module.imports.len())?;
        for index in 0..self.module.imports.len() {
            let existing = &self.module.imports[index];
            if self.module.bindings[existing.binding.index()].spelling != *name {
                continue;
            }
            if existing.imported != import.imported
                || existing.source.as_unicode() != Some(import.source.as_str())
            {
                return Err(self.error(declaration, "foreign binding imported from conflicting modules"));
            }
            let binding = existing.binding;
            self.budget
                .push(AllocationClass::Scratch, &mut self.foreign_bindings, (cell, binding))?;
            return Ok(Some(binding));
        }
        let scope = self.module.regions[self.module.root.index()].scope;
        let spelling = self.text(&program.cells[cell.index()].name)?;
        // The local name is invisible at runtime, but ports' builds match
        // the import text (katex inlines its font metrics that way), and the
        // legacy linker keeps the extern's name. Pinning keeps that contract.
        let binding = self.module.binding_in(
            js::Binding {
                source_symbol: None,
                scope,
                spelling,
                pinned: true,
            },
            self.budget,
        )?;
        self.module
            .import_in(&import.source, &import.imported, binding, self.budget)?;
        self.budget
            .push(AllocationClass::Scratch, &mut self.foreign_bindings, (cell, binding))?;
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
            Place::Index { receiver, key } => {
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
    fn single_access_place(&mut self, unit: ContextId, place: PlaceId) -> Result<bool, FormationError> {
        let data = self.data(unit);
        self.work(data.operations.len())?;
        let mut accesses = 0;
        for operation in &data.operations {
            accesses += match operation.kind {
                OperationKind::Load(used) | OperationKind::Store(used) | OperationKind::CheckPlace(used) => {
                    usize::from(used == place)
                }
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
        let mut raw = self.place(unit, place)?;
        if self.compact {
            if let Some(projected) = js::literal_array_projection(&self.module, raw, self.budget)? {
                raw = projected;
            }
        }
        // Language absence and integer obligations are distinct from the raw
        // JavaScript property load, including loads from foreign containers.
        match load_result_recipe(self.program, self.data(unit), place, ty) {
            recipe @ (LoadResultRecipe::NullishNull | LoadResultRecipe::NullishEmptyString) => {
                let absent = match recipe {
                    LoadResultRecipe::NullishNull => js::Literal::Null,
                    _ => js::Literal::String("".into()),
                };
                let right = self.literal(absent)?;
                Ok(self.save_nullish(unit, operation, raw, right)?)
            }
            LoadResultRecipe::NormalizeInteger => {
                let value = self.expression(js::Expr::ToInt32(raw))?;
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
        origin: Option<crate::ast::SourceNodeId>,
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
                ValueStorage::Rematerialized => {
                    return Err(self.error(operation.span, "rematerialized load was formed"))
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
                        ValueStorage::Captured(_) | ValueStorage::Rematerialized
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
                        return Err(
                            self.error(operation.span, "non-void debugLog logging contract")
                        );
                    }
                    // Preparing a print never touches console. Its argument
                    // schedules have already been retained in `arguments`.
                    // Omit the host lookup/invocation, preserving all argument
                    // evaluations and abrupt completion at their source site.
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
                            crate::primitive::host_builtin(builtin)
                                && (self.contract.assumptions.pristine_builtins
                                    || operands_first(builtin))
                                && (builtin != BuiltinCall::JsObject
                                    || arguments.len() % 2 == 0
                                        && arguments
                                            .chunks_exact(2)
                                            .all(|pair| self.string_key(pair[0])))
                        })
                    {
                        let expression = self.host_builtin(builtin, arguments, operation.span)?;
                        let expression = self.expression(expression)?;
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
                        CallResultRecipe::NormalizeInteger => {
                            self.expression(js::Expr::ToInt32(expression))?
                        }
                        CallResultRecipe::Raw | CallResultRecipe::IntrinsicInteger => expression,
                    };
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
                let literal = js::Literal::String(
                    self.string(self.demand.strings()[family].payload(self.program))?,
                );
                self.literal(literal)
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
        let mut loaded = 0;
        let mut forwarded: Option<(CallId, ValueId)> = None;
        let mut returned = false;
        for &operation in &function.regions[function.entry.index()].operations {
            let operation = &function.operations[operation.index()];
            match operation.kind {
                OperationKind::PrepareCall(call)
                    if matches!(function.calls[call.index()].target, CallTarget::Builtin(_)) => {}
                OperationKind::Load(place) if forwarded.is_none() => {
                    let Place::Cell(cell) = function.places[place.index()] else {
                        return None;
                    };
                    if function.parameters.get(loaded) != Some(&cell) || operation.result.is_none() {
                        return None;
                    }
                    loaded += 1;
                }
                OperationKind::Call(call)
                    if forwarded.is_none() && loaded == function.parameters.len() =>
                {
                    forwarded = Some((call, operation.result?));
                }
                OperationKind::Return if !returned => {
                    let (_, result) = forwarded?;
                    if function.operands(operation.operands)? != [result] {
                        return None;
                    }
                    returned = true;
                }
                _ => return None,
            }
        }
        let (call, result) = forwarded.filter(|_| returned)?;
        let CallTarget::Builtin(builtin) = function.calls[call.index()].target else {
            return None;
        };
        // Each builtin operand is its parameter's load, in order.
        let operands = function.arguments(function.calls[call.index()].arguments)?;
        let mut position = 0;
        for &operation in &function.regions[function.entry.index()].operations {
            let operation = &function.operations[operation.index()];
            if let OperationKind::Load(_) = operation.kind {
                if operands.get(position) != Some(&CallArgument::Value(operation.result?)) {
                    return None;
                }
                position += 1;
            }
        }
        // An integer result would owe the builtin's own normalization.
        (position == operands.len()
            && !matches!(
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
        let OperationKind::Load(place) = data.operations[data.values[callee.index()].definition.index()].kind
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
                function.operands(operation.operands).unwrap_or(&[]).first().copied()
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
                undefined
                    && produced.result.is_some()
                    && returned(second) == Some(produced.result)
            }
            _ => false,
        }
    }

    fn call(
        &mut self,
        unit: ContextId,
        call: CallId,
        arguments: Vec<js::ExprId>,
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
        let node = match self.data(unit).calls[call.index()].target.clone() {
            CallTarget::Value { callee, invocation } => {
                if invocation == Invocation::DirectEval {
                    return Err(self.error(span, "semantic JavaScript direct eval contract"));
                }
                let callee = self.value(unit, callee)?;
                js::Expr::Call {
                    callee,
                    arguments,
                    invocation,
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
                    let node = js::Expr::Host(self.text(host)?);
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
            CallTarget::Builtin(builtin) if crate::primitive::host_builtin(builtin) => {
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
            // host value, spelled as the legacy emitter spells them.
            CallTarget::Intrinsic {
                operation:
                    ResolvedIntrinsic::Method(
                        operation @ (Intrinsic::JsTruthy | Intrinsic::JsIsArray | Intrinsic::JsIsObject),
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
                // in the legacy emitter; the host iterator may be patched, so
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
                let result = contract
                    .signature
                    .and_then(|signature| match &self.program.types[signature.index()] {
                        Type::Function(signature) => Some(signature.return_type.as_ref()),
                        _ => None,
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
                        return Err(self.error(span, "string Array.pop before ES2020"))
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
                if operation != Intrinsic::MapGet {
                    call
                } else if self
                    .contract
                    .ecmascript
                    .allows(JsSyntaxFeature::NullishCoalescing)
                {
                    // An absent key is `undefined` in the host and `null` in
                    // the language, exactly as the legacy `m.get(k)??null`.
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
                let callee = self.expression(js::Expr::Host(host))?;
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
                // A source constructor is an observable host lookup followed
                // by construction, not proof of a pristine builtin or a fresh
                // result. The common envelope leaves that lookup before every
                // argument effect without introducing a temporary or .call.
                let host = self.text("RegExp")?;
                let callee = self.expression(js::Expr::Host(host))?;
                js::Expr::Construct { callee, arguments }
            }
            _ => return Err(self.error(span, "semantic JavaScript call implementation")),
        };
        Ok(self.expression(node)?)
    }

    /// `Math.abs`, `Object.prototype.hasOwnProperty.call`: one host lookup
    /// followed by ordinary member reads, each observable in order.
    fn host_path(&mut self, path: &[&str]) -> Result<js::ExprId, FormationError> {
        let root = self.text(path[0])?;
        let mut callee = self.expression(js::Expr::Host(root))?;
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

    fn scheduled_expression(
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
                ValueStorage::Rematerialized
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
                    let literal = js::Literal::String(
                        self.string(self.demand.strings()[family].payload(self.program))?,
                    );
                    self.literal(literal)
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
        self.work(1)?;
        let operands = self.data(unit).operands(operation.operands).unwrap();
        let node = match operation.kind {
            OperationKind::Constant(_) => return Ok(None),
            OperationKind::PrepareReference { call, position } => {
                return Ok(Some(self.prepare_reference(unit, call, position)?))
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
                let target = self.place(unit, place)?;
                js::Expr::Assign { target, value }
            }
            OperationKind::Initialize(cell)
                if matches!(self.demand.context(unit).kind, ContextKind::Inline { .. }) =>
            {
                let binding = self.cell_binding(unit, cell)?;
                let target = self.reference(binding)?;
                let value = self.value(unit, operands[0])?;
                let value = self.carrier_value(unit, cell, value)?;
                js::Expr::Assign { target, value }
            }
            OperationKind::IsUndefined => {
                let left = self.value(unit, operands[0])?;
                let right = self.literal(js::Literal::Undefined)?;
                js::Expr::Binary {
                    op: js::Binary::StrictEqual,
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
                let mut members = self
                    .budget
                    .vector(AllocationClass::Retained, members_len)?;
                for index in 0..members_len {
                    let (name, cell) = &program.modules[module.index()].namespace[index];
                    let binding = self.cell_binding(unit, *cell)?;
                    let value = self.expression(js::Expr::Binding(binding))?;
                    let name = self.text(name)?;
                    self.budget
                        .push(AllocationClass::Retained, &mut members, (name, value))?;
                }
                let specifier = program.strings[specifier.index()]
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
                let mut arguments = self
                    .budget
                    .vector(AllocationClass::Retained, operands.len() - 1)?;
                for &argument in &operands[1..] {
                    let argument = self.value(unit, argument)?;
                    self.append(&mut arguments, argument)?;
                }
                js::Expr::Construct { callee, arguments }
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
                    None => return Err(self.error(operation.span, "type test without a runtime test")),
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
                        js::Expr::Literal(js::Literal::String(_)) => 1,
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
                            js::Expr::Literal(js::Literal::String(text)) => js::TemplatePart::String(
                                self.budget.string_value(AllocationClass::Retained, text)?,
                            ),
                            js::Expr::Template(inner) => match &inner[index] {
                                js::TemplatePart::String(text) => js::TemplatePart::String(
                                    self.budget.string_value(AllocationClass::Retained, text)?,
                                ),
                                js::TemplatePart::Expression(id) => js::TemplatePart::Expression(*id),
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
                // A typed nullable value is `T` or `null`, but a host may hand
                // one over as `undefined` (a missing element, say): the loose
                // test treats both as absent, as the legacy route does. A
                // `JsValue` keeps strict equality, where they differ.
                let program = self.program;
                let ty = |value: ValueId| &program.types[self.data(unit).values[value.index()].ty.index()];
                let null_test = matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && operands[..2].iter().any(|&value| matches!(ty(value), Type::Null))
                    && !operands[..2]
                        .iter()
                        .any(|&value| matches!(ty(value), Type::TypeParameter("$js")));
                // Two numbers, two strings or two booleans compare the same
                // loosely: `==` converts nothing when the types already agree.
                let primitive = |ty: &Type<'_>| match ty {
                    Type::Int | Type::Float | Type::Enum(_) => Some(0),
                    Type::String => Some(1),
                    Type::Bool => Some(2),
                    _ => None,
                };
                let same_primitive = self.compact
                    && matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && primitive(ty(operands[0])).is_some()
                    && primitive(ty(operands[0])) == primitive(ty(operands[1]));
                js::Expr::Binary {
                    op: match (null_test || same_primitive, op) {
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
                // Under pristine builtins a string or array length is at most
                // 2^30 (the bound counting loops already use), so arithmetic
                // near it needs no int32 normalization.
                if self.contract.assumptions.pristine_builtins
                    && matches!(property, Intrinsic::StringLength | Intrinsic::ArrayLength)
                {
                    self.transfer_number(unit, operation, false, |_| {
                        NumberFacts::integer_range(0, 1 << 30, false).unwrap_or(NumberFacts::UNKNOWN)
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
                            value = self.expression(js::Expr::Spread(value))?;
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
                        self.append(&mut values, value)?;
                    }
                    js::Expr::Array(values)
                }
                AllocationKind::Record(keys) | AllocationKind::Object(keys) => {
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
                    for (&key, &operand) in keys.iter().zip(operands) {
                        self.work(1)?;
                        let value = self.value(unit, operand)?;
                        // Kept a string occurrence; the printer spells an
                        // identifier key `name:` (never `__proto__:`).
                        let payload = self.string(&self.program.strings[key.index()])?;
                        let key = self.literal(js::Literal::String(payload))?;
                        self.append(&mut entries, (js::Property::Computed(key), value))?;
                    }
                    js::Expr::Object(entries)
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
            OperationKind::Closure(_) => {
                let region = self.plan(unit).regions[operation.region.index()];
                let body = self
                    .module
                    .region_in(self.module.regions[region.index()].scope, self.budget)?;
                let child = self
                    .demand
                    .child(unit, operation_id)
                    .ok_or_else(|| self.error(operation.span, "missing callable demand context"))?;
                self.plan_context(child, body)?;
                self.statement_region(child, self.data(child).entry)?;
                self.finish_unit(child)?;
                let parameters = self.physical_parameters(child)?;
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
                let requested_arrow = match self.contract.abi.public_function_spelling {
                    Some(FunctionSpelling::Arrow) => true,
                    Some(FunctionSpelling::Function) => false,
                    None => {
                        self.data(child).kind == UnitKind::Closure
                            || self.compact && (private || private_cell.is_some())
                    }
                };
                // A declaration supplies its own receiver/arguments, including
                // observations in lexical descendants. That language contract
                // takes precedence over arrow spelling, as with the legacy
                // arguments rule. Closures keep their original lexical owner.
                let suspension = match self.data(child).suspension {
                    Suspension::None => js::Suspension::None,
                    Suspension::Async => js::Suspension::Async,
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
                // The exact name is allocated only when it is kept.
                let name = if private || private_cell.is_some() || unobserved {
                    js::FunctionName::Unobserved
                } else {
                    js::FunctionName::Exact(self.string(&self.program.strings[name.index()])?)
                };
                let strict = self.plan(child).strict_frame;
                if strict && self.plan(child).observes_activation {
                    // Strictness would change this frame's `this`/`arguments`.
                    return Err(self.error(
                        operation.span,
                        "value-struct script callable frame adaptation",
                    ));
                }
                // A host-derived class's constructor: its instance parameter
                // is `this`, bound where `super(...)` returns.
                let host_class = self.data(child).host_class;
                let mut parameters = parameters;
                if host_class.is_some() {
                    parameters.remove(0);
                }
                self.budget.push(
                    AllocationClass::Retained,
                    &mut self.module.functions,
                    js::Function {
                        parameters,
                        body,
                        arrow: arrow && host_class.is_none(),
                        name: if host_class.is_some() {
                            js::FunctionName::Unobserved
                        } else {
                            name
                        },
                        strict: strict && host_class.is_none(),
                        length: None,
                        suspension,
                    },
                )?;
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut self.unit_functions,
                    (body_unit, function),
                )?;
                // A private function goes straight into its cell's binding;
                // its name is not observable, so any spelling will do.
                if let Some(result) = operation.result.filter(|_| private_cell.is_some()) {
                    if let Some(cell) = private_cell {
                        let _ = result;
                        let value = self.expression(js::Expr::Function(function))?;
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
                match host_class {
                    Some(class) => {
                        let definition = &self.program.classes[class as usize];
                        let base = definition.base.as_deref().ok_or_else(|| {
                            self.error(operation.span, "host-derived class without a base")
                        })?;
                        let base = self.expression(js::Expr::Host(base.to_owned()))?;
                        js::Expr::Class {
                            name: definition.name.clone(),
                            base,
                            constructor: function,
                        }
                    }
                    None => js::Expr::Function(function),
                }
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
    fn statement_operations(
        &mut self,
        unit: ContextId,
        region: RegionId,
        operations: &'program [OpId],
    ) -> Result<(), FormationError> {
        let target_region = self.plan(unit).regions[region.index()];
        let mut cursor = 0;
        while cursor < operations.len() {
            self.work(1)?;
            let operation_id = operations[cursor];
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
                OperationKind::Initialize(cell) => {
                    let value = self.value(unit, operands[0])?;
                    let value = self.carrier_value(unit, cell, value)?;
                    js::Statement::Let {
                        binding: self.cell_binding(unit, cell)?,
                        value: Some(value),
                    }
                }
                OperationKind::Return => js::Statement::Return(
                    operands
                        .first()
                        .map(|value| self.value(unit, *value))
                        .transpose()?,
                ),
                OperationKind::Throw => js::Statement::Throw(self.value(unit, operands[0])?),
                // `super(...)`, then the instance parameter names `this`.
                OperationKind::SuperConstruct => {
                    let mut arguments = self
                        .budget
                        .vector(AllocationClass::Retained, operands.len())?;
                    for &argument in operands {
                        let argument = self.value(unit, argument)?;
                        self.append(&mut arguments, argument)?;
                    }
                    let call = self.expression(js::Expr::SuperCall { arguments })?;
                    self.statement(target_region, js::Statement::Evaluate(call))?;
                    let instance = self.data(unit).parameters[0];
                    let this = self.expression(js::Expr::This)?;
                    js::Statement::Let {
                        binding: self.cell_binding(unit, instance)?,
                        value: Some(this),
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
                    if let Some(expression) =
                        self.scheduled_expression(unit, &operations, &mut cursor)?
                    {
                        self.statement(target_region, js::Statement::Evaluate(expression))?;
                    }
                    continue;
                }
            };
            cursor += 1;
            self.statement(target_region, statement)?;
        }
        Ok(())
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
