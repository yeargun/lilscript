//! Checked language meaning, before choosing a JavaScript or native layout.
//!
//! Units own ordered operations and structured completion. Values name results;
//! cells name mutable storage. Neither is an emitted identifier. The frontend
//! conversion consumes checker facts once and does not retain an AST or CFG.
//!
//! The target files (`javascript*.rs`, `native*.rs`) live here for now:
//! JavaScript formation moves to `crate::js` in M8, native to `src/native/` in M11.

mod activation;
mod aggregates;
mod ambient;
mod artifact_provenance;
mod artifacts;
pub mod call_graph;
mod callable_inputs;
mod cell_ssa;
mod classes;
mod dataflow;
mod defaults;
mod demand;
pub mod effects;
mod enums;
mod entries;
pub mod facts;
mod from_source;
mod function_layout;
mod helper_family;
pub mod ids;
mod implementation_identity;
mod implementations;
pub mod initialization;
mod javascript;
mod module_contract;
mod native;
mod native_memory;
mod native_plan;
mod native_runtime;
mod native_string_runtime;
mod physical_storage;
#[cfg(test)]
mod physical_storage_tests;
mod private_fields;
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
pub(crate) mod ranges;
mod record_family;
mod rewrite_lineage;
pub(crate) mod rules;
mod schema;
mod search_entries;
mod search_opportunities;
pub mod storage;
mod string_family;
pub mod uses;
mod value_placement;
mod verify;
pub mod views;

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
mod call_graph_tests;

#[cfg(test)]
mod effects_tests;

#[cfg(test)]
mod initialization_tests;

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
/// The rule-free conversions, for tests that inspect conversion's own output.
#[cfg(test)]
pub(crate) use from_source::{from_checked_modules_admitted, from_checked_source_admitted};
pub(crate) use from_source::{
    from_checked_modules_with_rules, from_checked_source_with_rules, ConversionError,
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
    /// Set on the constructor of a class kept as a JavaScript class (its
    /// identity is observed: `ClassDefinition::observed`): that class. The
    /// body is the class's constructor. Its first parameter is the instance:
    /// `this`, bound on entry, or once `SuperConstruct` has run in a derived
    /// class.
    pub constructor_of: Option<NominalId>,
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
    /// For a private callable whose reflected arity is unobservable, the
    /// first parameter that JavaScript may spell with native default syntax.
    /// This is target-boundary metadata: the program keeps its explicit
    /// default operations, and native lowering ignores it.
    pub native_default_length: Option<u32>,
    /// First declaration default affecting observable JavaScript length.
    pub declared_length: Option<u32>,
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
/// `JsValue`). Such a call preserves omitted arguments. A source callee also
/// receives the supplied count and applies its declaration's defaults itself.
/// This classification describes direct syntax, not the identity of an aliased
/// function value after forwarding.
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
            Place::Member { receiver, .. } | Place::ClassField { receiver, .. } => {
                host_receiver(program, data, receiver)
            }
            _ => false,
        },
        CallTarget::Builtin(_) | CallTarget::Intrinsic { .. } => false,
    }
}

/// Whether a value is a host object: a `JsValue` or an extern class
/// instance. Its members are host properties and methods.
pub(crate) fn host_receiver(program: &Program<'_>, data: &UnitData, receiver: ValueId) -> bool {
    match &program.types[data.values[receiver.index()].ty.index()] {
        Type::Dynamic => true,
        Type::Class(declaration) | Type::ClassInstance { declaration, .. } => program
            .class(declaration.identity)
            .is_some_and(|class| class.external),
        _ => false,
    }
}

