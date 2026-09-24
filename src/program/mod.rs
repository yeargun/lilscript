//! Checked language meaning, before choosing a JavaScript or native layout.
//!
//! Units own ordered operations and structured completion. Values name results;
//! cells name mutable storage. Neither is an emitted identifier. The frontend
//! conversion consumes checker facts once and does not retain an AST or CFG.
//!
//! The target files (`javascript*.rs`, `native*.rs`) live here for now:
//! JavaScript formation moves to `crate::js` in M8, native to `src/native/` in M11.

mod activation;
mod ambient;
mod artifact_provenance;
mod artifacts;
mod callable_inputs;
mod demand;
pub mod facts;
mod from_source;
mod function_layout;
mod helper_family;
pub mod ids;
mod implementation_identity;
mod implementations;
mod javascript;
mod module_contract;
mod native;
mod native_memory;
mod native_plan;
mod native_runtime;
mod native_string_runtime;
#[cfg(test)]
mod product_demand_tests;
mod product_family;
#[cfg(test)]
mod product_family_tests;
#[cfg(test)]
mod product_javascript_tests;
#[cfg(test)]
mod product_publication_tests;
pub mod publication;
mod raw_domains;
mod record_family;
mod rewrite_lineage;
mod search_entries;
mod search_opportunities;
pub mod storage;
mod string_family;
pub mod uses;
mod value_placement;
mod verify;

#[cfg(test)]
mod call_contract_tests;

#[cfg(test)]
mod constructor_verify_tests;

#[cfg(test)]
mod regex_call_tests;

#[cfg(test)]
mod string_call_tests;

#[cfg(test)]
mod string_call_verify_tests;

#[cfg(test)]
mod frame_contract_tests;

#[cfg(test)]
mod script_observation_tests;

#[cfg(test)]
mod observation_output_tests;

#[cfg(test)]
mod observation_edit_tests;

#[cfg(test)]
mod callable_inputs_tests;

#[cfg(test)]
mod raw_domains_call_tests;

#[cfg(test)]
mod helper_context_tests;

#[cfg(test)]
mod native_tests;

#[cfg(test)]
mod native_reference_tests;

#[cfg(test)]
mod javascript_reference_tests;

#[cfg(test)]
mod reference_core_tests;

#[cfg(test)]
mod nominal_core_tests;

#[cfg(test)]
mod nominal_target_tests;

#[cfg(test)]
mod module_native_tests;

#[cfg(test)]
mod module_javascript_tests;

#[cfg(test)]
mod module_publication_tests;

#[cfg(test)]
mod module_helper_tests;

#[cfg(test)]
mod class_javascript_tests;
#[cfg(test)]
mod d3_clause_tests;
#[cfg(test)]
mod source_legality_tests;
#[cfg(test)]
mod syntax_javascript_tests;

#[cfg(test)]
mod suspension_javascript_tests;

#[cfg(test)]
mod recipe_descriptor_tests;

pub use from_source::{from_checked_modules, from_checked_source, ModuleUnsupported, Unsupported};
pub(crate) use from_source::{
    from_checked_modules_admitted, from_checked_source_admitted, ConversionError,
};
pub use ids::*;
pub use storage::{FrozenUnit, WorkingUnit};

use crate::ast::{BinaryOp, SourceNodeId, UnaryOp};
use crate::check::{BuiltinCall, NominalId, NominalMemberId, SymbolId, Type};
use crate::literal::StringValue;
pub use crate::primitive::{DefaultConvention, Invocation};
use crate::primitive::{IntBinary, ResolvedIntrinsic};
use crate::span::Span;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    ModuleInitialization,
    Function,
    Closure,
}

