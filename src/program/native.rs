//! A direct, admitted C11 executable client of the checked program.
//! The target plan owns storage/call legality; this writer only schedules those
//! recipes in the original structured order. It creates neither CFG nor C AST.

use super::native_memory;
use super::native_plan::{
    NativePlan, NativeType, PlaceRecipe, PreparedTarget, Tagged, ValueStorage,
};
use super::native_runtime::{self, Helper};
#[path = "native_arrays.rs"]
mod arrays;
#[path = "native_binary.rs"]
mod binary;
#[path = "native_classes.rs"]
mod classes;
#[path = "native_collections.rs"]
mod collections;
#[path = "native_dynamic.rs"]
mod dynamic;
#[path = "native_interface.rs"]
mod interface;
#[path = "native_ownership.rs"]
mod ownership;
#[path = "native_strings.rs"]
mod strings;
use super::publication::PublicationError;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
pub(in crate::program) use binary::RUNTIME as BINARY_RUNTIME;
pub(in crate::program) use collections::RUNTIME as COLLECTIONS_RUNTIME;
pub(in crate::program) use dynamic::RUNTIME as DYNAMIC_RUNTIME;
use std::fmt;
use std::mem::{self, size_of};

#[derive(Debug)]
pub enum NativeError {
    Publication(PublicationError),
    Allocation(AllocationError),
    Admission(crate::compilation_policy::AdmissionError),
    Artifact(super::publication::CandidateError),
    Unsupported {
        unit: Option<UnitId>,
        operation: Option<OpId>,
        span: Span,
        feature: &'static str,
    },
    WrongTarget,
    UnsupportedAbi,
}
impl NativeError {
    pub(super) fn unsupported(
        unit: Option<UnitId>,
        operation: Option<OpId>,
        span: Span,
        feature: &'static str,
    ) -> Self {
        Self::Unsupported {
            unit,
            operation,
            span,
            feature,
        }
    }
}
impl From<PublicationError> for NativeError {
    fn from(error: PublicationError) -> Self {
        Self::Publication(error)
    }
}
impl From<AllocationError> for NativeError {
    fn from(error: AllocationError) -> Self {
        Self::Allocation(error)
    }
}
impl From<super::publication::CandidateError> for NativeError {
    fn from(error: super::publication::CandidateError) -> Self {
        Self::Artifact(error)
    }
}

/// A declared native provider is mapped to an exact checked Foreign cell.
/// Provider link names occupy the versioned callback ABI's `host_` namespace.
#[derive(Debug, Clone, Copy)]
pub struct NativeHostBinding<'a> {
    pub cell: CellId,
    pub link_name: &'a str,
}

/// Callback ABI v1 supports synchronous entry/reentry on the originating
/// thread. Input handles are borrowed; retaining them acquires an owner.
/// A returned handle transfers an owner. Hosts release retained callbacks
/// before execution ends; concurrent calls and foreign unwinding are excluded.
/// String payloads (including struct fields) borrow immutable UTF-16 storage
/// valid until execution ends, matching this native client's static-string
/// representation. Hosts must not return pointers to temporary string storage.
#[derive(Debug, Clone, Copy)]
pub struct NativeHostBindings<'a> {
    pub callback_abi_version: u32,
    pub bindings: &'a [NativeHostBinding<'a>],
}
impl NativeHostBindings<'_> {
    pub const EMPTY: Self = Self {
        callback_abi_version: 1,
        bindings: &[],
    };
}

pub(super) struct NativeArtifacts {
    pub(super) c: String,
    pub(super) header: String,
}

/// Unqualified inspection text stays allocation-admitted until this facade
/// drops or transfers it. Deployment uses retain_native_c and a qualified receipt.
/// The callback cannot return a borrow of the facade or its backing storage.
///
/// Borrowed text cannot escape its compilation callback:
///
/// ```compile_fail
/// use lilscript::compilation_policy::{ResolvedPolicy, WorkDomain};
/// use lilscript::program::publication::{Compilation, SemanticId};
///
/// fn cannot_escape<'src>(compilation: &mut Compilation<'src>,
///     source: SemanticId, policy: &ResolvedPolicy)
/// {
///     let borrowed = compilation.with_native_c(source, policy, WorkDomain::Baseline,
///         |output| output.as_str()).unwrap();
///     let _ = borrowed.len();
/// }
/// ```
///
/// A live text borrow also prevents a handoff from invalidating that borrow:
///
/// ```compile_fail,E0502
/// use lilscript::compilation_policy::{ResolvedPolicy, WorkDomain};
/// use lilscript::program::publication::{Compilation, SemanticId};
///
/// fn cannot_take_while_borrowed<'src>(compilation: &mut Compilation<'src>,
///     source: SemanticId, policy: &ResolvedPolicy)
/// {
///     compilation.with_native_c(source, policy, WorkDomain::Baseline, |output| {
///         let borrowed = output.as_str();
///         let handed_off = output.take_c();
///         let _ = (borrowed.len(), handed_off.len());
///     }).unwrap();
/// }
/// ```
pub struct BudgetedNativeOutput<'budget, 'ledger> {
    text: String,
    header: String,
    budget: &'budget mut AllocationBudget<'ledger>,
}
impl<'budget, 'ledger> BudgetedNativeOutput<'budget, 'ledger> {
    pub(super) fn new(
        artifacts: NativeArtifacts,
        budget: &'budget mut AllocationBudget<'ledger>,
    ) -> Self {
        Self {
            text: artifacts.c,
            header: artifacts.header,
            budget,
        }
    }
    /// Generated declarations for a separately compiled host translation unit.
    /// Empty when no host bindings were requested. The C artifact is always a
    /// self-contained translation unit and does not include this header.
    pub fn header(&self) -> &str {
        &self.header
    }
    pub fn take_header(&mut self) -> String {
        let text = mem::take(&mut self.header);
        self.budget
            .release(AllocationClass::Retained, text.capacity() as u64)
            .expect("native header owns its exact admitted capacity");
        text
    }
    pub fn as_str(&self) -> &str {
        &self.text
    }
    /// Transfer the existing buffer to the caller, ending compiler ownership.
    /// Afterwards as_str and any later take_c return empty text.
    pub fn take_c(&mut self) -> String {
        let text = mem::take(&mut self.text);
        self.budget
            .release(AllocationClass::Retained, text.capacity() as u64)
            .expect("native output owns its exact admitted capacity");
        text
    }
}
impl Drop for BudgetedNativeOutput<'_, '_> {
    fn drop(&mut self) {
        let text = mem::take(&mut self.text);
        let header = mem::take(&mut self.header);
        let capacity = text.capacity() + header.capacity();
        drop((text, header));
        self.budget
            .release(AllocationClass::Retained, capacity as u64)
            .expect("native output releases after its actual buffer drops");
    }
}