impl UnitData {
    pub fn empty(kind: UnitKind) -> Self {
        Self {
            kind,
            suspension: Suspension::None,
            constructor_of: None,
            module: ModuleId::from_index(0).unwrap(),
            instantiation_prefix: 0,
            function_name: None,
            callable_type: None,
            native_default_length: None,
            declared_length: None,
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
    /// Native bounds traps or JavaScript development checks are observable
    /// before optimization. Immutable under the table revision and preserved
    /// by every candidate; a dead result cannot erase a required bounds check.
    trap_index_reads: bool,
    source_contract: crate::config::LanguageConfig,
    absence_abi: bool,
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
    /// The program's entries, in name order (plan M3.3): each is a root of
    /// the module graph and publishes its module's exports.
    entries: Arc<Vec<ProgramEntry>>,
    /// The public surface: every entry's exports, a range of the export
    /// table. One entry's is its module's range; several entries' is their
    /// concatenation, appended once, so one cell exported by two entries
    /// appears twice and is still one cell.
    public: std::ops::Range<usize>,
    /// Derived views (call graph, effect summaries), filled on demand.
    views: views::ProgramViews,
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
    /// A class declaration by its identity. The table is in identity order.
    pub fn class(&self, identity: NominalId) -> Option<&ClassDefinition> {
        self.class_index(identity).map(|index| &self.classes[index])
    }
    /// An enum declaration by its identity. The table is in identity order.
    pub fn enum_definition(&self, identity: NominalId) -> Option<&EnumDefinition> {
        self.enums
            .binary_search_by_key(&identity, |definition| definition.identity)
            .ok()
            .map(|index| &self.enums[index])
    }
    /// A class's position in `classes()`, by its identity.
    pub fn class_index(&self, identity: NominalId) -> Option<usize> {
        self.classes
            .binary_search_by_key(&identity, |class| class.identity)
            .ok()
    }
    /// A class field's spelling and declared type, by its identity.
    pub fn class_field(&self, field: FieldRef) -> Option<(StringId, TypeId)> {
        self.class(field.nominal)?
            .fields
            .get(field.slot as usize)
            .copied()
    }
    /// Every entry's public exports (see `public`).
    pub fn exports(&self) -> &[Export] {
        self.exports.get(self.public.clone()).unwrap_or(&[])
    }
    /// The program's entries, in name order.
    pub fn entries(&self) -> &[ProgramEntry] {
        &self.entries
    }
    /// One entry's public exports: its module's.
    pub fn entry_exports(&self, entry: usize) -> &[Export] {
        self.entries
            .get(entry)
            .and_then(|entry| self.modules.get(entry.module.index()))
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
                InterfaceTarget::Type(_) => None,
            })
    }
    pub fn modules(&self) -> &[ModuleInterface] {
        &self.modules
    }
    /// The first entry's module: the root diagnostics cite.
    pub fn entry_module(&self) -> ModuleId {
        self.entries[0].module
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
    /// a synthetic cell, an optional source binding supplying provenance.
    /// Expression-level temporaries have no checked binding to borrow.
    pub source_symbol: Option<SymbolId>,
    pub name: String,
    pub ty: TypeId,
    pub owner: UnitId,
    pub region: RegionId,
    pub declaration: Span,
    /// Written after its initialization: by an assignment, a mutable
    /// reference, or a parameter default.
    pub reassigned: bool,
    /// The checker's definite-initialization proof (M4.3) does not cover every
    /// occurrence: a read or write may run before the binding is initialized
    /// (in its own initializer, in a function, from another module, in a
    /// parameter default). The initialization owner decides those.
    pub observable_before_initialization: bool,
    pub binding: CellBinding,
    /// Conversion-owned storage with no checked symbol of its own (a
    /// `for...of` array and counter). Synthetic cells follow every checked one.
    pub synthetic: bool,
    /// The declaration is `pure`: a function or method whose summary the
    /// contract check holds to it, or a trusted `pure extern`.
    pub declared_pure: bool,
    /// The declaration is `debug` (R15): a direct call of it is strippable
    /// logging or an assertion, which `strip_debug` drops.
    pub debug: bool,
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
    pub type_parameters: Vec<crate::check::TypeParameterId>,
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
    /// A shape's immutable, declared literal identity (R13).
    pub discriminant: Option<(u32, Constant)>,
    pub shape: bool,
    /// Accessor slots of a shape, aligned with its field schema.
    pub accessors: Vec<bool>,
    /// The checked identity. Two modules' private classes of one name are two
    /// definitions; `name` is display data, and an ABI name only where the
    /// class meets the host (an extern class, a host-derived class's `name`).
    pub identity: NominalId,
    pub name: String,
    /// The module whose scope declares the class.
    pub module: ModuleId,
    pub external: bool,
    /// The checked reflected-set closure exposes this class's storage keys.
    /// Kept independently from constructor identity (`observed`).
    pub reflected: bool,
    pub base: Option<NominalId>,
    /// The class's own type parameters, and its base's type arguments in
    /// terms of them (empty for a non-generic base): an upcast of an
    /// instance substitutes these up the chain.
    pub type_params: Vec<crate::check::TypeParameterId>,
    pub base_arguments: Vec<TypeId>,
    /// An extern class's host constructor, as its checked function type:
    /// what `super(...)` of a subclass calls. Internal classes have none.
    pub constructor: Option<TypeId>,
    /// Flattened field names and types, base fields first; a field's slot is
    /// its index here, in the class that declares it (`FieldRef`).
    pub fields: Vec<(StringId, TypeId)>,
    /// The class's identity is observable, so it stays a JavaScript class:
    /// construction runs its constructor unit (`ConstructClass`), and a
    /// derived constructor calls its base's (`SuperConstruct`).
    pub observed: bool,
    /// An internal observed class's constructor, as a value: the cell of its
    /// constructor unit, which `export constructor` publishes.
    pub value: Option<CellId>,
    /// JavaScript callers reach the class's constructor and prototype: it or
    /// a class extending it is published. Its constructor and prototype
    /// methods are entry points with unknown callers.
    pub published: bool,
    /// The methods a JavaScript caller reaches on the prototype: every method
    /// the class declares, when it or a class extending it is published.
    /// Each is the spelling and the cell of its (static) method unit.
    pub prototype: Vec<(StringId, CellId)>,
}

/// A class field by identity: the class that declares it and its slot in
/// that class's flattened (base-first) field list. Every class instance
/// access is a `Place::ClassField` of one; the field's spelling is display
/// and ABI data on its class definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldRef {
    pub nominal: NominalId,
    pub slot: u32,
}