#[derive(Debug, Clone)]
pub struct UnitData {
    pub kind: UnitKind,
    /// How this callable body suspends; module initialization never does.
    pub suspension: Suspension,
    /// Set on the `init` of a class with a host ancestor: the index of that
    /// class in `Program::classes`. Such a class stays a real JavaScript
    /// subclass, and this body is its constructor: its first parameter is
    /// the instance, bound once `SuperConstruct` has run.
    pub host_class: Option<u32>,
    /// Original source owner; operation node IDs and spans stay local to it.
    pub module: ModuleId,
    /// Number of entry-region operations that instantiate named functions.
    /// All module prefixes execute before ordered module value initialization.
    pub instantiation_prefix: u32,
    /// Creation-time observable name. It is independent of any emitted binding.
    /// Module initialization is not callable; anonymous closures have an exact
    /// empty string, rather than an unspecified name.
    pub function_name: Option<StringId>,
    /// Checked callable signature owned by this unit. Return/parameter
    /// verification does not rediscover it through a closure's producer.
    /// Module initialization has no callable signature.
    pub callable_type: Option<TypeId>,
    pub parameters: Vec<CellId>,
    pub captures: Vec<CellId>,
    pub entry: RegionId,
    pub operations: Vec<Operation>,
    pub operands: Vec<ValueId>,
    pub values: Vec<Value>,
    pub regions: Vec<Region>,
    pub places: Vec<Place>,
    pub calls: Vec<CallSite>,
    /// Sparse checked generic instances; all type payloads remain in Program.types.
    pub call_instantiations: Vec<CallInstantiation>,
    /// Sole owner of ordered call arguments; invocation operations have no
    /// ordinary operands. Reference arguments name evaluated places.
    pub call_arguments: Vec<CallArgument>,
}

/// An async body awaits tasks and resolves its `Task<T>` with the `T` it
/// returns; a generator body yields `T` and returns nothing. Both are
/// JavaScript-only: native targets refuse them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Suspension {
    #[default]
    None,
    Async,
    Generator,
}

/// Whether a call reaches host code, which applies its own parameter
/// defaults and can observe how many arguments it received: an extern
/// function, or a method of a host object (an extern class instance or a
/// `JsValue`). Such a call preserves omitted arguments; a LilScript callee's
/// omitted defaults are evaluated by its caller instead.
pub(crate) fn host_call(program: &Program<'_>, data: &UnitData, target: &CallTarget) -> bool {
    match *target {
        CallTarget::Value { callee, .. } => {
            let definition = &data.operations[data.values[callee.index()].definition.index()];
            matches!(
                definition.kind,
                OperationKind::Load(place)
                    if matches!(
                        data.places[place.index()],
                        Place::Cell(cell) if program.cells[cell.index()].binding == CellBinding::Foreign
                    )
            )
        }
        CallTarget::Reference { place } => match data.places[place.index()] {
            Place::Member { receiver, .. } => host_receiver(program, data, receiver),
            _ => false,
        },
        CallTarget::Builtin(_) | CallTarget::Intrinsic { .. } => false,
    }
}

/// Whether a value is a host object: a `JsValue` or an extern class
/// instance. Its members are host properties and methods.
pub(crate) fn host_receiver(program: &Program<'_>, data: &UnitData, receiver: ValueId) -> bool {
    match &program.types[data.values[receiver.index()].ty.index()] {
        Type::TypeParameter("$js") => true,
        Type::Class(name) | Type::ClassInstance { name, .. } => {
            program.class(name).is_some_and(|class| class.external)
        }
        _ => false,
    }
}

impl UnitData {
    pub fn empty(kind: UnitKind) -> Self {
        Self {
            kind,
            suspension: Suspension::None,
            host_class: None,
            module: ModuleId::from_index(0).unwrap(),
            instantiation_prefix: 0,
            function_name: None,
            callable_type: None,
            parameters: Vec::new(),
            captures: Vec::new(),
            entry: RegionId::from_index(0).unwrap(),
            operations: Vec::new(),
            operands: Vec::new(),
            values: Vec::new(),
            regions: vec![Region {
                parent: None,
                operations: Vec::new(),
                result: None,
                span: Span::default(),
            }],
            places: Vec::new(),
            calls: Vec::new(),
            call_instantiations: Vec::new(),
            call_arguments: Vec::new(),
        }
    }