pub(super) fn form(
    program: &Program<'_>,
    uses: &UseIndex,
    hosts: &NativeHostBindings<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<NativeArtifacts, NativeError> {
    let mut phase = budget.scope();
    phase.retain(
        AllocationClass::Scratch,
        size_of::<NativePlan<'_, '_>>() as u64 + size_of::<Emitter<'_, '_, '_, '_, '_>>() as u64,
    )?;
    let plan = NativePlan::build_with_hosts(program, uses, hosts, &mut phase)?;
    let mut emitter = Emitter {
        plan: &plan,
        budget: &mut phase,
        text: String::new(),
        stack: Vec::new(),
        field_path: Vec::new(),
        active_regions: Vec::new(),
    };
    // Both artifacts use the same borrowed physical signature and presentation
    // owner. Nothing is recovered from printed C or a second source graph.
    if !hosts.bindings.is_empty() {
        emitter.header()?;
    }
    let header = mem::take(&mut emitter.text);
    emitter.program()?;
    let text = mem::take(&mut emitter.text);
    drop(emitter);
    drop(plan);
    // All actual plan and emission scratch buffers have gone. Only complete
    // text transfers to the publication owner; a failure exposes no partial C.
    phase.finish_retained()?;
    Ok(NativeArtifacts { c: text, header })
}

#[derive(Clone, Copy)]
enum Destination {
    Value(ValueId),
    Cell(CellId),
    Place(PlaceId),
}

#[derive(Clone, Copy)]
enum Task {
    Region {
        region: RegionId,
        next: usize,
        enclosing_loop: Option<OpId>,
        destination: Option<ValueId>,
    },
    EnterRegion {
        region: RegionId,
        next: usize,
        enclosing_loop: Option<OpId>,
        destination: Option<ValueId>,
    },
    Text(&'static str),
    LoopTest {
        operation: OpId,
        condition: ValueId,
    },
    LoopUpdate(OpId),
    LoopEnd(OpId),
}

struct Emitter<'plan, 'program, 'src, 'budget, 'ledger> {
    plan: &'plan NativePlan<'program, 'src>,
    budget: &'budget mut AllocationBudget<'ledger>,
    text: String,
    stack: Vec<Task>,
    field_path: Vec<usize>,
    // The current lexical path in the original region tree, not a cleanup CFG.
    active_regions: Vec<RegionId>,
}
impl Emitter<'_, '_, '_, '_, '_> {
    fn write(&mut self, arguments: fmt::Arguments<'_>) -> Result<(), NativeError> {
        self.budget
            .write_fmt(AllocationClass::Retained, &mut self.text, arguments)?;
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<(), NativeError> {
        self.budget
            .push_str(AllocationClass::Retained, &mut self.text, text)?;
        Ok(())
    }
    fn push(&mut self, task: Task) -> Result<(), NativeError> {
        self.budget
            .push(AllocationClass::Scratch, &mut self.stack, task)?;
        Ok(())
    }
    fn region(
        &mut self,
        region: RegionId,
        enclosing_loop: Option<OpId>,
    ) -> Result<(), NativeError> {
        self.region_result(region, enclosing_loop, None)
    }
    fn region_result(
        &mut self,
        region: RegionId,
        enclosing_loop: Option<OpId>,
        destination: Option<ValueId>,
    ) -> Result<(), NativeError> {
        self.push(Task::EnterRegion {
            region,
            next: 0,
            enclosing_loop,
            destination,
        })
    }
    fn program(&mut self) -> Result<(), NativeError> {
        self.text(native_runtime::PROLOGUE)?;
        for helper in self.plan.helpers.iter() {
            self.text(helper.definition())?;
        }
        let program = self.plan.program;
        self.value_types()?;
        if self.plan.needs_callable_runtime() {
            self.text(native_memory::INTERFACE)?;
            self.text(native_memory::QUALIFICATION_IMPLEMENTATION)?;
            // Callable signatures may mention arrays; arrays hold no callables.
            self.array_declarations()?;
            self.callable_types()?;
            self.dynamic_callables()?;
            self.adapters()?;
            self.array_types()?;
            if self.plan.helpers.contains(Helper::Strings) {
                if let Some(array) = self.plan.string_array() {
                    self.string_split_runtime(array)?;
                }
            }
            self.class_types()?;
            self.host_interface()?;
            self.capture_types()?;
        }
        for (index, needed) in self.plan.strings.iter().copied().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if !needed {
                continue;
            }
            let value = &program.strings[index];
            // The empty value has no backing allocation or zero-length array.
            if value.code_units().next().is_none() {
                continue;
            }
            self.write(format_args!("static const uint16_t ls_s{index}[] = {{"))?;
            for unit in value.code_units() {
                self.write(format_args!("{unit},"))?;
            }
            self.text("};\n")?;
        }
        self.globals()?;
        for frozen in &program.units {
            self.signature(frozen.id())?;
            self.text(";\n")?;
        }
        if self.plan.needs_callable_runtime() {
            self.closure_recipes()?;
        }
        for frozen in &program.units {
            self.unit(frozen.id())?;
        }
        self.text("int main(void) {\nif (!ls_runtime_init()) return 1;\n")?;
        if self.plan.needs_callable_runtime() {
            self.write(format_args!(
                "ls_native_identity_counter = UINT64_C({});\n",
                program.units.len()
            ))?;
        }
        for unit in program.initialization.iter() {
            self.write(format_args!("ls_init{}();\n", unit.index()))?;
        }
        if self.plan.helpers.contains(Helper::Strings) {
            self.text("fflush(stdout);\n")?;
        }
        // Module bindings outlive their initializers; execution ends here.
        for (index, cell) in self.plan.cells.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if !cell.global {
                continue;
            }
            if let ValueStorage::Value(ty) = cell.storage {
                if let Some(prefix) = ty.owner_prefix() {
                    self.write(format_args!("{prefix}_clear(&ls_c{index});\n"))?;
                }
            }
        }
        if self.plan.helpers.contains(Helper::Strings) {
            self.text("ls_strings_release();\n")?;
        }
        self.text("return 0;\n}\n")
    }
    /// File-scope slots for module bindings that functions use. A function
    /// reaches one through its guard, since JavaScript throws when a binding
    /// is read before its initializer runs; the initializer itself is proven
    /// to follow the declaration and writes the slot directly.
    fn globals(&mut self) -> Result<(), NativeError> {
        let mut first = true;
        for (index, cell) in self.plan.cells.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if !cell.global {
                continue;
            }
            if first {
                self.text("#include <stdlib.h>\nstatic void ls_native_unbound(void) {\nfputs(\"LilScript native module binding used before initialization\\n\", stderr);\nabort();\n}\n")?;
                first = false;
            }
            let ValueStorage::Value(ty) = cell.storage else {
                unreachable!("native globals are value storage")
            };
            self.write(format_args!(
                "static {ty} ls_c{index}{};\nstatic bool ls_ready{index};\nstatic {ty} *ls_g{index}(void) {{\nif (!ls_ready{index}) ls_native_unbound();\nreturn &ls_c{index};\n}}\n",
                ty.empty_slot()
            ))?;
        }
        Ok(())
    }
    fn signature(&mut self, id: UnitId) -> Result<(), NativeError> {
        let unit = self.plan.program.unit(id).unwrap();
        let result = self.plan.units[id.index()].return_type;
        let name = if unit.kind == UnitKind::ModuleInitialization {
            "ls_init"
        } else {
            "ls_fn"
        };
        self.write(format_args!("static {} {name}{}(", result, id.index()))?;
        let environment = self.plan.units[id.index()].has_environment;
        if environment {
            self.text("void *ls_env")?;
        }
        if unit.parameters.is_empty() && !environment {
            self.text("void")?;
        }
        for (index, cell) in unit.parameters.iter().copied().enumerate() {
            if index != 0 || environment {
                self.text(",")?;
            }
            let ValueStorage::Value(ty) = self.plan.cell_storage(cell) else {
                unreachable!("native plan has only by-value parameters")
            };
            self.write(format_args!(
                "{} {}ls_{}{}",
                ty,
                if self.plan.reference_parameter(cell) {
                    "*"
                } else {
                    ""
                },
                if self.plan.boxed_cell(cell) { "p" } else { "c" },
                cell.index()
            ))?;
        }
        self.text(")")
    }
    fn unit(&mut self, id: UnitId) -> Result<(), NativeError> {
        debug_assert!(self.active_regions.is_empty());
        self.signature(id)?;
        self.text(" {\n")?;
        let unit = self.plan.program.unit(id).unwrap();
        if self.plan.needs_callable_runtime() {
            self.parameter_owners(id)?;
        }
        // Initialize sites are unique by plan validation. This visits only this
        // unit's operations, never all program cells for each function.
        for operation in &unit.operations {
            self.budget.work(WorkKind::Render, 1)?;
            if let OperationKind::Initialize(cell) = operation.kind {
                if self.plan.global_cell(cell) {
                    continue;
                }
                if let ValueStorage::Value(ty) = self.plan.cell_storage(cell) {
                    if self.plan.boxed_cell(cell) {
                        self.write(format_args!(
                            "ls_box{} *ls_c{} = NULL;\n",
                            cell.index(),
                            cell.index()
                        ))?;
                    } else {
                        self.write(format_args!(
                            "{} ls_c{}{};\n",
                            ty,
                            cell.index(),
                            ty.empty_slot()
                        ))?;
                    }
                }
            }
        }
        for (index, storage) in self.plan.units[id.index()]
            .values
            .iter()
            .copied()
            .enumerate()
        {
            self.budget.work(WorkKind::Render, 1)?;
            if let ValueStorage::Value(ty) = storage {
                if ty != NativeType::Void {
                    self.write(format_args!("{} ls_v{index}{};\n", ty, ty.empty_slot()))?;
                }
            }
        }
        // A callable read from a place when its call is prepared.
        for (call_index, target) in self.plan.units[id.index()].calls.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if let PreparedTarget::Placed { signature, .. } = *target {
                self.write(format_args!(
                    "ls_callable{signature} ls_pc{call_index} = {{0}};\n"
                ))?;
            }
        }
        // Every reference argument owns one ordered address temporary. Later
        // arguments can change the pointee value, never the selected location.
        for (call_index, call) in unit.calls.iter().enumerate() {
            for (position, argument) in unit.arguments(call.arguments).unwrap().iter().enumerate() {
                self.budget.work(WorkKind::Render, 1)?;
                if let CallArgument::Reference(place) = *argument {
                    let ty = self.plan.place_type(id, place);
                    self.write(format_args!("{ty} *ls_a{call_index}_{position};\n"))?;
                }
            }
        }
        // Static function symbols implement all roots' verified instantiation
        // prefixes before main starts evaluation. Only this module's suffix
        // executes here; its local scalar storage is never made global.
        let next = if unit.kind == UnitKind::ModuleInitialization {
            unit.instantiation_prefix as usize
        } else {
            0
        };
        self.push(Task::EnterRegion {
            region: unit.entry,
            next,
            enclosing_loop: None,
            destination: None,
        })?;
        while let Some(task) = self.stack.pop() {
            self.budget.work(WorkKind::Render, 1)?;
            match task {
                Task::EnterRegion {
                    region,
                    next,
                    enclosing_loop,
                    destination,
                } => {
                    if self.plan.needs_callable_runtime() {
                        self.budget.push(
                            AllocationClass::Scratch,
                            &mut self.active_regions,
                            region,
                        )?;
                    }
                    self.push(Task::Region {
                        region,
                        next,
                        enclosing_loop,
                        destination,
                    })?;
                }
                Task::Region {
                    region,
                    next,
                    enclosing_loop,
                    destination,
                } => {
                    if let Some(operation) =
                        unit.regions[region.index()].operations.get(next).copied()
                    {
                        self.push(Task::Region {
                            region,
                            next: next + 1,
                            enclosing_loop,
                            destination,
                        })?;
                        self.operation(id, operation, enclosing_loop)?;
                    } else {
                        if let Some(destination) = destination {
                            let source = unit.regions[region.index()].result.unwrap();
                            self.copy_value(id, Destination::Value(destination), source)?;
                        }
                        if self.plan.needs_callable_runtime() {
                            self.cleanup_region(id, region)?;
                            let completed = self.active_regions.pop();
                            debug_assert_eq!(completed, Some(region));
                        }
                    }
                }
                Task::Text(text) => self.text(text)?,
                Task::LoopTest {
                    operation,
                    condition,
                } => {
                    self.write(format_args!(
                        "if (!ls_v{}) goto ls_end{};\n",
                        condition.index(),
                        operation.index()
                    ))?;
                }
                Task::LoopUpdate(operation) => {
                    self.write(format_args!("ls_update{}: ;\n", operation.index()))?
                }
                Task::LoopEnd(operation) => self.write(format_args!(
                    "goto ls_test{};\nls_end{}: ;\n",
                    operation.index(),
                    operation.index()
                ))?,
            }
        }
        debug_assert!(self.active_regions.is_empty());
        // The plan proves totality for nonvoid functions; there is no invented
        // default value, legacy fallback or undefined nonvoid fallthrough.
        self.text("}\n")
    }
    fn operation(
        &mut self,
        id: UnitId,
        op: OpId,
        enclosing_loop: Option<OpId>,
    ) -> Result<(), NativeError> {
        let unit = self.plan.program.unit(id).unwrap();
        let operation = &unit.operations[op.index()];
        let args = unit.operands(operation.operands).unwrap();
        let result = operation.result;
        let stored_result = result.filter(|value| matches!(self.plan.units[id.index()].values[value.index()], ValueStorage::Value(ty) if ty != NativeType::Void));
        match &operation.kind {
            OperationKind::Constant(value) => {
                let result = result.unwrap().index();
                match value {
                    Constant::Integer(value) => self.write(format_args!(
                        "ls_v{result} = ls_from_u32(UINT32_C({}));\n",
                        *value as u32
                    ))?,
                    Constant::Number(bits) => self.write(format_args!(
                        "ls_v{result} = ls_f64_bits(UINT64_C({bits}));\n"
                    ))?,
                    Constant::Boolean(value) => self.write(format_args!(
                        "ls_v{result} = {};\n",
                        if *value { "true" } else { "false" }
                    ))?,
                    Constant::String(string) => {
                        let value = &self.plan.program.strings[string.index()];
                        if value.code_units().next().is_none() {
                            self.write(format_args!("ls_v{result} = (ls_string){{NULL,0}};\n"))?;
                        } else {
                            self.write(format_args!(
                                "ls_v{result} = (ls_string){{ls_s{0},sizeof ls_s{0}/sizeof *ls_s{0}}};\n",
                                string.index(),
                            ))?;
                        }
                    }
                    Constant::Null => {
                        let destination = Destination::Value(ValueId::from_index(result).unwrap());
                        self.assignment_start(id, destination, true)?;
                        self.text("(ls_value){0}")?;
                        self.assignment_end(id, destination)?;
                    }
                    _ => unreachable!("native plan rejects unsupported constants"),
                }
            }
            OperationKind::Initialize(cell) => {
                if self.plan.boxed_cell(*cell) {
                    self.write(format_args!(
                        "ls_native_release(ls_c{0});\nls_c{0} = ls_box_new{0}(",
                        cell.index()
                    ))?;
                    let ty = self.plan.value_type(self.plan.cell_storage(*cell));
                    self.converted(id, args[0], ty)?;
                    self.text(");\n")?;
                } else if matches!(self.plan.cell_storage(*cell), ValueStorage::Value(_)) {
                    self.copy_value(id, Destination::Cell(*cell), args[0])?;
                    if self.plan.global_cell(*cell) {
                        self.write(format_args!("ls_ready{} = true;\n", cell.index()))?;
                    }
                }
            }
            OperationKind::Load(place) => {
                if let Some(result) = stored_result {
                    let destination = Destination::Value(result);
                    let from = self.plan.place_type(id, *place);
                    let to = self
                        .plan
                        .value_type(self.plan.units[id.index()].values[result.index()]);
                    // An optional read of a position: past the end is null.
                    if let (
                        PlaceRecipe::Element {
                            receiver,
                            index,
                            array,
                        },
                        NativeType::Dynamic(_),
                        false,
                    ) = (
                        self.plan.units[id.index()].places[place.index()].recipe,
                        to,
                        matches!(from, NativeType::Dynamic(_)),
                    ) {
                        self.assignment_start(id, destination, false)?;
                        self.write(format_args!(
                            "ls_array{array}_optional(ls_v{},ls_v{})",
                            receiver.index(),
                            index.index()
                        ))?;
                        return self.assignment_end(id, destination);
                    }
                    let (prefix, suffix) = Self::conversion(from, to);
                    self.assignment_start(id, destination, false)?;
                    self.text(&prefix)?;
                    self.place(id, *place)?;
                    self.text(suffix)?;
                    self.assignment_end(id, destination)?;
                }
            }
            OperationKind::Store(place) => {
                if let PlaceRecipe::IndexedUnion {
                    receiver,
                    index: Some(index),
                    array,
                    kind,
                } = self.plan.units[id.index()].places[place.index()].recipe
                {
                    let (r, i) = (receiver.index(), index.index());
                    let element = self.plan.arrays[array];
                    self.write(format_args!(
                        "if (ls_v{r}.tag == LS_ARRAY) ls_array{array}_set((ls_array{array} *)ls_v{r}.as.o,ls_v{i},"
                    ))?;
                    self.converted(id, args[0], element)?;
                    self.write(format_args!(
                        ");\nelse ls_typed_set_{}(ls_v{r}.as.o,ls_v{i},",
                        binary::kind_name(kind)
                    ))?;
                    self.converted(id, args[0], element)?;
                    self.text(");\n")?;
                } else if let PlaceRecipe::TypedElement {
                    receiver,
                    index,
                    kind,
                } = self.plan.units[id.index()].places[place.index()].recipe
                {
                    self.write(format_args!(
                        "ls_typed_set_{}(ls_v{},ls_v{},",
                        binary::kind_name(kind),
                        receiver.index(),
                        index.index()
                    ))?;
                    let element = if kind.element_is_float() {
                        NativeType::F64
                    } else {
                        NativeType::I32
                    };
                    self.converted(id, args[0], element)?;
                    self.text(");\n")?;
                } else if let PlaceRecipe::Element {
                    receiver,
                    index,
                    array,
                } = self.plan.units[id.index()].places[place.index()].recipe
                {
                    // Setting one past the end appends, as in JavaScript.
                    self.write(format_args!(
                        "ls_array{array}_set(ls_v{},ls_v{},",
                        receiver.index(),
                        index.index()
                    ))?;
                    self.converted(id, args[0], self.plan.arrays[array])?;
                    self.text(
                        ");
",
                    )?;
                } else {
                    self.copy_value(id, Destination::Place(*place), args[0])?;
                }
                if let Some(result) = stored_result {
                    self.copy_value(id, Destination::Value(result), args[0])?;
                }
            }
            OperationKind::Allocate {
                kind: AllocationKind::Array,
                ..
            } => {
                let result = result.unwrap();
                let ValueStorage::Value(NativeType::Array(array)) =
                    self.plan.units[id.index()].values[result.index()]
                else {
                    unreachable!("native array allocation is a value")
                };
                self.write(format_args!(
                    "ls_array{array}_take(&(ls_v{}),ls_array{array}_new({}));\n",
                    result.index(),
                    args.len()
                ))?;
                for &value in args {
                    self.write(format_args!("ls_array{array}_push(ls_v{},", result.index()))?;
                    self.converted(id, value, self.plan.arrays[array])?;
                    self.text(");\n")?;
                }
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(intrinsic))
                if *intrinsic == crate::primitive::Intrinsic::BufferByteLength
                    || crate::typed_array::classify_typed_array_intrinsic(*intrinsic).is_some() =>
            {
                self.binary_property(id, result.unwrap(), args[0], *intrinsic)?;
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                crate::primitive::Intrinsic::MapSize | crate::primitive::Intrinsic::SetSize,
            )) => {
                self.write(format_args!(
                    "ls_v{} = ls_map_size(ls_v{});\n",
                    result.unwrap().index(),
                    args[0].index()
                ))?;
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                crate::primitive::Intrinsic::ArrayLength,
            )) => {
                self.write(format_args!(
                    "ls_v{} = (int32_t)ls_v{}->length;
",
                    result.unwrap().index(),
                    args[0].index()
                ))?;
            }
            OperationKind::Allocate {
                kind: AllocationKind::Object(_),
                ..
            } => {
                let result = result.unwrap();
                let ValueStorage::Value(NativeType::Object(class)) =
                    self.plan.units[id.index()].values[result.index()]
                else {
                    unreachable!("native class instance is a value")
                };
                self.allocate_object(id, result, class, args)?;
            }
            OperationKind::Allocate {
                kind: AllocationKind::Struct(_),
                ..
            } => {
                let result = result.unwrap();
                let ValueStorage::Value(ty) = self.plan.units[id.index()].values[result.index()]
                else {
                    unreachable!("native struct allocation is a value")
                };
                self.write(format_args!("ls_v{} = ({ty}){{", result.index()))?;
                if args.is_empty() {
                    self.text("0")?;
                }
                let NativeType::Struct(layout) = ty else {
                    unreachable!("native struct allocation has a struct layout")
                };
                let fields = self.plan.program.structs[layout].fields.clone();
                for (index, (value, field)) in args.iter().zip(fields).enumerate() {
                    if index != 0 {
                        self.text(",")?;
                    }
                    self.converted(id, *value, self.plan.field_type(field))?;
                }
                self.text("};\n")?;
            }
            OperationKind::CopyValue => {
                if let Some(result) = stored_result {
                    self.copy_value(id, Destination::Value(result), args[0])?;
                }
            }
            OperationKind::IntBinary(kind) => {
                let result = result.unwrap().index();
                let left = args[0].index();
                let right = args[1].index();
                match kind {
                    IntBinary::Add | IntBinary::Subtract => {
                        let token = if *kind == IntBinary::Add { "+" } else { "-" };
                        self.write(format_args!("ls_v{result} = ls_from_u32((uint32_t)((uint32_t)ls_v{left} {token} (uint32_t)ls_v{right}));\n"))?;
                    }
                    _ => {
                        let helper = match kind {
                            IntBinary::Multiply => Helper::Multiply,
                            IntBinary::Divide => Helper::Divide,
                            IntBinary::Remainder => Helper::Remainder,
                            IntBinary::UnsignedShiftRight => Helper::UnsignedShiftRight,
                            _ => unreachable!(),
                        };
                        self.write(format_args!(
                            "ls_v{result} = {}(ls_v{left},ls_v{right});\n",
                            helper.name()
                        ))?;
                    }
                }
            }
            OperationKind::Binary(kind) => {
                self.binary(id, *kind, result.unwrap(), args[0], args[1])?;
            }
            OperationKind::Template => {
                self.write(format_args!("ls_v{} = ", result.unwrap().index()))?;
                self.concatenation(id, args)?;
                self.text(";\n")?;
            }
            OperationKind::Unary { op, integer } => {
                let result = result.unwrap().index();
                let value = args[0].index();
                match op {
                    UnaryOp::Not => self.write(format_args!("ls_v{result} = !ls_v{value};\n"))?,
                    UnaryOp::Neg if *integer => self.write(format_args!("ls_v{result} = ls_from_u32((uint32_t)(UINT32_C(0) - (uint32_t)ls_v{value}));\n"))?,
                    UnaryOp::Neg => self.write(format_args!("ls_v{result} = ls_f64(-(double)ls_v{value});\n"))?,
                }
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                crate::primitive::Intrinsic::StringLength,
            )) => {
                self.write(format_args!(
                    "ls_v{} = ls_string_length(ls_v{});\n",
                    result.unwrap().index(),
                    args[0].index()
                ))?;
            }
            OperationKind::Call(call) => self.call(id, *call, result)?,
            OperationKind::TypeTest(target) => {
                let test =
                    crate::primitive::runtime_type_test(&self.plan.program.types[target.index()])
                        .expect("native plan admits runtime type tests");
                self.type_test(id, result.unwrap(), args[0], test)?;
            }
            OperationKind::IsUndefined => {
                let result = result.unwrap().index();
                match self.plan.units[id.index()].values[args[0].index()] {
                    ValueStorage::Value(NativeType::Callable(_)) => self.write(format_args!(
                        "ls_v{result} = ls_v{}.code == NULL;\n",
                        args[0].index()
                    ))?,
                    _ => self.write(format_args!("ls_v{result} = false;\n"))?,
                }
            }
            OperationKind::PrepareReference { call, position } => {
                let CallArgument::Reference(place) =
                    unit.arguments(unit.calls[call.index()].arguments).unwrap()[*position as usize]
                else {
                    unreachable!("verified native reference preparation")
                };
                self.write(format_args!("ls_a{}_{} = &(", call.index(), position))?;
                self.place(id, place)?;
                self.text(");\n")?;
            }
            // The native plan proves initialization and static addressability
            // at this exact schedule position. No leaf read/coercion is needed.
            OperationKind::CheckPlace(_) => {}
            OperationKind::PrepareCall(call) => {
                // An argument may replace the stored callable: hold our own.
                if let PreparedTarget::Placed { place, signature } =
                    self.plan.units[id.index()].calls[call.index()]
                {
                    self.write(format_args!(
                        "ls_callable{signature}_copy(&ls_pc{},",
                        call.index()
                    ))?;
                    self.place(id, place)?;
                    self.text(");\n")?;
                }
            }
            OperationKind::Closure(body) => {
                if let Some(result) = stored_result {
                    self.create_closure(id, *body, result)?;
                }
            }
            OperationKind::Block(region) => self.region(*region, enclosing_loop)?,
            OperationKind::If { yes, no } => {
                self.write(format_args!("if (ls_v{}) {{\n", args[0].index()))?;
                self.push(Task::Text("}\n"))?;
                if let Some(no) = no {
                    self.region(*no, enclosing_loop)?;
                    self.push(Task::Text("} else {\n"))?;
                }
                self.region(*yes, enclosing_loop)?;
            }
            OperationKind::Select { yes, no } => {
                let destination = result.unwrap();
                self.write(format_args!("if (ls_v{}) {{\n", args[0].index()))?;
                self.push(Task::Text("}\n"))?;
                self.region_result(*no, enclosing_loop, Some(destination))?;
                self.push(Task::Text("} else {\n"))?;
                self.region_result(*yes, enclosing_loop, Some(destination))?;
            }
            OperationKind::ShortCircuit {
                kind: ShortCircuit::Nullish,
                right,
            } => {
                let destination = result.unwrap();
                self.write(format_args!(
                    "if (ls_v{}.tag != LS_NULL) {{\n",
                    args[0].index()
                ))?;
                self.copy_value(id, Destination::Value(destination), args[0])?;
                self.text("} else {\n")?;
                self.push(Task::Text("}\n"))?;
                self.region_result(*right, enclosing_loop, Some(destination))?;
            }
            OperationKind::ShortCircuit { kind, right } => {
                let destination = result.unwrap();
                self.write(format_args!(
                    "ls_v{} = ls_v{};\nif ({}ls_v{}) {{\n",
                    destination.index(),
                    args[0].index(),
                    if *kind == ShortCircuit::BooleanOr {
                        "!"
                    } else {
                        ""
                    },
                    args[0].index()
                ))?;
                self.push(Task::Text("}\n"))?;
                self.region_result(*right, enclosing_loop, Some(destination))?;
            }
            OperationKind::Loop { test, body, update } => {
                self.write(format_args!("ls_test{}: ;\n", op.index()))?;
                self.push(Task::LoopEnd(op))?;
                self.region(*update, Some(op))?;
                self.push(Task::LoopUpdate(op))?;
                self.region(*body, Some(op))?;
                if let Some(condition) = unit.regions[test.index()].result {
                    self.push(Task::LoopTest {
                        operation: op,
                        condition,
                    })?;
                }
                self.region(*test, Some(op))?;
            }
            OperationKind::Return if self.plan.needs_callable_runtime() => {
                self.return_value(id, args.first().copied())?;
            }
            OperationKind::Return => {
                if self.plan.units[id.index()].return_type == NativeType::Void {
                    self.text("return;\n")?;
                } else if let Some(value) = args.first() {
                    self.write(format_args!("return ls_v{};\n", value.index()))?;
                } else {
                    self.text("return;\n")?;
                }
            }
            OperationKind::Break | OperationKind::Continue => {
                let enclosing = enclosing_loop.expect("verified loop completion");
                if self.plan.needs_callable_runtime() {
                    self.cleanup_path(id, Some(unit.operations[enclosing.index()].region))?;
                }
                self.write(format_args!(
                    "goto ls_{}{};\n",
                    if matches!(operation.kind, OperationKind::Break) {
                        "end"
                    } else {
                        "update"
                    },
                    enclosing.index()
                ))?;
            }
            _ => unreachable!("native plan validates every supported recipe before emission"),
        }
        Ok(())
    }
    /// Emit the chosen root plus field slots without recursive formatting or
    /// cloning complete paths for every projection. Operand effects already
    /// occurred in source order before this scheduled load/store.
    fn place(&mut self, unit: UnitId, mut place: PlaceId) -> Result<(), NativeError> {
        debug_assert!(self.field_path.is_empty());
        loop {
            self.budget.work(WorkKind::Render, 1)?;
            match self.plan.units[unit.index()].places[place.index()].recipe {
                PlaceRecipe::Cell(cell) => {
                    self.cell_place(unit, cell)?;
                    break;
                }
                PlaceRecipe::Value(value) => {
                    self.write(format_args!("ls_v{}", value.index()))?;
                    break;
                }
                PlaceRecipe::Member {
                    receiver,
                    class,
                    slot,
                } => {
                    self.write(format_args!(
                        "((ls_object{class} *)ls_v{})->ls_m{slot}",
                        receiver.index()
                    ))?;
                    break;
                }
                // An element read by value; stores use `set` instead.
                PlaceRecipe::IndexedUnion {
                    receiver,
                    index,
                    array,
                    kind,
                } => {
                    let r = receiver.index();
                    match index {
                        Some(index) => self.write(format_args!(
                            "(ls_v{r}.tag == LS_ARRAY ? ls_array{array}_get((ls_array{array} *)ls_v{r}.as.o,ls_v{i}) : ls_typed_get_{}(ls_v{r}.as.o,ls_v{i}))",
                            binary::kind_name(kind),
                            i = index.index()
                        ))?,
                        None => self.write(format_args!(
                            "(ls_v{r}.tag == LS_ARRAY ? (int32_t)((ls_array{array} *)ls_v{r}.as.o)->length : ls_typed_length(ls_v{r}.as.o))"
                        ))?,
                    }
                    break;
                }
                PlaceRecipe::TypedElement {
                    receiver,
                    index,
                    kind,
                } => {
                    self.write(format_args!(
                        "ls_typed_get_{}(ls_v{},ls_v{})",
                        binary::kind_name(kind),
                        receiver.index(),
                        index.index()
                    ))?;
                    break;
                }
                PlaceRecipe::Element {
                    receiver,
                    index,
                    array,
                } => {
                    self.write(format_args!(
                        "ls_array{array}_get(ls_v{},ls_v{})",
                        receiver.index(),
                        index.index()
                    ))?;
                    break;
                }
                PlaceRecipe::Field { base, slot } => {
                    self.budget
                        .push(AllocationClass::Scratch, &mut self.field_path, slot)?;
                    place = base;
                }
            }
        }
        while let Some(slot) = self.field_path.pop() {
            self.write(format_args!(".ls_f{slot}"))?;
        }
        Ok(())
    }

    fn binary(
        &mut self,
        unit: UnitId,
        kind: BinaryOp,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    ) -> Result<(), NativeError> {
        if self.string_binary(unit, kind, result, left, right)? {
            return Ok(());
        }
        let values = &self.plan.units[unit.index()].values;
        if matches!(kind, BinaryOp::Eq | BinaryOp::NotEq)
            && [left, right].iter().any(|value| {
                matches!(
                    values[value.index()],
                    ValueStorage::Value(NativeType::Dynamic(_))
                )
            })
        {
            self.write(format_args!(
                "ls_v{} = {}ls_value_equal(",
                result.index(),
                if kind == BinaryOp::NotEq { "!" } else { "" }
            ))?;
            self.converted(unit, left, NativeType::Dynamic(Tagged::ANY))?;
            self.text(",")?;
            self.converted(unit, right, NativeType::Dynamic(Tagged::ANY))?;
            return self.text(");\n");
        }
        if matches!(kind, BinaryOp::Eq | BinaryOp::NotEq)
            && matches!(
                self.plan
                    .value_type(self.plan.units[unit.index()].values[left.index()]),
                NativeType::Callable(_)
            )
        {
            self.write(format_args!("ls_v{} = (", result.index()))?;
            self.value(unit, left)?;
            self.text(").identity ")?;
            self.text(if kind == BinaryOp::Eq { "== (" } else { "!= (" })?;
            self.value(unit, right)?;
            return self.text(").identity;\n");
        }
        let (result, left, right) = (result.index(), left.index(), right.index());
        let token = match kind {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::Xor => "^",
            BinaryOp::Eq => "==",
            BinaryOp::NotEq => "!=",
            BinaryOp::Less => "<",
            BinaryOp::LessEq => "<=",
            BinaryOp::Greater => ">",
            BinaryOp::GreaterEq => ">=",
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight | BinaryOp::UnsignedShiftRight => {
                let helper = match kind {
                    BinaryOp::ShiftLeft => Helper::ShiftLeft,
                    BinaryOp::ShiftRight => Helper::ShiftRight,
                    _ => Helper::UnsignedShiftRight,
                };
                return self.write(format_args!(
                    "ls_v{result} = {}(ls_v{left},ls_v{right});\n",
                    helper.name()
                ));
            }
            _ => unreachable!("native plan supported binary recipe"),
        };
        match kind {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                self.write(format_args!(
                    "ls_v{result} = ls_f64((double)ls_v{left} {token} (double)ls_v{right});\n"
                ))
            }
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::Xor => self.write(format_args!(
                "ls_v{result} = ls_from_u32((uint32_t)ls_v{left} {token} (uint32_t)ls_v{right});\n"
            )),
            _ => self.write(format_args!(
                "ls_v{result} = ls_v{left} {token} ls_v{right};\n"
            )),
        }
    }
    fn call(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: Option<ValueId>,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let args = data.arguments(data.calls[call.index()].arguments).unwrap();
        let target = self.plan.units[unit.index()].calls[call.index()];
        if let PreparedTarget::Print = target {
            let CallArgument::Value(value) = args[0] else {
                unreachable!("native Print takes a value")
            };
            match self.plan.units[unit.index()].values[value.index()] {
                ValueStorage::Value(NativeType::I32) => self.write(format_args!(
                    "printf(\"%ld\\n\",(long)ls_v{});\n",
                    value.index()
                ))?,
                ValueStorage::Value(NativeType::Bool) => self.write(format_args!(
                    "puts(ls_v{} ? \"true\" : \"false\");\n",
                    value.index()
                ))?,
                ValueStorage::Value(NativeType::F64) => {
                    self.write(format_args!("ls_print_number(ls_v{});\n", value.index()))?
                }
                ValueStorage::Value(NativeType::String) => {
                    self.write(format_args!("ls_print_string(ls_v{});\n", value.index()))?
                }
                ValueStorage::Value(NativeType::Dynamic(_)) => {
                    self.write(format_args!("ls_print_value(ls_v{});\n", value.index()))?
                }
                _ => unreachable!("native plan print contract"),
            }
            return Ok(());
        }
        if let PreparedTarget::ArrayMethod {
            receiver,
            array,
            method,
        } = target
        {
            return self.array_method(unit, call, result, receiver, array, method);
        }
        if let PreparedTarget::ScalarMethod { receiver, method } = target {
            return self.scalar_method(unit, call, result, receiver, method);
        }
        if let PreparedTarget::Construct(intrinsic) = target {
            if matches!(
                intrinsic,
                crate::primitive::Intrinsic::MapNew
                    | crate::primitive::Intrinsic::SetNew
                    | crate::primitive::Intrinsic::SymbolNew
            ) {
                return self.construct(unit, result.unwrap(), intrinsic);
            }
            return self.construct_binary(unit, call, result.unwrap(), intrinsic);
        }
        if let PreparedTarget::BinaryMethod { receiver, method } = target {
            return self.binary_method(unit, call, result.unwrap(), receiver, method);
        }
        if let PreparedTarget::CollectionMethod { receiver, method } = target {
            return self.collection_method(unit, call, result.unwrap(), receiver, method);
        }
        let storage = result.map(|value| self.plan.units[unit.index()].values[value.index()]);
        // What the callee produces and what each of its parameters takes.
        let signature = match target {
            PreparedTarget::Function(function) => Some(self.plan.signature_for_unit(function)),
            PreparedTarget::Callable { signature, .. }
            | PreparedTarget::Placed { signature, .. }
            | PreparedTarget::Host { signature, .. } => Some(signature),
            _ => None,
        };
        let produced = match target {
            PreparedTarget::ArrayPop { array, .. } => Some(self.plan.arrays[array]),
            _ => signature.map(|signature| self.plan.signatures[signature].result),
        };
        let (prefix, suffix) = match (produced, storage) {
            (Some(from), Some(ValueStorage::Value(to))) if to != NativeType::Void => {
                Self::conversion(from, to)
            }
            _ => (String::new(), ""),
        };
        // A callable argument of another physical signature travels in an
        // adapter the caller owns for the duration of the call.
        let parameter_type = |plan: &NativePlan<'_, '_>, index: usize| match target {
            PreparedTarget::ArrayPush { array, .. } => Some(plan.arrays[array]),
            _ => signature.map(|signature| plan.signatures[signature].parameters[index]),
        };
        let mut adapted = Vec::new();
        for (index, argument) in args.iter().enumerate() {
            if let (CallArgument::Value(value), Some(parameter)) =
                (*argument, parameter_type(self.plan, index))
            {
                let actual = self.plan.units[unit.index()].values[value.index()];
                if !self.plan.compatible(ValueStorage::Value(parameter), actual) {
                    if let Some(pair) = self.plan.adaptation(ValueStorage::Value(parameter), actual)
                    {
                        adapted.push((index, value, pair));
                    }
                }
            }
        }
        // Each adapted argument is a fresh owner: a callable of the target
        // signature, or a tagged value when both sides are tagged.
        let tagged = |plan: &NativePlan<'_, '_>, value: ValueId| {
            matches!(
                plan.units[unit.index()].values[value.index()],
                ValueStorage::Value(NativeType::Dynamic(_))
            )
        };
        if !adapted.is_empty() {
            self.text("{\n")?;
            for &(index, value, (from, to)) in &adapted {
                let parameter = parameter_type(self.plan, index).unwrap();
                match (tagged(self.plan, value), parameter) {
                    (true, NativeType::Dynamic(_)) => {
                        self.write(format_args!(
                            "ls_value ls_adapted{index} = ls_adapt_value{from}_{to}("
                        ))?;
                        self.value(unit, value)?;
                        self.text(");\n")?;
                    }
                    (true, _) => {
                        self.write(format_args!(
                            "ls_callable{to} ls_adapted{index} = ls_adapt{from}_{to}(ls_value_to_callable{from}("
                        ))?;
                        self.value(unit, value)?;
                        self.text("));\n")?;
                    }
                    (false, _) => {
                        self.write(format_args!(
                            "ls_callable{to} ls_adapted{index} = ls_adapt{from}_{to}("
                        ))?;
                        self.value(unit, value)?;
                        self.text(");\n")?;
                    }
                }
            }
        }
        if let Some(ValueStorage::Value(ty)) = storage {
            if ty != NativeType::Void {
                self.assignment_start(unit, Destination::Value(result.unwrap()), true)?;
            }
        }
        self.text(&prefix)?;
        let floating = produced.unwrap_or(match storage {
            Some(ValueStorage::Value(ty)) => ty,
            _ => NativeType::Void,
        }) == NativeType::F64;
        if floating {
            self.text("ls_f64(")?;
        }
        let leading_argument = match target {
            PreparedTarget::Function(unit) => {
                self.write(format_args!("ls_fn{}(", unit.index()))?;
                false
            }
            PreparedTarget::Placed { .. } => {
                self.write(format_args!(
                    "ls_pc{0}.code(ls_pc{0}.environment",
                    call.index()
                ))?;
                true
            }
            PreparedTarget::Callable { callee, .. } => {
                self.write(format_args!(
                    "ls_v{0}.code(ls_v{0}.environment",
                    callee.index()
                ))?;
                // The owning SSA receiver was frozen before later arguments.
                // A public host invocation acquires its own protective owner.
                true
            }
            PreparedTarget::Host { binding, .. } => {
                self.write(format_args!(
                    "{}(",
                    self.plan.hosts.bindings[binding].link_name
                ))?;
                false
            }
            PreparedTarget::MathImul => {
                self.text("ls_imul(")?;
                false
            }
            PreparedTarget::CharCodeAt { receiver } => {
                self.write(format_args!("ls_char_code_at(ls_v{}", receiver.index()))?;
                true
            }
            PreparedTarget::CharAt { receiver } => {
                self.write(format_args!("ls_char_at(ls_v{}", receiver.index()))?;
                true
            }
            PreparedTarget::ArrayPush { receiver, array } => {
                self.write(format_args!(
                    "ls_array{array}_push(ls_v{}",
                    receiver.index()
                ))?;
                true
            }
            PreparedTarget::ArrayPop { receiver, array } => {
                self.write(format_args!("ls_array{array}_pop(ls_v{}", receiver.index()))?;
                true
            }
            PreparedTarget::Print
            | PreparedTarget::ArrayMethod { .. }
            | PreparedTarget::ScalarMethod { .. }
            | PreparedTarget::Construct(_)
            | PreparedTarget::BinaryMethod { .. }
            | PreparedTarget::CollectionMethod { .. } => unreachable!(),
        };
        for (index, value) in args.iter().enumerate() {
            if index != 0 || leading_argument {
                self.text(",")?;
            }
            let parameter = parameter_type(self.plan, index);
            if let Some(&(_, value, (_, to))) = adapted.iter().find(|entry| entry.0 == index) {
                let held = if tagged(self.plan, value)
                    && matches!(parameter, Some(NativeType::Dynamic(_)))
                {
                    parameter.unwrap()
                } else {
                    NativeType::Callable(to)
                };
                let (prefix, suffix) = Self::conversion(held, parameter.unwrap());
                self.write(format_args!("{prefix}ls_adapted{index}{suffix}"))?;
                continue;
            }
            match *value {
                CallArgument::Value(value) => match parameter {
                    Some(parameter) => self.converted(unit, value, parameter)?,
                    None => self.value(unit, value)?,
                },
                CallArgument::Reference(_) => {
                    self.write(format_args!("ls_a{}_{}", call.index(), index))?
                }
            }
        }
        // Omitted arrow defaults: the empty callable, which the callee's
        // guard replaces.
        if let (Some(signature), false) = (signature, matches!(target, PreparedTarget::Host { .. }))
        {
            let parameters = self.plan.signatures[signature].parameters.len();
            for position in args.len()..parameters {
                if position != 0 || leading_argument {
                    self.text(",")?;
                }
                self.write(format_args!(
                    "({}){{0}}",
                    self.plan.signatures[signature].parameters[position]
                ))?;
            }
        }
        if floating {
            self.text(")")?;
        }
        self.text(")")?;
        self.text(suffix)?;
        let mut ended = false;
        if let Some(ValueStorage::Value(ty)) = storage {
            if ty != NativeType::Void {
                self.assignment_end(unit, Destination::Value(result.unwrap()))?;
                ended = true;
            }
        }
        if !ended {
            self.text(";\n")?;
        }
        if let PreparedTarget::Placed { signature, .. } = target {
            self.write(format_args!(
                "ls_callable{signature}_clear(&ls_pc{});\n",
                call.index()
            ))?;
        }
        if !adapted.is_empty() {
            for &(index, value, (_, to)) in &adapted {
                if tagged(self.plan, value)
                    && matches!(
                        parameter_type(self.plan, index),
                        Some(NativeType::Dynamic(_))
                    )
                {
                    self.write(format_args!("ls_value_clear(&ls_adapted{index});\n"))?;
                } else {
                    self.write(format_args!("ls_callable{to}_clear(&ls_adapted{index});\n"))?;
                }
            }
            self.text("}\n")?;
        }
        Ok(())
    }
}