#[derive(Debug, Clone)]
pub struct EnumDefinition {
    pub identity: NominalId,
    pub name: String,
    pub abi: crate::ast::EnumAbi,
    pub flag_mask: u32,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub value: Constant,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub identity: NominalMemberId,
    pub owner: NominalId,
    pub name: String,
    pub ty: TypeId,
    pub index: usize,
}

/// What an export names: a runtime value's cell, or a nominal type, which
/// has no storage, binding or execution event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterfaceTarget {
    Value(CellId),
    Type(NominalId),
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub target: InterfaceTarget,
}

/// One entry of a program: a named root of its module graph whose module's
/// exports are a public surface (plan M3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntry {
    pub name: String,
    pub module: ModuleId,
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
    /// `...value`: an iterable spread into a host call's arguments (R7).
    /// Only a call that reaches JavaScript takes one.
    Spread(ValueId),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// A field of a class instance (or of an extern class's host object):
    /// the evaluated reference and the field's identity. It reads and writes
    /// the instance's own data property of the field's spelling.
    ClassField {
        receiver: ValueId,
        field: FieldRef,
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
    /// An internal class instance: an object holding the class's flattened
    /// fields in order. The keys are those fields' spellings.
    Instance {
        class: NominalId,
        keys: Vec<StringId>,
    },
    Struct(NominalId),
}

impl AllocationKind {
    /// The property keys of an object an allocation creates, in order: a JS
    /// object literal's or a class instance's fields.
    pub fn object_keys(&self) -> Option<&[StringId]> {
        match self {
            Self::Object(keys) | Self::Instance { keys, .. } => Some(keys),
            _ => None,
        }
    }
}

/// One call owns its target and checked invocation contract. The signature is
/// shared through the program type table; arguments remain ordered operations.
#[derive(Debug, Clone)]
pub struct CallSite {
    pub target: CallTarget,
    pub contract: CallContract,
    pub arguments: ArgumentRange,
    /// Trailing materialized scalar defaults that JavaScript may omit at this
    /// call. The arguments remain in the semantic program (and in native
    /// lowering); formation drops only their target expressions. Keeping the
    /// count here also prevents the callee's explicit default guard from
    /// being folded as though every emitted call supplied the value.
    pub omit_trailing: u32,
    /// The source calls a `debug` declaration by its name (R15): the call is
    /// strippable logging, which `strip_debug` drops. Decided once, at
    /// conversion, so no rewrite that exposes a callee makes a call one.
    pub debug: bool,
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
    Enum { declaration: NominalId, operation: crate::primitive::EnumOperation },
    Initialize(CellId),
    /// `let x;` (R3): the cell exists, holding no value until its first
    /// store, which the checker proves precedes every read.
    Declare(CellId),
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
    /// Whether the operand is `undefined`. Declaration guards also retain the
    /// parameter ordinal so native calls can transport omission independently
    /// of the initialized parameter's value representation.
    IsUndefined {
        parameter: Option<u32>,
        /// Unified absence defaults also accept null. This is operation data,
        /// so all evaluators, refinements and replayed candidates agree.
        nullish: bool,
    },
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

#[cfg(test)]
mod nominal_identity_tests;