    pub fn operands(&self, range: OperandRange) -> Option<&[ValueId]> {
        let end = range.start.checked_add(range.len)?;
        self.operands.get(range.start as usize..end as usize)
    }

    pub fn arguments(&self, range: ArgumentRange) -> Option<&[CallArgument]> {
        let end = range.start.checked_add(range.len)?;
        self.call_arguments.get(range.start as usize..end as usize)
    }

    /// Borrow the effective checked signature without re-inferring arguments.
    /// Semantic verification validates the instance/declaration relationship.
    pub fn call_signature(&self, call: &CallSite) -> Option<TypeId> {
        match call.contract.instantiation {
            Some(id) => Some(self.call_instantiations.get(id.index())?.signature),
            None => call.contract.signature,
        }
    }

    /// The unit points into the shared callable table; parameter metadata is
    /// borrowed from that one owner even while a frontend unit is being built.
    pub fn parameter<'types, 'src>(
        &self,
        position: u32,
        types: &'types [Type<'src>],
    ) -> Option<&'types crate::check::FunctionParameter<'src>> {
        let signature = match types.get(self.callable_type?.index())? {
            Type::Function(signature) => signature,
            Type::GenericFunction(function) => &function.signature,
            _ => return None,
        };
        signature.params.get(position as usize)
    }
}

#[derive(Debug, Clone)]
pub struct Program<'src> {
    /// Revision of the checked interface/type/string tables. Unit-only edits
    /// retain this stamp; any changed table meaning must issue a fresh one.
    tables_revision: RevisionId,
    units: Vec<FrozenUnit>,
    // Published tables are immutable. Retaining a candidate shares their
    // buffers and nested payloads; a unit-only edit does not copy every cell,
    // type, string or interface in the program.
    cells: Arc<Vec<Cell>>,
    types: Arc<Vec<Type<'src>>>,
    strings: Arc<Vec<StringValue>>,
    structs: Arc<Vec<StructDefinition>>,
    enums: Arc<Vec<EnumDefinition>>,
    fields: Arc<Vec<Field>>,
    classes: Arc<Vec<ClassDefinition>>,
    /// One export table; interfaces own disjoint ranges. Public observations
    /// use only the selected entry's range, not every private module export.
    exports: Arc<Vec<Export>>,
    initialization: Arc<Vec<UnitId>>,
    modules: Arc<Vec<ModuleInterface>>,
    entry: ModuleId,
}

/// Checked module meaning after original source aliases resolve to declarations.
/// The ordered edges own initialization dependencies; the schedule is derived.
#[derive(Debug, Clone)]
pub struct ModuleInterface {
    pub source: crate::ast::SourceIdentity,
    pub initializer: UnitId,
    pub dependencies: Vec<ModuleId>,
    /// Modules this one loads with `import()`. A module only these edges
    /// reach is initialization-free and initializes after every other.
    pub dynamic_dependencies: Vec<ModuleId>,
    /// The runtime exports some code reads through an `import()` namespace
    /// of this module, by name.
    pub namespace: Vec<(String, CellId)>,
    pub imports: Vec<ModuleImport>,
    pub exports: std::ops::Range<usize>,
    /// `import extern` edges: each binds one of this module's foreign cells
    /// to a named export of a JavaScript module.
    pub foreign_imports: Vec<ForeignImport>,
}

/// A foreign cell's runtime source: `import { imported as local } from source`.
/// A resolved relative path is spelled relative to the root module's
/// directory, as the old route's linker spelled it; a bare specifier is kept.
#[derive(Debug, Clone)]
pub struct ForeignImport {
    pub cell: CellId,
    pub source: String,
    pub imported: String,
}

#[derive(Debug, Clone)]
pub struct ModuleImport {
    pub module: ModuleId,
    pub name: String,
    pub target: InterfaceTarget,
    pub span: Span,
}

impl<'src> Program<'src> {
    /// Read the single retained parameter record through its verified ordinal.
    pub fn parameter(&self, cell: CellId) -> Option<&crate::check::FunctionParameter<'src>> {
        let cell = self.cells.get(cell.index())?;
        let CellBinding::Parameter(position) = cell.binding else {
            return None;
        };
        self.unit(cell.owner)?.parameter(position, &self.types)
    }

    pub fn is_reference_parameter(&self, cell: CellId) -> bool {
        self.parameter(cell).is_some_and(|parameter| {
            parameter.passing == crate::primitive::ParameterPassing::MutableReference
        })
    }
    pub fn verify(&self) -> Result<(), String> {
        verify::verify(self)
    }
    /// Unconfigured inspection output that preserves printing and exports.
    /// Project builds use publication::Compilation with a ResolvedPolicy.
    pub fn to_javascript(&self) -> Result<crate::js::Module, Unsupported> {
        javascript::lower(self)
    }
    pub fn units(&self) -> &[FrozenUnit] {
        &self.units
    }
    /// The checked type a value, cell or signature names.
    pub fn ty(&self, id: TypeId) -> Option<&Type<'src>> {
        self.types.get(id.index())
    }
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
    /// Canonical member lookup in the declaration-ordered field arena. Source
    /// construction and common admission certify strict member identity order.
    pub fn field(&self, identity: NominalMemberId) -> Option<&Field> {
        self.fields
            .binary_search_by_key(&identity.index(), |field| field.identity.index())
            .ok()
            .map(|index| &self.fields[index])
    }
    pub fn structs(&self) -> &[StructDefinition] {
        &self.structs
    }
    pub fn enums(&self) -> &[EnumDefinition] {
        &self.enums
    }
    pub fn classes(&self) -> &[ClassDefinition] {
        &self.classes
    }
    /// The declaration of a nominal class type, by its checked name.
    pub fn class(&self, name: &str) -> Option<&ClassDefinition> {
        self.classes.iter().find(|class| class.name == name)
    }
    pub fn exports(&self) -> &[Export] {
        self.modules
            .get(self.entry.index())
            .and_then(|module| self.exports.get(module.exports.clone()))
            .unwrap_or(&[])
    }
    /// Runtime exports borrow the one complete interface table. Type exports
    /// have no storage, emitted binding, or execution event.
    pub fn value_exports(&self) -> impl Iterator<Item = (&str, CellId)> {
        self.exports()
            .iter()
            .filter_map(|export| match export.target {
                InterfaceTarget::Value(cell) => Some((export.name.as_str(), cell)),
                InterfaceTarget::Struct(_) => None,
            })
    }
    pub fn modules(&self) -> &[ModuleInterface] {
        &self.modules
    }
    pub fn entry_module(&self) -> ModuleId {
        self.entry
    }
    pub fn initialization(&self) -> &[UnitId] {
        &self.initialization
    }
    pub fn unit(&self, id: UnitId) -> Option<&UnitData> {
        let unit = self.units.get(id.index())?;
        (unit.id() == id).then(|| unit.data())
    }
}

#[derive(Debug, Clone)]
pub struct Cell {
    /// For a checked binding, its own symbol (`cells[i]` is symbol `i`). For
    /// a synthetic cell, the source construct it implements.
    pub source_symbol: SymbolId,
    pub name: String,
    pub ty: TypeId,
    pub owner: UnitId,
    pub region: RegionId,
    pub declaration: Span,
    pub assigned: bool,
    pub binding: CellBinding,
    /// Conversion-owned storage with no checked symbol of its own (a
    /// `for...of` array and counter). Synthetic cells follow every checked one.
    pub synthetic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellBinding {
    Local,
    /// Position in the owning unit's callable signature; passing mode has one
    /// owner, the corresponding semantic FunctionParameter record.
    Parameter(u32),
    Function(UnitId),
    Foreign,
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub identity: NominalId,
    pub name: String,
    /// Original declaration owner; source identity is borrowed through modules.
    pub module: ModuleId,
    pub span: Span,
    pub type_parameters: Vec<String>,
    /// Declaration-order fields, including members never accessed locally.
    pub fields: std::ops::Range<usize>,
}

/// A class declaration. An internal class instance is a mutable object whose
/// fields (base-first) are its own data properties under their declared
/// names; its methods and `init` are function units that take the instance
/// first, never members. Host code holding an instance sees that data object:
/// an ordinary class may dissolve, as on the old route. An `extern class` names a
/// host object type: its fields are the declared exact host properties (its
/// own, not flattened with a base) and it has no units.
#[derive(Debug, Clone)]
pub struct ClassDefinition {
    pub name: String,
    pub external: bool,
    pub base: Option<String>,
    /// The class's own type parameters, and its base's type arguments in
    /// terms of them (empty for a non-generic base): an upcast of an
    /// instance substitutes these up the chain.
    pub type_params: Vec<String>,
    pub base_arguments: Vec<TypeId>,
    /// An extern class's host constructor, as its checked function type:
    /// what `super(...)` of a subclass calls. Internal classes have none.
    pub constructor: Option<TypeId>,
    /// Flattened field names and types, base fields first.
    pub fields: Vec<(StringId, TypeId)>,
}

#[derive(Debug, Clone)]
pub struct EnumDefinition {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub value: i32,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub identity: NominalMemberId,
    pub owner: NominalId,
    pub name: String,
    pub ty: TypeId,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterfaceTarget {
    Value(CellId),
    Struct(NominalId),
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub target: InterfaceTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandRange {
    pub start: u32,
    pub len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgumentRange {
    pub start: u32,
    pub len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallArgument {
    Value(ValueId),
    Reference(PlaceId),
}

#[derive(Debug, Clone)]
pub struct Value {
    pub ty: TypeId,
    pub definition: OpId,
}

#[derive(Debug, Clone)]
pub struct Region {
    pub parent: Option<RegionId>,
    pub operations: Vec<OpId>,
    /// An expression region yields this value on normal completion only.
    pub result: Option<ValueId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub kind: OperationKind,
    pub operands: OperandRange,
    pub result: Option<ValueId>,
    pub region: RegionId,
    pub origin: Option<SourceNodeId>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constant {
    Integer(i32),
    Number(u64),
    String(StringId),
    Boolean(bool),
    Null,
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortCircuit {
    BooleanAnd,
    BooleanOr,
    JavaScriptAnd,
    JavaScriptOr,
    Nullish,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Cell(CellId),
    /// A value can supply a read-only projection, never mutable storage.
    Value(ValueId),
    Field {
        /// Earlier place in this unit. Value fields retain their storage path;
        /// crossing a reference field evaluates that reference into a new root.
        base: PlaceId,
        field: NominalMemberId,
    },
    Member {
        receiver: ValueId,
        key: StringId,
    },
    Index {
        receiver: ValueId,
        key: ValueId,
    },
}

#[derive(Debug, Clone)]
pub enum AllocationKind {
    Array,
    /// `[a, ...b]`: an array literal whose `true` operands are spread through
    /// their iterator protocol and whose `false` operands are single elements.
    SpreadArray(Vec<bool>),
    Record(Vec<StringId>),
    Object(Vec<StringId>),
    Struct(NominalId),
}

/// One call owns its target and checked invocation contract. The signature is
/// shared through the program type table; arguments remain ordered operations.
#[derive(Debug, Clone)]
pub struct CallSite {
    pub target: CallTarget,
    pub contract: CallContract,
    pub arguments: ArgumentRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallContract {
    /// Checked callee type, including generic/dynamic callable forms. Builtin
    /// and constructor checker paths may have no separately checked callee
    /// expression; their intrinsic contract owns the argument/result schema.
    pub signature: Option<TypeId>,
    /// This original call's checker-resolved type arguments and effective signature.
    pub instantiation: Option<CallInstantiationId>,
    /// Source-provided arity, distinct from the complete typed operand list
    /// when a caller evaluates omitted defaults. Explicit values are not gaps.
    pub supplied: u32,
    pub defaults: DefaultConvention,
}

/// One original generic call's checked semantic decision. The declaration is
/// unchanged even when another call instantiates the same body differently.
#[derive(Debug, Clone)]
pub struct CallInstantiation {
    pub declaration: TypeId,
    pub arguments: Vec<TypeId>,
    pub signature: TypeId,
}

/// Preparing a call fixes its callable/reference before evaluating arguments.
/// This remains explicit even when a target has no first-class reference value.
#[derive(Debug, Clone)]
pub enum CallTarget {
    Value {
        callee: ValueId,
        invocation: Invocation,
    },
    Reference {
        place: PlaceId,
    },
    Builtin(BuiltinCall),
    Intrinsic {
        operation: ResolvedIntrinsic,
        receiver: Option<ValueId>,
    },
}

#[derive(Debug, Clone)]
pub enum OperationKind {
    Constant(Constant),
    Initialize(CellId),
    Load(PlaceId),
    /// Validate an evaluated mutable location before its RHS. This observes
    /// access failure, never the old leaf's value or a value conversion.
    CheckPlace(PlaceId),
    Store(PlaceId),
    CopyValue,
    IntBinary(IntBinary),
    Binary(BinaryOp),
    Unary {
        op: UnaryOp,
        integer: bool,
    },
    Intrinsic(ResolvedIntrinsic),
    PrepareCall(CallId),
    PrepareReference {
        call: CallId,
        position: u32,
    },
    Call(CallId),
    Closure(UnitId),
    Allocate {
        identity: AllocationId,
        kind: AllocationKind,
    },
    ShortCircuit {
        kind: ShortCircuit,
        right: RegionId,
    },
    Select {
        yes: RegionId,
        no: RegionId,
    },
    If {
        yes: RegionId,
        no: Option<RegionId>,
    },
    Loop {
        test: RegionId,
        body: RegionId,
        update: RegionId,
    },
    Try {
        body: RegionId,
        catch: Option<(Option<CellId>, RegionId)>,
        finally: Option<RegionId>,
    },
    Block(RegionId),
    /// `for (string key in object) body`, the object being the one operand:
    /// each iteration initializes `key` (declared in `body`, like a catch
    /// binding) to the next enumerable string key, own or inherited, that
    /// still exists. Break/continue in `body` target this loop.
    ForIn {
        key: CellId,
        body: RegionId,
    },
    /// `for (T item of iterable) body` over a generator, the iterable being
    /// the one operand: each step initializes `item` (declared in `body`) to
    /// the next yielded value. Break/continue in `body` target this loop;
    /// leaving it early closes the generator. Arrays use a counted loop.
    ForOf {
        item: CellId,
        body: RegionId,
    },
    /// Suspends an async body until its `Task<T>` operand settles: the result
    /// is the fulfilled `T`; a rejection throws here. Any code may run in
    /// between.
    Await,
    /// `import(specifier)`: a task fulfilled, on a later turn, with the
    /// module's namespace, whose members are its `namespace` exports. A
    /// failed load rejects with a `ModuleLoadError`.
    LoadModule {
        module: ModuleId,
        specifier: StringId,
    },
    /// Suspends a generator body, yielding its operand to the consumer, or
    /// with `delegate` each element of an array, typed array or generator.
    Yield {
        delegate: bool,
    },
    Return,
    Throw,
    Break,
    Continue,
    /// Whether a parameter arrived as `undefined` (omitted by a host or an
    /// erased caller), so its checked default applies. Never true natively.
    IsUndefined,
    /// `value is T` on a union or nullable: the target's runtime test
    /// (`typeof` for primitives and functions, `Array.isArray` for arrays).
    TypeTest(TypeId),
    /// A template literal: the string conversion of each operand, left to
    /// right. Conversion uses the template rule (string hint; a Symbol
    /// throws), not binary `+`. Operands are strings or stringable values.
    Template,
    /// `new C(arguments)` of a class with a host ancestor, the result's
    /// class: runs its constructor unit and yields the new host object.
    ConstructClass,
    /// `super(arguments)` in such a constructor: runs the base constructor,
    /// after which the instance parameter is the new object.
    SuperConstruct,
}

impl OperationKind {
    pub(crate) fn child_regions(&self) -> impl Iterator<Item = RegionId> {
        let children = match *self {
            Self::ShortCircuit { right, .. } => [Some(right), None, None],
            Self::Select { yes, no } => [Some(yes), Some(no), None],
            Self::If { yes, no } => [Some(yes), no, None],
            Self::Loop { test, body, update } => [Some(test), Some(body), Some(update)],
            Self::Try {
                body,
                catch,
                finally,
            } => [Some(body), catch.map(|(_, r)| r), finally],
            Self::Block(body) | Self::ForIn { body, .. } | Self::ForOf { body, .. } => {
                [Some(body), None, None]
            }
            _ => [None, None, None],
        };
        children.into_iter().flatten()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod implementations_tests;
#[cfg(test)]
mod javascript_tests;

#[cfg(test)]
mod verify_tests;

#[cfg(test)]
mod facts_tests;

#[cfg(test)]
mod uses_tests;

#[cfg(test)]
mod publication_tests;

#[cfg(test)]
mod compilation_facts_tests;
#[cfg(test)]
mod helper_family_tests;
#[cfg(test)]
mod helper_javascript_tests;
#[cfg(test)]
mod record_family_tests;
#[cfg(test)]
mod record_javascript_tests;
#[cfg(test)]
mod target_contract_tests;

#[cfg(test)]
mod demand_output_tests;

#[cfg(test)]
mod demand_javascript_tests;

#[cfg(test)]
mod string_demand_tests;
#[cfg(test)]
mod string_family_tests;
#[cfg(test)]
mod string_javascript_tests;
#[cfg(test)]
mod string_publication_tests;

#[cfg(test)]
mod artifact_lifetime_tests;
#[cfg(test)]
mod demand_occurrence_tests;
#[cfg(test)]
mod formation_budget_tests;
#[cfg(test)]
mod output_tactics_tests;
#[cfg(test)]
mod search_staged_tests;
#[cfg(test)]
mod search_tests;

#[cfg(test)]
mod search_target_reuse_tests;
#[cfg(test)]
mod value_placement_tests;

#[cfg(test)]
mod value_place_tests;

#[cfg(test)]
mod check_place_tests;

#[cfg(test)]
mod value_place_budget_tests;

#[cfg(test)]
mod value_struct_facts_tests;

#[cfg(test)]
mod native_struct_tests;

#[cfg(test)]
mod javascript_struct_tests;

#[cfg(test)]
mod javascript_struct_boundary_tests;

#[cfg(test)]
mod parameter_contract_tests;

#[cfg(test)]
mod function_layout_tests;

#[cfg(test)]
mod function_publication_tests;

#[cfg(test)]
mod product_call_javascript_tests;

#[cfg(test)]
mod function_demand_tests;

#[cfg(test)]
mod integrated_javascript_tests;

#[cfg(test)]
mod publication_key_budget_tests;

#[cfg(test)]
mod product_reference_javascript_tests;
#[cfg(test)]
mod search_reference_runtime_tests;

#[cfg(test)]
mod cell_location_demand_tests;

#[cfg(test)]
mod reference_support_tests;

#[cfg(test)]
mod reference_support_demand_tests;

#[cfg(test)]
mod facts_return_origin_tests;

#[cfg(test)]
mod generic_call_contract_tests;

#[cfg(test)]
mod generic_product_javascript_tests;
#[cfg(test)]
mod generic_product_tests;
#[cfg(test)]
mod generic_transport_tests;

#[cfg(test)]
mod generic_inline_budget_tests;

#[cfg(test)]
mod search_string_group_tests;

#[cfg(test)]
mod search_naming_neighborhood_tests;
