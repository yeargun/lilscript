use crate::ast::ExprKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::primitive::ParameterPassing;
use std::fmt;

use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};
use indexmap::IndexMap;

use crate::ast::{
    Argument, ArrayBinding, ArrayElement, ArrowBody, AssignmentOp, BinaryOp, ClassDecl,
    ClassMember, ConstructorDecl, Expr, ExternClassMember, ExternDecl, ForInitializer,
    FunctionDecl, Ident, Item, MatchPattern, Program, RecordElement, SourceNodeId, Stmt,
    StructDecl, TemplatePart, TypeKind, TypeRef, UnaryOp, UpdateOp, VarDecl,
};
use crate::span::Span;
use crate::typed_array::TypedArrayKind;

pub(crate) mod binary_types;
mod modules;
mod struct_cycles;
pub(crate) mod type_admission;
pub(crate) mod type_payload;
pub(crate) mod type_relation;
pub(crate) mod type_substitution;
pub use modules::{
    analyze_modules, CheckedModules, InterfaceTarget, ModuleExport, ModuleImport, ModuleInterface,
    ModuleSemanticError,
};
pub(crate) use modules::{with_analyzed_modules, AdmittedModuleSemanticError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeState {
    LocalOnly,
    EscapesToTypedCode,
    EscapesToUntypedBoundary,
}

/// A call whose identity was resolved by semantic analysis rather than by a
/// runtime binding. Keeping this fact in the semantic model prevents later
/// stages from guessing from identifier spelling (and therefore accidentally
/// treating a shadowed user binding as a language intrinsic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinCall {
    Print,
    MathImul,
    ObjectKeys,
    ObjectValues,
    ObjectHasOwn,
    ObjectAssign,
    JsonStringify,
    JsonParse,
    TaskResolve,
    TaskReject,
    TaskAll,
    JsObject,
    JsArray,
    JsUndefined,
    JsTypeOf,
    JsIsNullish,
    JsIsFalse,
    JsIsUndefined,
    JsString,
    JsNumber,
    JsAdd,
    JsMod,
    JsLessThan,
    JsLessThanOrEqual,
    JsGreaterThan,
    JsGreaterThanOrEqual,
    JsAssume,
    JsStrictEqual,
    JsStrictNotEqual,
    JsOr,
    JsAnd,
    JsCall,
    JsConstruct,
    JsInvoke,
    JsApply,
    JsMethod0,
    JsMethod1,
    JsMethod2,
    JsMethod3,
    JsMethod4,
    JsMethod5,
    JsMethod6,
    JsMethod7,
    JsMethod8,
    JsMethod9,
    JsMethod10,
    JsMethodRest,
    JsStaticRest,
    JsGet,
    JsSet,
    JsDelete,
    JsHas,
    JsIn,
    JsBox,
    JsArrayPush,
    JsArrayPop,
    JsArraySlice,
    JsArrayIndexOf,
    JsArraySort,
    JsArraySplice,
    JsArrayConcatApply,
    JsArrayJoin,
    JsArrayShift,
    JsArrayUnshift,
    JsIsArray,
    JsStringSlice,
    JsStringIndexOf,
    JsStringReplace,
    JsStringMatch,
    JsStringSplit,
    JsRegexTest,
    JsRegexExec,
    JsEncodeURI,
    JsEncodeURIComponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type<'src> {
    Int,
    Float,
    Enum(&'src str),
    String,
    Bool,
    Null,
    Void,
    Array(Box<Type<'src>>),
    Record(Box<Type<'src>>),
    Map(Box<Type<'src>>, Box<Type<'src>>),
    Set(Box<Type<'src>>),
    ArrayBuffer,
    SharedArrayBuffer,
    Int8Array,
    Uint8Array,
    Uint8ClampedArray,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    Float32Array,
    Float64Array,
    Symbol,
    Regex,
    Task(Box<Type<'src>>),
    Generator(Box<Type<'src>>),
    ModuleNamespace(u32),
    ModuleLoadError,
    Nullable(Box<Type<'src>>),
    Union(Vec<Type<'src>>),
    Struct(StructType<'src>),
    Class(&'src str),
    StructInstance {
        declaration: StructType<'src>,
        args: Vec<Type<'src>>,
    },
    ClassInstance {
        name: &'src str,
        args: Vec<Type<'src>>,
    },
    TypeParameter(&'src str),
    Function(FunctionType<'src>),
    GenericFunction(GenericFunctionType<'src>),
}

impl Type<'_> {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }

    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    /// Borrow existing type nodes only. Nominal schema fields remain the
    /// caller's table traversal; no cloned type/default compatibility view.
    pub fn contains_mutable_reference_parameters(&self) -> bool {
        fn nested(ty: &Type<'_>) -> bool {
            matches!(
                ty,
                Type::Array(_)
                    | Type::Record(_)
                    | Type::Set(_)
                    | Type::Task(_)
                    | Type::Generator(_)
                    | Type::Nullable(_)
                    | Type::Map(_, _)
                    | Type::Union(_)
                    | Type::StructInstance { .. }
                    | Type::ClassInstance { .. }
                    | Type::Function(_)
                    | Type::GenericFunction(_)
            )
        }
        let mut pending = Vec::new();
        let mut current = self;
        loop {
            match current {
                Self::Array(value)
                | Self::Record(value)
                | Self::Set(value)
                | Self::Task(value)
                | Self::Generator(value)
                | Self::Nullable(value) => {
                    current = value;
                    continue;
                }
                Self::Map(key, value) => {
                    if nested(value) {
                        pending.push(value.as_ref());
                    }
                    current = key;
                    continue;
                }
                Self::Union(values)
                | Self::StructInstance { args: values, .. }
                | Self::ClassInstance { args: values, .. } => {
                    pending.extend(values.iter().filter(|ty| nested(ty)))
                }
                Self::Function(signature) => {
                    for parameter in &signature.params {
                        if parameter.passing == ParameterPassing::MutableReference {
                            return true;
                        }
                        if nested(&parameter.ty) {
                            pending.push(&parameter.ty);
                        }
                    }
                    current = &signature.return_type;
                    continue;
                }
                Self::GenericFunction(function) => {
                    let signature = &function.signature;
                    for parameter in &signature.params {
                        if parameter.passing == ParameterPassing::MutableReference {
                            return true;
                        }
                        if nested(&parameter.ty) {
                            pending.push(&parameter.ty);
                        }
                    }
                    current = &signature.return_type;
                    continue;
                }
                _ => {}
            }
            let Some(next) = pending.pop() else {
                return false;
            };
            current = next;
        }
    }
}

impl fmt::Display for Type<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => f.write_str("int"),
            Self::Float => f.write_str("float"),
            Self::Enum(name) => f.write_str(name),
            Self::String => f.write_str("string"),
            Self::Bool => f.write_str("bool"),
            Self::Null => f.write_str("null"),
            Self::Void => f.write_str("void"),
            Self::Array(element) => match element.as_ref() {
                Self::Union(_) => write!(f, "({element})[]"),
                _ => write!(f, "{element}[]"),
            },
            Self::Record(value) => write!(f, "Record<{value}>"),
            Self::Map(key, value) => write!(f, "Map<{key}, {value}>"),
            Self::Set(element) => write!(f, "Set<{element}>"),
            Self::ArrayBuffer => f.write_str("ArrayBuffer"),
            Self::SharedArrayBuffer => f.write_str("SharedArrayBuffer"),
            Self::Int8Array => f.write_str("Int8Array"),
            Self::Uint8Array => f.write_str("Uint8Array"),
            Self::Uint8ClampedArray => f.write_str("Uint8ClampedArray"),
            Self::Int16Array => f.write_str("Int16Array"),
            Self::Uint16Array => f.write_str("Uint16Array"),
            Self::Int32Array => f.write_str("Int32Array"),
            Self::Uint32Array => f.write_str("Uint32Array"),
            Self::Float32Array => f.write_str("Float32Array"),
            Self::Float64Array => f.write_str("Float64Array"),
            Self::Symbol => f.write_str("Symbol"),
            Self::Regex => f.write_str("Regex"),
            Self::Task(value) => write!(f, "Task<{value}>"),
            Self::Generator(value) => write!(f, "Generator<{value}>"),
            Self::ModuleNamespace(module) => write!(f, "module#{module}"),
            Self::ModuleLoadError => f.write_str("ModuleLoadError"),
            Self::Nullable(inner) => match inner.as_ref() {
                Self::Union(_) => write!(f, "({inner})?"),
                _ => write!(f, "{inner}?"),
            },
            Self::Union(members) => {
                for (index, member) in members.iter().enumerate() {
                    if index != 0 {
                        f.write_str(" | ")?;
                    }
                    write!(f, "{member}")?;
                }
                Ok(())
            }
            Self::Struct(declaration) => f.write_str(declaration.name),
            Self::Class(name) => f.write_str(name),
            Self::StructInstance {
                declaration: StructType { name, .. },
                args,
            }
            | Self::ClassInstance { name, args } => {
                write!(f, "{name}<")?;
                for (index, argument) in args.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str(">")
            }
            Self::TypeParameter("$js") => f.write_str("JsValue"),
            Self::TypeParameter(name) => f.write_str(name),
            Self::Function(signature) => {
                f.write_str("function(")?;
                for (index, parameter) in signature.params.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    if parameter.passing == ParameterPassing::MutableReference {
                        f.write_str("mutable-reference ")?;
                    }
                    write!(f, "{}", parameter.ty)?;
                }
                write!(f, ") -> {}", signature.return_type)
            }
            Self::GenericFunction(function) => {
                f.write_str("function<")?;
                for (index, parameter) in function.type_params.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(parameter)?;
                }
                write!(f, ">({})", Type::Function(function.signature.clone()))
            }
        }
    }
}

/// Shared callable metadata keeps primitive type slots compact. Cloning a
/// type retains its signature; contextual finalization explicitly detaches a
/// shared payload before changing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType<'src>(std::sync::Arc<FunctionSignature<'src>>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature<'src> {
    pub params: Vec<FunctionParameter<'src>>,
    pub return_type: Box<Type<'src>>,
}

/// One parameter owns its transfer convention and optional default together
/// with its type. Defaults never belong to mutable-reference parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionParameter<'src> {
    pub ty: Type<'src>,
    pub passing: ParameterPassing,
    pub default: Option<DefaultValue<'src>>,
}
impl<'src> FunctionParameter<'src> {
    pub fn value(ty: Type<'src>) -> Self {
        Self {
            ty,
            passing: ParameterPassing::Value,
            default: None,
        }
    }
    pub fn defaulted(ty: Type<'src>, default: DefaultValue<'src>) -> Self {
        Self {
            ty,
            passing: ParameterPassing::Value,
            default: Some(default),
        }
    }
}

impl FunctionSignature<'_> {
    /// Structural parameter validity. Default expression types and binding
    /// identities remain the source checker's separate semantic obligation.
    pub fn validate_parameters(&self) -> Result<(), &'static str> {
        let mut optional = false;
        for parameter in &self.params {
            if parameter.passing == ParameterPassing::MutableReference
                && parameter.default.is_some()
            {
                return Err("mutable-reference parameters cannot have defaults");
            }
            if parameter.default.is_some() {
                optional = true;
            } else if optional {
                return Err("required parameters cannot follow defaulted parameters");
            }
        }
        Ok(())
    }
}

impl<'src> FunctionType<'src> {
    pub fn new(signature: FunctionSignature<'src>) -> Self {
        Self(std::sync::Arc::new(signature))
    }

    fn make_mut(&mut self) -> &mut FunctionSignature<'src> {
        std::sync::Arc::make_mut(&mut self.0)
    }
}

impl<'src> std::ops::Deref for FunctionType<'src> {
    type Target = FunctionSignature<'src>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FunctionType<'_> {
    pub fn required_params(&self) -> usize {
        self.params
            .iter()
            .position(|parameter| parameter.default.is_some())
            .unwrap_or(self.params.len())
    }

    pub fn accepts_arity(&self, arity: usize) -> bool {
        arity >= self.required_params() && arity <= self.params.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultValue<'src> {
    Int(i64),
    Float(u64),
    String(&'src str),
    Bool(bool),
    Null,
    /// The exact unshadowed `JS.undefined()` language primitive.
    Undefined,
    /// A non-parameter identifier default resolved to its exact semantic
    /// binding. Call-site lowering must never recover this from its spelling.
    Symbol(SymbolId),
    /// An identifier default bound to an earlier parameter of the same
    /// callable. The index preserves binding identity through detached
    /// function types so lowering can reuse that call's already-evaluated
    /// actual argument rather than resolving the spelling in the caller.
    Parameter(usize),
    /// Declaration signatures are collected before their default expressions
    /// are analyzed. This source occurrence is replaced with `Symbol`
    /// before the completed semantic model is returned.
    PendingIdentifier {
        expression: SourceNodeId,
        span: Span,
    },
    /// A syntactic `JS.undefined()` candidate awaiting semantic builtin
    /// resolution. Its spelling alone is never accepted as value proof.
    PendingUndefined {
        expression: SourceNodeId,
        span: Span,
    },
    Array(Vec<DefaultValue<'src>>),
    Arrow(SourceNodeId),
    Struct {
        name: &'src str,
        values: Vec<DefaultValue<'src>>,
    },
    NewClass {
        name: &'src str,
        args: Vec<DefaultValue<'src>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericFunctionType<'src> {
    pub type_params: Vec<&'src str>,
    pub signature: FunctionType<'src>,
}

/// A program-local declaration handle. The two existing nominal registries
/// own their definitions; the low tag distinguishes their indexed namespaces.
/// No source spelling or diagnostic span is needed to dereference this handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NominalId(std::num::NonZeroU32);

impl NominalId {
    fn new(index: usize, class: bool) -> Self {
        let encoded = u32::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(2))
            .and_then(|index| index.checked_add(1 + u32::from(class)))
            .and_then(std::num::NonZeroU32::new)
            .expect("nominal declaration capacity exceeded");
        Self(encoded)
    }

    pub(crate) fn index(self) -> usize {
        ((self.0.get() - 1) / 2) as usize
    }
    pub fn is_class(self) -> bool {
        (self.0.get() - 1) & 1 != 0
    }
}

/// A canonical declaration reference within one checked declaration owner.
/// The spelling is diagnostic metadata; aliases never change identity.
#[derive(Debug, Clone, Copy)]
pub struct StructType<'src> {
    pub identity: NominalId,
    pub name: &'src str,
}

impl PartialEq for StructType<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl Eq for StructType<'_> {}
impl std::hash::Hash for StructType<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.identity, state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NominalMemberId(std::num::NonZeroU32);

impl NominalMemberId {
    fn new(index: usize) -> Self {
        Self(
            u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .and_then(std::num::NonZeroU32::new)
                .expect("nominal member capacity exceeded"),
        )
    }
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

#[derive(Debug, Clone, Copy)]
enum MemberSlot {
    Field(u32),
    Method(u32),
}

impl MemberSlot {
    fn field(index: usize) -> Self {
        Self::Field(u32::try_from(index).expect("nominal field capacity exceeded"))
    }
    fn method(index: usize) -> Self {
        Self::Method(u32::try_from(index).expect("nominal method capacity exceeded"))
    }
}

#[derive(Debug, Clone, Copy)]
struct MemberDefinition {
    owner: NominalId,
    slot: MemberSlot,
}

/// A borrow of the declaration's existing facts, not a copied member table.
#[derive(Debug, Clone, Copy)]
pub enum NominalMember<'sem, 'src> {
    Field {
        owner: NominalId,
        field: &'sem FieldInfo<'src>,
    },
    Method {
        owner: NominalId,
        name: &'src str,
        method: &'sem MethodInfo<'src>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo<'src> {
    pub member: NominalMemberId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub index: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructInfo<'src> {
    pub declaration: StructType<'src>,
    pub module: Option<crate::module::ModuleId>,
    pub type_params: Vec<&'src str>,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassInfo<'src> {
    pub name: &'src str,
    pub type_params: Vec<&'src str>,
    pub base: Option<Type<'src>>,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub methods: IndexMap<&'src str, MethodInfo<'src>>,
    pub constructor: Option<FunctionType<'src>>,
    pub external: bool,
    pub object: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo<'src> {
    pub member: NominalMemberId,
    pub owner: &'src str,
    pub type_params: Vec<&'src str>,
    pub signature: FunctionType<'src>,
    pub declared_pure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumInfo<'src> {
    pub name: &'src str,
    pub variants: IndexMap<&'src str, i64>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol<'src> {
    pub id: SymbolId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub span: Span,
    pub escape_state: EscapeState,
    /// Classification belongs to the canonical declaration, including extern
    /// aliases shared by several checked modules.
    origin: DeclarationOrigin,
    /// Distinct source spans currently registered to this identity, including
    /// its declaration. Maintained by ModuleFacts::record_identifier across source owners; this
    /// is source-binding knowledge, not a use count for a rewritten program.
    identifier_occurrences: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclarationOrigin {
    Source,
    Foreign,
}

impl Symbol<'_> {
    pub(crate) fn is_foreign(&self) -> bool {
        self.origin == DeclarationOrigin::Foreign
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    pub span: Span,
    pub message: String,
}

impl SemanticError {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for SemanticError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdmittedSemanticError {
    Semantic(SemanticError),
    Resources(AllocationError),
}

impl From<AllocationError> for AdmittedSemanticError {
    fn from(error: AllocationError) -> Self {
        Self::Resources(error)
    }
}

impl From<SemanticError> for AdmittedSemanticError {
    fn from(error: SemanticError) -> Self {
        Self::Semantic(error)
    }
}

impl AdmittedSemanticError {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self::Semantic(SemanticError::new(span, message))
    }
}

/// Resolution belongs to the checked source occurrence, independently of its
/// diagnostic location or the target operation that eventually represents it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExpressionResolution {
    #[default]
    None,
    Binding(SymbolId),
    Builtin(BuiltinCall),
    Primitive(crate::primitive::ResolvedIntrinsic),
    NominalMember(NominalMemberId),
    NominalConstruction(NominalId),
}

/// The original generic call's resolved arguments and effective signature.
/// This belongs to one checked SourceNodeId; callee declaration identity stays
/// on that callee's existing expression type.
#[derive(Debug, Clone)]
pub struct CheckedCallInstantiation<'src> {
    pub type_arguments: Vec<Type<'src>>,
    pub signature: FunctionType<'src>,
}

#[derive(Clone, Copy, Default)]
struct SourceInfo<'ast, 'src> {
    expression: Option<&'ast Expr<'ast, 'src>>,
    resolution: ExpressionResolution,
}

impl fmt::Debug for SourceInfo<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A source borrow is provenance, not another recursively owned tree.
        f.debug_struct("SourceInfo")
            .field(
                "source",
                &self.expression.map(|expr| (expr.id, expr.span())),
            )
            .field("resolution", &self.resolution)
            .finish()
    }
}

/// Canonical declarations are owned once, including all local symbols.
#[derive(Debug, Clone, Default)]
struct DeclarationTables<'src> {
    assigned_symbols: AHashSet<SymbolId>,
    symbols: Vec<Symbol<'src>>,
    structs: Vec<StructInfo<'src>>,
    classes: IndexMap<&'src str, ClassInfo<'src>>,
    nominal_members: Vec<MemberDefinition>,
    enums: AHashMap<&'src str, EnumInfo<'src>>,
    symbol_modules: Vec<Option<crate::module::ModuleId>>,
    foreign_symbols: AHashMap<&'src str, (SymbolId, bool)>,
}

/// Source-node and span indexes belong to exactly one original source.
#[derive(Debug, Clone)]
struct ModuleFacts<'ast, 'src> {
    source: crate::ast::SourceIdentity,
    expression_types: Vec<Option<Type<'src>>>,
    source_info: Vec<SourceInfo<'ast, 'src>>,
    call_instantiations: AHashMap<SourceNodeId, CheckedCallInstantiation<'src>>,
    struct_bindings: AHashMap<&'src str, NominalId>,
    optional_present_types: AHashMap<Span, Type<'src>>,
    type_check_types: AHashMap<Span, Type<'src>>,
    binding_types: AHashMap<Span, BindingType<'src>>,
    identifier_symbols: AHashMap<Span, SymbolId>,
    enum_variant_values: AHashMap<Span, i64>,
    dynamic_import_modules: AHashMap<Span, u32>,
    module_exports: AHashMap<u32, AHashMap<&'src str, &'src str>>,
    /// Direct module checking: a dynamically imported module's runtime
    /// exports, resolved through its interface rather than a merged scope.
    dynamic_export_symbols: AHashMap<(u32, &'src str), SymbolId>,
    used_dynamic_exports: AHashSet<(u32, &'src str)>,
}

// Value declarations and import aliases share the canonical symbol's payload.
// Type-only bindings still need a type without introducing a value identity.
#[derive(Debug, Clone)]
enum BindingType<'src> {
    Symbol(SymbolId),
    Inline(Type<'src>),
}

#[cfg(test)]
thread_local! {
    static LIVE_FACTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// Only the private, same-thread admitted callback owns this test wrapper.
// Public semantic models retain their original layout and cross-thread behavior.
#[cfg(test)]
struct AdmittedFactsOwner<T> {
    value: Option<T>,
    count: usize,
}

#[cfg(test)]
impl<T> AdmittedFactsOwner<T> {
    fn new(value: T, count: usize) -> Self {
        LIVE_FACTS.with(|live| live.set(live.get() + count));
        Self {
            value: Some(value),
            count,
        }
    }
}

#[cfg(test)]
impl<T> std::ops::Deref for AdmittedFactsOwner<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_ref().expect("live admitted checker result")
    }
}

#[cfg(test)]
impl<T> Drop for AdmittedFactsOwner<T> {
    fn drop(&mut self) {
        drop(self.value.take());
        LIVE_FACTS.with(|live| live.set(live.get() - self.count));
    }
}

#[cfg(test)]
fn live_facts_for_test() -> usize {
    LIVE_FACTS.with(std::cell::Cell::get)
}

#[derive(Debug, Clone)]
pub struct SemanticModel<'ast, 'src> {
    declarations: DeclarationTables<'src>,
    facts: ModuleFacts<'ast, 'src>,
}

/// A read-only source qualification over the compilation's shared declarations.
#[derive(Debug, Clone, Copy)]
pub struct SemanticView<'view, 'ast, 'src> {
    declarations: &'view DeclarationTables<'src>,
    facts: &'view ModuleFacts<'ast, 'src>,
}

impl<'ast, 'src> ModuleFacts<'ast, 'src> {
    fn new(source: &crate::ast::SourceIdentity) -> Self {
        Self::from_buffers(
            source,
            vec![None; source.len()],
            vec![SourceInfo::default(); source.len()],
        )
    }

    fn new_admitted(
        source: &crate::ast::SourceIdentity,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.work(
            crate::compilation_policy::WorkKind::Render,
            u64::try_from(source.len()).map_err(|_| AllocationError::Capacity)?,
        )?;
        let mut expression_types = budget.vector(AllocationClass::Scratch, source.len())?;
        expression_types.resize_with(source.len(), || None);
        let source_info = budget.filled(
            AllocationClass::Scratch,
            source.len(),
            SourceInfo::default(),
        )?;
        Ok(Self::from_buffers(source, expression_types, source_info))
    }

    fn from_buffers(
        source: &crate::ast::SourceIdentity,
        expression_types: Vec<Option<Type<'src>>>,
        source_info: Vec<SourceInfo<'ast, 'src>>,
    ) -> Self {
        Self {
            source: source.clone(),
            expression_types,
            source_info,
            call_instantiations: AHashMap::default(),
            struct_bindings: AHashMap::default(),
            optional_present_types: AHashMap::default(),
            type_check_types: AHashMap::default(),
            binding_types: AHashMap::default(),
            identifier_symbols: AHashMap::default(),
            enum_variant_values: AHashMap::default(),
            dynamic_import_modules: AHashMap::default(),
            module_exports: AHashMap::default(),
            dynamic_export_symbols: AHashMap::default(),
            used_dynamic_exports: AHashSet::default(),
        }
    }

    fn record_identifier(
        &mut self,
        declarations: &mut DeclarationTables<'src>,
        span: Span,
        symbol: SymbolId,
    ) {
        match self.identifier_symbols.insert(span, symbol) {
            Some(previous) if previous == symbol => return,
            Some(previous) => declarations.symbols[previous.0 as usize].identifier_occurrences -= 1,
            None => {}
        }
        declarations.symbols[symbol.0 as usize].identifier_occurrences += 1;
    }
}

impl<'src> DeclarationTables<'src> {
    fn add_symbol(
        &mut self,
        symbol: Symbol<'src>,
        module: Option<crate::module::ModuleId>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<SymbolId, AllocationError> {
        let id = SymbolId(
            u32::try_from(self.symbols.len()).map_err(|_| AllocationError::Capacity)?,
        );
        debug_assert_eq!(symbol.id, id);
        debug_assert_eq!(self.symbols.len(), self.symbol_modules.len());
        budget.reserve_vec(AllocationClass::Scratch, &mut self.symbols, 1)?;
        budget.reserve_vec(AllocationClass::Scratch, &mut self.symbol_modules, 1)?;
        budget.work(crate::compilation_policy::WorkKind::Render, 2)?;
        // Both capacities and both publication operations are admitted before
        // either parallel row becomes visible to the checker.
        self.symbols.push(symbol);
        self.symbol_modules.push(module);
        Ok(id)
    }

    fn declare_member(
        &mut self,
        owner: NominalId,
        slot: MemberSlot,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<NominalMemberId, AllocationError> {
        let id = NominalMemberId::new(self.nominal_members.len());
        budget.push(
            AllocationClass::Scratch,
            &mut self.nominal_members,
            MemberDefinition { owner, slot },
        )?;
        Ok(id)
    }

    #[cfg(any(test, debug_assertions))]
    fn identifier_index_is_consistent<'ast>(&self, facts: &[ModuleFacts<'ast, 'src>]) -> bool {
        let mut observed = vec![0; self.symbols.len()];
        for module in facts {
            for symbol in module.identifier_symbols.values() {
                observed[symbol.0 as usize] += 1;
            }
        }
        self.symbols
            .iter()
            .zip(observed)
            .all(|(symbol, count)| symbol.identifier_occurrences == count)
    }
}

impl<'ast, 'src> SemanticModel<'ast, 'src> {
    /// The first declaration whose type passes a parameter by reference, or
    /// whose signature holds such a callable. Consumers that represent
    /// parameters by value only (the reference interpreter) refuse the
    /// program there.
    pub(crate) fn reference_parameter_span(&self) -> Option<Span> {
        let contains_signature = |signature: &FunctionType<'_>| {
            signature.params.iter().any(|parameter| {
                parameter.passing != crate::primitive::ParameterPassing::Value
                    || parameter.ty.contains_mutable_reference_parameters()
            }) || signature
                .return_type
                .contains_mutable_reference_parameters()
        };
        for symbol in self.symbols() {
            if symbol.ty.contains_mutable_reference_parameters() {
                return Some(symbol.span);
            }
        }
        for definition in self.structs() {
            for field in definition.fields.values() {
                if field.ty.contains_mutable_reference_parameters() {
                    return Some(field.span);
                }
            }
        }
        for definition in self.classes() {
            for field in definition.fields.values() {
                if field.ty.contains_mutable_reference_parameters() {
                    return Some(field.span);
                }
            }
            if definition
                .constructor
                .as_ref()
                .is_some_and(contains_signature)
                || definition
                    .methods
                    .values()
                    .any(|method| contains_signature(&method.signature))
            {
                return Some(definition.span);
            }
        }
        None
    }

    pub fn view(&self) -> SemanticView<'_, 'ast, 'src> {
        SemanticView {
            declarations: &self.declarations,
            facts: &self.facts,
        }
    }

    #[cfg(test)]
    fn record_identifier(&mut self, span: Span, symbol: SymbolId) {
        self.facts
            .record_identifier(&mut self.declarations, span, symbol);
    }

    #[cfg(any(test, debug_assertions))]
    fn identifier_index_is_consistent(&self) -> bool {
        self.declarations
            .identifier_index_is_consistent(std::slice::from_ref(&self.facts))
    }

    pub fn expression_type(&self, id: crate::ast::SourceNodeId) -> Option<&Type<'src>> {
        self.view().expression_type(id)
    }

    pub fn call_instantiation(&self, id: SourceNodeId) -> Option<&CheckedCallInstantiation<'src>> {
        self.view().call_instantiation(id)
    }

    pub fn source_expression(&self, id: SourceNodeId) -> Option<&'ast Expr<'ast, 'src>> {
        self.view().source_expression(id)
    }

    pub fn expression_resolution(&self, id: SourceNodeId) -> ExpressionResolution {
        self.view().expression_resolution(id)
    }

    pub fn belongs_to(&self, source: &crate::ast::SourceIdentity) -> bool {
        self.view().belongs_to(source)
    }

    pub fn binding_type(&self, span: Span) -> Option<&Type<'src>> {
        self.view().binding_type(span)
    }

    pub(crate) fn builtin_call(&self, id: SourceNodeId) -> Option<BuiltinCall> {
        self.view().builtin_call(id)
    }

    pub(crate) fn resolved_intrinsic(
        &self,
        id: SourceNodeId,
    ) -> Option<crate::primitive::ResolvedIntrinsic> {
        self.view().resolved_intrinsic(id)
    }

    pub fn identifier_symbol(&self, span: Span) -> Option<SymbolId> {
        self.view().identifier_symbol(span)
    }

    pub(crate) fn symbol_is_assigned(&self, symbol: SymbolId) -> bool {
        self.view().symbol_is_assigned(symbol)
    }

    pub(crate) fn type_check_type(&self, span: Span) -> Option<&Type<'src>> {
        self.view().type_check_type(span)
    }

    pub(crate) fn optional_present_type(&self, span: Span) -> Option<&Type<'src>> {
        self.view().optional_present_type(span)
    }

    pub fn symbols(&self) -> &[Symbol<'src>] {
        self.view().symbols()
    }

    pub fn struct_type(&self, name: &str) -> Option<StructType<'src>> {
        self.view().struct_type(name)
    }

    pub fn export_target(&self, span: Span) -> Option<InterfaceTarget> {
        self.view().export_target(span)
    }

    pub fn struct_info(&self, name: &str) -> Option<&StructInfo<'src>> {
        self.view().struct_info(name)
    }

    pub fn enum_info(&self, name: &str) -> Option<&EnumInfo<'src>> {
        self.view().enum_info(name)
    }

    pub fn class_info(&self, name: &str) -> Option<&ClassInfo<'src>> {
        self.view().class_info(name)
    }

    pub fn nominal_id(&self, ty: &Type<'src>) -> Option<NominalId> {
        self.view().nominal_id(ty)
    }

    pub fn nominal_name(&self, id: NominalId) -> Option<&'src str> {
        self.view().nominal_name(id)
    }

    pub fn nominal_struct(&self, id: NominalId) -> Option<&StructInfo<'src>> {
        self.view().nominal_struct(id)
    }

    pub fn nominal_class(&self, id: NominalId) -> Option<&ClassInfo<'src>> {
        self.view().nominal_class(id)
    }

    pub fn nominal_member(&self, id: NominalMemberId) -> Option<NominalMember<'_, 'src>> {
        self.view().nominal_member(id)
    }

    pub fn resolved_member(&self, id: SourceNodeId) -> Option<NominalMember<'_, 'src>> {
        self.view().resolved_member(id)
    }

    pub fn is_extern_class(&self, name: &str) -> bool {
        self.view().is_extern_class(name)
    }

    pub fn is_object(&self, name: &str) -> bool {
        self.view().is_object(name)
    }

    pub(crate) fn class_method_owner(&self, class: &str, method: &str) -> Option<&'src str> {
        self.view().class_method_owner(class, method)
    }

    pub(crate) fn base_class_name(&self, class: &str) -> Option<&'src str> {
        self.view().base_class_name(class)
    }

    pub(crate) fn base_constructor(&self, class: &str) -> Option<(&'src str, FunctionType<'src>)> {
        self.view().base_constructor(class)
    }

    pub(crate) fn enum_variant_value(&self, span: Span) -> Option<i64> {
        self.view().enum_variant_value(span)
    }

    pub(crate) fn structs(&self) -> impl Iterator<Item = &StructInfo<'src>> {
        self.view().structs()
    }

    pub(crate) fn classes(&self) -> impl Iterator<Item = &ClassInfo<'src>> {
        self.view().classes()
    }

    pub(crate) fn dynamic_import_module(&self, span: Span) -> Option<u32> {
        self.view().dynamic_import_module(span)
    }

    pub(crate) fn dynamic_export_used(&self, module: u32, name: &str) -> bool {
        self.view().dynamic_export_used(module, name)
    }
}

impl<'view, 'ast, 'src> SemanticView<'view, 'ast, 'src> {
    pub fn expression_type(&self, id: crate::ast::SourceNodeId) -> Option<&'view Type<'src>> {
        self.facts
            .expression_types
            .get(id.index())
            .and_then(Option::as_ref)
    }

    pub fn call_instantiation(
        &self,
        id: SourceNodeId,
    ) -> Option<&'view CheckedCallInstantiation<'src>> {
        self.facts.call_instantiations.get(&id)
    }

    pub fn source_expression(&self, id: SourceNodeId) -> Option<&'ast Expr<'ast, 'src>> {
        self.facts.source_info.get(id.index())?.expression
    }

    pub fn expression_resolution(&self, id: SourceNodeId) -> ExpressionResolution {
        self.facts
            .source_info
            .get(id.index())
            .map_or(ExpressionResolution::None, |info| info.resolution)
    }

    pub fn belongs_to(&self, source: &crate::ast::SourceIdentity) -> bool {
        self.facts.source.same(source)
    }

    pub fn binding_type(&self, span: Span) -> Option<&'view Type<'src>> {
        Some(match self.facts.binding_types.get(&span)? {
            BindingType::Symbol(symbol) => &self.declarations.symbols[symbol.0 as usize].ty,
            BindingType::Inline(ty) => ty,
        })
    }

    pub(crate) fn builtin_call(&self, id: SourceNodeId) -> Option<BuiltinCall> {
        match self.expression_resolution(id) {
            ExpressionResolution::Builtin(builtin) => Some(builtin),
            _ => None,
        }
    }

    pub(crate) fn resolved_intrinsic(
        &self,
        id: SourceNodeId,
    ) -> Option<crate::primitive::ResolvedIntrinsic> {
        match self.expression_resolution(id) {
            ExpressionResolution::Primitive(operation) => Some(operation),
            _ => None,
        }
    }

    pub fn identifier_symbol(&self, span: Span) -> Option<SymbolId> {
        self.facts.identifier_symbols.get(&span).copied()
    }

    pub(crate) fn symbol_is_assigned(&self, symbol: SymbolId) -> bool {
        self.declarations.assigned_symbols.contains(&symbol)
    }

    pub(crate) fn type_check_type(&self, span: Span) -> Option<&'view Type<'src>> {
        self.facts.type_check_types.get(&span)
    }

    pub(crate) fn optional_present_type(&self, span: Span) -> Option<&'view Type<'src>> {
        self.facts.optional_present_types.get(&span)
    }

    pub fn symbols(&self) -> &'view [Symbol<'src>] {
        &self.declarations.symbols
    }

    pub fn struct_type(&self, name: &str) -> Option<StructType<'src>> {
        self.nominal_struct(*self.facts.struct_bindings.get(name)?)
            .map(|info| info.declaration)
    }

    pub fn export_target(&self, span: Span) -> Option<InterfaceTarget> {
        if let Some(symbol) = self.identifier_symbol(span) {
            return Some(InterfaceTarget::Value(symbol));
        }
        match self.binding_type(span) {
            Some(Type::Struct(declaration) | Type::StructInstance { declaration, .. }) => {
                Some(InterfaceTarget::Struct(declaration.identity))
            }
            _ => None,
        }
    }

    pub fn struct_info(&self, name: &str) -> Option<&'view StructInfo<'src>> {
        self.nominal_struct(*self.facts.struct_bindings.get(name)?)
    }

    pub fn enum_info(&self, name: &str) -> Option<&'view EnumInfo<'src>> {
        self.declarations.enums.get(name)
    }

    pub fn class_info(&self, name: &str) -> Option<&'view ClassInfo<'src>> {
        self.declarations.classes.get(name)
    }

    pub fn nominal_id(&self, ty: &Type<'src>) -> Option<NominalId> {
        match ty {
            Type::Struct(declaration) | Type::StructInstance { declaration, .. } => {
                Some(declaration.identity)
            }
            Type::Class(name) | Type::ClassInstance { name, .. } => self
                .declarations
                .classes
                .get_index_of(name)
                .map(|index| NominalId::new(index, true)),
            _ => None,
        }
    }

    pub fn nominal_name(&self, id: NominalId) -> Option<&'src str> {
        if id.is_class() {
            self.declarations
                .classes
                .get_index(id.index())
                .map(|(name, _)| *name)
        } else {
            self.declarations
                .structs
                .get(id.index())
                .map(|info| info.declaration.name)
        }
    }

    pub fn nominal_struct(&self, id: NominalId) -> Option<&'view StructInfo<'src>> {
        (!id.is_class())
            .then(|| self.declarations.structs.get(id.index()))
            .flatten()
    }

    pub fn nominal_class(&self, id: NominalId) -> Option<&'view ClassInfo<'src>> {
        id.is_class()
            .then(|| {
                self.declarations
                    .classes
                    .get_index(id.index())
                    .map(|(_, info)| info)
            })
            .flatten()
    }

    pub fn nominal_member(&self, id: NominalMemberId) -> Option<NominalMember<'view, 'src>> {
        let MemberDefinition { owner, slot } =
            *self.declarations.nominal_members.get(id.index())?;
        let value = if owner.is_class() {
            let (_, class) = self.declarations.classes.get_index(owner.index())?;
            match slot {
                MemberSlot::Field(index) => NominalMember::Field {
                    owner,
                    field: class.fields.get_index(index as usize)?.1,
                },
                MemberSlot::Method(index) => {
                    let (name, method) = class.methods.get_index(index as usize)?;
                    NominalMember::Method {
                        owner,
                        name,
                        method,
                    }
                }
            }
        } else {
            let MemberSlot::Field(index) = slot else {
                return None;
            };
            NominalMember::Field {
                owner,
                field: self
                    .declarations
                    .structs
                    .get(owner.index())?
                    .fields
                    .get_index(index as usize)?
                    .1,
            }
        };
        debug_assert_eq!(
            id,
            match value {
                NominalMember::Field { field, .. } => field.member,
                NominalMember::Method { method, .. } => method.member,
            }
        );
        Some(value)
    }

    pub fn resolved_member(&self, id: SourceNodeId) -> Option<NominalMember<'view, 'src>> {
        let ExpressionResolution::NominalMember(member) = self.expression_resolution(id) else {
            return None;
        };
        self.nominal_member(member)
    }

    pub fn is_extern_class(&self, name: &str) -> bool {
        self.declarations
            .classes
            .get(name)
            .is_some_and(|class| class.external)
    }

    pub fn is_object(&self, name: &str) -> bool {
        self.declarations
            .classes
            .get(name)
            .is_some_and(|class| class.object)
    }

    pub(crate) fn class_method_owner(&self, class: &str, method: &str) -> Option<&'src str> {
        self.declarations
            .classes
            .get(class)
            .and_then(|class| class.methods.get(method))
            .map(|method| method.owner)
    }

    pub(crate) fn base_class_name(&self, class: &str) -> Option<&'src str> {
        self.declarations
            .classes
            .get(class)
            .and_then(|class| class.base.as_ref())
            .and_then(class_type_name)
    }

    pub(crate) fn base_constructor(&self, class: &str) -> Option<(&'src str, FunctionType<'src>)> {
        let class = self.declarations.classes.get(class)?;
        let base_ty = class.base.as_ref()?;
        let (base_name, base_args) = class_type_parts(base_ty)?;
        let base = self.declarations.classes.get(base_name)?;
        let signature = base.constructor.clone()?;
        let substitutions = substitutions_for(&base.type_params, base_args);
        let Type::Function(signature) = substitute_type(&Type::Function(signature), &substitutions)
        else {
            unreachable!("constructor substitution preserves function type")
        };
        Some((base_name, signature))
    }

    pub(crate) fn enum_variant_value(&self, span: Span) -> Option<i64> {
        self.facts.enum_variant_values.get(&span).copied()
    }

    pub(crate) fn structs(&self) -> impl Iterator<Item = &'view StructInfo<'src>> + 'view {
        self.declarations.structs.iter()
    }

    pub(crate) fn classes(&self) -> impl Iterator<Item = &'view ClassInfo<'src>> + 'view {
        self.declarations.classes.values()
    }

    pub(crate) fn dynamic_import_module(&self, span: Span) -> Option<u32> {
        self.facts.dynamic_import_modules.get(&span).copied()
    }

    pub(crate) fn dynamic_export_used(&self, module: u32, name: &str) -> bool {
        self.facts.used_dynamic_exports.contains(&(module, name))
    }

    /// Every `(module, export)` this module's code reads from a namespace.
    pub(crate) fn used_dynamic_exports(&self) -> impl Iterator<Item = (u32, &'src str)> + '_ {
        self.facts.used_dynamic_exports.iter().copied()
    }
}

pub fn analyze<'ast, 'src>(
    program: &Program<'ast, 'src>,
) -> Result<SemanticModel<'ast, 'src>, SemanticError> {
    analyze_with_facts(
        program,
        ModuleFacts::new(program.source_identity()),
        &mut AllocationBudget::new(None),
    )
    .map_err(|failure| match failure {
        AdmittedSemanticError::Semantic(error) => error,
        AdmittedSemanticError::Resources(reason) => SemanticError::new(
            program.span,
            format!("semantic checking allocation failed: {reason}"),
        ),
    })
}

/// Fixed source-node tables and canonical declaration vectors are admitted.
/// Nested types, maps, diagnostics and comprehensive traversal remain separate.
pub(crate) fn with_analyzed_source<'ast, 'src, R>(
    program: &Program<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
    client: impl FnOnce(&SemanticModel<'ast, 'src>, &mut AllocationBudget<'_>) -> R,
) -> Result<R, AdmittedSemanticError> {
    let mut scope = budget.scope();
    let facts = ModuleFacts::new_admitted(program.source_identity(), &mut scope)?;
    let model = analyze_with_facts(program, facts, &mut scope)?;
    #[cfg(test)]
    let model = AdmittedFactsOwner::new(model, 1);
    let output = client(&model, &mut scope);
    drop(model);
    scope
        .finish_retained()
        .expect("checked-source callback transfers within its allocation owner");
    Ok(output)
}

#[cfg(test)]
#[path = "semantic/admission_tests.rs"]
mod admission_tests;

#[cfg(test)]
#[path = "semantic/declaration_admission_tests.rs"]
mod declaration_admission_tests;

#[cfg(test)]
#[path = "semantic/analyzer_admission_tests.rs"]
mod analyzer_admission_tests;

#[cfg(test)]
#[path = "semantic/binary_admission_tests.rs"]
mod binary_admission_tests;

#[cfg(test)]
#[path = "semantic/narrowing_admission_tests.rs"]
mod narrowing_admission_tests;

#[cfg(test)]
#[path = "semantic/narrowing_input_tests.rs"]
mod narrowing_input_tests;

fn analyze_with_facts<'ast, 'src>(
    program: &Program<'ast, 'src>,
    facts: ModuleFacts<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<SemanticModel<'ast, 'src>, AdmittedSemanticError> {
    let mut model = SemanticModel {
        declarations: DeclarationTables::default(),
        facts,
    };
    let mut initialization = ModuleInitialization::default();
    Analyzer::new(
        &mut model.facts,
        &mut model.declarations,
        &mut initialization,
        None,
        budget,
    )?
    .analyze_program(program)?;
    #[cfg(debug_assertions)]
    assert!(
        model.identifier_index_is_consistent(),
        "inconsistent source identifier index"
    );
    Ok(model)
}

struct Analyzer<'check, 'budget, 'ast, 'src> {
    facts: &'check mut ModuleFacts<'ast, 'src>,
    declarations: &'check mut DeclarationTables<'src>,
    initialization: &'check mut ModuleInitialization,
    module: Option<crate::module::ModuleId>,
    budget: &'check mut AllocationBudget<'budget>,
    scopes: Vec<AHashMap<&'src str, SymbolId>>,
    narrowings: Vec<AHashMap<SymbolId, Type<'src>>>,
    return_contexts: Vec<ReturnContext<'src>>,
    type_parameter_scopes: Vec<AHashSet<&'src str>>,
    loop_depth: usize,
    async_depth: usize,
    callable_depth: usize,
    reference_parameters: AHashMap<SymbolId, usize>,
    pending_references: bool,
    current_reference_formals: bool,
    initializing: Option<(SymbolId, usize)>,
    module_binding_declarations: AHashMap<Span, SymbolId>,
    constructor_classes: Vec<Option<&'src str>>,
    generator_contexts: Vec<Option<Type<'src>>>,
}

enum BinaryContinuation<'ast, 'src> {
    Left {
        expression: &'ast Expr<'ast, 'src>,
        expected: Option<Type<'src>>,
    },
    Right {
        expression: &'ast Expr<'ast, 'src>,
        left: Type<'src>,
        left_narrowing: NarrowingInput<'ast, 'src>,
        narrowed_scope: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct ModuleBindingState {
    declaration: Span,
    owner: BindingOwner,
}

#[derive(Debug, Clone, Copy)]
enum BindingOwner {
    LegacySpan(Span),
    Module(crate::module::ModuleId),
}

#[derive(Default)]
struct ModuleInitialization {
    bindings: AHashMap<SymbolId, ModuleBindingState>,
    initialized: AHashSet<SymbolId>,
}

type Narrowing<'src> = AHashMap<SymbolId, Type<'src>>;

enum NarrowingStep<'ast, 'src> {
    Visit(&'ast Expr<'ast, 'src>),
    Not,
    Join(BinaryOp),
}

enum NarrowingLeaf<'ast, 'src> {
    TypeCheck {
        ident: &'ast Ident<'src>,
        span: Span,
    },
    NullComparison {
        ident: &'ast Ident<'src>,
        present_when_true: bool,
    },
}

fn narrowing_leaf<'ast, 'src>(
    condition: &'ast Expr<'ast, 'src>,
) -> Option<NarrowingLeaf<'ast, 'src>> {
    match &condition.kind {
        ExprKind::TypeCheck { value, span, .. } => {
            let ExprKind::Ident(ident) = &value.kind else {
                return None;
            };
            Some(NarrowingLeaf::TypeCheck { ident, span: *span })
        }
        ExprKind::Binary {
            op: op @ (BinaryOp::Eq | BinaryOp::NotEq), lhs, rhs, ..
        } => {
            let ident = match (&lhs.kind, &rhs.kind) {
                (ExprKind::Ident(ident), ExprKind::Null(_))
                | (ExprKind::Null(_), ExprKind::Ident(ident)) => ident,
                _ => return None,
            };
            Some(NarrowingLeaf::NullComparison {
                ident,
                present_when_true: *op == BinaryOp::NotEq,
            })
        }
        _ => None,
    }
}

/// Syntax-only input, not a cached narrowing result. A discarded projection
/// still retains its guard so scope-sensitive lookup and diagnostics run.
#[derive(Clone, Copy)]
struct NarrowingInput<'ast, 'src> {
    expression: Option<&'ast Expr<'ast, 'src>>,
    when_true: bool,
    when_false: bool,
}

impl<'ast, 'src> NarrowingInput<'ast, 'src> {
    fn leaf(expression: &'ast Expr<'ast, 'src>) -> Self {
        let relevant = narrowing_leaf(expression).is_some()
            || matches!(expression.kind, ExprKind::Unary { op: UnaryOp::Not, .. });
        Self {
            expression: relevant.then_some(expression),
            when_true: true,
            when_false: true,
        }
    }

    fn join(self, other: Self, expression: &'ast Expr<'ast, 'src>, op: BinaryOp) -> Self {
        debug_assert!(matches!(op, BinaryOp::And | BinaryOp::Or));
        let mut input = match (self.expression, other.expression) {
            (None, _) => other,
            (_, None) => self,
            (Some(_), Some(_)) => Self {
                expression: Some(expression),
                when_true: true,
                when_false: true,
            },
        };
        input.when_true &= op == BinaryOp::And;
        input.when_false &= op == BinaryOp::Or;
        input
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaceIntent {
    Write,
    MutableArgument,
}

fn empty_narrowing<'src>() -> Narrowing<'src> {
    AHashMap::default()
}

fn merge_narrowing<'src>(mut left: Narrowing<'src>, right: Narrowing<'src>) -> Narrowing<'src> {
    for (symbol, ty) in right {
        left.insert(symbol, ty);
    }
    left
}

#[derive(Debug)]
enum ReturnContext<'src> {
    Declared {
        ty: Type<'src>,
        saw_return: bool,
    },
    Inferred {
        ty: Option<Type<'src>>,
        saw_return: bool,
    },
}

impl Drop for Analyzer<'_, '_, '_, '_> {
    fn drop(&mut self) {
        // Module passes share declarations, but these buffers die with this
        // analyzer. Drop their storage before releasing only their charges.
        fn release<T>(values: &mut Vec<T>, budget: &mut AllocationBudget<'_>) {
            let values = std::mem::take(values);
            let bytes = values
                .capacity()
                .checked_mul(std::mem::size_of::<T>())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .expect("admitted analyzer vector capacity fits its original layout");
            drop(values);
            budget
                .release(AllocationClass::Scratch, bytes)
                .expect("analyzer backing belongs to its callback budget");
        }
        release(&mut self.scopes, self.budget);
        release(&mut self.narrowings, self.budget);
        release(&mut self.return_contexts, self.budget);
        release(&mut self.type_parameter_scopes, self.budget);
        release(&mut self.constructor_classes, self.budget);
        release(&mut self.generator_contexts, self.budget);
    }
}

impl<'check, 'budget, 'ast, 'src> Analyzer<'check, 'budget, 'ast, 'src> {
    fn new(
        facts: &'check mut ModuleFacts<'ast, 'src>,
        declarations: &'check mut DeclarationTables<'src>,
        initialization: &'check mut ModuleInitialization,
        module: Option<crate::module::ModuleId>,
        budget: &'check mut AllocationBudget<'budget>,
    ) -> Result<Self, AllocationError> {
        let mut analyzer = Self {
            facts,
            declarations,
            initialization,
            module,
            budget,
            scopes: Vec::new(),
            narrowings: Vec::new(),
            return_contexts: Vec::new(),
            type_parameter_scopes: Vec::new(),
            loop_depth: 0,
            async_depth: 0,
            callable_depth: 0,
            reference_parameters: AHashMap::default(),
            pending_references: false,
            current_reference_formals: false,
            initializing: None,
            module_binding_declarations: AHashMap::default(),
            constructor_classes: Vec::new(),
            generator_contexts: Vec::new(),
        };
        analyzer.scopes = analyzer.budget.vector(AllocationClass::Scratch, 1)?;
        analyzer.narrowings = analyzer.budget.vector(AllocationClass::Scratch, 1)?;
        analyzer.push_scope()?;
        Ok(analyzer)
    }

    fn analyze_program(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        if let Some(import) = program.imports.first() {
            return Err(AdmittedSemanticError::new(
                import.span,
                "imports require file-based compilation so the module graph can be resolved",
            ));
        }
        for import in program.dynamic_imports {
            self.facts
                .dynamic_import_modules
                .insert(import.span, import.module);
            let exports = self.facts.module_exports.entry(import.module).or_default();
            for export in import.exports {
                exports.insert(export.exported, export.binding);
            }
        }
        self.declare_nominal_types(program)?;
        self.define_enums(program)?;
        self.define_structs(program)?;
        struct_cycles::validate(&self.declarations.structs).map_err(|(_, error)| error)?;
        self.define_classes(program)?;
        self.define_extern_classes(program)?;
        self.resolve_class_hierarchies()?;
        self.declare_functions(program)?;
        self.instantiate_module_bindings(program)?;

        self.analyze_items(program)?;

        self.finalize_parameter_default_bindings()?;
        for export in program.exports {
            let target = if let Some(target) = self.view().export_target(export.local.span) {
                Some(target)
            } else {
                match (
                    self.scopes[0].get(export.local.name).copied(),
                    self.facts.struct_bindings.get(export.local.name).copied(),
                ) {
                    (Some(_), Some(_)) => {
                        return Err(AdmittedSemanticError::new(
                            export.span,
                            "ambiguous export names both a value and a struct type; export the declaration directly",
                        ));
                    }
                    (Some(symbol), None) => Some(InterfaceTarget::Value(symbol)),
                    (None, Some(identity)) => Some(InterfaceTarget::Struct(identity)),
                    (None, None) => None, // Existing enum/type-only and unresolved-export owners remain unchanged.
                }
            };
            match target {
                Some(InterfaceTarget::Value(symbol)) => {
                    if self.declarations.symbols[symbol.0 as usize]
                        .ty
                        .contains_mutable_reference_parameters()
                    {
                        return Err(AdmittedSemanticError::new(
                            export.span,
                            "public exports do not yet support mutable-reference callable contracts",
                        ));
                    }
                    self.record_identifier(export.local.span, symbol);
                }
                Some(InterfaceTarget::Struct(identity)) => {
                    self.facts.binding_types.insert(
                        export.local.span,
                        BindingType::Inline(Type::Struct(
                            self.declarations.structs[identity.index()].declaration,
                        )),
                    );
                }
                None => {}
            }
        }
        Ok(())
    }

    fn analyze_items(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        for item in program.items {
            match item {
                Item::Enum(_) => {}
                Item::Struct(_) => {}
                Item::Class(class) => self.analyze_class(class)?,
                Item::ExternClass(class) => self.analyze_extern_class_defaults(class)?,
                Item::Function(function) => self.analyze_function(function, None)?,
                Item::Extern(extern_decl) => self.analyze_extern_defaults(extern_decl)?,
                Item::ExternGlobal(_) => {}
                Item::Stmt(statement) => self.analyze_stmt(statement)?,
            }
        }

        Ok(())
    }

    fn view(&self) -> SemanticView<'_, 'ast, 'src> {
        SemanticView {
            declarations: self.declarations,
            facts: self.facts,
        }
    }

    fn record_identifier(&mut self, span: Span, symbol: SymbolId) {
        self.facts
            .record_identifier(self.declarations, span, symbol);
    }

    fn instantiate_module_bindings(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedSemanticError> {
        for binding in program.module_bindings {
            let mut ty = self.resolve_value_type(binding.ty, "module binding")?;
            strip_parameter_defaults_from_type(&mut ty);
            let id = self.declare(binding.name, ty)?;
            self.module_binding_declarations
                .insert(binding.name.span, id);
            self.initialization.bindings.insert(
                id,
                ModuleBindingState {
                    declaration: binding.name.span,
                    owner: BindingOwner::LegacySpan(binding.module_span),
                },
            );
        }
        Ok(())
    }

    fn declare_nominal_types(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedSemanticError> {
        for item in program.items {
            let (name, type_params, span, is_struct, external, object) = match item {
                Item::Struct(decl) => (
                    decl.name.name,
                    decl.type_params,
                    decl.span,
                    true,
                    false,
                    false,
                ),
                Item::Class(decl) => (
                    decl.name.name,
                    decl.type_params,
                    decl.span,
                    false,
                    false,
                    decl.object,
                ),
                Item::ExternClass(decl) => (
                    decl.name.name,
                    decl.type_params,
                    decl.span,
                    false,
                    true,
                    false,
                ),
                _ => continue,
            };
            let type_params = validate_type_params(type_params)?;

            if self.facts.struct_bindings.contains_key(name) {
                return Err(AdmittedSemanticError::new(
                    span,
                    format!("duplicate type declaration `{name}`"),
                ));
            }
            if let Some(existing) = self.declarations.classes.get(name) {
                if existing.object && object {
                    continue;
                }
                return Err(AdmittedSemanticError::new(
                    span,
                    format!("duplicate type declaration `{name}`"),
                ));
            }

            if is_struct {
                let identity = NominalId::new(self.declarations.structs.len(), false);
                let declaration = StructType { identity, name };
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut self.declarations.structs,
                    StructInfo {
                        declaration,
                        module: self.module,
                        type_params,
                        fields: IndexMap::new(),
                        span,
                    },
                )?;
                self.facts.struct_bindings.insert(name, identity);
                if let Item::Struct(decl) = item {
                    self.facts.binding_types.insert(
                        decl.name.span,
                        BindingType::Inline(Type::Struct(declaration)),
                    );
                }
            } else {
                self.declarations.classes.insert(
                    name,
                    ClassInfo {
                        name,
                        type_params,
                        base: None,
                        fields: IndexMap::new(),
                        methods: IndexMap::new(),
                        constructor: None,
                        external,
                        object,
                        span,
                    },
                );
            }
        }
        Ok(())
    }

    fn define_enums(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        for item in program.items {
            let Item::Enum(decl) = item else {
                continue;
            };
            if self.declarations.enums.contains_key(decl.name.name)
                || self.facts.struct_bindings.contains_key(decl.name.name)
                || self.declarations.classes.contains_key(decl.name.name)
            {
                return Err(AdmittedSemanticError::new(
                    decl.span,
                    format!("duplicate type declaration `{}`", decl.name.name),
                ));
            }
            let mut variants = IndexMap::new();
            for (index, variant) in decl.variants.iter().enumerate() {
                if variants.insert(variant.name, index as i64).is_some() {
                    return Err(AdmittedSemanticError::new(
                        variant.span,
                        format!(
                            "duplicate variant `{}` in enum `{}`",
                            variant.name, decl.name.name
                        ),
                    ));
                }
            }
            self.declarations.enums.insert(
                decl.name.name,
                EnumInfo {
                    name: decl.name.name,
                    variants,
                    span: decl.span,
                },
            );
        }
        Ok(())
    }

    fn define_structs(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        for item in program.items {
            let Item::Struct(decl) = item else {
                continue;
            };
            self.push_type_params(decl.type_params)?;

            let fields = self.resolve_fields(decl)?;
            self.pop_type_params();
            let identity = self.facts.struct_bindings[decl.name.name];
            self.declarations.structs[identity.index()].fields = fields;
        }
        Ok(())
    }

    fn define_classes(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        for item in program.items {
            let Item::Class(decl) = item else {
                continue;
            };

            let owner = self
                .view()
                .nominal_id(&Type::Class(decl.name.name))
                .expect("class declared before member definitions");

            self.push_type_params(decl.type_params)?;

            let base = decl
                .base
                .map(|base| self.resolve_value_type(base, "base class"))
                .transpose()?;
            if base
                .as_ref()
                .is_some_and(|base| !matches!(base, Type::Class(_) | Type::ClassInstance { .. }))
            {
                return Err(AdmittedSemanticError::new(
                    decl.base.expect("checked base").span,
                    "`extends` requires a class type",
                ));
            }

            let mut fields = IndexMap::new();
            let mut methods = IndexMap::new();
            let mut constructor = None;
            for member in decl.members {
                match member {
                    ClassMember::Field(field) => {
                        if fields.contains_key(field.name.name)
                            || methods.contains_key(field.name.name)
                        {
                            return Err(AdmittedSemanticError::new(
                                field.name.span,
                                format!(
                                    "duplicate member `{}` in class `{}`",
                                    field.name.name, decl.name.name
                                ),
                            ));
                        }
                        let ty = self.resolve_value_type(field.ty, "class field")?;
                        let index = fields.len();
                        fields.insert(
                            field.name.name,
                            FieldInfo {
                                member: self
                                    .declarations
                                    .declare_member(owner, MemberSlot::field(index), self.budget)?,
                                name: field.name.name,
                                ty,
                                index,
                                span: field.span,
                            },
                        );
                    }
                    ClassMember::Method(method) => {
                        if fields.contains_key(method.name.name)
                            || methods.contains_key(method.name.name)
                        {
                            return Err(AdmittedSemanticError::new(
                                method.name.span,
                                format!(
                                    "duplicate member `{}` in class `{}`",
                                    method.name.name, decl.name.name
                                ),
                            ));
                        }
                        let signature = self.function_type(method)?;
                        if decl.object {
                            let index = fields.len();
                            fields.insert(
                                method.name.name,
                                FieldInfo {
                                    member: self
                                        .declarations
                                        .declare_member(owner, MemberSlot::field(index), self.budget)?,
                                    name: method.name.name,
                                    ty: Type::Function(signature.clone()),
                                    index,
                                    span: method.span,
                                },
                            );
                        }
                        methods.insert(
                            method.name.name,
                            MethodInfo {
                                member: self
                                    .declarations
                                    .declare_member(
                                        owner,
                                        MemberSlot::method(methods.len()),
                                        self.budget,
                                    )?,
                                owner: decl.name.name,
                                type_params: validate_type_params(method.type_params)?,
                                signature,
                                declared_pure: method.declared_pure,
                            },
                        );
                    }
                    ClassMember::Constructor(constructor_decl) => {
                        if constructor.is_some() {
                            return Err(AdmittedSemanticError::new(
                                constructor_decl.span,
                                format!("class `{}` has more than one constructor", decl.name.name),
                            ));
                        }
                        let mut params = Vec::with_capacity(constructor_decl.params.len());
                        for param in constructor_decl.params {
                            params
                                .push(self.resolve_parameter_type(&param.parameter, "parameter")?);
                        }
                        if constructor_decl.params.iter().any(|param| {
                            param.parameter.passing == ParameterPassing::MutableReference
                        }) {
                            return Err(AdmittedSemanticError::new(
                                constructor_decl.span,
                                "constructors do not support mutable-reference parameters",
                            ));
                        }
                        resolve_parameter_defaults(constructor_decl.params, &mut params)?;
                        constructor = Some(FunctionType::new(FunctionSignature {
                            params,
                            return_type: Box::new(applied_class_type(
                                decl.name.name,
                                &validate_type_params(decl.type_params)?,
                            )),
                        }));
                    }
                }
            }

            let merge_object = {
                let info = self
                    .declarations
                    .classes
                    .get_mut(decl.name.name)
                    .expect("class name was declared in the first semantic pass");
                if decl.object
                    && info.object
                    && self
                        .scopes
                        .last()
                        .is_some_and(|scope| scope.contains_key(decl.name.name))
                {
                    for name in methods.keys() {
                        if info.methods.contains_key(name) || info.fields.contains_key(name) {
                            return Err(AdmittedSemanticError::new(
                                decl.span,
                                format!("duplicate member `{name}` in object `{}`", decl.name.name),
                            ));
                        }
                    }
                    for (next_index, (name, mut field)) in (info.fields.len()..).zip(fields) {
                        field.index = next_index;
                        self.declarations.nominal_members[field.member.index()].slot =
                            MemberSlot::field(next_index);
                        info.fields.insert(name, field);
                    }
                    for (offset, method) in methods.values().enumerate() {
                        self.declarations.nominal_members[method.member.index()].slot =
                            MemberSlot::method(info.methods.len() + offset);
                    }
                    info.methods.extend(methods);
                    true
                } else {
                    info.fields = fields;
                    info.methods = methods;
                    info.base = base;
                    info.constructor = constructor.clone();
                    info.object = decl.object;
                    false
                }
            };
            self.pop_type_params();
            if merge_object {
                if let Some(&symbol) = self
                    .scopes
                    .last()
                    .and_then(|scope| scope.get(decl.name.name))
                {
                    self.record_identifier(decl.name.span, symbol);
                    self.facts.binding_types.insert(
                        decl.name.span,
                        BindingType::Inline(Type::Class(decl.name.name)),
                    );
                }
                continue;
            }

            if decl.object {
                self.declare(decl.name, Type::Class(decl.name.name))?;
                continue;
            }

            let constructor_signature =
                constructor.unwrap_or(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(applied_class_type(
                        decl.name.name,
                        &validate_type_params(decl.type_params)?,
                    )),
                }));
            let constructor = if decl.type_params.is_empty() {
                Type::Function(constructor_signature)
            } else {
                Type::GenericFunction(GenericFunctionType {
                    type_params: validate_type_params(decl.type_params)?,
                    signature: constructor_signature,
                })
            };
            self.declare(decl.name, constructor)?;
        }
        Ok(())
    }

    pub(super) fn define_extern_classes(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedSemanticError> {
        for item in program.items {
            let Item::ExternClass(decl) = item else {
                continue;
            };
            let owner = self
                .view()
                .nominal_id(&Type::Class(decl.name.name))
                .expect("extern class declared before member definitions");
            self.push_type_params(decl.type_params)?;
            let base = decl
                .base
                .map(|base| self.resolve_value_type(base, "base extern class"))
                .transpose()?;
            if base
                .as_ref()
                .is_some_and(|base| !matches!(base, Type::Class(_) | Type::ClassInstance { .. }))
            {
                return Err(AdmittedSemanticError::new(
                    decl.base.expect("checked base").span,
                    "`extends` requires a class type",
                ));
            }
            let mut fields = IndexMap::new();
            let mut methods = IndexMap::new();
            let mut host_constructor = None;
            for member in decl.members {
                match member {
                    ExternClassMember::Constructor(constructor) => {
                        // The host constructor's parameters, for `super(...)`
                        // from an internal subclass. Defaults would need the
                        // host's own default semantics, so they are refused.
                        if let Some(param) = constructor.params.iter().find(|param| param.default.is_some()) {
                            return Err(AdmittedSemanticError::new(
                                param.span,
                                "a host constructor signature cannot declare parameter defaults",
                            ));
                        }
                        let mut params = Vec::with_capacity(constructor.params.len());
                        for param in constructor.params {
                            params.push(self.resolve_parameter_type(&param.parameter, "host constructor parameter")?);
                        }
                        let signature = FunctionType::new(FunctionSignature {
                            params,
                            return_type: Box::new(Type::Void),
                        });
                        if Type::Function(signature.clone()).contains_mutable_reference_parameters() {
                            return Err(AdmittedSemanticError::new(
                                constructor.span,
                                "foreign callable contracts do not support mutable-reference parameters",
                            ));
                        }
                        host_constructor = Some(signature);
                    }
                    ExternClassMember::Field(field) => {
                        if fields.contains_key(field.name.name)
                            || methods.contains_key(field.name.name)
                        {
                            return Err(AdmittedSemanticError::new(
                                field.name.span,
                                format!(
                                    "duplicate member `{}` in extern class `{}`",
                                    field.name.name, decl.name.name
                                ),
                            ));
                        }
                        let index = fields.len();
                        fields.insert(
                            field.name.name,
                            FieldInfo {
                                member: self
                                    .declarations
                                    .declare_member(owner, MemberSlot::field(index), self.budget)?,
                                name: field.name.name,
                                ty: self.resolve_value_type(field.ty, "extern class field")?,
                                index,
                                span: field.span,
                            },
                        );
                    }
                    ExternClassMember::Method(method) => {
                        if fields.contains_key(method.name.name)
                            || methods.contains_key(method.name.name)
                        {
                            return Err(AdmittedSemanticError::new(
                                method.name.span,
                                format!(
                                    "duplicate member `{}` in extern class `{}`",
                                    method.name.name, decl.name.name
                                ),
                            ));
                        }
                        methods.insert(
                            method.name.name,
                            MethodInfo {
                                member: self
                                    .declarations
                                    .declare_member(
                                        owner,
                                        MemberSlot::method(methods.len()),
                                        self.budget,
                                    )?,
                                owner: decl.name.name,
                                type_params: validate_type_params(method.type_params)?,
                                signature: self.extern_type(method)?,
                                declared_pure: method.declared_pure,
                            },
                        );
                    }
                }
            }
            self.pop_type_params();
            let info = self
                .declarations
                .classes
                .get_mut(decl.name.name)
                .expect("extern class name was declared in the first semantic pass");
            info.fields = fields;
            info.methods = methods;
            info.base = base;
            if host_constructor.is_some() {
                info.constructor = host_constructor;
            }
        }
        Ok(())
    }

    fn resolve_class_hierarchies(&mut self) -> Result<(), AdmittedSemanticError> {
        let names = self
            .declarations
            .classes
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut visiting = AHashSet::default();
        let mut complete = AHashSet::default();
        for name in names {
            self.resolve_class_hierarchy(name, &mut visiting, &mut complete)?;
        }
        Ok(())
    }

    fn resolve_class_hierarchy(
        &mut self,
        name: &'src str,
        visiting: &mut AHashSet<&'src str>,
        complete: &mut AHashSet<&'src str>,
    ) -> Result<(), AdmittedSemanticError> {
        if complete.contains(name) {
            return Ok(());
        }
        let info = self
            .declarations
            .classes
            .get(name)
            .expect("class hierarchy names come from the semantic model");
        let span = info.span;
        let external = info.external;
        if !visiting.insert(name) {
            return Err(AdmittedSemanticError::new(
                span,
                format!("inheritance cycle involving class `{name}`"),
            ));
        }

        let Some(base_ty) = &info.base else {
            visiting.remove(name);
            complete.insert(name);
            return Ok(());
        };
        let base_name =
            class_type_name(base_ty).expect("base classes were validated while defining classes");
        let base =
            self.declarations.classes.get(base_name).ok_or_else(|| {
                AdmittedSemanticError::new(span, format!("unknown base class `{base_name}`"))
            })?;
        // An internal class may extend a host (`extern`) class: that is how a
        // typed class becomes a real `Error` subclass, with a native prototype
        // chain, `instanceof`, `stack` and `message`, instead of hand-written
        // `JsValue` prototype ceremony. The reverse is still meaningless: a
        // host interface cannot inherit an implementation the host never sees.
        if external && !base.external {
            return Err(AdmittedSemanticError::new(
                span,
                "an extern class cannot extend an internal class",
            ));
        }
        self.resolve_class_hierarchy(base_name, visiting, complete)?;
        let info = self
            .declarations
            .classes
            .get(name)
            .expect("derived class remains declared");
        let (_, base_args) = class_type_parts(
            info.base
                .as_ref()
                .expect("derived class keeps its base type"),
        )
        .expect("base classes were validated while defining classes");
        let base = self
            .declarations
            .classes
            .get(base_name)
            .expect("resolved base class remains declared");
        let substitutions = substitutions_for(&base.type_params, base_args);
        let mut fields = IndexMap::new();
        for field in base.fields.values() {
            fields.insert(
                field.name,
                FieldInfo {
                    member: field.member,
                    name: field.name,
                    ty: substitute_type(&field.ty, &substitutions),
                    index: field.index,
                    span: field.span,
                },
            );
        }
        let mut methods = IndexMap::new();
        for (method_name, method) in &base.methods {
            let signature =
                match substitute_type(&Type::Function(method.signature.clone()), &substitutions) {
                    Type::Function(signature) => signature,
                    _ => unreachable!("substituting a method preserves its function type"),
                };
            methods.insert(
                *method_name,
                MethodInfo {
                    member: method.member,
                    owner: method.owner,
                    type_params: method.type_params.clone(),
                    signature,
                    declared_pure: method.declared_pure,
                },
            );
        }
        // Before its first resolution, each class owns only its declared members.
        // Keep those maps intact through diagnostics, then move their payloads.
        for (offset, field) in info.fields.values().enumerate() {
            if fields.contains_key(field.name) || methods.contains_key(field.name) {
                return Err(AdmittedSemanticError::new(
                    field.span,
                    format!(
                        "class `{name}` cannot shadow inherited member `{}`",
                        field.name
                    ),
                ));
            }
            self.declarations.nominal_members[field.member.index()].slot =
                MemberSlot::field(fields.len() + offset);
        }
        for (offset, (method_name, method)) in info.methods.iter().enumerate() {
            if fields.contains_key(method_name)
                || info.fields.contains_key(method_name)
                || methods.contains_key(method_name)
            {
                return Err(AdmittedSemanticError::new(
                    span,
                    format!("class `{name}` cannot override inherited member `{method_name}`"),
                ));
            }
            self.declarations.nominal_members[method.member.index()].slot =
                MemberSlot::method(methods.len() + offset);
        }
        let resolved = self
            .declarations
            .classes
            .get_mut(name)
            .expect("derived class remains declared");
        for (_, mut field) in std::mem::take(&mut resolved.fields) {
            field.index = fields.len();
            fields.insert(field.name, field);
        }
        methods.extend(std::mem::take(&mut resolved.methods));
        resolved.fields = fields;
        resolved.methods = methods;
        visiting.remove(name);
        complete.insert(name);
        Ok(())
    }

    fn resolve_fields(
        &mut self,
        decl: &'ast StructDecl<'ast, 'src>,
    ) -> Result<IndexMap<&'src str, FieldInfo<'src>>, AdmittedSemanticError> {
        let owner = self
            .view()
            .struct_type(decl.name.name)
            .expect("struct declared before member definitions")
            .identity;
        let mut fields = IndexMap::new();
        for field in decl.fields {
            if fields.contains_key(field.name.name) {
                return Err(AdmittedSemanticError::new(
                    field.name.span,
                    format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name.name, decl.name.name
                    ),
                ));
            }
            let ty = self.resolve_value_type(field.ty, "struct field")?;
            let index = fields.len();
            fields.insert(
                field.name.name,
                FieldInfo {
                    member: self
                        .declarations
                        .declare_member(owner, MemberSlot::field(index), self.budget)?,
                    name: field.name.name,
                    ty,
                    index,
                    span: field.span,
                },
            );
        }
        Ok(fields)
    }

    fn declare_functions(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        for item in program.items {
            match item {
                Item::Function(function) => {
                    let signature = self.function_type(function)?;
                    let ty = if function.type_params.is_empty() {
                        Type::Function(signature)
                    } else {
                        Type::GenericFunction(GenericFunctionType {
                            type_params: validate_type_params(function.type_params)?,
                            signature,
                        })
                    };
                    self.declare(function.name, ty)?;
                }
                Item::Extern(extern_decl) => {
                    let signature = self.extern_type(extern_decl)?;
                    let ty = if extern_decl.type_params.is_empty() {
                        Type::Function(signature.clone())
                    } else {
                        Type::GenericFunction(GenericFunctionType {
                            type_params: validate_type_params(extern_decl.type_params)?,
                            signature: signature.clone(),
                        })
                    };
                    self.declare_foreign(extern_decl.name, ty, true)?;
                    let mut names = AHashMap::default();
                    for (param, ty) in extern_decl.params.iter().zip(signature.params.iter()) {
                        if names.insert(param.name.name, param.name.span).is_some() {
                            return Err(AdmittedSemanticError::new(
                                param.name.span,
                                format!("duplicate extern parameter `{}`", param.name.name),
                            ));
                        }
                        self.record_detached(param.name, ty.ty.clone())?;
                    }
                }
                Item::ExternGlobal(global) => {
                    let ty = self.resolve_value_type(global.ty, "extern global")?;
                    if ty.contains_mutable_reference_parameters() {
                        return Err(AdmittedSemanticError::new(
                            global.span,
                            "foreign bindings do not support mutable-reference callable contracts",
                        ));
                    }
                    self.declare_foreign(global.name, ty, false)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn function_type(
        &mut self,
        function: &'ast FunctionDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, AdmittedSemanticError> {
        self.check_module_defaults(function.params)?;
        self.push_type_params(function.type_params)?;
        let signature = self.function_type_in_current_scope(function);
        self.pop_type_params();
        signature
    }

    fn function_type_in_current_scope(
        &self,
        function: &'ast FunctionDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, AdmittedSemanticError> {
        if (function.is_async || function.is_generator)
            && function
                .params
                .iter()
                .any(|parameter| parameter.parameter.passing == ParameterPassing::MutableReference)
        {
            return Err(AdmittedSemanticError::new(
                function.span,
                "async and generator functions cannot have mutable-reference parameters",
            ));
        }
        let mut params = Vec::with_capacity(function.params.len());
        for param in function.params {
            params.push(self.resolve_parameter_type(&param.parameter, "parameter")?);
        }
        resolve_parameter_defaults(function.params, &mut params)?;
        let declared_return = self.resolve_type(function.return_type, true, "return type")?;
        let return_type = if function.is_async {
            Type::Task(Box::new(declared_return))
        } else if function.is_generator {
            if declared_return == Type::Void {
                return Err(AdmittedSemanticError::new(
                    function.return_type.span,
                    "generator element type cannot be `void`",
                ));
            }
            Type::Generator(Box::new(declared_return))
        } else {
            declared_return
        };
        Ok(FunctionType::new(FunctionSignature {
            params,
            return_type: Box::new(return_type),
        }))
    }

    fn extern_type(
        &mut self,
        extern_decl: &'ast ExternDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, AdmittedSemanticError> {
        self.check_module_defaults(extern_decl.params)?;
        self.push_type_params(extern_decl.type_params)?;
        let mut params = Vec::with_capacity(extern_decl.params.len());
        for param in extern_decl.params {
            params.push(self.resolve_parameter_type(&param.parameter, "extern parameter")?);
        }
        resolve_parameter_defaults(extern_decl.params, &mut params)?;
        let return_type = self.resolve_type(extern_decl.return_type, true, "extern return type")?;
        let signature = FunctionType::new(FunctionSignature {
            params,
            return_type: Box::new(return_type),
        });
        if Type::Function(signature.clone()).contains_mutable_reference_parameters() {
            return Err(AdmittedSemanticError::new(
                extern_decl.span,
                "foreign callable contracts do not support mutable-reference parameters",
            ));
        }
        self.pop_type_params();
        Ok(signature)
    }

    fn analyze_class(&mut self, class: &'ast ClassDecl<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        self.push_type_params(class.type_params)?;
        let requires_super = self
            .declarations
            .classes
            .get(class.name.name)
            .and_then(|info| info.base.as_ref())
            .and_then(class_type_name)
            .and_then(|base| self.declarations.classes.get(base))
            .is_some_and(|base| base.constructor.is_some());
        if requires_super
            && !class
                .members
                .iter()
                .any(|member| matches!(member, ClassMember::Constructor(_)))
        {
            self.pop_type_params();
            return Err(AdmittedSemanticError::new(
                class.span,
                format!(
                    "class `{}` must declare `init` and call its base constructor",
                    class.name.name
                ),
            ));
        }
        for member in class.members {
            match member {
                ClassMember::Method(method) => {
                    self.analyze_function(method, Some(class.name.name))?
                }
                ClassMember::Constructor(constructor) => {
                    self.analyze_constructor(constructor, class.name.name)?
                }
                ClassMember::Field(_) => {}
            }
        }
        self.pop_type_params();
        Ok(())
    }

    fn analyze_constructor(
        &mut self,
        constructor: &'ast ConstructorDecl<'ast, 'src>,
        class_name: &'src str,
    ) -> Result<(), AdmittedSemanticError> {
        let class_info = self
            .declarations
            .classes
            .get(class_name)
            .expect("constructors belong to declared classes");
        let super_calls = count_super_calls(constructor.body);
        match class_info.base.as_ref().and_then(class_type_name) {
            None if super_calls != 0 => {
                return Err(AdmittedSemanticError::new(
                    constructor.span,
                    "`super` is only valid in a derived class constructor",
                ));
            }
            Some(base_name) => {
                if super_calls > 1 {
                    return Err(AdmittedSemanticError::new(
                        constructor.span,
                        "a derived constructor may call `super` only once",
                    ));
                }
                let base_has_constructor = self
                    .declarations
                    .classes
                    .get(base_name)
                    .is_some_and(|base| base.constructor.is_some());
                if base_has_constructor && super_calls == 0 {
                    return Err(AdmittedSemanticError::new(
                        constructor.span,
                        format!(
                            "derived constructor must begin with `super(...)` for `{base_name}`"
                        ),
                    ));
                }
                if super_calls != 0
                    && !matches!(constructor.body.first(), Some(Stmt::SuperCall { .. }))
                {
                    return Err(AdmittedSemanticError::new(
                        constructor.span,
                        "`super(...)` must be the first statement in a derived constructor",
                    ));
                }
            }
            None => {}
        }
        let parameters = constructor
            .params
            .iter()
            .map(|param| self.resolve_parameter_type(&param.parameter, "parameter"))
            .collect::<Result<Vec<_>, _>>()?;
        self.callable_depth += 1;
        self.analyze_parameter_defaults(constructor.params, &parameters)?;
        self.push_scope()?;
        let class_type_params = self
            .declarations
            .classes
            .get(class_name)
            .map(|class| class.type_params.as_slice())
            .unwrap_or_default();
        self.declare(
            Ident {
                name: "this",
                span: constructor.span,
            },
            applied_class_type(class_name, class_type_params),
        )?;
        for (param, parameter) in constructor.params.iter().zip(parameters) {
            self.declare(param.name, parameter.ty)?;
        }
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.return_contexts,
            ReturnContext::Declared {
                ty: Type::Void,
                saw_return: false,
            },
        )?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.constructor_classes,
            Some(class_name),
        )?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.generator_contexts,
            None,
        )?;
        for statement in constructor.body {
            self.analyze_stmt(statement)?;
        }
        self.generator_contexts.pop();
        self.constructor_classes.pop();
        self.return_contexts.pop();
        self.pop_scope();
        self.callable_depth -= 1;
        Ok(())
    }

    fn analyze_function(
        &mut self,
        function: &'ast FunctionDecl<'ast, 'src>,
        class_name: Option<&'src str>,
    ) -> Result<(), AdmittedSemanticError> {
        self.push_type_params(function.type_params)?;
        let signature = self.function_type_in_current_scope(function)?;
        if class_name.is_some() {
            self.require_value_parameters(&signature, function.span)?;
        }
        let outer_pending = std::mem::take(&mut self.pending_references);
        let outer_formals = std::mem::replace(
            &mut self.current_reference_formals,
            signature
                .params
                .iter()
                .any(|parameter| parameter.passing == ParameterPassing::MutableReference),
        );
        self.callable_depth += 1;
        self.analyze_parameter_defaults(function.params, &signature.params)?;
        self.push_scope()?;

        if let Some(class_name) = class_name {
            self.declare(
                Ident {
                    name: "this",
                    span: function.name.span,
                },
                applied_class_type(
                    class_name,
                    &self
                        .declarations
                        .classes
                        .get(class_name)
                        .map(|class| class.type_params.clone())
                        .unwrap_or_default(),
                ),
            )?;
        }

        for (param, ty) in function.params.iter().zip(&signature.params) {
            let symbol = self.declare(param.name, ty.ty.clone())?;
            if ty.passing == ParameterPassing::MutableReference {
                self.reference_parameters
                    .insert(symbol, self.callable_depth);
            }
        }

        let generator_element = if function.is_generator {
            match signature.return_type.as_ref() {
                Type::Generator(value) => Some((**value).clone()),
                _ => unreachable!("generator signatures return Generator<T>"),
            }
        } else {
            None
        };
        let body_return_type = if function.is_async {
            match signature.return_type.as_ref() {
                Type::Task(value) => (**value).clone(),
                _ => unreachable!("async signatures return Task<T>"),
            }
        } else if function.is_generator {
            Type::Void
        } else {
            (*signature.return_type).clone()
        };
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.return_contexts,
            ReturnContext::Declared {
                ty: body_return_type,
                saw_return: false,
            },
        )?;
        self.async_depth += usize::from(function.is_async);
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.generator_contexts,
            generator_element,
        )?;
        for statement in function.body {
            self.analyze_stmt(statement)?;
        }
        self.generator_contexts.pop();
        self.async_depth -= usize::from(function.is_async);
        let context = self
            .return_contexts
            .pop()
            .expect("function analysis pushed a return context");
        self.pop_scope();
        self.callable_depth -= 1;
        self.pending_references = outer_pending;
        self.current_reference_formals = outer_formals;
        self.pop_type_params();

        if let ReturnContext::Declared { ty, .. } = context {
            if !ty.is_void() && !statements_guarantee_return(function.body) {
                return Err(AdmittedSemanticError::new(
                    function.name.span,
                    format!(
                        "function `{}` must return a value of type `{ty}`",
                        function.name.name
                    ),
                ));
            }
        }
        Ok(())
    }

    fn analyze_extern_defaults(
        &mut self,
        extern_decl: &'ast ExternDecl<'ast, 'src>,
    ) -> Result<(), AdmittedSemanticError> {
        self.push_type_params(extern_decl.type_params)?;
        let types = extern_decl
            .params
            .iter()
            .map(|param| self.resolve_parameter_type(&param.parameter, "extern parameter"))
            .collect::<Result<Vec<_>, _>>()?;
        self.callable_depth += 1;
        let result = self.analyze_parameter_defaults(extern_decl.params, &types);
        self.callable_depth -= 1;
        self.pop_type_params();
        result
    }

    fn analyze_extern_class_defaults(
        &mut self,
        class: &'ast crate::ast::ExternClassDecl<'ast, 'src>,
    ) -> Result<(), AdmittedSemanticError> {
        self.push_type_params(class.type_params)?;
        for member in class.members {
            if let ExternClassMember::Method(method) = member {
                self.analyze_extern_defaults(method)?;
            }
        }
        self.pop_type_params();
        Ok(())
    }

    fn finalize_parameter_default_bindings(&mut self) -> Result<(), AdmittedSemanticError> {
        let source_info = &self.facts.source_info;
        for ty in self.facts.expression_types.iter_mut().flatten() {
            finalize_default_bindings_in_type(ty, source_info, false)?;
        }
        for ty in self.facts.optional_present_types.values_mut() {
            finalize_default_bindings_in_type(ty, source_info, false)?;
        }
        for ty in self.facts.type_check_types.values_mut() {
            finalize_default_bindings_in_type(ty, source_info, false)?;
        }
        for binding in self.facts.binding_types.values_mut() {
            if let BindingType::Inline(ty) = binding {
                finalize_default_bindings_in_type(ty, source_info, false)?;
            }
        }
        for symbol in &mut self.declarations.symbols {
            finalize_default_bindings_in_type(&mut symbol.ty, source_info, false)?;
        }
        for info in self.declarations.structs.iter_mut() {
            for field in info.fields.values_mut() {
                finalize_default_bindings_in_type(&mut field.ty, source_info, false)?;
            }
        }
        for info in self.declarations.classes.values_mut() {
            if let Some(base) = &mut info.base {
                finalize_default_bindings_in_type(base, source_info, false)?;
            }
            for field in info.fields.values_mut() {
                finalize_default_bindings_in_type(&mut field.ty, source_info, false)?;
            }
            for method in info.methods.values_mut() {
                finalize_default_bindings_in_signature(&mut method.signature, source_info, false)?;
            }
            if let Some(constructor) = &mut info.constructor {
                finalize_default_bindings_in_signature(constructor, source_info, false)?;
            }
        }
        Ok(())
    }

    /// The module checker's pass, run after each module's bodies: this
    /// module's facts and the declarations it owns.
    pub(crate) fn finalize_module_parameter_defaults(
        &mut self,
        module: crate::module::ModuleId,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedSemanticError> {
        let source_info = &self.facts.source_info;
        for ty in self.facts.expression_types.iter_mut().flatten() {
            finalize_default_bindings_in_type(ty, source_info, true)?;
        }
        for ty in self.facts.optional_present_types.values_mut() {
            finalize_default_bindings_in_type(ty, source_info, true)?;
        }
        for ty in self.facts.type_check_types.values_mut() {
            finalize_default_bindings_in_type(ty, source_info, true)?;
        }
        for binding in self.facts.binding_types.values_mut() {
            if let BindingType::Inline(ty) = binding {
                finalize_default_bindings_in_type(ty, source_info, true)?;
            }
        }
        for (index, symbol) in self.declarations.symbols.iter_mut().enumerate() {
            if self.declarations.symbol_modules.get(index).copied().flatten() == Some(module) {
                finalize_default_bindings_in_type(&mut symbol.ty, source_info, true)?;
            }
        }
        for info in self.declarations.structs.iter_mut() {
            if info.module == Some(module) {
                for field in info.fields.values_mut() {
                    finalize_default_bindings_in_type(&mut field.ty, source_info, true)?;
                }
            }
        }
        for item in program.items {
            let Item::Class(class) = item else {
                continue;
            };
            let Some(info) = self.declarations.classes.get_mut(class.name.name) else {
                continue;
            };
            for field in info.fields.values_mut() {
                finalize_default_bindings_in_type(&mut field.ty, source_info, true)?;
            }
            for method in info.methods.values_mut() {
                finalize_default_bindings_in_signature(&mut method.signature, source_info, true)?;
            }
            if let Some(constructor) = &mut info.constructor {
                finalize_default_bindings_in_signature(constructor, source_info, true)?;
            }
        }
        Ok(())
    }

    fn analyze_parameter_defaults(
        &mut self,
        params: &'ast [crate::ast::Param<'ast, 'src>],
        parameters: &[FunctionParameter<'src>],
    ) -> Result<(), AdmittedSemanticError> {
        for (param, parameter) in params.iter().zip(parameters) {
            let expected = &parameter.ty;
            let Some(expression) = &param.default else {
                continue;
            };
            if scalar_default_value(expression).is_some() {
                continue;
            }
            let contextual = match expression {
                Expr {
                    kind: ExprKind::ArrayLiteral { .. },
                    ..
                } => expected_array_type(expected).unwrap_or(expected),
                _ => expected,
            };
            let actual = self.analyze_expr(expression, Some(contextual))?;
            self.require_assignable(expected, &actual, expression.span())?;
        }
        Ok(())
    }

    fn analyze_stmt(&mut self, statement: &'ast Stmt<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        match statement {
            Stmt::VarDecl(decl) => self.analyze_var_decl(decl),
            Stmt::ArrayDestructure {
                bindings, value, ..
            } => {
                let actual = self.analyze_expr(value, None)?;
                let Type::Array(element) = actual else {
                    return Err(AdmittedSemanticError::new(
                        value.span(),
                        format!("array destructuring requires an array, found `{actual}`"),
                    ));
                };
                for binding in *bindings {
                    match binding {
                        ArrayBinding::Hole(_) => {}
                        ArrayBinding::Name(name) => {
                            self.declare(*name, Type::Nullable(element.clone()))?;
                        }
                        ArrayBinding::Rest(name) => {
                            self.declare(*name, Type::Array(element.clone()))?;
                        }
                    }
                }
                Ok(())
            }
            Stmt::RecordDestructure {
                bindings,
                rest,
                value,
                ..
            } => {
                let actual = self.analyze_expr(value, None)?;
                let Type::Record(element) = actual else {
                    return Err(AdmittedSemanticError::new(
                        value.span(),
                        format!("record destructuring requires a record, found `{actual}`"),
                    ));
                };
                let mut keys = AHashSet::default();
                for binding in *bindings {
                    if !keys.insert(decode_source_string(binding.key.name, binding.key.span)?) {
                        return Err(AdmittedSemanticError::new(
                            binding.key.span,
                            format!("duplicate record binding key `{}`", binding.key.name),
                        ));
                    }
                    self.declare(binding.name, Type::Nullable(element.clone()))?;
                }
                if let Some(rest) = rest {
                    self.declare(*rest, Type::Record(element.clone()))?;
                }
                Ok(())
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr, None)?;
                Ok(())
            }
            Stmt::Return { value, span } => self.analyze_return(value.as_ref(), *span),
            Stmt::Throw { value, .. } => {
                let thrown = self.analyze_expr(value, None)?;
                if thrown == Type::Void {
                    return Err(AdmittedSemanticError::new(
                        value.span(),
                        "cannot throw a `void` expression",
                    ));
                }
                Ok(())
            }
            Stmt::SuperCall { args, span } => self.analyze_super_call(args, *span),
            Stmt::Yield {
                value,
                delegate,
                span,
            } => self.analyze_yield(value, *delegate, *span),
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                self.push_scope()?;
                for statement in *body {
                    self.analyze_stmt(statement)?;
                }
                self.pop_scope();
                if let Some(clause) = catch {
                    self.push_scope()?;
                    if let Some(binding) = clause.binding {
                        let ty = if binding.ty.is_auto() {
                            Type::TypeParameter("$js")
                        } else {
                            self.resolve_value_type(binding.ty, "catch binding")?
                        };
                        if !is_js_value(&ty) {
                            return Err(AdmittedSemanticError::new(
                                binding.ty.span,
                                format!(
                                    "catch bindings must use `auto` or `JsValue`, found `{ty}`"
                                ),
                            ));
                        }
                        self.declare(binding.name, Type::TypeParameter("$js"))?;
                    }
                    for statement in clause.body {
                        self.analyze_stmt(statement)?;
                    }
                    self.pop_scope();
                }
                if let Some(finally) = finally {
                    self.push_scope()?;
                    for statement in *finally {
                        self.analyze_stmt(statement)?;
                    }
                    self.pop_scope();
                }
                Ok(())
            }
            Stmt::Block { body, .. } => {
                self.push_scope()?;
                for statement in *body {
                    self.analyze_stmt(statement)?;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                let (then_narrowing, else_narrowing) = self.condition_narrowing(condition)?;
                let then_returns = statement_guarantees_return(then_branch);
                let else_returns =
                    else_branch.is_some_and(|branch| statement_guarantees_return(branch));
                self.push_scope()?;
                self.apply_narrowing(then_narrowing.clone());
                self.analyze_stmt(then_branch)?;
                let then_survives = self.current_scope_preserves(&then_narrowing);
                self.pop_scope();
                let mut else_survives = else_branch.is_none() && !else_narrowing.is_empty();
                if let Some(else_branch) = else_branch {
                    self.push_scope()?;
                    self.apply_narrowing(else_narrowing.clone());
                    self.analyze_stmt(else_branch)?;
                    else_survives = self.current_scope_preserves(&else_narrowing);
                    self.pop_scope();
                }
                if then_returns && !else_returns && else_survives {
                    self.apply_narrowing(else_narrowing);
                } else if else_returns && !then_returns && then_survives {
                    self.apply_narrowing(then_narrowing);
                }
                Ok(())
            }
            Stmt::While {
                condition, body, ..
            } => {
                let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                let (body_narrowing, _) = self.condition_narrowing(condition)?;
                self.loop_depth += 1;
                self.push_scope()?;
                self.apply_narrowing(body_narrowing);
                self.analyze_stmt(body)?;
                self.pop_scope();
                self.loop_depth -= 1;
                Ok(())
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                self.push_scope()?;
                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VarDecl(decl) => self.analyze_var_decl(decl)?,
                        ForInitializer::Expr(expr) => {
                            self.analyze_expr(expr, None)?;
                        }
                    }
                }
                if let Some(condition) = condition {
                    let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                    self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                }
                if let Some(update) = update {
                    self.analyze_expr(update, None)?;
                }
                self.loop_depth += 1;
                self.analyze_stmt(body)?;
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(())
            }
            Stmt::ForIn {
                key_type,
                key,
                object,
                body,
                ..
            } => {
                self.push_scope()?;
                let key_ty = self.resolve_value_type(*key_type, "for-in key")?;
                if key_ty != Type::String {
                    return Err(AdmittedSemanticError::new(
                        key_type.span,
                        format!("for-in keys must have type `string`, found `{key_ty}`"),
                    ));
                }
                let object_ty = self.analyze_expr(object, None)?;
                if !is_js_value(&object_ty) && !matches!(object_ty, Type::Record(_)) {
                    return Err(AdmittedSemanticError::new(
                        object.span(),
                        format!(
                            "for-in requires a `JsValue` or `Record<T>` object, found `{object_ty}`"
                        ),
                    ));
                }
                self.declare(*key, Type::String)?;
                self.loop_depth += 1;
                self.analyze_stmt(body)?;
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(())
            }
            Stmt::ForOf {
                element_type,
                element,
                iterable,
                body,
                inline,
                ..
            } => {
                self.push_scope()?;
                let declared = self.resolve_value_type(*element_type, "for-of element")?;
                let iterable_type = self.analyze_expr(iterable, None)?;
                if *inline {
                    if iterable.const_list_literals().is_none() {
                        return Err(AdmittedSemanticError::new(
                            iterable.span(),
                            "`inline for` requires a constant array literal of int, float, string, or bool values",
                        ));
                    }
                    if statement_contains_loop_control(body, false) {
                        return Err(AdmittedSemanticError::new(
                            body.span(),
                            "`inline for` cannot contain `break` or `continue`",
                        ));
                    }
                }
                let actual = match iterable_type {
                    Type::Array(element) => *element,
                    Type::Generator(element) => *element,
                    ty if TypedArrayKind::from_type(&ty).is_some() => {
                        if TypedArrayKind::from_type(&ty)
                            .is_some_and(TypedArrayKind::element_is_float)
                        {
                            Type::Float
                        } else {
                            Type::Int
                        }
                    }
                    other => {
                        return Err(AdmittedSemanticError::new(
                            iterable.span(),
                            format!(
                                "for-of requires an array or typed array, or Generator<T>, found `{other}`"
                            ),
                        ));
                    }
                };
                self.require_assignable(&declared, &actual, element_type.span)?;
                self.declare(*element, declared)?;
                if *inline {
                    self.analyze_stmt(body)?;
                } else {
                    self.loop_depth += 1;
                    self.analyze_stmt(body)?;
                    self.loop_depth -= 1;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::Break(span) | Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    Err(AdmittedSemanticError::new(
                        *span,
                        "loop control statement outside a loop",
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn analyze_super_call(
        &mut self,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    ) -> Result<(), AdmittedSemanticError> {
        self.require_value_arguments(
            args,
            "super calls do not support mutable-reference arguments",
        )?;
        let class_name = self
            .constructor_classes
            .last()
            .copied()
            .flatten()
            .ok_or_else(|| {
                AdmittedSemanticError::new(span, "`super` is only valid in a derived class constructor")
            })?;
        let class = self
            .declarations
            .classes
            .get(class_name)
            .expect("constructor class metadata exists");
        let base_ty = class.base.as_ref().ok_or_else(|| {
            AdmittedSemanticError::new(span, "`super` is only valid in a derived class constructor")
        })?;
        let (base_name, base_args) =
            class_type_parts(base_ty).expect("derived class bases are class types");
        let base = self
            .declarations
            .classes
            .get(base_name)
            .expect("base class was resolved");
        let substitutions = substitutions_for(&base.type_params, base_args);
        let signature = base.constructor.as_ref().map(|signature| {
            match substitute_type(&Type::Function(signature.clone()), &substitutions) {
                Type::Function(signature) => signature,
                _ => unreachable!("constructor substitution preserves function type"),
            }
        });
        match signature {
            Some(signature) => {
                self.analyze_call(&Type::Function(signature), args, span, None, None)?;
            }
            None if !args.is_empty() => {
                return Err(AdmittedSemanticError::new(
                    span,
                    format!("implicit base constructor `{base_name}` expects no arguments"),
                ));
            }
            None => {}
        }
        Ok(())
    }

    fn analyze_yield(
        &mut self,
        value: &'ast Expr<'ast, 'src>,
        delegate: bool,
        span: Span,
    ) -> Result<(), AdmittedSemanticError> {
        self.require_no_pending_reference(span)?;
        let expected = self
            .generator_contexts
            .last()
            .and_then(Clone::clone)
            .ok_or_else(|| AdmittedSemanticError::new(span, "`yield` is only valid inside a generator"))?;
        if delegate {
            let iterable = self.analyze_expr(value, None)?;
            let actual = match iterable {
                Type::Array(element) | Type::Generator(element) => *element,
                ty if TypedArrayKind::from_type(&ty).is_some() => {
                    if TypedArrayKind::from_type(&ty).is_some_and(TypedArrayKind::element_is_float)
                    {
                        Type::Float
                    } else {
                        Type::Int
                    }
                }
                other => {
                    return Err(AdmittedSemanticError::new(
                        value.span(),
                        format!("`yield*` requires an iterable, found `{other}`"),
                    ));
                }
            };
            self.require_assignable(&expected, &actual, value.span())
        } else {
            let actual = self.analyze_expr(value, Some(&expected))?;
            self.require_assignable(&expected, &actual, value.span())
        }
    }

    fn analyze_var_decl(&mut self, decl: &'ast VarDecl<'ast, 'src>) -> Result<(), AdmittedSemanticError> {
        if decl.initializer.is_none() {
            return Err(AdmittedSemanticError::new(
                decl.span,
                "variable declarations require an initializer",
            ));
        }
        if decl.ty.is_auto() {
            let initializer = decl.initializer.as_ref().ok_or_else(|| {
                AdmittedSemanticError::new(decl.span, "`auto` declarations require an initializer")
            })?;
            let inferred = self.analyze_expr(initializer, None)?;
            if inferred.is_void() {
                return Err(AdmittedSemanticError::new(
                    initializer.span(),
                    "cannot infer a variable type from a void expression",
                ));
            }
            if inferred == Type::Null {
                return Err(AdmittedSemanticError::new(
                    initializer.span(),
                    "cannot infer a variable type from `null`; add an explicit nullable type",
                ));
            }
            let mut ty = inferred;
            // A named callable carries declaration-stable default metadata, so
            // an inferred alias can retain its optional-call contract. Defaults
            // originating in a computed first-class value are erased when that
            // value enters mutable storage; otherwise a later call could cache
            // an initializer's defaults independently of the stored callable.
            if !matches!(
                initializer,
                Expr {
                    kind: ExprKind::Ident(_),
                    ..
                }
            ) {
                strip_parameter_defaults_from_type(&mut ty);
            }
            self.declare(decl.name, ty)?;
            return Ok(());
        }

        let declared = self.resolve_value_type(decl.ty, "variable")?;
        let mut binding_ty = declared.clone();
        strip_parameter_defaults_from_type(&mut binding_ty);
        let id = if let Some(id) = self
            .module_binding_declarations
            .get(&decl.name.span)
            .copied()
        {
            id
        } else {
            self.declare(decl.name, binding_ty)?
        };
        let previous = self.initializing;
        self.initializing = Some((id, self.callable_depth));
        let analyzed = if let Some(initializer) = &decl.initializer {
            let actual = self.analyze_expr(initializer, Some(&declared));
            self.initializing = previous;
            let actual = actual?;
            self.require_assignable(&declared, &actual, initializer.span())
        } else {
            self.initializing = previous;
            Ok(())
        };
        analyzed?;
        let referenced = self.declarations.symbols[id.0 as usize].identifier_occurrences > 1;
        if referenced {
            self.declarations.assigned_symbols.insert(id);
        }
        self.initialization.initialized.insert(id);
        Ok(())
    }

    fn analyze_return(
        &mut self,
        value: Option<&'ast Expr<'ast, 'src>>,
        span: Span,
    ) -> Result<(), AdmittedSemanticError> {
        let expected = match self.return_contexts.last() {
            Some(ReturnContext::Declared { ty, .. }) => Some(ty.clone()),
            Some(ReturnContext::Inferred { ty, .. }) => ty.clone(),
            None => return Err(AdmittedSemanticError::new(span, "`return` outside a function")),
        };

        let actual = match value {
            Some(value) => self.analyze_expr(value, expected.as_ref())?,
            None => Type::Void,
        };
        if matches!(
            self.return_contexts.last(),
            Some(ReturnContext::Declared { .. })
        ) {
            let expected = expected
                .as_ref()
                .expect("declared returns have an expected type");
            if !self.is_assignable(expected, &actual) {
                return Err(AdmittedSemanticError::new(
                    span,
                    format!("expected return type `{expected}`, found `{actual}`"),
                ));
            }
        }

        let context = self
            .return_contexts
            .last_mut()
            .expect("return context was checked above");
        match context {
            ReturnContext::Declared { saw_return, .. } => {
                *saw_return = true;
            }
            ReturnContext::Inferred { ty, saw_return } => {
                if let Some(previous) = ty {
                    let Some(common) = common_type(previous, &actual) else {
                        return Err(AdmittedSemanticError::new(
                            span,
                            format!(
                                "incompatible inferred return types `{previous}` and `{actual}`"
                            ),
                        ));
                    };
                    *previous = common;
                } else {
                    *ty = Some(actual);
                }
                *saw_return = true;
            }
        }
        Ok(())
    }

    fn analyze_expr(
        &mut self,
        expr: &'ast Expr<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        self.facts.source_info[expr.id.index()] = SourceInfo {
            expression: Some(expr),
            resolution: ExpressionResolution::None,
        };
        let ty = match expr {
            Expr {
                kind: ExprKind::Int(value, span),
                ..
            } => {
                if i32::try_from(*value).is_err() {
                    return Err(AdmittedSemanticError::new(
                        *span,
                        "integer literal is outside the signed 32-bit range",
                    ));
                }
                Type::Int
            }
            Expr {
                kind: ExprKind::Float(_, _),
                ..
            } => Type::Float,
            Expr {
                kind: ExprKind::String(_, _),
                ..
            } => Type::String,
            Expr {
                kind: ExprKind::Bool(_, _),
                ..
            } => Type::Bool,
            Expr {
                kind: ExprKind::Null(_),
                ..
            } => Type::Null,
            Expr {
                kind: ExprKind::DynamicImport { span, .. },
                ..
            } => {
                let module = self.facts.dynamic_import_modules
                    .get(span)
                    .copied()
                    .ok_or_else(|| {
                        AdmittedSemanticError::new(
                            *span,
                            "dynamic imports require file-based compilation so their module interface can be resolved",
                        )
                    })?;
                Type::Task(Box::new(Type::ModuleNamespace(module)))
            }
            Expr {
                kind: ExprKind::Ident(ident),
                ..
            } => {
                let (id, declared) = {
                    let symbol = self.resolve(ident)?;
                    (symbol.id, symbol.ty.clone())
                };
                if let Some((initializing, depth)) = self.initializing {
                    if id == initializing && self.callable_depth == depth {
                        return Err(AdmittedSemanticError::new(
                            ident.span,
                            format!(
                                "cannot read `{}` in its own initializer; nest the reference in a function",
                                ident.name
                            ),
                        ));
                    }
                }
                if let Some(binding) = self.initialization.bindings.get(&id) {
                    let from_owner = match binding.owner {
                        BindingOwner::LegacySpan(span) => {
                            ident.span.start >= span.start && ident.span.end <= span.end
                        }
                        BindingOwner::Module(module) => self.module == Some(module),
                    };
                    if from_owner && ident.span.start < binding.declaration.start {
                        return Err(AdmittedSemanticError::new(
                            ident.span,
                            format!("cannot read `{}` before its declaration", ident.name),
                        ));
                    }
                    if !self.initialization.initialized.contains(&id) && self.callable_depth == 0 {
                        return Err(AdmittedSemanticError::new(
                            ident.span,
                            format!(
                                "cannot eagerly read module binding `{}` before it is initialized",
                                ident.name
                            ),
                        ));
                    }
                }
                self.record_identifier(ident.span, id);
                self.facts.source_info[expr.id.index()].resolution =
                    ExpressionResolution::Binding(id);
                self.narrowed_type(id).cloned().unwrap_or(declared)
            }
            Expr {
                kind: ExprKind::ArrayLiteral { elements, span },
                ..
            } => {
                let expected_element = match expected {
                    Some(Type::Array(element)) => Some(element.as_ref()),
                    _ => None,
                };
                let mut element_type = expected_element.cloned();
                for element in *elements {
                    let actual = match element {
                        ArrayElement::Value(value) => self.analyze_expr(value, expected_element)?,
                        ArrayElement::Spread { value, .. } => {
                            let expected_array = expected_element
                                .map(|element| Type::Array(Box::new(element.clone())));
                            let spread = self.analyze_expr(value, expected_array.as_ref())?;
                            let Type::Array(actual) = spread else {
                                return Err(AdmittedSemanticError::new(
                                    value.span(),
                                    format!("array spread requires an array, found `{spread}`"),
                                ));
                            };
                            *actual
                        }
                    };
                    if let Some(expected) = expected_element {
                        // A contextual literal creates fresh storage with the
                        // declared element representation, so one-way element
                        // conversion is safe here even though mutable array
                        // references themselves are invariant.
                        self.require_assignable(expected, &actual, element.span())?;
                        continue;
                    }
                    element_type = match element_type {
                        Some(ref previous) => {
                            Some(common_type(previous, &actual).ok_or_else(|| {
                                AdmittedSemanticError::new(
                                    element.span(),
                                    format!(
                                        "array element has type `{actual}`, expected `{previous}`"
                                    ),
                                )
                            })?)
                        }
                        None => Some(actual),
                    };
                }
                let element_type = element_type.ok_or_else(|| {
                    AdmittedSemanticError::new(
                        *span,
                        "cannot infer the element type of an empty array; add an explicit array type",
                    )
                })?;
                Type::Array(Box::new(element_type))
            }
            Expr {
                kind: ExprKind::ObjectLiteral { entries, .. },
                ..
            } => {
                let mut seen = AHashSet::default();
                for entry in *entries {
                    match entry {
                        RecordElement::Entry(entry) => {
                            if !seen.insert(decode_source_string(entry.key.name, entry.key.span)?) {
                                return Err(AdmittedSemanticError::new(
                                    entry.key.span,
                                    format!("duplicate object key `{}`", entry.key.name),
                                ));
                            }
                            self.analyze_expr(&entry.value, Some(&Type::TypeParameter("$js")))?;
                        }
                        RecordElement::Spread { span, .. } => {
                            return Err(AdmittedSemanticError::new(
                                *span,
                                "ordinary object spread is not supported yet",
                            ));
                        }
                    }
                }
                Type::TypeParameter("$js")
            }
            Expr {
                kind: ExprKind::RecordLiteral { entries, span },
                ..
            } => {
                if expected.is_some_and(is_js_value_or_nullable_js_value) {
                    let mut seen = AHashSet::default();
                    for entry in *entries {
                        match entry {
                            RecordElement::Entry(entry) => {
                                if !seen
                                    .insert(decode_source_string(entry.key.name, entry.key.span)?)
                                {
                                    return Err(AdmittedSemanticError::new(
                                        entry.key.span,
                                        format!("duplicate record key `{}`", entry.key.name),
                                    ));
                                }
                                self.analyze_expr(&entry.value, Some(&Type::TypeParameter("$js")))?;
                            }
                            RecordElement::Spread { value, .. } => {
                                self.analyze_expr(value, Some(&Type::TypeParameter("$js")))?;
                            }
                        }
                    }
                    Type::TypeParameter("$js")
                } else {
                    let expected_value = match expected {
                        Some(Type::Record(value)) => Some(value.as_ref()),
                        _ => None,
                    };
                    let mut seen = AHashSet::default();
                    let mut value_type = expected_value.cloned();
                    for entry in *entries {
                        let actual = match entry {
                            RecordElement::Entry(entry) => {
                                if !seen
                                    .insert(decode_source_string(entry.key.name, entry.key.span)?)
                                {
                                    return Err(AdmittedSemanticError::new(
                                        entry.key.span,
                                        format!("duplicate record key `{}`", entry.key.name),
                                    ));
                                }
                                self.analyze_expr(&entry.value, expected_value)?
                            }
                            RecordElement::Spread { value, .. } => {
                                let expected_record = expected_value
                                    .map(|value| Type::Record(Box::new(value.clone())));
                                let spread = self.analyze_expr(value, expected_record.as_ref())?;
                                let Type::Record(actual) = spread else {
                                    return Err(AdmittedSemanticError::new(
                                        value.span(),
                                        format!(
                                            "record spread requires a record, found `{spread}`"
                                        ),
                                    ));
                                };
                                *actual
                            }
                        };
                        if let Some(expected) = expected_value {
                            // Like an array literal, a record literal creates fresh
                            // storage. Each copied value may be widened into the declared
                            // slot type without making mutable Record references covariant.
                            if !self.is_assignable(expected, &actual) {
                                return Err(AdmittedSemanticError::new(
                                    entry.span(),
                                    format!(
                                        "expected `Record<{expected}>`, found `Record<{actual}>`"
                                    ),
                                ));
                            }
                            continue;
                        }
                        value_type = match value_type {
                            Some(ref previous) => {
                                Some(common_type(previous, &actual).ok_or_else(|| {
                                    AdmittedSemanticError::new(
                                        entry.span(),
                                        format!(
                                        "record value has type `{actual}`, expected `{previous}`"
                                    ),
                                    )
                                })?)
                            }
                            None => Some(actual),
                        };
                    }
                    let value_type = value_type.ok_or_else(|| {
                    AdmittedSemanticError::new(
                        *span,
                        "cannot infer the value type of an empty record; add an explicit `Record<T>` type",
                    )
                })?;
                    Type::Record(Box::new(value_type))
                }
            }
            Expr {
                kind: ExprKind::StructLiteral { name, values, span },
                ..
            } => {
                let info = self
                    .declarations
                    .structs
                    .get(
                        self.facts
                            .struct_bindings
                            .get(name.name)
                            .map(|id| id.index())
                            .unwrap_or(usize::MAX),
                    )
                    .ok_or_else(|| {
                        AdmittedSemanticError::new(name.span, format!("unknown struct `{}`", name.name))
                    })?;
                if values.len() != info.fields.len() {
                    return Err(AdmittedSemanticError::new(
                        *span,
                        format!(
                            "struct `{}` expects {} values, found {}",
                            name.name,
                            info.fields.len(),
                            values.len()
                        ),
                    ));
                }
                let (field_types, result_type) = if info.type_params.is_empty() {
                    (
                        info.fields
                            .values()
                            .map(|field| field.ty.clone())
                            .collect::<Vec<_>>(),
                        Type::Struct(info.declaration),
                    )
                } else {
                    let Some(Type::StructInstance {
                        declaration: expected_declaration,
                        args,
                    }) = expected
                    else {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!(
                                "generic struct literal `{}` requires a contextual `{}<...>` type",
                                name.name, name.name
                            ),
                        ));
                    };
                    if *expected_declaration != info.declaration
                        || args.len() != info.type_params.len()
                    {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!(
                                "generic struct literal `{}` requires a contextual `{}<...>` type",
                                name.name, name.name
                            ),
                        ));
                    }
                    let substitutions = substitutions_for(&info.type_params, args);
                    (
                        info.fields
                            .values()
                            .map(|field| substitute_type(&field.ty, &substitutions))
                            .collect::<Vec<_>>(),
                        Type::StructInstance {
                            declaration: info.declaration,
                            args: args.clone(),
                        },
                    )
                };
                let identity = info.declaration.identity;
                self.facts.source_info[expr.id.index()].resolution =
                    ExpressionResolution::NominalConstruction(identity);
                for (value, field_type) in values.iter().zip(&field_types) {
                    let actual = self.analyze_expr(value, Some(field_type))?;
                    self.require_assignable(field_type, &actual, value.span())?;
                }
                result_type
            }
            Expr {
                kind:
                    ExprKind::New {
                        class,
                        type_args,
                        args,
                        span,
                    },
                ..
            } => {
                self.require_value_arguments(
                    args,
                    "constructors do not support mutable-reference arguments",
                )?;
                if let Some(ty) = self.analyze_builtin_constructor(
                    *class, type_args, args, expr.id, *span, expected,
                )? {
                    ty
                } else {
                    let info = self
                        .declarations
                        .classes
                        .get(class.name)
                        .ok_or_else(|| {
                            AdmittedSemanticError::new(
                                class.span,
                                format!("unknown class `{}`", class.name),
                            )
                        })?;
                    self.facts.source_info[expr.id.index()].resolution =
                        ExpressionResolution::NominalConstruction(
                            self.view()
                                .nominal_id(&Type::Class(class.name))
                                .expect("checked class declaration"),
                        );
                    if info.external {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!("extern class `{}` cannot be constructed", class.name),
                        ));
                    }
                    if info.object {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!("object `{}` cannot be constructed with `new`", class.name),
                        ));
                    }
                    if let Some(signature) = &info.constructor {
                        self.require_value_parameters(signature, *span)?;
                    }
                    let accepts_arity = info
                        .constructor
                        .as_ref()
                        .map_or(args.is_empty(), |signature| {
                            signature.accepts_arity(args.len())
                        });
                    if !accepts_arity {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!(
                                "class `{}` constructor expects {} arguments, found {}",
                                class.name,
                                info.constructor
                                    .as_ref()
                                    .map_or(0, FunctionType::required_params),
                                args.len()
                            ),
                        ));
                    }
                    let type_params = info.type_params.clone();
                    let constructor = info.constructor.clone();
                    let params = constructor
                        .as_ref()
                        .map_or(&[][..], |signature| signature.params.as_slice());
                    let parameter_names = type_params.iter().copied().collect::<AHashSet<_>>();
                    let mut substitutions = AHashMap::default();
                    if !type_args.is_empty() {
                        let resolved = self.resolve_type_arguments(
                            class.name,
                            type_args,
                            &type_params,
                            *span,
                        )?;
                        substitutions.extend(type_params.iter().copied().zip(resolved));
                    } else if let Some(Type::ClassInstance {
                        name,
                        args: expected_args,
                    }) = expected
                    {
                        if *name == class.name && expected_args.len() == type_params.len() {
                            substitutions.extend(
                                type_params
                                    .iter()
                                    .copied()
                                    .zip(expected_args.iter().cloned()),
                            );
                        }
                    } else if type_params.is_empty() {
                        self.resolve_type_arguments(
                            class.name,
                            type_args,
                            &type_params,
                            *span,
                        )?;
                    }
                    let mut actual_args = Vec::with_capacity(args.len());
                    for (arg, pattern) in args.iter().zip(params) {
                        let resolved = substitute_type(&pattern.ty, &substitutions);
                        let expected = (!contains_type_parameter(&resolved, &parameter_names))
                            .then_some(&resolved);
                        let actual = self.analyze_value_argument(arg, expected)?;
                        infer_type_arguments(
                            &pattern.ty,
                            &actual,
                            &parameter_names,
                            &mut substitutions,
                            arg.span,
                        )?;
                        let resolved = substitute_type(&pattern.ty, &substitutions);
                        if !contains_type_parameter(&resolved, &parameter_names) {
                            self.require_assignable(&resolved, &actual, arg.span)?;
                        }
                        actual_args.push(actual);
                    }
                    let resolved_args = type_params
                        .iter()
                        .map(|parameter| {
                            substitutions.get(parameter).cloned().ok_or_else(|| {
                                AdmittedSemanticError::new(
                                    *span,
                                    format!("cannot infer type argument `{parameter}`"),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for ((arg, pattern), actual) in args.iter().zip(params).zip(&actual_args) {
                        let resolved = substitute_type(&pattern.ty, &substitutions);
                        self.require_assignable(&resolved, actual, arg.span)?;
                    }
                    if type_params.is_empty() {
                        Type::Class(class.name)
                    } else {
                        Type::ClassInstance {
                            name: class.name,
                            args: resolved_args,
                        }
                    }
                }
            }
            Expr {
                kind:
                    ExprKind::Member {
                        object:
                            Expr {
                                kind: ExprKind::Ident(enum_name),
                                ..
                            },
                        property,
                        span,
                    },
                ..
            } if self.declarations.enums.contains_key(enum_name.name) => {
                let value = self
                    .declarations
                    .enums
                    .get(enum_name.name)
                    .and_then(|info| info.variants.get(property.name))
                    .copied()
                    .ok_or_else(|| {
                        AdmittedSemanticError::new(
                            property.span,
                            format!(
                                "enum `{}` has no variant `{}`",
                                enum_name.name, property.name
                            ),
                        )
                    })?;
                self.facts.enum_variant_values.insert(*span, value);
                Type::Enum(enum_name.name)
            }
            Expr {
                kind:
                    ExprKind::Member {
                        object,
                        property,
                        span,
                    },
                ..
            } => self.analyze_member(object, *property, expr.id, *span)?,
            Expr {
                kind:
                    ExprKind::OptionalMember {
                        object,
                        property,
                        span,
                    },
                ..
            } => {
                let object_type = self.analyze_expr(object, None)?;
                let Type::Nullable(inner) = object_type else {
                    return Err(AdmittedSemanticError::new(
                        object.span(),
                        format!(
                            "optional access requires a nullable receiver, found `{object_type}`"
                        ),
                    ));
                };
                let member = self.analyze_member_type(*inner, *property, expr.id, *span)?;
                self.facts
                    .optional_present_types
                    .insert(*span, member.clone());
                optional_result_type(member, *span)?
            }
            Expr {
                kind: ExprKind::Call { callee, args, span },
                ..
            } => {
                if let Some((builtin, result)) =
                    self.analyze_static_namespace_call(callee, args, *span, expected)?
                {
                    self.facts.source_info[expr.id.index()].resolution =
                        ExpressionResolution::Builtin(builtin);
                    result
                } else if self.builtin_namespace_is_unshadowed("Math")
                    && matches!(
                        callee,
                        Expr { kind: ExprKind::Member {
                            object,
                            property: Ident { name: "imul", .. },
                            ..
                        }, .. } if matches!(object, Expr { kind: ExprKind::Ident(Ident { name: "Math", .. }), .. })
                    )
                {
                    let contract = crate::primitive::builtin_call_contract(BuiltinCall::MathImul)
                        .expect("checked Math.imul contract");
                    if args.len() != contract.arity {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!("`Math.imul` expects two arguments, found {}", args.len()),
                        ));
                    }
                    let argument_type = contract
                        .argument
                        .as_ref()
                        .expect("checked Math.imul parameter type");
                    for arg in *args {
                        let actual = self.analyze_value_argument(arg, Some(argument_type))?;
                        self.require_assignable(argument_type, &actual, arg.span)?;
                    }
                    self.facts.source_info[expr.id.index()].resolution =
                        ExpressionResolution::Builtin(BuiltinCall::MathImul);
                    contract.result
                } else if self.builtin_namespace_is_unshadowed("print")
                    && matches!(
                        callee,
                        Expr {
                            kind: ExprKind::Ident(Ident { name: "print", .. }),
                            ..
                        }
                    )
                {
                    let contract = crate::primitive::builtin_call_contract(BuiltinCall::Print)
                        .expect("checked print contract");
                    if args.len() != contract.arity {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!("`print` expects one argument, found {}", args.len()),
                        ));
                    }
                    self.analyze_value_argument(&args[0], None)?;
                    self.facts.source_info[expr.id.index()].resolution =
                        ExpressionResolution::Builtin(BuiltinCall::Print);
                    contract.result
                } else if let Expr {
                    kind:
                        ExprKind::Member {
                            object, property, ..
                        },
                    ..
                } = callee
                {
                    self.analyze_receiver_call(
                        expr.id, callee, object, *property, args, *span, expected,
                    )?
                } else {
                    let callee_type = self.analyze_expr(callee, None)?;
                    self.analyze_call(&callee_type, args, *span, expected, Some(expr.id))?
                }
            }
            Expr {
                kind: ExprKind::ArrowFunction { params, body, .. },
                ..
            } => self.analyze_arrow(params, body, expected)?,
            Expr {
                kind: ExprKind::Unary { op, expr, span },
                ..
            } => {
                let operand = self.analyze_expr(expr, None)?;
                match op {
                    UnaryOp::Neg if operand.is_numeric() => operand,
                    UnaryOp::Not if operand == Type::Bool => Type::Bool,
                    UnaryOp::Neg => {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!("unary `-` requires a numeric operand, found `{operand}`"),
                        ));
                    }
                    UnaryOp::Not => {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            format!("unary `!` requires a bool operand, found `{operand}`"),
                        ));
                    }
                }
            }
            Expr {
                kind: ExprKind::Await { task, span },
                ..
            } => {
                self.require_no_pending_reference(*span)?;
                if self.async_depth == 0 {
                    return Err(AdmittedSemanticError::new(
                        *span,
                        "`await` is only valid inside an async function or method",
                    ));
                }
                let expected_task = expected.map(|value| Type::Task(Box::new(value.clone())));
                let task_type = self.analyze_expr(task, expected_task.as_ref())?;
                let Type::Task(value) = task_type else {
                    return Err(AdmittedSemanticError::new(
                        task.span(),
                        format!("`await` requires a `Task<T>`, found `{task_type}`"),
                    ));
                };
                *value
            }
            Expr {
                kind: ExprKind::Binary { .. },
                ..
            } => {
                return self.analyze_binary_expression(expr, expected);
            }
            Expr {
                kind:
                    ExprKind::TypeCheck {
                        value,
                        target,
                        span,
                    },
                ..
            } => {
                let value_type = self.analyze_expr(value, None)?;
                let target_type = self.resolve_value_type(*target, "type guard")?;
                validate_type_guard(&value_type, &target_type, *span)?;
                self.facts.type_check_types.insert(*span, target_type);
                Type::Bool
            }
            Expr {
                kind:
                    ExprKind::Index {
                        object,
                        index,
                        span,
                    },
                ..
            } => {
                let object_type = self.analyze_expr(object, None)?;
                if is_js_value(&object_type) {
                    let index_type = self.analyze_expr(index, None)?;
                    if !is_js_index_type(&index_type) {
                        return Err(AdmittedSemanticError::new(
                            index.span(),
                            format!(
                                "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                            ),
                        ));
                    }
                } else {
                    let expected_index = index_key_type(&object_type).ok_or_else(|| {
                        AdmittedSemanticError::new(
                            *span,
                            format!("cannot index a value of type `{object_type}`"),
                        )
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                }
                index_value_type(&object_type, false).ok_or_else(|| {
                    AdmittedSemanticError::new(
                        *span,
                        format!("cannot index a value of type `{object_type}`"),
                    )
                })?
            }
            Expr {
                kind:
                    ExprKind::OptionalIndex {
                        object,
                        index,
                        span,
                    },
                ..
            } => {
                let object_type = self.analyze_expr(object, None)?;
                let Type::Nullable(inner) = object_type else {
                    return Err(AdmittedSemanticError::new(
                        object.span(),
                        format!(
                            "optional indexing requires a nullable receiver, found `{object_type}`"
                        ),
                    ));
                };
                if is_js_value(&inner) {
                    let index_type = self.analyze_expr(index, None)?;
                    if !is_js_index_type(&index_type) {
                        return Err(AdmittedSemanticError::new(
                            index.span(),
                            format!(
                                "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                            ),
                        ));
                    }
                } else {
                    let expected_index = index_key_type(&inner).ok_or_else(|| {
                        AdmittedSemanticError::new(*span, format!("cannot index a value of type `{inner}`"))
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                }
                let element = index_value_type(&inner, false).ok_or_else(|| {
                    AdmittedSemanticError::new(*span, format!("cannot index a value of type `{inner}`"))
                })?;
                self.facts
                    .optional_present_types
                    .insert(*span, element.clone());
                optional_result_type(element, *span)?
            }
            Expr {
                kind:
                    ExprKind::If {
                        condition,
                        then_value,
                        else_value,
                        span,
                    },
                ..
            } => {
                let condition_type = self.analyze_expr(condition, Some(&Type::Bool))?;
                self.require_assignable(&Type::Bool, &condition_type, condition.span())?;
                let (then_narrowing, else_narrowing) = self.condition_narrowing(condition)?;
                self.push_scope()?;
                self.apply_narrowing(then_narrowing);
                let then_type = self.analyze_expr(then_value, expected)?;
                self.pop_scope();
                self.push_scope()?;
                self.apply_narrowing(else_narrowing);
                let else_type = self.analyze_expr(else_value, expected)?;
                self.pop_scope();
                common_type(&then_type, &else_type).ok_or_else(|| {
                    AdmittedSemanticError::new(
                        *span,
                        format!(
                            "expression-if arms have incompatible types `{then_type}` and `{else_type}`"
                        ),
                    )
                })?
            }
            Expr {
                kind: ExprKind::Match { value, arms, span },
                ..
            } => self.analyze_match(value, arms, expected, *span)?,
            Expr {
                kind:
                    ExprKind::Assignment {
                        op,
                        target,
                        value,
                        span,
                    },
                ..
            } => {
                if *op != AssignmentOp::Assign && self.is_record_place(target)? {
                    return Err(AdmittedSemanticError::new(
                        target.span(),
                        "record entries currently support only direct `=` assignment; read with `??` before computing an update",
                    ));
                }
                let target_type = self.analyze_lvalue(target)?;
                let value_expected = if *op == AssignmentOp::Nullish {
                    Some(nullish_present_type(&target_type).ok_or_else(|| {
                        AdmittedSemanticError::new(
                            target.span(),
                            format!(
                                "operator `??=` requires a nullable target, found `{target_type}`"
                            ),
                        )
                    })?)
                } else {
                    Some(&target_type)
                };
                let value_type = self.analyze_expr(value, value_expected)?;
                let result_type = if *op == AssignmentOp::Assign {
                    self.require_assignable(&target_type, &value_type, value.span())?;
                    target_type.clone()
                } else if *op == AssignmentOp::Nullish {
                    self.require_assignable(&target_type, &value_type, value.span())?;
                    self.analyze_binary(BinaryOp::Nullish, &target_type, &value_type, *span)?
                } else {
                    let binary_op = assignment_binary_op(*op);
                    let result =
                        self.analyze_binary(binary_op, &target_type, &value_type, *span)?;
                    self.require_assignable(&target_type, &result, *span)?;
                    target_type.clone()
                };
                self.invalidate_assigned_narrowing(target);
                result_type
            }
            Expr {
                kind: ExprKind::Update {
                    target, op, span, ..
                },
                ..
            } => {
                if self.is_record_place(target)? {
                    return Err(AdmittedSemanticError::new(
                        target.span(),
                        "record entries cannot be incremented directly because the key may be absent",
                    ));
                }
                let target_type = self.analyze_lvalue(target)?;
                if !target_type.is_numeric() {
                    return Err(AdmittedSemanticError::new(
                        *span,
                        format!(
                            "operator `{}` requires a numeric target, found `{target_type}`",
                            match op {
                                UpdateOp::Increment => "++",
                                UpdateOp::Decrement => "--",
                            }
                        ),
                    ));
                }
                self.invalidate_assigned_narrowing(target);
                target_type
            }
            Expr {
                kind: ExprKind::Template { parts, .. },
                ..
            } => {
                for part in *parts {
                    if let TemplatePart::Expr(expression) = part {
                        let ty = self.analyze_expr(expression, None)?;
                        if !is_stringable(&ty) {
                            return Err(AdmittedSemanticError::new(
                                expression.span(),
                                format!("type `{ty}` cannot be interpolated into a string"),
                            ));
                        }
                    }
                }
                Type::String
            }
        };

        self.facts.expression_types[expr.id.index()] = Some(ty.clone());
        Ok(ty)
    }

    /// Binary trees are common even in flat source such as `a + b + c`.
    /// Keep their continuations on the heap rather than retaining the large
    /// expression-checker frame for every operator. Leaves still use the same
    /// contextual checker; no expression is prechecked in a different scope.
    fn analyze_binary_expression(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        let original_scope_depth = self.scopes.len();
        let mut pending = Vec::new();
        let result = (|| {
            let mut next = expression;
            let mut next_expected = expected.cloned();
            'visit: loop {
                self.budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                if let ExprKind::Binary { lhs, .. } = &next.kind {
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut pending,
                        BinaryContinuation::Left {
                            expression: next,
                            expected: next_expected.take(),
                        },
                    )?;
                    self.facts.source_info[next.id.index()] = SourceInfo {
                        expression: Some(next),
                        resolution: ExpressionResolution::None,
                    };
                    next = lhs;
                    continue;
                }

                let mut ty = self.analyze_expr(next, next_expected.as_ref())?;
                let mut narrowing = NarrowingInput::leaf(next);
                loop {
                    self.budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                    match pending.pop() {
                        None => return Ok(ty),
                        Some(BinaryContinuation::Left {
                            expression,
                            expected,
                        }) => {
                            let ExprKind::Binary { op, rhs, .. } = &expression.kind else {
                                unreachable!("binary continuation owns a binary expression")
                            };
                            let narrowed_scope = matches!(op, BinaryOp::And | BinaryOp::Or);
                            next_expected = if narrowed_scope {
                                let (when_true, when_false) = self.narrowing_from_input(narrowing)?;
                                self.push_scope()?;
                                self.apply_narrowing(if *op == BinaryOp::And {
                                    when_true
                                } else {
                                    when_false
                                });
                                Some(Type::Bool)
                            } else if *op == BinaryOp::Nullish {
                                nullish_present_type(&ty).cloned().or(expected)
                            } else {
                                None
                            };
                            self.budget.push(
                                AllocationClass::Scratch,
                                &mut pending,
                                BinaryContinuation::Right {
                                    expression,
                                    left: ty,
                                    left_narrowing: narrowing,
                                    narrowed_scope,
                                },
                            )?;
                            next = rhs;
                            continue 'visit;
                        }
                        Some(BinaryContinuation::Right {
                            expression,
                            left,
                            left_narrowing,
                            narrowed_scope,
                        }) => {
                            if narrowed_scope {
                                self.pop_scope();
                            }
                            let ExprKind::Binary { op, span, .. } = &expression.kind else {
                                unreachable!("binary continuation owns a binary expression")
                            };
                            ty = self.analyze_binary(*op, &left, &ty, *span)?;
                            narrowing = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                                left_narrowing.join(narrowing, expression, *op)
                            } else {
                                NarrowingInput::leaf(expression)
                            };
                            self.facts.expression_types[expression.id.index()] = Some(ty.clone());
                        }
                    }
                }
            }
        })();
        // An error can bypass pending logical RHS continuations. Unwind their
        // lexical/narrowing scopes just as successful continuations do.
        debug_assert!(result.is_err() || self.scopes.len() == original_scope_depth);
        while self.scopes.len() > original_scope_depth {
            self.pop_scope();
        }
        let bytes = pending
            .capacity()
            .checked_mul(std::mem::size_of::<BinaryContinuation<'_, '_>>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .expect("admitted binary continuation capacity fits its original layout");
        drop(pending);
        self.budget
            .release(AllocationClass::Scratch, bytes)
            .expect("binary continuation backing belongs to its callback budget");
        result
    }

    fn analyze_lvalue(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        self.analyze_place(expression, PlaceIntent::Write)
    }

    fn analyze_place(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
        intent: PlaceIntent,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        self.facts.source_info[expression.id.index()] = SourceInfo {
            expression: Some(expression),
            resolution: ExpressionResolution::None,
        };
        let ty = match expression {
            Expr {
                kind: ExprKind::Ident(ident),
                ..
            } => {
                if intent == PlaceIntent::MutableArgument {
                    // Preparation observes initialization, but its storage type
                    // remains the declaration's type even inside a narrowing.
                    self.analyze_expr(expression, None)?;
                }
                let (id, ty) = {
                    let (_, symbol) = self.resolve_with_scope(ident)?;
                    (symbol.id, symbol.ty.clone())
                };
                if intent == PlaceIntent::MutableArgument
                    && self.declarations.symbols[id.0 as usize].is_foreign()
                {
                    return Err(AdmittedSemanticError::new(
                        ident.span,
                        "mutable-reference arguments require lexical storage, not a foreign binding",
                    ));
                }
                self.declarations.assigned_symbols.insert(id);
                if intent == PlaceIntent::MutableArgument {
                    for narrowing in &mut self.narrowings {
                        narrowing.remove(&id);
                        narrowing
                            .retain(|symbol, _| !self.reference_parameters.contains_key(symbol));
                    }
                }
                self.record_identifier(ident.span, id);
                self.facts.source_info[expression.id.index()].resolution =
                    ExpressionResolution::Binding(id);
                ty
            }
            Expr {
                kind:
                    ExprKind::Member {
                        object,
                        property,
                        span,
                    },
                ..
            } => {
                let object_type = if intent == PlaceIntent::MutableArgument {
                    let ty = self.analyze_place(object, intent)?;
                    if !matches!(&ty, Type::Struct(declaration) if self.view().nominal_struct(declaration.identity).is_some_and(|info| info.type_params.is_empty()))
                    {
                        return Err(AdmittedSemanticError::new(
                            *span,
                            "mutable-reference field arguments require a nonnullable, nongeneric value-struct path rooted in a lexical cell",
                        ));
                    }
                    ty
                } else {
                    self.analyze_expr(object, None)?
                };
                if matches!(&object_type, Type::Union(_)) {
                    return Err(AdmittedSemanticError::new(
                        *span,
                        format!(
                            "cannot assign through member `{}` on union `{object_type}`",
                            property.name
                        ),
                    ));
                }
                match object_type {
                    Type::Record(value) => *value,
                    other => self.analyze_member_type(other, *property, expression.id, *span)?,
                }
            }
            Expr {
                kind:
                    ExprKind::Index {
                        object,
                        index,
                        span,
                    },
                ..
            } => {
                if intent == PlaceIntent::MutableArgument {
                    return Err(AdmittedSemanticError::new(
                        *span,
                        "mutable-reference arguments do not yet support indexed or host-backed locations",
                    ));
                }
                let object_type = self.analyze_expr(object, None)?;
                if is_js_value(&object_type) {
                    let index_type = self.analyze_expr(index, None)?;
                    if !is_js_index_type(&index_type) {
                        return Err(AdmittedSemanticError::new(
                            index.span(),
                            format!(
                                "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                            ),
                        ));
                    }
                    Type::TypeParameter("$js")
                } else {
                    let expected_index = index_key_type(&object_type).ok_or_else(|| {
                        AdmittedSemanticError::new(
                            *span,
                            format!("cannot assign through an index on `{object_type}`"),
                        )
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                    index_value_type(&object_type, true).ok_or_else(|| {
                        AdmittedSemanticError::new(
                            *span,
                            format!("cannot assign through an index on `{object_type}`"),
                        )
                    })?
                }
            }
            _ => {
                return Err(AdmittedSemanticError::new(
                    expression.span(),
                    if intent == PlaceIntent::MutableArgument {
                        "mutable-reference argument must name a writable lexical place"
                    } else {
                        "expression is not an assignable location"
                    },
                ));
            }
        };
        self.facts.expression_types[expression.id.index()] = Some(ty.clone());
        Ok(ty)
    }

    fn is_record_place(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
    ) -> Result<bool, AdmittedSemanticError> {
        let object = match expression {
            Expr {
                kind: ExprKind::Member { object, .. },
                ..
            }
            | Expr {
                kind: ExprKind::Index { object, .. },
                ..
            } => object,
            _ => return Ok(false),
        };
        Ok(matches!(self.analyze_expr(object, None)?, Type::Record(_)))
    }

    fn analyze_member(
        &mut self,
        object: &'ast Expr<'ast, 'src>,
        property: Ident<'src>,
        id: SourceNodeId,
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        let object_type = self.analyze_expr(object, None)?;
        self.analyze_member_type(object_type, property, id, span)
    }

    fn analyze_match(
        &mut self,
        value: &'ast Expr<'ast, 'src>,
        arms: &'ast [crate::ast::MatchArm<'ast, 'src>],
        expected: Option<&Type<'src>>,
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        let value_type = self.analyze_expr(value, None)?;
        if !matches!(
            value_type,
            Type::Enum(_) | Type::Int | Type::String | Type::Bool
        ) {
            return Err(AdmittedSemanticError::new(
                value.span(),
                format!("match requires an enum, int, string, or bool value, found `{value_type}`"),
            ));
        }
        let variants = match value_type {
            Type::Enum(enum_name) => Some((
                enum_name,
                self.declarations
                    .enums
                    .get(enum_name)
                    .expect("checked enum type has metadata")
                    .variants
                    .clone(),
            )),
            _ => None,
        };
        let mut covered = AHashSet::default();
        let mut wildcard = false;
        let mut result = None;
        for (index, arm) in arms.iter().enumerate() {
            if wildcard {
                return Err(AdmittedSemanticError::new(
                    arm.pattern.span(),
                    "match arms after `_` are unreachable",
                ));
            }
            match arm.pattern {
                MatchPattern::EnumVariant {
                    enum_name: pattern_enum,
                    variant,
                    span: pattern_span,
                } => {
                    let Some((enum_name, variants)) = &variants else {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("enum pattern cannot match `{value_type}`"),
                        ));
                    };
                    if pattern_enum.name != *enum_name {
                        return Err(AdmittedSemanticError::new(
                            pattern_enum.span,
                            format!(
                                "match pattern uses enum `{}`, expected `{enum_name}`",
                                pattern_enum.name
                            ),
                        ));
                    }
                    let discriminant = variants.get(variant.name).copied().ok_or_else(|| {
                        AdmittedSemanticError::new(
                            variant.span,
                            format!("enum `{enum_name}` has no variant `{}`", variant.name),
                        )
                    })?;
                    let key = format!("enum:{enum_name}:{}", variant.name);
                    if !covered.insert(key) {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{enum_name}.{}`", variant.name),
                        ));
                    }
                    self.facts
                        .enum_variant_values
                        .insert(pattern_span, discriminant);
                }
                MatchPattern::Int(value, pattern_span) => {
                    if value_type != Type::Int {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("integer pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("int:{value}")) {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::String(value, pattern_span) => {
                    if value_type != Type::String {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("string pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("string:{value}")) {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::Bool(value, pattern_span) => {
                    if value_type != Type::Bool {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("boolean pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("bool:{value}")) {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::Wildcard(pattern_span) => {
                    if wildcard || index + 1 != arms.len() {
                        return Err(AdmittedSemanticError::new(
                            pattern_span,
                            "the `_` match arm must appear once and last",
                        ));
                    }
                    wildcard = true;
                }
            }
            let arm_type = self.analyze_expr(&arm.value, expected)?;
            result = Some(match result {
                Some(previous) => common_type(&previous, &arm_type).ok_or_else(|| {
                    AdmittedSemanticError::new(
                        arm.span,
                        format!("match arm has type `{arm_type}`, incompatible with `{previous}`"),
                    )
                })?,
                None => arm_type,
            });
        }
        if !wildcard {
            match (&value_type, variants) {
                (Type::Enum(_), Some((enum_name, variants))) if covered.len() != variants.len() => {
                    let missing = variants
                        .keys()
                        .filter(|variant| !covered.contains(&format!("enum:{enum_name}:{variant}")))
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!("non-exhaustive match on `{enum_name}`; missing {missing}"),
                    ));
                }
                (Type::Bool, _) => {
                    let missing = [false, true]
                        .into_iter()
                        .filter(|value| !covered.contains(&format!("bool:{value}")))
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>();
                    if !missing.is_empty() {
                        return Err(AdmittedSemanticError::new(
                            span,
                            format!(
                                "non-exhaustive match on `bool`; missing {}",
                                missing.join(", ")
                            ),
                        ));
                    }
                }
                (Type::Int | Type::String, _) => {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!("match on `{value_type}` requires a final `_` arm"),
                    ));
                }
                _ => {}
            }
        }
        result.ok_or_else(|| AdmittedSemanticError::new(span, "match expression has no arms"))
    }

    fn analyze_member_type(
        &mut self,
        object_type: Type<'src>,
        property: Ident<'src>,
        id: SourceNodeId,
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        // The checked receiver owns resolution. Re-analysis replaces the fact;
        // later consumers do not recover it from a property spelling.
        let intrinsic = crate::primitive::resolve_member(&object_type, property.name);
        self.facts.source_info[id.index()].resolution =
            intrinsic.map_or(ExpressionResolution::None, ExpressionResolution::Primitive);
        if let Some(contract) = intrinsic.and_then(crate::primitive::intrinsic_call_contract) {
            return Ok(Type::Function(contract.signature()));
        }
        if property.name == "length" {
            if let Type::Union(members) = &object_type {
                if members.iter().all(indexed_collection_has_length) {
                    return Ok(Type::Int);
                }
            }
        }
        match object_type {
            Type::Struct(declaration) => {
                let name = declaration.name;
                let field = self
                    .declarations
                    .structs
                    .get(declaration.identity.index())
                    .and_then(|info| info.fields.get(property.name))
                    .ok_or_else(|| {
                        AdmittedSemanticError::new(
                            property.span,
                            format!("struct `{name}` has no field `{}`", property.name),
                        )
                    })?;
                self.facts.source_info[id.index()].resolution =
                    ExpressionResolution::NominalMember(field.member);
                Ok(field.ty.clone())
            }
            Type::StructInstance { declaration, args } => {
                let name = declaration.name;
                let info = self
                    .declarations
                    .structs
                    .get(declaration.identity.index())
                    .expect("struct instances always have struct metadata");
                let substitutions = substitutions_for(&info.type_params, &args);
                let field = info.fields.get(property.name).ok_or_else(|| {
                    AdmittedSemanticError::new(
                        property.span,
                        format!("struct `{name}` has no field `{}`", property.name),
                    )
                })?;
                self.facts.source_info[id.index()].resolution =
                    ExpressionResolution::NominalMember(field.member);
                Ok(substitute_type(&field.ty, &substitutions))
            }
            Type::Class(name) => {
                let class = self
                    .declarations
                    .classes
                    .get(name)
                    .expect("class types always have class metadata");
                if let Some(field) = class.fields.get(property.name) {
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(field.member);
                    return Ok(field.ty.clone());
                }
                if let Some(method) = class.methods.get(property.name) {
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(method.member);
                    return Ok(method_callable_type(method, &AHashMap::default()));
                }
                Err(AdmittedSemanticError::new(
                    property.span,
                    format!("class `{name}` has no member `{}`", property.name),
                ))
            }
            Type::ClassInstance { name, args } => {
                let class = self
                    .declarations
                    .classes
                    .get(name)
                    .expect("class instances always have class metadata");
                let substitutions = substitutions_for(&class.type_params, &args);
                if let Some(field) = class.fields.get(property.name) {
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(field.member);
                    return Ok(substitute_type(&field.ty, &substitutions));
                }
                if let Some(method) = class.methods.get(property.name) {
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(method.member);
                    return Ok(method_callable_type(method, &substitutions));
                }
                Err(AdmittedSemanticError::new(
                    property.span,
                    format!("class `{name}` has no member `{}`", property.name),
                ))
            }
            Type::Array(_) | Type::String if property.name == "length" => Ok(Type::Int),
            Type::TypeParameter("$js") => match property.name {
                "length" => Ok(Type::Float),
                "message" | "specifier" => Ok(Type::Nullable(Box::new(Type::String))),
                "truthy" | "isArray" | "isObject" => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: Vec::new(),
                        return_type: Box::new(Type::Bool),
                    })))
                }
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("JsValue has no member `{}`", property.name),
                )),
            },
            Type::Map(_, _) | Type::Set(_) if property.name == "size" => Ok(Type::Int),
            Type::ArrayBuffer | Type::SharedArrayBuffer if property.name == "byteLength" => {
                Ok(Type::Int)
            }
            ty if crate::typed_array::is_typed_array_type(&ty)
                && matches!(property.name, "length" | "byteLength" | "byteOffset") =>
            {
                Ok(Type::Int)
            }
            ty if crate::typed_array::is_typed_array_type(&ty) && property.name == "buffer" => {
                Ok(normalize_union(vec![
                    Type::ArrayBuffer,
                    Type::SharedArrayBuffer,
                ]))
            }
            Type::ModuleNamespace(module) => {
                let symbol = if let Some(symbol) = self
                    .facts
                    .dynamic_export_symbols
                    .get(&(module, property.name))
                {
                    self.declarations.symbols.get(symbol.0 as usize)
                } else {
                    let binding = self
                        .facts
                        .module_exports
                        .get(&module)
                        .and_then(|exports| exports.get(property.name))
                        .copied()
                        .ok_or_else(|| {
                            AdmittedSemanticError::new(
                                property.span,
                                format!("dynamic module has no runtime export `{}`", property.name),
                            )
                        })?;
                    self.scopes
                        .first()
                        .and_then(|scope| scope.get(binding))
                        .and_then(|symbol| self.declarations.symbols.get(symbol.0 as usize))
                }
                .ok_or_else(|| {
                    AdmittedSemanticError::new(
                        property.span,
                        format!("dynamic export `{}` is type-only", property.name),
                    )
                })?;
                let ty = symbol.ty.clone();
                self.facts
                    .used_dynamic_exports
                    .insert((module, property.name));
                Ok(ty)
            }
            Type::Task(_) if matches!(property.name, "then" | "catch" | "finally") => Err(
                AdmittedSemanticError::new(span, format!("Task `{}` must be called", property.name)),
            ),
            Type::ModuleLoadError if matches!(property.name, "message" | "specifier") => {
                Ok(Type::String)
            }
            Type::Float => match property.name {
                "abs" | "floor" | "ceil" | "round" | "sqrt" | "sin" | "cos" | "acos" | "exp"
                | "log" | "tan" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Float),
                }))),
                "min" | "max" | "atan2" | "hypot" => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::value(Type::Float)],
                        return_type: Box::new(Type::Float),
                    })))
                }
                "toInt" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Int),
                }))),
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("float has no member `{}`", property.name),
                )),
            },
            Type::Int => match property.name {
                "toString" | "toUnsignedString" => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::defaulted(
                            Type::Int,
                            DefaultValue::Int(10),
                        )],
                        return_type: Box::new(Type::String),
                    })))
                }
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("int has no member `{}`", property.name),
                )),
            },
            Type::Array(element) => match property.name {
                "map" | "filter" | "forEach" | "reduce" | "some" | "every" | "findIndex" => Err(
                    AdmittedSemanticError::new(span, format!("array `{}` must be called", property.name)),
                ),
                "push" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(*element)],
                    return_type: Box::new(Type::Int),
                }))),
                "pop" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: element,
                }))),
                "indexOf" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(*element)],
                    return_type: Box::new(Type::Int),
                }))),
                "includes" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(element.as_ref().clone()),
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(0)),
                    ],
                    return_type: Box::new(Type::Bool),
                }))),
                "join" if is_stringifiable_array_element(&element) => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::defaulted(
                            Type::String,
                            DefaultValue::String(","),
                        )],
                        return_type: Box::new(Type::String),
                    })))
                }
                "join" => Err(AdmittedSemanticError::new(
                    span,
                    format!("array element type `{element}` cannot be joined portably"),
                )),
                "concat" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::Array(element.clone()))],
                    return_type: Box::new(Type::Array(element)),
                }))),
                "copyWithin" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(i32::MAX as i64)),
                    ],
                    return_type: Box::new(Type::Array(element)),
                }))),
                "reverse" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Array(element)),
                }))),
                "slice" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(0)),
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(i32::MAX as i64)),
                    ],
                    return_type: Box::new(Type::Array(element)),
                }))),
                "splice" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::value(Type::Int),
                    ],
                    return_type: Box::new(Type::Array(element)),
                }))),
                "fill" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(*element.clone())],
                    return_type: Box::new(Type::Array(element)),
                }))),
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("array has no member `{}`", property.name),
                )),
            },
            Type::Record(value) => Ok(nullable_type(*value)),
            Type::Map(key, value) => match property.name {
                "get" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(key.as_ref().clone())],
                    return_type: Box::new(nullable_type(value.as_ref().clone())),
                }))),
                "set" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(key.as_ref().clone()),
                        FunctionParameter::value(value.as_ref().clone()),
                    ],
                    return_type: Box::new(Type::Map(key, value)),
                }))),
                "has" | "delete" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(*key)],
                    return_type: Box::new(Type::Bool),
                }))),
                "clear" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Void),
                }))),
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("map has no member `{}`", property.name),
                )),
            },
            Type::Set(element) => match property.name {
                "add" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(element.as_ref().clone())],
                    return_type: Box::new(Type::Set(element)),
                }))),
                "has" | "delete" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(*element)],
                    return_type: Box::new(Type::Bool),
                }))),
                "clear" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Void),
                }))),
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("set has no member `{}`", property.name),
                )),
            },
            Type::ArrayBuffer => {
                buffer_member(property, span, Type::ArrayBuffer).map_err(Into::into)
            }
            Type::SharedArrayBuffer => {
                buffer_member(property, span, Type::SharedArrayBuffer).map_err(Into::into)
            }
            ty if crate::typed_array::is_typed_array_type(&ty) => match property.name {
                "slice" | "subarray" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(i32::MAX as i64)),
                    ],
                    return_type: Box::new(ty),
                }))),
                "set" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(ty.clone()),
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(0)),
                    ],
                    return_type: Box::new(Type::Void),
                }))),
                "fill" => {
                    let element = crate::typed_array::TypedArrayKind::from_type(&ty)
                        .expect("typed-array branch has a kind")
                        .index_value_type();
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![
                            FunctionParameter::value(element),
                            FunctionParameter::defaulted(Type::Int, DefaultValue::Int(0)),
                            FunctionParameter::defaulted(
                                Type::Int,
                                DefaultValue::Int(i32::MAX as i64),
                            ),
                        ],
                        return_type: Box::new(ty),
                    })))
                }
                "copyWithin" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(i32::MAX as i64)),
                    ],
                    return_type: Box::new(ty),
                }))),
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("{ty} has no member `{}`", property.name),
                )),
            },
            Type::String => match property.name {
                "includes" | "startsWith" | "endsWith" => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::value(Type::String)],
                        return_type: Box::new(Type::Bool),
                    })))
                }
                "lastIndexOf" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(Type::String),
                        FunctionParameter::defaulted(Type::Int, DefaultValue::Int(i32::MAX as i64)),
                    ],
                    return_type: Box::new(Type::Int),
                }))),
                "search" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::Regex)],
                    return_type: Box::new(Type::Int),
                }))),
                "codePointLength" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Int),
                }))),
                "truthy" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Bool),
                }))),
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("string has no member `{}`", property.name),
                )),
            },
            Type::Regex => match property.name {
                "source" | "flags" => Ok(Type::String),
                "lastIndex" => Ok(Type::Float),
                "global" | "ignoreCase" | "multiline" | "dotAll" | "sticky" | "unicode" => {
                    Ok(Type::Bool)
                }
                _ => Err(AdmittedSemanticError::new(
                    span,
                    format!("Regex has no member `{}`", property.name),
                )),
            },
            other => Err(AdmittedSemanticError::new(
                span,
                format!("type `{other}` has no member `{}`", property.name),
            )),
        }
    }

    /// Receiver checking owns operation resolution, including contextual
    /// callback signatures that have no standalone method-value type.
    fn analyze_receiver_call(
        &mut self,
        call_node: SourceNodeId,
        member: &'ast Expr<'ast, 'src>,
        object: &'ast Expr<'ast, 'src>,
        property: Ident<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        use crate::primitive::{Intrinsic, ResolvedIntrinsic};
        self.facts.source_info[member.id.index()] = SourceInfo {
            expression: Some(member),
            resolution: ExpressionResolution::None,
        };
        let receiver = self.analyze_expr(object, None)?;
        let (operation, result) = match (receiver, property.name) {
            (Type::Array(element), "map") => (
                Intrinsic::ArrayMap,
                self.analyze_array_map(*element, args, span)?,
            ),
            (Type::Array(element), "filter") => (
                Intrinsic::ArrayFilter,
                self.analyze_array_filter(*element, args, span)?,
            ),
            (Type::Array(element), "forEach") => (
                Intrinsic::ArrayForEach,
                self.analyze_array_for_each(*element, args, span)?,
            ),
            (Type::Array(element), "reduce") => (
                Intrinsic::ArrayReduce,
                self.analyze_array_reduce(*element, args, span)?,
            ),
            (Type::Array(element), "some") => (
                Intrinsic::ArraySome,
                self.analyze_array_predicate(*element, args, property.name, span, Type::Bool)?,
            ),
            (Type::Array(element), "every") => (
                Intrinsic::ArrayEvery,
                self.analyze_array_predicate(*element, args, property.name, span, Type::Bool)?,
            ),
            (Type::Array(element), "findIndex") => (
                Intrinsic::ArrayFindIndex,
                self.analyze_array_predicate(*element, args, property.name, span, Type::Int)?,
            ),
            (Type::Task(value), "then" | "catch" | "finally") => {
                let result = self.analyze_task_call(property.name, *value, args, span)?;
                // A host method with one checked callback; record its
                // contract on the callee like any other checked call.
                let callback = self.facts.expression_types[args[0].expression.id.index()]
                    .clone()
                    .ok_or_else(|| {
                        AdmittedSemanticError::new(args[0].span, "task callback lost its checked type")
                    })?;
                self.facts.expression_types[member.id.index()] =
                    Some(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::value(callback)],
                        return_type: Box::new(result.clone()),
                    })));
                return Ok(result);
            }
            (receiver, _) => {
                let callee =
                    self.analyze_member_type(receiver, property, member.id, member.span())?;
                self.facts.expression_types[member.id.index()] = Some(callee.clone());
                return self.analyze_call(&callee, args, span, expected, Some(call_node));
            }
        };
        // These methods have no standalone value type, but each call was
        // checked against one contextual signature. Record it on the callee so
        // later owners read the checked contract instead of re-deriving it.
        let mut params = Vec::with_capacity(args.len());
        for argument in args {
            let ty = self.facts.expression_types[argument.expression.id.index()]
                .clone()
                .ok_or_else(|| {
                    AdmittedSemanticError::new(argument.span, "array callback argument lost its checked type")
                })?;
            params.push(FunctionParameter::value(ty));
        }
        self.facts.expression_types[member.id.index()] =
            Some(Type::Function(FunctionType::new(FunctionSignature {
                params,
                return_type: Box::new(result.clone()),
            })));
        self.facts.source_info[member.id.index()].resolution =
            ExpressionResolution::Primitive(ResolvedIntrinsic::Method(operation));
        Ok(result)
    }

    fn analyze_array_map(
        &mut self,
        element_type: Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        if args.len() != 1 {
            return Err(AdmittedSemanticError::new(
                span,
                format!(
                    "array `map` expects one callback, found {} arguments",
                    args.len()
                ),
            ));
        }

        let callback_expected = Type::Function(FunctionType::new(FunctionSignature {
            params: vec![FunctionParameter::value(element_type.clone())],
            return_type: Box::new(Type::Void),
        }));
        let callback = self.analyze_value_argument(&args[0], Some(&callback_expected))?;
        let Type::Function(signature) = callback else {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!("array `map` expects a function, found `{callback}`"),
            ));
        };
        self.require_value_parameters(&signature, args[0].span)?;
        if signature.params.len() != 1
            || !is_type_assignable(&signature.params[0].ty, &element_type)
            || !is_type_assignable(&element_type, &signature.params[0].ty)
        {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!(
                    "array `map` callback must accept `{}`, found `{}`",
                    element_type,
                    signature
                        .params
                        .first()
                        .map(|parameter| parameter.ty.to_string())
                        .unwrap_or_else(|| "no parameter".to_string())
                ),
            ));
        }
        Ok(Type::Array(signature.return_type.clone()))
    }

    fn analyze_array_filter(
        &mut self,
        element_type: Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        let signature = self.analyze_array_callback(args, &element_type, "filter", span)?;
        if signature.return_type.as_ref() != &Type::Bool {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!(
                    "array `filter` callback must return `bool`, found `{}`",
                    signature.return_type
                ),
            ));
        }
        Ok(Type::Array(Box::new(element_type)))
    }

    fn analyze_array_for_each(
        &mut self,
        element_type: Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        self.analyze_array_callback(args, &element_type, "forEach", span)?;
        Ok(Type::Void)
    }

    fn analyze_array_predicate(
        &mut self,
        element_type: Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        method: &str,
        span: Span,
        return_type: Type<'src>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        let signature = self.analyze_array_callback(args, &element_type, method, span)?;
        if signature.return_type.as_ref() != &Type::Bool {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!(
                    "array `{method}` callback must return `bool`, found `{}`",
                    signature.return_type
                ),
            ));
        }
        Ok(return_type)
    }

    fn analyze_array_reduce(
        &mut self,
        element_type: Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        if args.len() != 2 {
            return Err(AdmittedSemanticError::new(
                span,
                format!(
                    "array `reduce` expects a callback and initial value, found {} arguments",
                    args.len()
                ),
            ));
        }
        let accumulator = self.analyze_value_argument(&args[1], None)?;
        let expected = Type::Function(FunctionType::new(FunctionSignature {
            params: vec![
                FunctionParameter::value(accumulator.clone()),
                FunctionParameter::value(element_type),
            ],
            return_type: Box::new(accumulator.clone()),
        }));
        let callback = self.analyze_value_argument(&args[0], Some(&expected))?;
        let Type::Function(signature) = callback else {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                "array `reduce` expects a function callback",
            ));
        };
        self.require_value_parameters(&signature, args[0].span)?;
        if signature.params.len() != 2
            || !signature
                .params
                .iter()
                .zip(match &expected {
                    Type::Function(expected) => &expected.params,
                    _ => unreachable!(),
                })
                .all(|(actual, expected)| {
                    actual.passing == expected.passing && actual.ty == expected.ty
                })
            || !is_type_assignable(&accumulator, &signature.return_type)
        {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                "array `reduce` callback signature does not match its accumulator and element types",
            ));
        }
        Ok(accumulator)
    }

    fn analyze_array_callback(
        &mut self,
        args: &'ast [Argument<'ast, 'src>],
        element_type: &Type<'src>,
        method: &str,
        span: Span,
    ) -> Result<FunctionType<'src>, AdmittedSemanticError> {
        if args.len() != 1 {
            return Err(AdmittedSemanticError::new(
                span,
                format!(
                    "array `{method}` expects one callback, found {} arguments",
                    args.len()
                ),
            ));
        }
        let expected = Type::Function(FunctionType::new(FunctionSignature {
            params: vec![FunctionParameter::value(element_type.clone())],
            return_type: Box::new(Type::Void),
        }));
        let callback = self.analyze_value_argument(&args[0], Some(&expected))?;
        let Type::Function(signature) = callback else {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!("array `{method}` expects a function callback"),
            ));
        };
        self.require_value_parameters(&signature, args[0].span)?;
        if signature.params.len() != 1 || signature.params[0].ty != *element_type {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!("array `{method}` callback parameter must be `{element_type}`"),
            ));
        }
        Ok(signature)
    }

    fn require_value_parameters(
        &self,
        signature: &FunctionType<'src>,
        span: Span,
    ) -> Result<(), AdmittedSemanticError> {
        signature
            .validate_parameters()
            .map_err(|message| AdmittedSemanticError::new(span, message))?;
        if signature
            .params
            .iter()
            .any(|parameter| parameter.passing != ParameterPassing::Value)
        {
            return Err(AdmittedSemanticError::new(
                span,
                "this callable boundary does not support mutable-reference parameters",
            ));
        }
        Ok(())
    }

    fn require_value_arguments(
        &self,
        args: &[Argument<'ast, 'src>],
        message: &'static str,
    ) -> Result<(), AdmittedSemanticError> {
        if let Some(argument) = args
            .iter()
            .find(|argument| argument.passing != ParameterPassing::Value)
        {
            return Err(AdmittedSemanticError::new(argument.span, message));
        }
        Ok(())
    }

    fn analyze_value_argument(
        &mut self,
        argument: &'ast Argument<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        if argument.passing != ParameterPassing::Value {
            return Err(AdmittedSemanticError::new(
                argument.span,
                "a value parameter cannot receive a mutable-reference argument",
            ));
        }
        self.analyze_expr(&argument.expression, expected)
    }

    fn require_no_pending_reference(&self, span: Span) -> Result<(), AdmittedSemanticError> {
        if self.pending_references {
            return Err(AdmittedSemanticError::new(
                span,
                "cannot suspend after preparing a mutable-reference argument",
            ));
        }
        if self.current_reference_formals {
            return Err(AdmittedSemanticError::new(
                span,
                "cannot suspend in a callable with mutable-reference parameters",
            ));
        }
        Ok(())
    }

    fn analyze_call(
        &mut self,
        callee: &Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
        expected_return: Option<&Type<'src>>,
        call_node: Option<SourceNodeId>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        if is_js_value(callee) {
            let js = Type::TypeParameter("$js");
            for arg in args {
                let actual = self.analyze_value_argument(arg, Some(&js))?;
                self.require_assignable(&js, &actual, arg.span)?;
            }
            return Ok(js);
        }
        if let Type::GenericFunction(function) = callee {
            return self.analyze_generic_call(function, args, span, expected_return, call_node);
        }
        if let Type::Union(members) = callee {
            let mut signatures = Vec::new();
            for member in members {
                let Type::Function(signature) = member else {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!("cannot call a value of type `{callee}`"),
                    ));
                };
                self.require_value_parameters(signature, span)?;
                signatures.push(signature);
            }
            if !signatures
                .iter()
                .all(|signature| args.len() == signature.params.len())
            {
                return Err(AdmittedSemanticError::new(
                    span,
                    format!(
                        "cannot call a value of type `{callee}` with {} arguments; union calls require every argument explicitly",
                        args.len()
                    ),
                ));
            }
            for (index, arg) in args.iter().enumerate() {
                let expected = &signatures[0].params[index].ty;
                let contextual = signatures
                    .iter()
                    .all(|signature| &signature.params[index].ty == expected)
                    .then_some(expected);
                let actual = self.analyze_value_argument(arg, contextual)?;
                for signature in &signatures {
                    let expected = &signature.params[index].ty;
                    self.require_assignable(expected, &actual, arg.span)?;
                }
            }
            let returns = signatures
                .iter()
                .map(|signature| (*signature.return_type).clone())
                .collect::<Vec<_>>();
            return Ok(normalize_union(returns));
        }
        let Type::Function(signature) = callee else {
            return Err(AdmittedSemanticError::new(
                span,
                format!("cannot call a value of type `{callee}`"),
            ));
        };
        signature
            .validate_parameters()
            .map_err(|message| AdmittedSemanticError::new(span, message))?;
        if !signature.accepts_arity(args.len()) {
            return Err(AdmittedSemanticError::new(
                span,
                format!(
                    "function expects {} to {} arguments, found {}",
                    signature.required_params(),
                    signature.params.len(),
                    args.len()
                ),
            ));
        }
        self.require_omitted_defaults_in_scope(signature, args.len(), span)?;
        let outer_pending = self.pending_references;
        let result = (|| {
            for (arg, parameter) in args.iter().zip(&signature.params) {
                if arg.passing != parameter.passing {
                    return Err(AdmittedSemanticError::new(
                        arg.span,
                        if parameter.passing == ParameterPassing::MutableReference {
                            "mutable-reference parameter requires an explicit `ref` place argument"
                        } else {
                            "a value parameter cannot receive a mutable-reference argument"
                        },
                    ));
                }
                if parameter.passing == ParameterPassing::MutableReference {
                    let actual =
                        self.analyze_place(&arg.expression, PlaceIntent::MutableArgument)?;
                    if actual != parameter.ty {
                        return Err(AdmittedSemanticError::new(
                            arg.span,
                            format!(
                                "mutable-reference storage type must be exactly `{}`, found `{actual}`",
                                parameter.ty
                            ),
                        ));
                    }
                    self.pending_references = true;
                } else {
                    let actual = self.analyze_value_argument(arg, Some(&parameter.ty))?;
                    self.require_assignable(&parameter.ty, &actual, arg.span)?;
                }
            }
            Ok(())
        })();
        self.pending_references = outer_pending;
        result?;
        Ok((*signature.return_type).clone())
    }

    fn require_omitted_defaults_in_scope(
        &self,
        signature: &FunctionType<'src>,
        provided: usize,
        span: Span,
    ) -> Result<(), AdmittedSemanticError> {
        for default in signature
            .params
            .iter()
            .skip(provided)
            .filter_map(|parameter| parameter.default.as_ref())
        {
            self.require_default_in_scope(default, span)?;
        }
        Ok(())
    }

    fn require_default_in_scope(
        &self,
        default: &DefaultValue<'src>,
        span: Span,
    ) -> Result<(), AdmittedSemanticError> {
        let symbol = match default {
            DefaultValue::Symbol(symbol) => Some(*symbol),
            DefaultValue::PendingIdentifier { expression, .. } => {
                match self.view().expression_resolution(*expression) {
                    ExpressionResolution::Binding(symbol) => Some(symbol),
                    _ => None,
                }
            }
            DefaultValue::Array(values) => {
                for value in values {
                    self.require_default_in_scope(value, span)?;
                }
                None
            }
            DefaultValue::Struct { values, .. } => {
                for value in values {
                    self.require_default_in_scope(value, span)?;
                }
                None
            }
            DefaultValue::NewClass { args, .. } => {
                for argument in args {
                    self.require_default_in_scope(argument, span)?;
                }
                None
            }
            DefaultValue::Int(_)
            | DefaultValue::Float(_)
            | DefaultValue::String(_)
            | DefaultValue::Bool(_)
            | DefaultValue::Null
            | DefaultValue::Undefined
            | DefaultValue::Parameter(_)
            | DefaultValue::PendingUndefined { .. }
            | DefaultValue::Arrow(_) => None,
        };
        if symbol.is_some_and(|symbol| {
            !self
                .scopes
                .iter()
                .any(|scope| scope.values().any(|candidate| *candidate == symbol))
        }) {
            return Err(AdmittedSemanticError::new(
                span,
                "parameter default depends on a local binding that is unavailable at this call site",
            ));
        }
        Ok(())
    }

    fn analyze_static_namespace_call(
        &mut self,
        callee: &'ast Expr<'ast, 'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Option<(BuiltinCall, Type<'src>)>, AdmittedSemanticError> {
        let Expr {
            kind:
                ExprKind::Member {
                    object:
                        Expr {
                            kind: ExprKind::Ident(namespace),
                            ..
                        },
                    property,
                    ..
                },
            ..
        } = callee
        else {
            return Ok(None);
        };
        if !self.builtin_namespace_is_unshadowed(namespace.name) {
            return Ok(None);
        }
        match (namespace.name, property.name) {
            ("Object", "keys" | "values") => {
                let [record] = args else {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!("`Object.{}` expects one record", property.name),
                    ));
                };
                let actual = self.analyze_value_argument(record, None)?;
                let Type::Record(value) = actual else {
                    return Err(AdmittedSemanticError::new(
                        record.span,
                        format!(
                            "`Object.{}` requires a `Record<T>`, found `{actual}`",
                            property.name
                        ),
                    ));
                };
                Ok(Some((
                    if property.name == "keys" {
                        BuiltinCall::ObjectKeys
                    } else {
                        BuiltinCall::ObjectValues
                    },
                    if property.name == "keys" {
                        Type::Array(Box::new(Type::String))
                    } else {
                        Type::Array(value)
                    },
                )))
            }
            ("Object", "hasOwn") => {
                let [record, key] = args else {
                    return Err(AdmittedSemanticError::new(
                        span,
                        "`Object.hasOwn` expects a record and string key",
                    ));
                };
                let actual = self.analyze_value_argument(record, None)?;
                if !matches!(actual, Type::Record(_)) {
                    return Err(AdmittedSemanticError::new(
                        record.span,
                        format!("`Object.hasOwn` requires a `Record<T>`, found `{actual}`"),
                    ));
                }
                let key_type = self.analyze_value_argument(key, Some(&Type::String))?;
                self.require_assignable(&Type::String, &key_type, key.span)?;
                Ok(Some((BuiltinCall::ObjectHasOwn, Type::Bool)))
            }
            ("Object", "assign") => {
                let [target, source] = args else {
                    return Err(AdmittedSemanticError::new(
                        span,
                        "`Object.assign` expects two records",
                    ));
                };
                let target_type = self.analyze_value_argument(target, None)?;
                let Type::Record(_) = target_type else {
                    return Err(AdmittedSemanticError::new(
                        target.span,
                        format!(
                            "`Object.assign` target must be a `Record<T>`, found `{target_type}`"
                        ),
                    ));
                };
                let source_type = self.analyze_value_argument(source, Some(&target_type))?;
                if source_type != target_type {
                    return Err(AdmittedSemanticError::new(
                        source.span,
                        format!(
                            "`Object.assign` source has type `{source_type}`, expected `{target_type}`"
                        ),
                    ));
                }
                Ok(Some((BuiltinCall::ObjectAssign, target_type)))
            }
            ("JSON", "stringify") => {
                let [value] = args else {
                    return Err(AdmittedSemanticError::new(
                        span,
                        "`JSON.stringify` expects one value",
                    ));
                };
                let actual = self.analyze_value_argument(value, None)?;
                if !json_stringify_type_supported(&actual) {
                    return Err(AdmittedSemanticError::new(
                        value.span,
                        format!("`JSON.stringify` does not support `{actual}` portably"),
                    ));
                }
                Ok(Some((BuiltinCall::JsonStringify, Type::String)))
            }
            ("JSON", "parse") => {
                let [value] = args else {
                    return Err(AdmittedSemanticError::new(span, "`JSON.parse` expects one string"));
                };
                let actual = self.analyze_value_argument(value, Some(&Type::String))?;
                self.require_assignable(&Type::String, &actual, value.span)?;
                Ok(Some((BuiltinCall::JsonParse, Type::TypeParameter("$js"))))
            }
            ("Task", "resolve") => {
                let [value] = args else {
                    return Err(AdmittedSemanticError::new(span, "`Task.resolve` expects one value"));
                };
                let expected_value = match expected {
                    Some(Type::Task(value)) => Some(value.as_ref()),
                    _ => None,
                };
                let value = self.analyze_value_argument(value, expected_value)?;
                Ok(Some((
                    BuiltinCall::TaskResolve,
                    Type::Task(Box::new(value)),
                )))
            }
            ("Task", "reject") => {
                let [reason] = args else {
                    return Err(AdmittedSemanticError::new(span, "`Task.reject` expects one reason"));
                };
                let reason_type = self.analyze_value_argument(reason, None)?;
                if reason_type == Type::Void {
                    return Err(AdmittedSemanticError::new(
                        reason.span,
                        "`Task.reject` reason cannot be `void`",
                    ));
                }
                let Some(Type::Task(value)) = expected else {
                    return Err(AdmittedSemanticError::new(
                        span,
                        "cannot infer rejected task value type; provide an expected `Task<T>` type",
                    ));
                };
                Ok(Some((BuiltinCall::TaskReject, Type::Task(value.clone()))))
            }
            ("Task", "all") => {
                let [tasks] = args else {
                    return Err(AdmittedSemanticError::new(
                        span,
                        "`Task.all` expects one task array",
                    ));
                };
                let tasks = self.analyze_value_argument(tasks, None)?;
                let Type::Array(ref task) = tasks else {
                    return Err(AdmittedSemanticError::new(
                        args[0].span,
                        format!("`Task.all` requires a `Task<T>[]`, found `{tasks}`"),
                    ));
                };
                let Type::Task(value) = task.as_ref() else {
                    return Err(AdmittedSemanticError::new(
                        args[0].span,
                        format!("`Task.all` requires a `Task<T>[]`, found `{tasks}`"),
                    ));
                };
                Ok(Some((
                    BuiltinCall::TaskAll,
                    Type::Task(Box::new(Type::Array(value.clone()))),
                )))
            }
            ("JS", method) => self.analyze_javascript_builtin(method, args, span, expected),
            _ => Ok(None),
        }
    }

    fn builtin_namespace_is_unshadowed(&self, name: &str) -> bool {
        !self
            .scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn analyze_javascript_builtin(
        &mut self,
        method: &str,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Option<(BuiltinCall, Type<'src>)>, AdmittedSemanticError> {
        let js = Type::TypeParameter("$js");
        let require_arity = |expected: std::ops::RangeInclusive<usize>| {
            if expected.contains(&args.len()) {
                Ok(())
            } else {
                let start = *expected.start();
                let end = *expected.end();
                let count = if start == end {
                    start.to_string()
                } else if end == usize::MAX {
                    // The builtin forwards its arguments to the callee, so the
                    // only real bound is the lower one.
                    format!("at least {start}")
                } else {
                    format!("{start} to {end}")
                };
                Err(AdmittedSemanticError::new(
                    span,
                    format!(
                        "`JS.{method}` expects {count} arguments, found {}",
                        args.len()
                    ),
                ))
            }
        };

        let (builtin, result, expected_args): (BuiltinCall, Type<'src>, Vec<Type<'src>>) =
            match method {
                "object" => {
                    if !args.len().is_multiple_of(2) {
                        return Err(AdmittedSemanticError::new(
                            span,
                            format!(
                                "`JS.object` expects an even number of key/value arguments, found {}",
                                args.len()
                            ),
                        ));
                    }
                    let mut expected_args = Vec::with_capacity(args.len());
                    for index in 0..args.len() {
                        if index % 2 == 0 {
                            expected_args.push(Type::String);
                        } else {
                            expected_args.push(js.clone());
                        }
                    }
                    (BuiltinCall::JsObject, js.clone(), expected_args)
                }
                "array" => {
                    // Variadic, like `JS.object`. Without it there is no way to
                    // write a `JsValue` array literal, so ports build every
                    // array by allocating an empty one and pushing into it.
                    let expected_args = vec![js.clone(); args.len()];
                    (BuiltinCall::JsArray, js.clone(), expected_args)
                }
                "undefined" => {
                    require_arity(0..=0)?;
                    (BuiltinCall::JsUndefined, js.clone(), Vec::new())
                }
                "typeOf" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsTypeOf, Type::String, vec![js.clone()])
                }
                "isNullish" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsNullish, Type::Bool, vec![js.clone()])
                }
                "isFalse" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsFalse, Type::Bool, vec![js.clone()])
                }
                "isUndefined" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsUndefined, Type::Bool, vec![js.clone()])
                }
                "string" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsString, Type::String, vec![js.clone()])
                }
                "number" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsNumber, Type::Float, vec![js.clone()])
                }
                "add" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsAdd, js.clone(), vec![js.clone(), js.clone()])
                }
                "mod" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsMod, js.clone(), vec![js.clone(), js.clone()])
                }
                "lessThan" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsLessThan,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "lessThanOrEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsLessThanOrEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "greaterThan" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsGreaterThan,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "greaterThanOrEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsGreaterThanOrEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "assume" => {
                    require_arity(1..=1)?;
                    let result = expected.cloned().ok_or_else(|| {
                        AdmittedSemanticError::new(
                            span,
                            "cannot infer `JS.assume` result type; provide an expected type",
                        )
                    })?;
                    if result.is_void() {
                        return Err(AdmittedSemanticError::new(
                            span,
                            "`JS.assume` result cannot be `void`",
                        ));
                    }
                    (BuiltinCall::JsAssume, result, vec![js.clone()])
                }
                "strictEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStrictEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "strictNotEqual" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStrictNotEqual,
                        Type::Bool,
                        vec![js.clone(), js.clone()],
                    )
                }
                "or" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsOr, js.clone(), vec![js.clone(), js.clone()])
                }
                "and" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsAnd, js.clone(), vec![js.clone(), js.clone()])
                }
                "call" => {
                    // The lowering forwards `values[1..]` verbatim, so the argument
                    // count is bounded by the callee, not by this builtin.
                    require_arity(2..=usize::MAX)?;
                    (
                        BuiltinCall::JsCall,
                        js.clone(),
                        vec![js.clone(); args.len()],
                    )
                }
                "construct" => {
                    require_arity(1..=usize::MAX)?;
                    (
                        BuiltinCall::JsConstruct,
                        js.clone(),
                        vec![js.clone(); args.len()],
                    )
                }
                "invoke" => {
                    require_arity(2..=usize::MAX)?;
                    let mut types = vec![js.clone(), Type::String];
                    types.resize(args.len(), js.clone());
                    (BuiltinCall::JsInvoke, js.clone(), types)
                }
                "apply" => {
                    require_arity(3..=3)?;
                    (BuiltinCall::JsApply, js.clone(), vec![js.clone(); 3])
                }
                "method0" | "method1" | "method2" | "method3" | "method4" | "method5"
                | "method6" | "method7" | "method8" | "method9" | "method10" | "methodRest"
                | "staticRest" => {
                    require_arity(1..=1)?;
                    let parameter_count = match method {
                        "method0" | "staticRest" => 1,
                        "method1" | "methodRest" => 2,
                        "method2" => 3,
                        "method3" => 4,
                        "method4" => 5,
                        "method5" => 6,
                        "method6" => 7,
                        "method7" => 8,
                        "method8" => 9,
                        "method9" => 10,
                        "method10" => 11,
                        _ => unreachable!(),
                    };
                    let callback = Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::value(js.clone()); parameter_count],
                        return_type: Box::new(js.clone()),
                    }));
                    (
                        match method {
                            "method0" => BuiltinCall::JsMethod0,
                            "method1" => BuiltinCall::JsMethod1,
                            "method2" => BuiltinCall::JsMethod2,
                            "method3" => BuiltinCall::JsMethod3,
                            "method4" => BuiltinCall::JsMethod4,
                            "method5" => BuiltinCall::JsMethod5,
                            "method6" => BuiltinCall::JsMethod6,
                            "method7" => BuiltinCall::JsMethod7,
                            "method8" => BuiltinCall::JsMethod8,
                            "method9" => BuiltinCall::JsMethod9,
                            "method10" => BuiltinCall::JsMethod10,
                            "methodRest" => BuiltinCall::JsMethodRest,
                            "staticRest" => BuiltinCall::JsStaticRest,
                            _ => unreachable!(),
                        },
                        js.clone(),
                        vec![callback],
                    )
                }
                "get" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsGet,
                        js.clone(),
                        vec![js.clone(), Type::String],
                    )
                }
                "set" => {
                    require_arity(3..=3)?;
                    (
                        BuiltinCall::JsSet,
                        Type::Void,
                        vec![js.clone(), Type::String, js.clone()],
                    )
                }
                "delete" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsDelete,
                        Type::Void,
                        vec![js.clone(), js.clone()],
                    )
                }
                "has" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsHas,
                        Type::Bool,
                        vec![js.clone(), Type::String],
                    )
                }
                "in" => {
                    require_arity(2..=2)?;
                    (BuiltinCall::JsIn, Type::Bool, vec![js.clone(), js.clone()])
                }
                "box" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsBox, js.clone(), vec![js.clone()])
                }
                "push" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayPush,
                        Type::Float,
                        vec![js.clone(), js.clone()],
                    )
                }
                "pop" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsArrayPop, js.clone(), vec![js.clone()])
                }
                "slice" => {
                    require_arity(1..=3)?;
                    let mut types = vec![js.clone()];
                    types.resize(args.len(), Type::Float);
                    (BuiltinCall::JsArraySlice, js.clone(), types)
                }
                "indexOf" => {
                    require_arity(2..=3)?;
                    let mut types = vec![js.clone(), js.clone()];
                    if args.len() == 3 {
                        types.push(Type::Float);
                    }
                    (BuiltinCall::JsArrayIndexOf, Type::Float, types)
                }
                "sort" => {
                    require_arity(1..=2)?;
                    (
                        BuiltinCall::JsArraySort,
                        js.clone(),
                        vec![js.clone(); args.len()],
                    )
                }
                "splice" => {
                    require_arity(2..=4)?;
                    let mut types = vec![js.clone(), Type::Float];
                    if args.len() >= 3 {
                        types.push(Type::Float);
                    }
                    if args.len() == 4 {
                        types.push(js.clone());
                    }
                    (BuiltinCall::JsArraySplice, js.clone(), types)
                }
                "concatApply" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayConcatApply,
                        js.clone(),
                        vec![js.clone(), js.clone()],
                    )
                }
                "join" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayJoin,
                        Type::String,
                        vec![js.clone(), Type::String],
                    )
                }
                "shift" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsArrayShift, js.clone(), vec![js.clone()])
                }
                "unshift" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsArrayUnshift,
                        Type::Float,
                        vec![js.clone(), js.clone()],
                    )
                }
                "isArray" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsIsArray, Type::Bool, vec![js.clone()])
                }
                "stringSlice" => {
                    require_arity(2..=3)?;
                    let mut types = vec![Type::String, Type::Float];
                    if args.len() == 3 {
                        types.push(Type::Float);
                    }
                    (BuiltinCall::JsStringSlice, Type::String, types)
                }
                "stringIndexOf" => {
                    require_arity(2..=3)?;
                    let mut types = vec![Type::String, Type::String];
                    if args.len() == 3 {
                        types.push(Type::Float);
                    }
                    (BuiltinCall::JsStringIndexOf, Type::Float, types)
                }
                "stringReplace" => {
                    require_arity(3..=3)?;
                    (
                        BuiltinCall::JsStringReplace,
                        Type::String,
                        vec![Type::String, Type::Regex, js.clone()],
                    )
                }
                "stringMatch" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStringMatch,
                        js.clone(),
                        vec![Type::String, Type::Regex],
                    )
                }
                "stringSplit" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsStringSplit,
                        Type::Array(Box::new(Type::String)),
                        vec![Type::String, Type::String],
                    )
                }
                "regexTest" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsRegexTest,
                        Type::Bool,
                        vec![Type::Regex, js.clone()],
                    )
                }
                "regexExec" => {
                    require_arity(2..=2)?;
                    (
                        BuiltinCall::JsRegexExec,
                        js.clone(),
                        vec![Type::Regex, js.clone()],
                    )
                }
                "encodeURI" => {
                    require_arity(1..=1)?;
                    (BuiltinCall::JsEncodeURI, Type::String, vec![Type::String])
                }
                "encodeURIComponent" => {
                    require_arity(1..=1)?;
                    (
                        BuiltinCall::JsEncodeURIComponent,
                        Type::String,
                        vec![Type::String],
                    )
                }
                _ => return Ok(None),
            };

        for (argument, expected) in args.iter().zip(&expected_args) {
            let actual = self.analyze_value_argument(argument, Some(expected))?;
            self.require_assignable(expected, &actual, argument.span)?;
        }
        Ok(Some((builtin, result)))
    }

    fn analyze_task_call(
        &mut self,
        method: &str,
        value: Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        if args.len() != 1 {
            return Err(AdmittedSemanticError::new(
                span,
                format!("Task `{method}` expects one callback, found {}", args.len()),
            ));
        }
        let parameters = match method {
            "then" => vec![FunctionParameter::value(value.clone())],
            "catch" => vec![FunctionParameter::value(Type::TypeParameter("$js"))],
            "finally" => Vec::new(),
            _ => unreachable!("task call dispatch validates the method name"),
        };
        let expected = Type::Function(FunctionType::new(FunctionSignature {
            params: parameters,
            return_type: Box::new(Type::Void),
        }));
        let callback = self.analyze_value_argument(&args[0], Some(&expected))?;
        let Type::Function(signature) = callback else {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!("Task `{method}` expects a function callback"),
            ));
        };
        self.require_value_parameters(&signature, args[0].span)?;
        let Type::Function(expected_signature) = &expected else {
            unreachable!()
        };
        if signature.params.len() != expected_signature.params.len()
            || !signature
                .params
                .iter()
                .zip(&expected_signature.params)
                .all(|(actual, expected)| {
                    actual.ty == expected.ty && actual.passing == expected.passing
                })
        {
            return Err(AdmittedSemanticError::new(
                args[0].span,
                format!("Task `{method}` callback has an incompatible parameter list"),
            ));
        }
        if method == "finally" {
            return Ok(Type::Task(Box::new(value)));
        }
        let returned = match signature.return_type.as_ref() {
            Type::Task(inner) => inner.as_ref().clone(),
            returned => returned.clone(),
        };
        Ok(Type::Task(Box::new(if method == "catch" {
            normalize_union(vec![value, returned])
        } else {
            returned
        })))
    }

    fn analyze_generic_call(
        &mut self,
        function: &GenericFunctionType<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
        expected_return: Option<&Type<'src>>,
        call_node: Option<SourceNodeId>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        self.require_value_parameters(&function.signature, span)?;
        if !function.signature.accepts_arity(args.len()) {
            return Err(AdmittedSemanticError::new(
                span,
                format!(
                    "function expects {} to {} arguments, found {}",
                    function.signature.required_params(),
                    function.signature.params.len(),
                    args.len()
                ),
            ));
        }
        self.require_omitted_defaults_in_scope(&function.signature, args.len(), span)?;
        let parameters = function
            .type_params
            .iter()
            .copied()
            .collect::<AHashSet<_>>();
        let mut substitutions = AHashMap::default();
        if let Some(expected_return) = expected_return {
            infer_type_arguments(
                &function.signature.return_type,
                expected_return,
                &parameters,
                &mut substitutions,
                span,
            )?;
        }
        let mut actual_args = Vec::with_capacity(args.len());
        for (arg, pattern) in args.iter().zip(&function.signature.params) {
            let partially_resolved = substitute_type(&pattern.ty, &substitutions);
            let expected = (!contains_type_parameter(&partially_resolved, &parameters))
                .then_some(&partially_resolved);
            let actual = self.analyze_value_argument(arg, expected)?;
            infer_type_arguments(
                &pattern.ty,
                &actual,
                &parameters,
                &mut substitutions,
                arg.span,
            )?;
            let resolved = substitute_type(&pattern.ty, &substitutions);
            if !contains_type_parameter(&resolved, &parameters) {
                self.require_assignable(&resolved, &actual, arg.span)?;
            }
            actual_args.push(actual);
        }
        for parameter in &function.type_params {
            if !substitutions.contains_key(parameter) {
                return Err(AdmittedSemanticError::new(
                    span,
                    format!("cannot infer type argument `{parameter}`"),
                ));
            }
        }
        for ((arg, pattern), actual) in args
            .iter()
            .zip(&function.signature.params)
            .zip(&actual_args)
        {
            let resolved = substitute_type(&pattern.ty, &substitutions);
            self.require_assignable(&resolved, actual, arg.span)?;
        }
        let Type::Function(signature) =
            substitute_type(&Type::Function(function.signature.clone()), &substitutions)
        else {
            unreachable!("substitution preserves callable signature kind")
        };
        if let Some(call_node) = call_node {
            self.facts.call_instantiations.insert(
                call_node,
                CheckedCallInstantiation {
                    type_arguments: function
                        .type_params
                        .iter()
                        .map(|name| substitutions[name].clone())
                        .collect(),
                    signature: signature.clone(),
                },
            );
        }
        Ok((*signature.return_type).clone())
    }

    fn analyze_arrow(
        &mut self,
        params: &'ast [crate::ast::Param<'ast, 'src>],
        body: &'ast ArrowBody<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        self.check_module_defaults(params)?;
        let expected_signature = match expected {
            Some(Type::Function(signature)) => Some(signature),
            _ => None,
        };
        if let Some(signature) = expected_signature {
            signature.validate_parameters().map_err(|message| {
                AdmittedSemanticError::new(
                    params.first().map_or(Span::empty(0), |param| param.span),
                    message,
                )
            })?;
            if params.len() != signature.params.len() {
                return Err(AdmittedSemanticError::new(
                    params.first().map_or(Span::empty(0), |param| param.span),
                    format!(
                        "callback expects {} parameters, found {}",
                        signature.params.len(),
                        params.len()
                    ),
                ));
            }
        }

        let outer_pending = std::mem::take(&mut self.pending_references);
        let outer_formals = std::mem::replace(
            &mut self.current_reference_formals,
            params
                .iter()
                .any(|parameter| parameter.parameter.passing == ParameterPassing::MutableReference),
        );
        self.callable_depth += 1;
        self.push_scope()?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.generator_contexts,
            None,
        )?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.constructor_classes,
            None,
        )?;
        let mut parameters = Vec::with_capacity(params.len());
        for (index, param) in params.iter().enumerate() {
            let ty = if param.parameter.ty.is_auto() {
                expected_signature
                    .and_then(|signature| signature.params.get(index))
                    .map(|parameter| parameter.ty.clone())
                    .ok_or_else(|| {
                        AdmittedSemanticError::new(
                            param.parameter.ty.span,
                            "`auto` arrow parameters require a contextual callback type",
                        )
                    })?
            } else {
                self.resolve_value_type(param.parameter.ty, "arrow parameter")?
            };
            if let Some(parameter) = expected_signature.and_then(|sig| sig.params.get(index)) {
                let expected = &parameter.ty;
                if parameter.passing != param.parameter.passing
                    || !is_type_assignable(expected, &ty)
                    || !is_type_assignable(&ty, expected)
                {
                    return Err(AdmittedSemanticError::new(
                        param.span,
                        format!("callback parameter must be `{expected}`, found `{ty}`"),
                    ));
                }
            }
            if param.parameter.passing == ParameterPassing::MutableReference
                && param.default.is_some()
            {
                return Err(AdmittedSemanticError::new(
                    param.span,
                    "mutable-reference parameters cannot have defaults",
                ));
            }
            let symbol = self.declare(param.name, ty.clone())?;
            if param.parameter.passing == ParameterPassing::MutableReference {
                self.reference_parameters
                    .insert(symbol, self.callable_depth);
            }
            parameters.push(FunctionParameter {
                ty,
                passing: param.parameter.passing,
                default: None,
            });
        }

        let return_type = match body {
            ArrowBody::Expr(body) => self.analyze_expr(body, None)?,
            ArrowBody::Block(statements) => {
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut self.return_contexts,
                    ReturnContext::Inferred {
                        ty: None,
                        saw_return: false,
                    },
                )?;
                for statement in *statements {
                    self.analyze_stmt(statement)?;
                }
                match self
                    .return_contexts
                    .pop()
                    .expect("arrow analysis pushed a return context")
                {
                    ReturnContext::Inferred { ty, saw_return }
                        if saw_return && statements_guarantee_return(statements) =>
                    {
                        ty.unwrap_or(Type::Void)
                    }
                    // Falling off the end yields `undefined`, itself a
                    // `JsValue`, so a callback returning `JsValue` on some
                    // paths returns `JsValue`: host callers read the value.
                    ReturnContext::Inferred {
                        ty: Some(ty),
                        saw_return: true,
                    } if is_js_value(&ty) => ty,
                    ReturnContext::Inferred { .. } => Type::Void,
                    ReturnContext::Declared { .. } => unreachable!(),
                }
            }
        };
        self.callable_depth -= 1;
        self.analyze_parameter_defaults(params, &parameters)?;
        self.pending_references = outer_pending;
        self.current_reference_formals = outer_formals;
        let global_symbols = self
            .scopes
            .first()
            .into_iter()
            .flat_map(|scope| scope.values().copied())
            .collect::<AHashSet<_>>();
        self.constructor_classes.pop();
        self.generator_contexts.pop();
        self.pop_scope();

        resolve_analyzed_parameter_defaults(
            params,
            &mut parameters,
            &self.view(),
            true,
            &global_symbols,
        )?;
        Ok(Type::Function(FunctionType::new(FunctionSignature {
            params: parameters,
            return_type: Box::new(return_type),
        })))
    }

    fn analyze_binary(
        &self,
        op: BinaryOp,
        lhs: &Type<'src>,
        rhs: &Type<'src>,
        span: Span,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        checked_binary_type(op, lhs, rhs, span).map_err(Into::into)
    }

    fn resolve_value_type(
        &self,
        ty: TypeRef<'ast, 'src>,
        context: &str,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        self.resolve_type(ty, false, context)
    }

    fn resolve_parameter_type(
        &self,
        parameter: &crate::ast::ParameterType<'ast, 'src>,
        context: &'static str,
    ) -> Result<FunctionParameter<'src>, AdmittedSemanticError> {
        Ok(FunctionParameter {
            ty: self.resolve_value_type(parameter.ty, context)?,
            passing: parameter.passing,
            default: None,
        })
    }

    fn analyze_builtin_constructor(
        &mut self,
        class: Ident<'src>,
        type_args: &'ast [TypeRef<'ast, 'src>],
        args: &'ast [Argument<'ast, 'src>],
        id: SourceNodeId,
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Option<Type<'src>>, AdmittedSemanticError> {
        let ty = match class.name {
            "Map" => {
                if !args.is_empty() {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!(
                            "`Map` constructor expects 0 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if type_args.is_empty() {
                    let Some(Type::Map(key, value)) = expected else {
                        return Err(AdmittedSemanticError::new(
                            span,
                            "cannot infer `Map` type arguments; write `new Map<K, V>()`",
                        ));
                    };
                    Type::Map(key.clone(), value.clone())
                } else {
                    let [key, value]: [Type<'src>; 2] = self
                        .resolve_type_arguments("Map", type_args, &["K", "V"], span)?
                        .try_into()
                        .expect("Map arity was checked");
                    validate_collection_key(&key, span, "Map key")?;
                    Type::Map(Box::new(key), Box::new(value))
                }
            }
            "Set" => {
                if !args.is_empty() {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!(
                            "`Set` constructor expects 0 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if type_args.is_empty() {
                    let Some(Type::Set(element)) = expected else {
                        return Err(AdmittedSemanticError::new(
                            span,
                            "cannot infer `Set` type argument; write `new Set<T>()`",
                        ));
                    };
                    Type::Set(element.clone())
                } else {
                    let [element]: [Type<'src>; 1] = self
                        .resolve_type_arguments("Set", type_args, &["T"], span)?
                        .try_into()
                        .expect("Set arity was checked");
                    validate_collection_key(&element, span, "Set element")?;
                    Type::Set(Box::new(element))
                }
            }
            "ArrayBuffer" | "SharedArrayBuffer" => {
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                if args.len() != 1 {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!(
                            "`{}` constructor expects 1 argument, found {}",
                            class.name,
                            args.len()
                        ),
                    ));
                }
                let actual = self.analyze_value_argument(&args[0], Some(&Type::Int))?;
                self.require_assignable(&Type::Int, &actual, args[0].span)?;
                if class.name == "ArrayBuffer" {
                    Type::ArrayBuffer
                } else {
                    Type::SharedArrayBuffer
                }
            }
            name if crate::typed_array::TypedArrayKind::from_name(name).is_some() => {
                let kind = crate::typed_array::TypedArrayKind::from_name(name)
                    .expect("typed array constructor name");
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                if args.len() != 1 {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!(
                            "`{}` constructor expects 1 argument, found {}",
                            class.name,
                            args.len()
                        ),
                    ));
                }
                let actual = self.analyze_value_argument(&args[0], None)?;
                if !matches!(
                    actual,
                    Type::Int | Type::ArrayBuffer | Type::SharedArrayBuffer
                ) {
                    return Err(AdmittedSemanticError::new(
                        args[0].span,
                        format!(
                            "`{}` expects an `int`, `ArrayBuffer`, or `SharedArrayBuffer`, found `{actual}`",
                            class.name
                        ),
                    ));
                }
                kind.as_type()
            }
            "Symbol" => {
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                if args.len() > 1 {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!(
                            "`Symbol` constructor expects 0 or 1 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if let Some(argument) = args.first() {
                    let actual = self.analyze_value_argument(argument, Some(&Type::String))?;
                    self.require_assignable(&Type::String, &actual, argument.span)?;
                }
                Type::Symbol
            }
            "Regex" => {
                self.resolve_type_arguments(class.name, type_args, &[], span)?;
                let contract = crate::primitive::intrinsic_call_contract(
                    crate::primitive::ResolvedIntrinsic::Constructor(
                        crate::primitive::Intrinsic::RegexNew,
                    ),
                )
                .expect("checked Regex constructor signature");
                if !contract.accepts_arity(args.len()) {
                    return Err(AdmittedSemanticError::new(
                        span,
                        format!(
                            "`Regex` constructor expects {} or {} arguments, found {}",
                            contract.required_params(),
                            contract.parameters.len(),
                            args.len()
                        ),
                    ));
                }
                for (argument, expected) in args.iter().zip(contract.parameters) {
                    let actual = self.analyze_value_argument(argument, Some(expected))?;
                    self.require_assignable(expected, &actual, argument.span)?;
                }
                contract.result.clone()
            }
            _ => return Ok(None),
        };
        let operation = crate::primitive::constructor_intrinsic(&ty)
            .expect("checked builtin constructor owns an intrinsic");
        self.facts.source_info[id.index()].resolution = ExpressionResolution::Primitive(
            crate::primitive::ResolvedIntrinsic::Constructor(operation),
        );
        Ok(Some(ty))
    }

    fn resolve_type(
        &self,
        ty: TypeRef<'ast, 'src>,
        allow_void: bool,
        context: &str,
    ) -> Result<Type<'src>, AdmittedSemanticError> {
        match ty.kind {
            TypeKind::Int => Ok(Type::Int),
            TypeKind::Float => Ok(Type::Float),
            TypeKind::String => Ok(Type::String),
            TypeKind::Bool => Ok(Type::Bool),
            TypeKind::Void if allow_void => Ok(Type::Void),
            TypeKind::Void => Err(AdmittedSemanticError::new(
                ty.span,
                format!("{context} cannot have type `void`"),
            )),
            TypeKind::Auto => Err(AdmittedSemanticError::new(
                ty.span,
                format!("`auto` is not allowed as a {context} type"),
            )),
            TypeKind::Named { name, args }
                if self
                    .type_parameter_scopes
                    .iter()
                    .rev()
                    .any(|scope| scope.contains(name)) =>
            {
                if !args.is_empty() {
                    return Err(AdmittedSemanticError::new(
                        ty.span,
                        format!("type parameter `{name}` does not accept type arguments"),
                    ));
                }
                Ok(Type::TypeParameter(name))
            }
            TypeKind::Named { name: "Map", args } => {
                let [key, value]: [Type<'src>; 2] = self
                    .resolve_type_arguments("Map", args, &["K", "V"], ty.span)?
                    .try_into()
                    .expect("Map arity was checked");
                validate_collection_key(&key, ty.span, "Map key")?;
                Ok(Type::Map(Box::new(key), Box::new(value)))
            }
            TypeKind::Named { name: "Set", args } => {
                let [element]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Set", args, &["T"], ty.span)?
                    .try_into()
                    .expect("Set arity was checked");
                validate_collection_key(&element, ty.span, "Set element")?;
                Ok(Type::Set(Box::new(element)))
            }
            TypeKind::Named { name: "Task", args }
                if !self.facts.struct_bindings.contains_key("Task")
                    && !self.declarations.classes.contains_key("Task") =>
            {
                let [value]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Task", args, &["T"], ty.span)?
                    .try_into()
                    .expect("Task arity was checked");
                Ok(Type::Task(Box::new(value)))
            }
            TypeKind::Named {
                name: "Generator",
                args,
            } if !self.facts.struct_bindings.contains_key("Generator")
                && !self.declarations.classes.contains_key("Generator") =>
            {
                let [value]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Generator", args, &["T"], ty.span)?
                    .try_into()
                    .expect("Generator arity was checked");
                Ok(Type::Generator(Box::new(value)))
            }
            TypeKind::Named {
                name: "ArrayBuffer",
                args,
            } => {
                self.resolve_type_arguments("ArrayBuffer", args, &[], ty.span)?;
                Ok(Type::ArrayBuffer)
            }
            TypeKind::Named {
                name: "SharedArrayBuffer",
                args,
            } => {
                self.resolve_type_arguments("SharedArrayBuffer", args, &[], ty.span)?;
                Ok(Type::SharedArrayBuffer)
            }
            TypeKind::Named { name, args }
                if let Some(kind) = crate::typed_array::TypedArrayKind::from_name(name) =>
            {
                self.resolve_type_arguments(name, args, &[], ty.span)?;
                Ok(kind.as_type())
            }
            TypeKind::Named {
                name: "Symbol",
                args,
            } => {
                self.resolve_type_arguments("Symbol", args, &[], ty.span)?;
                Ok(Type::Symbol)
            }
            TypeKind::Named {
                name: "Regex",
                args,
            } => {
                self.resolve_type_arguments("Regex", args, &[], ty.span)?;
                Ok(Type::Regex)
            }
            TypeKind::Named {
                name: "JsValue",
                args,
            } => {
                self.resolve_type_arguments("JsValue", args, &[], ty.span)?;
                Ok(Type::TypeParameter("$js"))
            }
            TypeKind::Named {
                name: "Record",
                args,
            } => {
                let [value]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Record", args, &["$value"], ty.span)?
                    .try_into()
                    .expect("Record arity was checked");
                Ok(Type::Record(Box::new(value)))
            }
            TypeKind::Named { name, args } if self.declarations.enums.contains_key(name) => {
                self.resolve_type_arguments(name, args, &[], ty.span)?;
                Ok(Type::Enum(name))
            }
            TypeKind::Named { name, args } if self.facts.struct_bindings.contains_key(name) => {
                let info = self
                    .view()
                    .struct_info(name)
                    .expect("source struct binding");
                let declaration = info.declaration;
                let parameters = &info.type_params;
                let arguments = self.resolve_type_arguments(name, args, parameters, ty.span)?;
                if parameters.is_empty() {
                    Ok(Type::Struct(declaration))
                } else {
                    Ok(Type::StructInstance {
                        declaration,
                        args: arguments,
                    })
                }
            }
            TypeKind::Named { name, args } if self.declarations.classes.contains_key(name) => {
                let parameters = &self.declarations.classes[name].type_params;
                let arguments = self.resolve_type_arguments(name, args, parameters, ty.span)?;
                if parameters.is_empty() {
                    Ok(Type::Class(name))
                } else {
                    Ok(Type::ClassInstance {
                        name,
                        args: arguments,
                    })
                }
            }
            TypeKind::Named { name, .. } => Err(AdmittedSemanticError::new(
                ty.span,
                if self.module.is_some() {
                    format!(
                        "direct module checking does not yet support nominal or unknown type `{name}`"
                    )
                } else {
                    format!("unknown type `{name}`")
                },
            )),
            TypeKind::Array(element) => {
                let element = self.resolve_value_type(*element, "array element")?;
                Ok(Type::Array(Box::new(element)))
            }
            TypeKind::Nullable(inner) => {
                let inner = self.resolve_value_type(*inner, "nullable value")?;
                if matches!(inner, Type::Nullable(_) | Type::Null) {
                    return Err(AdmittedSemanticError::new(
                        ty.span,
                        "nullable types cannot be nested",
                    ));
                }
                Ok(Type::Nullable(Box::new(inner)))
            }
            TypeKind::Union(members) => {
                let mut resolved = Vec::with_capacity(members.len());
                for member in members {
                    resolved.push(self.resolve_value_type(*member, "union member")?);
                }
                Ok(normalize_union(resolved))
            }
            TypeKind::Function {
                params,
                return_type,
            } => {
                let mut resolved_params = Vec::with_capacity(params.len());
                for param in params {
                    resolved_params.push(FunctionParameter {
                        ty: self.resolve_value_type(param.ty, "function parameter")?,
                        passing: param.passing,
                        default: None,
                    });
                }
                let return_type = self.resolve_type(*return_type, true, "function return")?;
                Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: resolved_params,
                    return_type: Box::new(return_type),
                })))
            }
        }
    }

    fn resolve_type_arguments(
        &self,
        name: &str,
        args: &'ast [TypeRef<'ast, 'src>],
        parameters: &[&'src str],
        span: Span,
    ) -> Result<Vec<Type<'src>>, AdmittedSemanticError> {
        if args.len() != parameters.len() {
            return Err(AdmittedSemanticError::new(
                span,
                format!(
                    "type `{name}` expects {} type arguments, found {}",
                    parameters.len(),
                    args.len()
                ),
            ));
        }
        args.iter()
            .map(|argument| self.resolve_value_type(*argument, "type argument"))
            .collect()
    }

    fn push_type_params(&mut self, params: &[Ident<'src>]) -> Result<(), AdmittedSemanticError> {
        let names = validate_type_params(params)?;
        for parameter in params {
            if self
                .type_parameter_scopes
                .iter()
                .any(|scope| scope.contains(parameter.name))
            {
                return Err(AdmittedSemanticError::new(
                    parameter.span,
                    format!(
                        "type parameter `{}` shadows an enclosing type parameter",
                        parameter.name
                    ),
                ));
            }
        }
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.type_parameter_scopes,
            names.into_iter().collect(),
        )?;
        Ok(())
    }

    fn pop_type_params(&mut self) {
        self.type_parameter_scopes
            .pop()
            .expect("type parameter scope was pushed before it was popped");
    }

    fn require_assignable(
        &self,
        expected: &Type<'src>,
        actual: &Type<'src>,
        span: Span,
    ) -> Result<(), AdmittedSemanticError> {
        if self.is_assignable(expected, actual) {
            Ok(())
        } else {
            let mut message = format!("expected `{expected}`, found `{actual}`");
            let declaration = |ty: &Type<'src>| match ty {
                Type::Struct(declaration) | Type::StructInstance { declaration, .. } => {
                    Some(*declaration)
                }
                _ => None,
            };
            if let (Some(expected), Some(actual)) = (declaration(expected), declaration(actual)) {
                if expected.name == actual.name && expected.identity != actual.identity {
                    let expected = &self.declarations.structs[expected.identity.index()];
                    let actual = &self.declarations.structs[actual.identity.index()];
                    use std::fmt::Write;
                    write!(message, " (distinct struct declarations: expected module {:?} at {}..{}, found module {:?} at {}..{})",
                        expected.module, expected.span.start, expected.span.end,
                        actual.module, actual.span.start, actual.span.end).expect("String formatting");
                }
            }
            Err(AdmittedSemanticError::new(span, message))
        }
    }

    fn is_assignable(&self, expected: &Type<'src>, actual: &Type<'src>) -> bool {
        if is_type_assignable(expected, actual) {
            return true;
        }
        match (expected, actual) {
            (Type::Array(expected), Type::Array(actual)) => {
                self.is_assignable(expected, actual) && self.is_assignable(actual, expected)
            }
            (Type::Task(expected), Type::Task(actual))
            | (Type::Generator(expected), Type::Generator(actual))
            | (Type::Nullable(expected), Type::Nullable(actual)) => {
                self.is_assignable(expected, actual)
            }
            (Type::Nullable(expected), actual) => self.is_assignable(expected, actual),
            (Type::Union(expected), Type::Union(actual)) => actual.iter().all(|actual| {
                expected
                    .iter()
                    .any(|expected| self.is_assignable(expected, actual))
            }),
            (Type::Union(expected), actual) => expected
                .iter()
                .any(|expected| self.is_assignable(expected, actual)),
            (expected, Type::Union(actual)) => actual
                .iter()
                .all(|actual| self.is_assignable(expected, actual)),
            (
                Type::Class(_) | Type::ClassInstance { .. },
                Type::Class(_) | Type::ClassInstance { .. },
            ) => {
                let mut current = actual.clone();
                while let Some((name, args)) = class_type_parts(&current) {
                    let info = match self.declarations.classes.get(name) {
                        Some(info) => info,
                        None => return false,
                    };
                    let Some(base) = &info.base else {
                        return false;
                    };
                    let substitutions = substitutions_for(&info.type_params, args);
                    current = substitute_type(base, &substitutions);
                    if is_type_assignable(expected, &current) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn narrowing_from_input(
        &mut self,
        input: NarrowingInput<'ast, 'src>,
    ) -> Result<(Narrowing<'src>, Narrowing<'src>), AdmittedSemanticError> {
        let Some(expression) = input.expression else {
            return Ok((empty_narrowing(), empty_narrowing()));
        };
        let (when_true, when_false) = self.condition_narrowing(expression)?;
        Ok((
            if input.when_true { when_true } else { empty_narrowing() },
            if input.when_false { when_false } else { empty_narrowing() },
        ))
    }

    fn condition_narrowing(
        &mut self,
        condition: &'ast Expr<'ast, 'src>,
    ) -> Result<(Narrowing<'src>, Narrowing<'src>), AdmittedSemanticError> {
        // Most guards are leaves; only Boolean compositions need a worklist.
        if !matches!(
            &condition.kind,
            ExprKind::Unary {
                op: UnaryOp::Not,
                ..
            } | ExprKind::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                ..
            }
        ) {
            self.budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
            return self.condition_narrowing_leaf(condition);
        }
        let mut pending = Vec::new();
        let mut answers = Vec::new();
        let result = (|| {
            self.budget.push(
                AllocationClass::Scratch,
                &mut pending,
                NarrowingStep::Visit(condition),
            )?;
            loop {
                self.budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                let Some(step) = pending.pop() else {
                    break;
                };
                match step {
                    NarrowingStep::Visit(expression) => match &expression.kind {
                        ExprKind::Unary {
                            op: UnaryOp::Not,
                            expr,
                            ..
                        } => {
                            self.budget.push(AllocationClass::Scratch, &mut pending, NarrowingStep::Not)?;
                            self.budget.push(
                                AllocationClass::Scratch,
                                &mut pending,
                                NarrowingStep::Visit(expr),
                            )?;
                        }
                        ExprKind::Binary {
                            op: op @ (BinaryOp::And | BinaryOp::Or),
                            lhs,
                            rhs,
                            ..
                        } => {
                            self.budget.push(
                                AllocationClass::Scratch,
                                &mut pending,
                                NarrowingStep::Join(*op),
                            )?;
                            self.budget.push(
                                AllocationClass::Scratch,
                                &mut pending,
                                NarrowingStep::Visit(rhs),
                            )?;
                            self.budget.push(
                                AllocationClass::Scratch,
                                &mut pending,
                                NarrowingStep::Visit(lhs),
                            )?;
                        }
                        _ => {
                            let answer = self.condition_narrowing_leaf(expression)?;
                            self.budget.push(AllocationClass::Scratch, &mut answers, answer)?;
                        }
                    },
                    NarrowingStep::Not => {
                        let (when_true, when_false) = answers.pop().expect("analyzed negated guard");
                        self.budget.push(
                            AllocationClass::Scratch,
                            &mut answers,
                            (when_false, when_true),
                        )?;
                    }
                    NarrowingStep::Join(op) => {
                        let (rhs_then, rhs_else) = answers.pop().expect("analyzed right guard");
                        let (lhs_then, lhs_else) = answers.pop().expect("analyzed left guard");
                        let answer = if op == BinaryOp::And {
                            (merge_narrowing(lhs_then, rhs_then), empty_narrowing())
                        } else {
                            (empty_narrowing(), merge_narrowing(lhs_else, rhs_else))
                        };
                        self.budget.push(AllocationClass::Scratch, &mut answers, answer)?;
                    }
                }
            }
            Ok(answers.pop().expect("analyzed condition"))
        })();
        let bytes = pending
            .capacity()
            .checked_mul(std::mem::size_of::<NarrowingStep<'_, '_>>())
            .and_then(|bytes| {
                answers
                    .capacity()
                    .checked_mul(std::mem::size_of::<(Narrowing<'_>, Narrowing<'_>)>())
                    .and_then(|answers| bytes.checked_add(answers))
            })
            .and_then(|bytes| u64::try_from(bytes).ok())
            .expect("admitted narrowing worklists fit their original layouts");
        drop(pending);
        drop(answers);
        self.budget
            .release(AllocationClass::Scratch, bytes)
            .expect("narrowing worklists belong to their callback budget");
        result
    }

    fn condition_narrowing_leaf(
        &self,
        condition: &'ast Expr<'ast, 'src>,
    ) -> Result<(Narrowing<'src>, Narrowing<'src>), AdmittedSemanticError> {
        let Some(leaf) = narrowing_leaf(condition) else {
            return Ok((empty_narrowing(), empty_narrowing()));
        };
        if let NarrowingLeaf::TypeCheck { ident, span } = leaf {
            let symbol = self.resolve(ident)?;
            let target = self
                .facts
                .type_check_types
                .get(&span)
                .cloned()
                .ok_or_else(|| {
                    AdmittedSemanticError::new(span, "type guard was not analyzed before narrowing")
                })?;
            let current = self.narrowed_type(symbol.id).unwrap_or(&symbol.ty);
            let remaining = subtract_guarded_type(current, &target);
            let mut then_narrowing = empty_narrowing();
            then_narrowing.insert(symbol.id, target);
            let mut else_narrowing = empty_narrowing();
            if let Some(ty) = remaining {
                else_narrowing.insert(symbol.id, ty);
            }
            return Ok((then_narrowing, else_narrowing));
        }
        let NarrowingLeaf::NullComparison { ident, present_when_true } = leaf else {
            unreachable!("type guard returned above")
        };
        let symbol = self.resolve(ident)?;
        let current = self.narrowed_type(symbol.id).unwrap_or(&symbol.ty);
        let Type::Nullable(inner) = current else {
            return Ok((empty_narrowing(), empty_narrowing()));
        };
        let mut present_narrowing = empty_narrowing();
        present_narrowing.insert(symbol.id, inner.as_ref().clone());
        Ok(if present_when_true {
            (present_narrowing, empty_narrowing())
        } else {
            (empty_narrowing(), present_narrowing)
        })
    }

    fn apply_narrowing(&mut self, narrowing: Narrowing<'src>) {
        if narrowing.is_empty() {
            return;
        }
        let scope = self
            .narrowings
            .last_mut()
            .expect("semantic analyzer always has a narrowing scope");
        for (symbol, ty) in narrowing {
            scope.insert(symbol, ty);
        }
    }

    fn current_scope_preserves(&self, narrowing: &Narrowing<'src>) -> bool {
        if narrowing.is_empty() {
            return false;
        }
        let Some(scope) = self.narrowings.last() else {
            return false;
        };
        narrowing
            .iter()
            .all(|(symbol, ty)| scope.get(symbol) == Some(ty))
    }

    fn narrowed_type(&self, symbol: SymbolId) -> Option<&Type<'src>> {
        self.narrowings
            .iter()
            .rev()
            .find_map(|scope| scope.get(&symbol))
    }

    fn invalidate_assigned_narrowing(&mut self, target: &'ast Expr<'ast, 'src>) {
        let Expr {
            kind: ExprKind::Ident(ident),
            ..
        } = target
        else {
            return;
        };
        let Some(symbol) = self.facts.identifier_symbols.get(&ident.span).copied() else {
            return;
        };
        for scope in &mut self.narrowings {
            scope.remove(&symbol);
        }
    }

    fn check_module_defaults(
        &self,
        params: &[crate::ast::Param<'ast, 'src>],
    ) -> Result<(), AdmittedSemanticError> {
        // Module checking now admits defaults: the semantic route evaluates
        // an omitted parameter's checked default at each call site.
        let _ = params;
        Ok(())
    }

    fn declare_foreign(
        &mut self,
        ident: Ident<'src>,
        ty: Type<'src>,
        callable: bool,
    ) -> Result<SymbolId, AdmittedSemanticError> {
        if self.module.is_none() {
            let symbol = self.declare(ident, ty)?;
            self.declarations.symbols[symbol.0 as usize].origin = DeclarationOrigin::Foreign;
            return Ok(symbol);
        }
        if self.scopes[0].contains_key(ident.name) {
            return Err(AdmittedSemanticError::new(
                ident.span,
                format!("duplicate binding `{}`", ident.name),
            ));
        }
        if let Some(&(symbol, prior_callable)) = self.declarations.foreign_symbols.get(ident.name) {
            if prior_callable != callable || self.declarations.symbols[symbol.0 as usize].ty != ty {
                return Err(AdmittedSemanticError::new(
                    ident.span,
                    format!("conflicting extern contracts for `{}`", ident.name),
                ));
            }
            self.scopes[0].insert(ident.name, symbol);
            self.facts
                .binding_types
                .insert(ident.span, BindingType::Symbol(symbol));
            self.record_identifier(ident.span, symbol);
            return Ok(symbol);
        }
        let symbol = self.declare(ident, ty)?;
        self.declarations.symbols[symbol.0 as usize].origin = DeclarationOrigin::Foreign;
        self.declarations
            .foreign_symbols
            .insert(ident.name, (symbol, callable));
        Ok(symbol)
    }

    fn declare(
        &mut self,
        ident: Ident<'src>,
        ty: Type<'src>,
    ) -> Result<SymbolId, AdmittedSemanticError> {
        let scope = self
            .scopes
            .last_mut()
            .expect("semantic analyzer always has a scope");
        if scope.contains_key(ident.name) {
            return Err(AdmittedSemanticError::new(
                ident.span,
                format!("duplicate binding `{}`", ident.name),
            ));
        }

        let id = SymbolId(
            u32::try_from(self.declarations.symbols.len()).map_err(|_| AllocationError::Capacity)?,
        );
        let symbol = Symbol {
            id,
            name: ident.name,
            ty,
            span: ident.span,
            escape_state: EscapeState::LocalOnly,
            origin: DeclarationOrigin::Source,
            identifier_occurrences: 0,
        };
        self.declarations
            .add_symbol(symbol, self.module, self.budget)?;
        scope.insert(ident.name, id);
        self.facts
            .binding_types
            .insert(ident.span, BindingType::Symbol(id));
        self.record_identifier(ident.span, id);
        Ok(id)
    }

    fn record_detached(
        &mut self,
        ident: Ident<'src>,
        ty: Type<'src>,
    ) -> Result<SymbolId, AdmittedSemanticError> {
        let id = SymbolId(
            u32::try_from(self.declarations.symbols.len()).map_err(|_| AllocationError::Capacity)?,
        );
        let symbol = Symbol {
            id,
            name: ident.name,
            ty,
            span: ident.span,
            escape_state: EscapeState::LocalOnly,
            origin: DeclarationOrigin::Source,
            identifier_occurrences: 0,
        };
        self.declarations
            .add_symbol(symbol, self.module, self.budget)?;
        self.facts
            .binding_types
            .insert(ident.span, BindingType::Symbol(id));
        self.record_identifier(ident.span, id);
        Ok(id)
    }

    fn resolve(&self, ident: &Ident<'src>) -> Result<&Symbol<'src>, AdmittedSemanticError> {
        self.resolve_with_scope(ident).map(|(_, symbol)| symbol)
    }

    fn resolve_with_scope(
        &self,
        ident: &Ident<'src>,
    ) -> Result<(usize, &Symbol<'src>), AdmittedSemanticError> {
        let (scope, id) = self
            .scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, scope)| scope.get(ident.name).map(|id| (index, id)))
            .ok_or_else(|| {
                AdmittedSemanticError::new(ident.span, format!("unknown identifier `{}`", ident.name))
            })?;
        if self
            .reference_parameters
            .get(id)
            .is_some_and(|owner| *owner != self.callable_depth)
        {
            return Err(AdmittedSemanticError::new(
                ident.span,
                "mutable-reference parameters cannot be captured; copy the value into a local first",
            ));
        }
        Ok((scope, &self.declarations.symbols[id.0 as usize]))
    }

    fn push_scope(&mut self) -> Result<(), AllocationError> {
        self.budget.reserve_vec(AllocationClass::Scratch, &mut self.scopes, 1)?;
        self.budget.reserve_vec(AllocationClass::Scratch, &mut self.narrowings, 1)?;
        self.budget.work(crate::compilation_policy::WorkKind::Render, 2)?;
        self.scopes.push(AHashMap::default());
        self.narrowings.push(AHashMap::default());
        Ok(())
    }

    fn pop_scope(&mut self) {
        debug_assert!(self.scopes.len() > 1);
        self.scopes.pop();
        self.narrowings.pop();
    }
}

fn validate_type_params<'src>(params: &[Ident<'src>]) -> Result<Vec<&'src str>, SemanticError> {
    let mut names = Vec::with_capacity(params.len());
    let mut seen = AHashSet::default();
    for parameter in params {
        if !seen.insert(parameter.name) {
            return Err(SemanticError::new(
                parameter.span,
                format!("duplicate type parameter `{}`", parameter.name),
            ));
        }
        names.push(parameter.name);
    }
    Ok(names)
}

fn resolve_parameter_defaults<'ast, 'src>(
    params: &[crate::ast::Param<'ast, 'src>],
    parameters: &mut [FunctionParameter<'src>],
) -> Result<(), SemanticError> {
    for (param, parameter) in params.iter().zip(parameters) {
        let ty = &parameter.ty;
        parameter.default = (|| {
            let Some(expression) = &param.default else {
                return Ok(None);
            };
            if parameter.passing == ParameterPassing::MutableReference {
                return Err(SemanticError::new(
                    param.span,
                    "mutable-reference parameters cannot have defaults",
                ));
            }
            if let Expr {
                kind: ExprKind::Ident(identifier),
                ..
            } = expression
            {
                return Ok(Some(DefaultValue::PendingIdentifier {
                    expression: expression.id,
                    span: identifier.span,
                }));
            }
            if syntactic_js_undefined_default(expression) {
                return Ok(Some(DefaultValue::PendingUndefined {
                    expression: expression.id,
                    span: expression.span(),
                }));
            }
            if let Some((_, actual)) = scalar_default_value(expression) {
                if !is_type_assignable(ty, &actual) {
                    return Err(SemanticError::new(
                        expression.span(),
                        format!("default value has type `{actual}`, expected `{ty}`"),
                    ));
                }
            }
            literal_default_value(expression, ty)
                .map(Some)
                .ok_or_else(|| {
                    SemanticError::new(
                        expression.span(),
                        format!(
                            "default value is not a supported literal for parameter type `{ty}`"
                        ),
                    )
                })
        })()?;
    }
    Ok(())
}

fn resolve_analyzed_parameter_defaults<'ast, 'src>(
    params: &[crate::ast::Param<'ast, 'src>],
    parameters: &mut [FunctionParameter<'src>],
    model: &SemanticView<'_, '_, 'src>,
    parameter_defaults_in_scope: bool,
    global_symbols: &AHashSet<SymbolId>,
) -> Result<(), SemanticError> {
    resolve_parameter_defaults(params, parameters)?;
    let parameter_symbols = if parameter_defaults_in_scope {
        params
            .iter()
            .filter_map(|parameter| model.identifier_symbol(parameter.name.span))
            .collect::<AHashSet<_>>()
    } else {
        AHashSet::default()
    };
    for (index, param) in params.iter().enumerate() {
        let Some(expression) = &param.default else {
            continue;
        };
        if parameter_defaults_in_scope
            && default_contains_arrow_capture(expression, &parameter_symbols, model)
        {
            return Err(SemanticError::new(
                expression.span(),
                "a parameter default arrow cannot capture a parameter of its containing callable",
            ));
        }
        if parameter_defaults_in_scope
            && default_contains_non_global_arrow_capture(expression, global_symbols, model)
        {
            return Err(SemanticError::new(
                expression.span(),
                "a parameter default arrow cannot capture a local binding outside its callable",
            ));
        }
        if matches!(
            parameters[index].default,
            Some(DefaultValue::PendingUndefined { .. })
        ) {
            if model.builtin_call(expression.id) == Some(BuiltinCall::JsUndefined) {
                parameters[index].default = Some(DefaultValue::Undefined);
                continue;
            }
            return Err(SemanticError::new(
                expression.span(),
                "parameter default is not the unshadowed `JS.undefined()` primitive",
            ));
        }
        let Expr {
            kind: ExprKind::Ident(identifier),
            ..
        } = expression
        else {
            continue;
        };
        let ExpressionResolution::Binding(bound) = model.expression_resolution(expression.id)
        else {
            continue;
        };
        if parameter_defaults_in_scope {
            let bound_parameter = params
                .iter()
                .position(|candidate| model.identifier_symbol(candidate.name.span) == Some(bound));
            if let Some(bound_parameter) = bound_parameter {
                if bound_parameter >= index {
                    return Err(SemanticError::new(
                        identifier.span,
                        "parameter defaults can only reference earlier parameters",
                    ));
                }
                parameters[index].default = Some(DefaultValue::Parameter(bound_parameter));
                continue;
            }
        }
        parameters[index].default = Some(DefaultValue::Symbol(bound));
    }
    Ok(())
}

fn syntactic_js_undefined_default(expression: &Expr<'_, '_>) -> bool {
    matches!(
        expression,
        Expr { kind: ExprKind::Call {
            callee,
            args,
            ..
        }, .. } if args.is_empty()
            && matches!(
                callee,
                Expr { kind: ExprKind::Member {
                    object,
                    property: Ident { name: "undefined", .. },
                    ..
                }, .. } if matches!(object, Expr { kind: ExprKind::Ident(Ident { name: "JS", .. }), .. })
            )
    )
}

fn default_contains_arrow_capture(
    expression: &Expr<'_, '_>,
    parameter_symbols: &AHashSet<SymbolId>,
    model: &SemanticView<'_, '_, '_>,
) -> bool {
    let mut arrow_spans = Vec::new();
    collect_default_arrow_spans(expression, &mut arrow_spans);
    model
        .facts
        .identifier_symbols
        .iter()
        .any(|(identifier, symbol)| {
            parameter_symbols.contains(symbol)
                && arrow_spans
                    .iter()
                    .any(|arrow| arrow.start <= identifier.start && identifier.end <= arrow.end)
        })
}

fn default_contains_non_global_arrow_capture(
    expression: &Expr<'_, '_>,
    global_symbols: &AHashSet<SymbolId>,
    model: &SemanticView<'_, '_, '_>,
) -> bool {
    let mut arrow_spans = Vec::new();
    collect_default_arrow_spans(expression, &mut arrow_spans);
    model
        .facts
        .identifier_symbols
        .iter()
        .any(|(identifier, symbol)| {
            !global_symbols.contains(symbol)
                && arrow_spans.iter().any(|arrow| {
                    arrow.start <= identifier.start
                        && identifier.end <= arrow.end
                        && model
                            .declarations
                            .symbols
                            .get(symbol.0 as usize)
                            .is_some_and(|symbol| {
                                symbol.span.start < arrow.start || symbol.span.end > arrow.end
                            })
                })
        })
}

fn collect_default_arrow_spans(expression: &Expr<'_, '_>, spans: &mut Vec<Span>) {
    match expression {
        Expr {
            kind: ExprKind::ArrowFunction { span, .. },
            ..
        } => spans.push(*span),
        Expr {
            kind: ExprKind::ArrayLiteral { elements, .. },
            ..
        } => {
            for element in *elements {
                if let ArrayElement::Value(value) = element {
                    collect_default_arrow_spans(value, spans);
                }
            }
        }
        Expr {
            kind: ExprKind::StructLiteral { values, .. },
            ..
        } => {
            for value in *values {
                collect_default_arrow_spans(value, spans);
            }
        }
        Expr {
            kind: ExprKind::New { args, .. },
            ..
        } => {
            for argument in *args {
                collect_default_arrow_spans(&argument.expression, spans);
            }
        }
        _ => {}
    }
}

// A read-only check avoids detaching an already resolved shared signature.
// Pending metadata is a semantic obligation, not a mutable cache flag.
fn default_has_pending_bindings(value: &DefaultValue<'_>) -> bool {
    match value {
        DefaultValue::PendingIdentifier { .. } | DefaultValue::PendingUndefined { .. } => true,
        DefaultValue::Array(values) | DefaultValue::Struct { values, .. } => {
            values.iter().any(default_has_pending_bindings)
        }
        DefaultValue::NewClass { args, .. } => args.iter().any(default_has_pending_bindings),
        _ => false,
    }
}

fn type_has_default_matching(ty: &Type<'_>, matches: fn(&DefaultValue<'_>) -> bool) -> bool {
    match ty {
        Type::Array(value)
        | Type::Record(value)
        | Type::Set(value)
        | Type::Task(value)
        | Type::Generator(value)
        | Type::Nullable(value) => type_has_default_matching(value, matches),
        Type::Map(key, value) => {
            type_has_default_matching(key, matches) || type_has_default_matching(value, matches)
        }
        Type::Union(members)
        | Type::StructInstance { args: members, .. }
        | Type::ClassInstance { args: members, .. } => members
            .iter()
            .any(|ty| type_has_default_matching(ty, matches)),
        Type::Function(signature) => signature_has_default_matching(signature, matches),
        Type::GenericFunction(function) => {
            signature_has_default_matching(&function.signature, matches)
        }
        _ => false,
    }
}

fn signature_has_pending_bindings(signature: &FunctionType<'_>) -> bool {
    signature_has_default_matching(signature, default_has_pending_bindings)
}

fn signature_has_default_matching(
    signature: &FunctionType<'_>,
    matches: fn(&DefaultValue<'_>) -> bool,
) -> bool {
    signature.params.iter().any(|parameter| {
        type_has_default_matching(&parameter.ty, matches)
            || parameter.default.as_ref().is_some_and(matches)
    }) || type_has_default_matching(&signature.return_type, matches)
}

fn finalize_default_bindings_in_signature<'src>(
    signature: &mut FunctionType<'src>,
    source_info: &[SourceInfo<'_, 'src>],
    owned: bool,
) -> Result<(), SemanticError> {
    if !signature_has_pending_bindings(signature) {
        return Ok(());
    }
    let signature = signature.make_mut();
    for parameter in &mut signature.params {
        finalize_default_bindings_in_type(&mut parameter.ty, source_info, owned)?;
        if let Some(default) = &mut parameter.default {
            finalize_default_binding(default, source_info, owned)?;
        }
    }
    finalize_default_bindings_in_type(&mut signature.return_type, source_info, owned)
}

/// `owned`: only finalize a pending default whose source entry is that exact
/// occurrence (same span). A module finalizes the defaults it declared; a
/// default another module owns is left for that module.
fn finalize_default_binding<'src>(
    default: &mut DefaultValue<'src>,
    source_info: &[SourceInfo<'_, 'src>],
    owned: bool,
) -> Result<(), SemanticError> {
    if owned {
        if let DefaultValue::PendingIdentifier { expression, span }
        | DefaultValue::PendingUndefined { expression, span } = default
        {
            let same = source_info
                .get(expression.index())
                .and_then(|info| info.expression)
                .is_some_and(|occurrence| occurrence.span() == *span);
            if !same {
                return Ok(());
            }
        }
    }
    match default {
        DefaultValue::PendingIdentifier { expression, span } => {
            let Some(ExpressionResolution::Binding(symbol)) = source_info
                .get(expression.index())
                .map(|info| info.resolution)
            else {
                return Err(SemanticError::new(
                    *span,
                    "missing analyzed parameter-default binding",
                ));
            };
            *default = DefaultValue::Symbol(symbol);
        }
        DefaultValue::PendingUndefined { expression, span } => {
            if source_info
                .get(expression.index())
                .map(|info| info.resolution)
                != Some(ExpressionResolution::Builtin(BuiltinCall::JsUndefined))
            {
                return Err(SemanticError::new(
                    *span,
                    "parameter default is not the unshadowed `JS.undefined()` primitive",
                ));
            }
            *default = DefaultValue::Undefined;
        }
        DefaultValue::Array(values) => {
            for value in values {
                finalize_default_binding(value, source_info, owned)?;
            }
        }
        DefaultValue::Struct { values, .. } => {
            for value in values {
                finalize_default_binding(value, source_info, owned)?;
            }
        }
        DefaultValue::NewClass { args, .. } => {
            for argument in args {
                finalize_default_binding(argument, source_info, owned)?;
            }
        }
        DefaultValue::Int(_)
        | DefaultValue::Float(_)
        | DefaultValue::String(_)
        | DefaultValue::Bool(_)
        | DefaultValue::Null
        | DefaultValue::Undefined
        | DefaultValue::Symbol(_)
        | DefaultValue::Parameter(_)
        | DefaultValue::Arrow(_) => {}
    }
    Ok(())
}

fn finalize_default_bindings_in_type<'src>(
    ty: &mut Type<'src>,
    source_info: &[SourceInfo<'_, 'src>],
    owned: bool,
) -> Result<(), SemanticError> {
    match ty {
        Type::Array(value)
        | Type::Record(value)
        | Type::Set(value)
        | Type::Task(value)
        | Type::Generator(value)
        | Type::Nullable(value) => finalize_default_bindings_in_type(value, source_info, owned)?,
        Type::Map(key, value) => {
            finalize_default_bindings_in_type(key, source_info, owned)?;
            finalize_default_bindings_in_type(value, source_info, owned)?;
        }
        Type::Union(members)
        | Type::StructInstance { args: members, .. }
        | Type::ClassInstance { args: members, .. } => {
            for member in members {
                finalize_default_bindings_in_type(member, source_info, owned)?;
            }
        }
        Type::Function(signature) => {
            finalize_default_bindings_in_signature(signature, source_info, owned)?;
        }
        Type::GenericFunction(function) => {
            finalize_default_bindings_in_signature(&mut function.signature, source_info, owned)?;
        }
        Type::Int
        | Type::Float
        | Type::Enum(_)
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::Regex
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Struct(_)
        | Type::Class(_)
        | Type::TypeParameter(_) => {}
    }
    Ok(())
}

fn strip_parameter_defaults_from_type(ty: &mut Type<'_>) {
    match ty {
        Type::Array(value)
        | Type::Record(value)
        | Type::Set(value)
        | Type::Task(value)
        | Type::Generator(value)
        | Type::Nullable(value) => strip_parameter_defaults_from_type(value),
        Type::Map(key, value) => {
            strip_parameter_defaults_from_type(key);
            strip_parameter_defaults_from_type(value);
        }
        Type::Union(members)
        | Type::StructInstance { args: members, .. }
        | Type::ClassInstance { args: members, .. } => {
            for member in members {
                strip_parameter_defaults_from_type(member);
            }
        }
        Type::Function(signature) => {
            strip_parameter_defaults_from_signature(signature);
        }
        Type::GenericFunction(function) => {
            strip_parameter_defaults_from_signature(&mut function.signature);
        }
        Type::Int
        | Type::Float
        | Type::Enum(_)
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::Regex
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Struct(_)
        | Type::Class(_)
        | Type::TypeParameter(_) => {}
    }
}

fn strip_parameter_defaults_from_signature(signature: &mut FunctionType<'_>) {
    if !signature_has_default_matching(signature, |_| true) {
        return;
    }
    let signature = signature.make_mut();
    for parameter in &mut signature.params {
        parameter.default = None;
        strip_parameter_defaults_from_type(&mut parameter.ty);
    }
    strip_parameter_defaults_from_type(&mut signature.return_type);
}

#[cfg(test)]
#[path = "semantic/default_ownership_tests.rs"]
mod default_ownership_tests;

#[cfg(test)]
#[path = "semantic/binding_ownership_tests.rs"]
mod binding_ownership_tests;

#[cfg(test)]
#[path = "semantic/class_ownership_tests.rs"]
mod class_ownership_tests;

#[cfg(test)]
#[path = "semantic/constructor_ownership_tests.rs"]
mod constructor_ownership_tests;

#[cfg(test)]
#[path = "semantic/type_resolution_tests.rs"]
mod type_resolution_tests;

fn literal_default_value<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
    expected: &Type<'src>,
) -> Option<DefaultValue<'src>> {
    if matches!(expression.kind, ExprKind::ArrowFunction { .. }) {
        return matches!(expected, Type::Function(_)).then_some(DefaultValue::Arrow(expression.id));
    }
    if let Expr {
        kind: ExprKind::StructLiteral { name, values, .. },
        ..
    } = expression
    {
        let expected_name = nominal_default_name(expected, false)?;
        if name.name != expected_name {
            return None;
        }
        return values
            .iter()
            .map(uncontextualized_default_value)
            .collect::<Option<Vec<_>>>()
            .map(|values| DefaultValue::Struct {
                name: name.name,
                values,
            });
    }
    if let Expr {
        kind: ExprKind::New { class, args, .. },
        ..
    } = expression
    {
        let expected_name = nominal_default_name(expected, true)?;
        if class.name != expected_name {
            return None;
        }
        return args
            .iter()
            .map(|argument| {
                (argument.passing == ParameterPassing::Value)
                    .then(|| uncontextualized_default_value(&argument.expression))
                    .flatten()
            })
            .collect::<Option<Vec<_>>>()
            .map(|args| DefaultValue::NewClass {
                name: class.name,
                args,
            });
    }
    if let Expr {
        kind: ExprKind::ArrayLiteral { elements, .. },
        ..
    } = expression
    {
        let element = expected_array_element(expected)?;
        return elements
            .iter()
            .map(|element_value| match element_value {
                ArrayElement::Value(value) => literal_default_value(value, element),
                ArrayElement::Spread { .. } => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(DefaultValue::Array);
    }
    let (value, actual) = scalar_default_value(expression)?;
    is_type_assignable(expected, &actual).then_some(value)
}

fn uncontextualized_default_value<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
) -> Option<DefaultValue<'src>> {
    if let Some((value, _)) = scalar_default_value(expression) {
        return Some(value);
    }
    match expression {
        Expr {
            kind: ExprKind::ArrayLiteral { elements, .. },
            ..
        } => elements
            .iter()
            .map(|element| match element {
                ArrayElement::Value(value) => uncontextualized_default_value(value),
                ArrayElement::Spread { .. } => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(DefaultValue::Array),
        Expr {
            kind: ExprKind::ArrowFunction { .. },
            ..
        } => Some(DefaultValue::Arrow(expression.id)),
        Expr {
            kind: ExprKind::StructLiteral { name, values, .. },
            ..
        } => values
            .iter()
            .map(uncontextualized_default_value)
            .collect::<Option<Vec<_>>>()
            .map(|values| DefaultValue::Struct {
                name: name.name,
                values,
            }),
        Expr {
            kind: ExprKind::New { class, args, .. },
            ..
        } => args
            .iter()
            .map(|argument| {
                (argument.passing == ParameterPassing::Value)
                    .then(|| uncontextualized_default_value(&argument.expression))
                    .flatten()
            })
            .collect::<Option<Vec<_>>>()
            .map(|args| DefaultValue::NewClass {
                name: class.name,
                args,
            }),
        _ => None,
    }
}

fn nominal_default_name<'src>(ty: &Type<'src>, class: bool) -> Option<&'src str> {
    match (class, ty) {
        (false, Type::Struct(declaration)) | (false, Type::StructInstance { declaration, .. }) => {
            Some(declaration.name)
        }
        (true, Type::Class(name)) | (true, Type::ClassInstance { name, .. }) => Some(name),
        (_, Type::Nullable(inner)) => nominal_default_name(inner, class),
        (_, Type::Union(members)) => members
            .iter()
            .find_map(|member| nominal_default_name(member, class)),
        _ => None,
    }
}

fn expected_array_type<'ty, 'src>(ty: &'ty Type<'src>) -> Option<&'ty Type<'src>> {
    match ty {
        Type::Array(_) => Some(ty),
        Type::Nullable(inner) => expected_array_type(inner),
        Type::Union(members) => members.iter().find_map(expected_array_type),
        _ => None,
    }
}

fn expected_array_element<'ty, 'src>(ty: &'ty Type<'src>) -> Option<&'ty Type<'src>> {
    match ty {
        Type::Array(element) => Some(element),
        Type::Nullable(inner) => expected_array_element(inner),
        Type::Union(members) => members.iter().find_map(expected_array_element),
        _ => None,
    }
}

fn scalar_default_value<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
) -> Option<(DefaultValue<'src>, Type<'src>)> {
    match expression {
        Expr {
            kind: ExprKind::Int(value, _),
            ..
        } => Some((DefaultValue::Int(*value), Type::Int)),
        Expr {
            kind: ExprKind::Float(value, _),
            ..
        } => Some((DefaultValue::Float(value.to_bits()), Type::Float)),
        Expr {
            kind: ExprKind::String(value, _),
            ..
        } => Some((DefaultValue::String(value), Type::String)),
        Expr {
            kind: ExprKind::Bool(value, _),
            ..
        } => Some((DefaultValue::Bool(*value), Type::Bool)),
        Expr {
            kind: ExprKind::Null(_),
            ..
        } => Some((DefaultValue::Null, Type::Null)),
        Expr {
            kind:
                ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr,
                    ..
                },
            ..
        } => match *expr {
            Expr {
                kind: ExprKind::Int(value, _),
                ..
            } => Some((DefaultValue::Int(value.wrapping_neg()), Type::Int)),
            Expr {
                kind: ExprKind::Float(value, _),
                ..
            } => Some((DefaultValue::Float((-value).to_bits()), Type::Float)),
            _ => None,
        },
        _ => None,
    }
}

fn applied_class_type<'src>(name: &'src str, parameters: &[&'src str]) -> Type<'src> {
    if parameters.is_empty() {
        Type::Class(name)
    } else {
        Type::ClassInstance {
            name,
            args: parameters
                .iter()
                .map(|parameter| Type::TypeParameter(parameter))
                .collect(),
        }
    }
}

fn substitute_type<'src>(
    ty: &Type<'src>,
    substitutions: &AHashMap<&'src str, Type<'src>>,
) -> Type<'src> {
    match type_substitution::substitute_type_with(
        ty,
        &mut |name, _: &mut type_relation::Unmetered| Ok(substitutions.get(name)),
        &mut type_relation::Unmetered,
    ) {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

fn substitutions_for<'src>(
    parameters: &[&'src str],
    arguments: &[Type<'src>],
) -> AHashMap<&'src str, Type<'src>> {
    parameters
        .iter()
        .copied()
        .zip(arguments.iter().cloned())
        .collect()
}

fn method_callable_type<'src>(
    method: &MethodInfo<'src>,
    substitutions: &AHashMap<&'src str, Type<'src>>,
) -> Type<'src> {
    let signature = match substitute_type(&Type::Function(method.signature.clone()), substitutions)
    {
        Type::Function(signature) => signature,
        _ => unreachable!("substituting a function signature preserves its kind"),
    };
    if method.type_params.is_empty() {
        Type::Function(signature)
    } else {
        Type::GenericFunction(GenericFunctionType {
            type_params: method.type_params.clone(),
            signature,
        })
    }
}

fn contains_type_parameter(ty: &Type<'_>, parameters: &AHashSet<&str>) -> bool {
    match ty {
        Type::TypeParameter(name) => parameters.contains(name),
        Type::Array(element) => contains_type_parameter(element, parameters),
        Type::Record(value) => contains_type_parameter(value, parameters),
        Type::Map(key, value) => {
            contains_type_parameter(key, parameters) || contains_type_parameter(value, parameters)
        }
        Type::Set(element) => contains_type_parameter(element, parameters),
        Type::Task(value) => contains_type_parameter(value, parameters),
        Type::Generator(value) => contains_type_parameter(value, parameters),
        Type::Nullable(inner) => contains_type_parameter(inner, parameters),
        Type::Union(members) => members
            .iter()
            .any(|member| contains_type_parameter(member, parameters)),
        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. } => args
            .iter()
            .any(|argument| contains_type_parameter(argument, parameters)),
        Type::Function(signature) => {
            signature
                .params
                .iter()
                .any(|parameter| contains_type_parameter(&parameter.ty, parameters))
                || contains_type_parameter(&signature.return_type, parameters)
        }
        Type::GenericFunction(function) => {
            function
                .signature
                .params
                .iter()
                .any(|parameter| contains_type_parameter(&parameter.ty, parameters))
                || contains_type_parameter(&function.signature.return_type, parameters)
        }
        _ => false,
    }
}

fn infer_type_arguments<'src>(
    pattern: &Type<'src>,
    actual: &Type<'src>,
    parameters: &AHashSet<&'src str>,
    substitutions: &mut AHashMap<&'src str, Type<'src>>,
    span: Span,
) -> Result<(), SemanticError> {
    match (pattern, actual) {
        (Type::TypeParameter(name), actual) if parameters.contains(name) => {
            if let Some(previous) = substitutions.get(name) {
                if is_type_assignable(previous, actual) {
                    return Ok(());
                }
                if is_type_assignable(actual, previous) {
                    substitutions.insert(name, actual.clone());
                } else {
                    return Err(SemanticError::new(
                        span,
                        format!("conflicting inferences for `{name}`: `{previous}` and `{actual}`"),
                    ));
                }
            } else {
                substitutions.insert(name, actual.clone());
            }
        }
        (Type::Array(pattern), Type::Array(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Record(pattern), Type::Record(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Map(pattern_key, pattern_value), Type::Map(actual_key, actual_value)) => {
            infer_type_arguments(pattern_key, actual_key, parameters, substitutions, span)?;
            infer_type_arguments(pattern_value, actual_value, parameters, substitutions, span)?;
        }
        (Type::Set(pattern), Type::Set(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Task(pattern), Type::Task(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Generator(pattern), Type::Generator(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Nullable(pattern), Type::Nullable(actual)) => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Nullable(pattern), actual) if actual != &Type::Null => {
            infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
        }
        (Type::Union(pattern), Type::Union(actual)) => {
            for actual_member in actual {
                if let Some(pattern_member) = pattern
                    .iter()
                    .find(|candidate| is_type_assignable(candidate, actual_member))
                {
                    infer_type_arguments(
                        pattern_member,
                        actual_member,
                        parameters,
                        substitutions,
                        span,
                    )?;
                }
            }
        }
        (Type::Function(pattern), Type::Function(actual))
            if pattern.params.len() == actual.params.len() =>
        {
            for (pattern, actual) in pattern.params.iter().zip(&actual.params) {
                if pattern.passing != actual.passing {
                    return Err(SemanticError::new(
                        span,
                        "callback parameter passing modes differ",
                    ));
                }
                infer_type_arguments(&pattern.ty, &actual.ty, parameters, substitutions, span)?;
            }
            infer_type_arguments(
                &pattern.return_type,
                &actual.return_type,
                parameters,
                substitutions,
                span,
            )?;
        }
        (
            Type::StructInstance {
                declaration: pattern,
                args: pattern_args,
            },
            Type::StructInstance {
                declaration: actual,
                args: actual_args,
            },
        ) if pattern == actual && pattern_args.len() == actual_args.len() => {
            for (pattern, actual) in pattern_args.iter().zip(actual_args) {
                infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
            }
        }
        (
            Type::ClassInstance {
                name: pattern,
                args: pattern_args,
            },
            Type::ClassInstance {
                name: actual,
                args: actual_args,
            },
        ) if pattern == actual && pattern_args.len() == actual_args.len() => {
            for (pattern, actual) in pattern_args.iter().zip(actual_args) {
                infer_type_arguments(pattern, actual, parameters, substitutions, span)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Shared language typing for a checked binary operation, independent of the
/// source checker or a target representation.
pub(crate) fn checked_binary_type<'src>(
    op: BinaryOp,
    lhs: &Type<'src>,
    rhs: &Type<'src>,
    span: Span,
) -> Result<Type<'src>, SemanticError> {
    binary_types::checked_binary_type_plain(op, lhs, rhs, span)
}

pub(crate) fn is_type_assignable(expected: &Type<'_>, actual: &Type<'_>) -> bool {
    type_relation::is_type_assignable_plain(expected, actual)
}

fn assignment_binary_op(op: AssignmentOp) -> BinaryOp {
    match op {
        AssignmentOp::Assign | AssignmentOp::Nullish => {
            unreachable!("non-binary assignment has no binary operator")
        }
        AssignmentOp::Add => BinaryOp::Add,
        AssignmentOp::Sub => BinaryOp::Sub,
        AssignmentOp::Mul => BinaryOp::Mul,
        AssignmentOp::Div => BinaryOp::Div,
        AssignmentOp::Mod => BinaryOp::Mod,
        AssignmentOp::BitAnd => BinaryOp::BitAnd,
        AssignmentOp::BitOr => BinaryOp::BitOr,
        AssignmentOp::Xor => BinaryOp::Xor,
        AssignmentOp::ShiftLeft => BinaryOp::ShiftLeft,
        AssignmentOp::ShiftRight => BinaryOp::ShiftRight,
        AssignmentOp::UnsignedShiftRight => BinaryOp::UnsignedShiftRight,
    }
}

fn statements_guarantee_return(statements: &[Stmt<'_, '_>]) -> bool {
    statements.iter().any(statement_guarantees_return)
}

fn statement_guarantees_return(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Return { .. } | Stmt::Throw { .. } => true,
        Stmt::Block { body, .. } => statements_guarantee_return(body),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => statement_guarantees_return(then_branch) && statement_guarantees_return(else_branch),
        Stmt::While {
            condition:
                Expr {
                    kind: ExprKind::Bool(true, _),
                    ..
                },
            body,
            ..
        } => statement_guarantees_return(body) && !statement_contains_break(body),
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            finally.is_some_and(|body| statements_guarantee_return(body))
                || (statements_guarantee_return(body)
                    && catch
                        .as_ref()
                        .is_none_or(|clause| statements_guarantee_return(clause.body)))
        }
        _ => false,
    }
}

fn class_type_name<'src>(ty: &Type<'src>) -> Option<&'src str> {
    class_type_parts(ty).map(|(name, _)| name)
}

fn class_type_parts<'ty, 'src>(ty: &'ty Type<'src>) -> Option<(&'src str, &'ty [Type<'src>])> {
    match ty {
        Type::Class(name) => Some((*name, &[])),
        Type::ClassInstance { name, args } => Some((*name, args)),
        _ => None,
    }
}

fn count_super_calls(statements: &[Stmt<'_, '_>]) -> usize {
    statements.iter().map(count_stmt_super_calls).sum()
}

fn count_stmt_super_calls(statement: &Stmt<'_, '_>) -> usize {
    match statement {
        Stmt::SuperCall { .. } => 1,
        Stmt::Block { body, .. } => count_super_calls(body),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            count_stmt_super_calls(then_branch)
                + else_branch.map_or(0, |branch| count_stmt_super_calls(branch))
        }
        Stmt::While { body, .. }
        | Stmt::For { body, .. }
        | Stmt::ForIn { body, .. }
        | Stmt::ForOf { body, .. } => count_stmt_super_calls(body),
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            count_super_calls(body)
                + catch
                    .as_ref()
                    .map_or(0, |clause| count_super_calls(clause.body))
                + finally.map_or(0, |body| count_super_calls(body))
        }
        _ => 0,
    }
}

fn statement_contains_loop_control(statement: &Stmt<'_, '_>, inside_loop: bool) -> bool {
    match statement {
        Stmt::Break(_) | Stmt::Continue(_) => !inside_loop,
        Stmt::Block { body, .. } => body
            .iter()
            .any(|stmt| statement_contains_loop_control(stmt, inside_loop)),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            statement_contains_loop_control(then_branch, inside_loop)
                || else_branch
                    .as_ref()
                    .is_some_and(|branch| statement_contains_loop_control(branch, inside_loop))
        }
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            body.iter()
                .any(|stmt| statement_contains_loop_control(stmt, inside_loop))
                || catch.as_ref().is_some_and(|clause| {
                    clause
                        .body
                        .iter()
                        .any(|stmt| statement_contains_loop_control(stmt, inside_loop))
                })
                || finally.as_ref().is_some_and(|body| {
                    body.iter()
                        .any(|stmt| statement_contains_loop_control(stmt, inside_loop))
                })
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } | Stmt::ForIn { body, .. } => {
            statement_contains_loop_control(body, true)
        }
        Stmt::ForOf {
            inline: true, body, ..
        } => statement_contains_loop_control(body, inside_loop),
        Stmt::ForOf { body, .. } => statement_contains_loop_control(body, true),
        _ => false,
    }
}

fn statement_contains_break(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Break(_) => true,
        Stmt::Block { body, .. } => body.iter().any(statement_contains_break),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            statement_contains_break(then_branch)
                || else_branch.is_some_and(statement_contains_break)
        }
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            body.iter().any(statement_contains_break)
                || catch
                    .as_ref()
                    .is_some_and(|clause| clause.body.iter().any(statement_contains_break))
                || finally.is_some_and(|body| body.iter().any(statement_contains_break))
        }
        Stmt::While { .. } | Stmt::For { .. } | Stmt::ForIn { .. } | Stmt::ForOf { .. } => false,
        _ => false,
    }
}

fn nullable_type<'src>(ty: Type<'src>) -> Type<'src> {
    match ty {
        Type::Null | Type::Nullable(_) => ty,
        Type::Union(members)
            if members
                .iter()
                .any(|member| matches!(member, Type::Null | Type::Nullable(_))) =>
        {
            Type::Union(members)
        }
        Type::Union(members) => Type::Nullable(Box::new(Type::Union(members))),
        ty => Type::Nullable(Box::new(ty)),
    }
}

fn validate_collection_key(ty: &Type<'_>, span: Span, context: &str) -> Result<(), SemanticError> {
    let supported = match ty {
        Type::TypeParameter("$js") => true,
        Type::Struct(_) | Type::StructInstance { .. } | Type::TypeParameter(_) | Type::Void => {
            false
        }
        Type::Nullable(inner) => validate_collection_key(inner, span, context).is_ok(),
        Type::Array(_) | Type::Map(_, _) | Type::Set(_) => true,
        Type::Union(members) => members
            .iter()
            .all(|member| validate_collection_key(member, span, context).is_ok()),
        _ => true,
    };
    if supported {
        Ok(())
    } else {
        Err(SemanticError::new(
            span,
            format!("{context} type `{ty}` has no portable identity contract"),
        ))
    }
}

fn buffer_member<'src>(
    property: Ident<'src>,
    span: Span,
    return_type: Type<'src>,
) -> Result<Type<'src>, SemanticError> {
    match property.name {
        "slice" => Ok(Type::Function(FunctionType::new(FunctionSignature {
            params: vec![
                FunctionParameter::value(Type::Int),
                FunctionParameter::defaulted(Type::Int, DefaultValue::Int(i32::MAX as i64)),
            ],
            return_type: Box::new(return_type),
        }))),
        _ => Err(SemanticError::new(
            span,
            format!("buffer has no member `{}`", property.name),
        )),
    }
}

fn is_stringifiable_array_element(ty: &Type<'_>) -> bool {
    match ty {
        // Native float formatting intentionally remains separate from JavaScript's
        // shortest Number-to-string algorithm, so accepting floats here would make
        // portable `join` silently target-dependent.
        Type::Int | Type::String | Type::Bool | Type::Null => true,
        Type::Nullable(inner) => is_stringifiable_array_element(inner),
        Type::Union(members) => members.iter().all(is_stringifiable_array_element),
        _ => false,
    }
}

fn json_stringify_type_supported(ty: &Type<'_>) -> bool {
    let scalar = |ty: &Type<'_>| {
        matches!(
            ty,
            Type::Int | Type::Enum(_) | Type::String | Type::Bool | Type::Null
        )
    };
    scalar(ty)
        || matches!(ty, Type::Nullable(inner) if scalar(inner))
        || matches!(ty, Type::Array(inner) | Type::Record(inner) if scalar(inner))
}

fn decode_source_string(
    value: &str,
    span: Span,
) -> Result<crate::literal::StringValue, SemanticError> {
    crate::literal::StringValue::decode_source(value)
        .map_err(|error| SemanticError::new(span, format!("invalid string escape: {error:?}")))
}

fn common_type<'src>(lhs: &Type<'src>, rhs: &Type<'src>) -> Option<Type<'src>> {
    binary_types::common_type_plain(lhs, rhs)
}

fn nullish_present_type<'a, 'src>(ty: &'a Type<'src>) -> Option<&'a Type<'src>> {
    match ty {
        Type::Nullable(inner) => Some(inner),
        Type::Null => Some(ty),
        _ => None,
    }
}

fn optional_result_type<'src>(ty: Type<'src>, span: Span) -> Result<Type<'src>, SemanticError> {
    match ty {
        Type::Void => Err(SemanticError::new(
            span,
            "optional access cannot materialize a nullable `void` value",
        )),
        Type::Function(_) | Type::GenericFunction(_) => Err(SemanticError::new(
            span,
            "optional method calls are not yet supported; coalesce the receiver before calling",
        )),
        Type::Nullable(_) => Ok(ty),
        ty => Ok(Type::Nullable(Box::new(ty))),
    }
}

fn normalize_union<'src>(members: Vec<Type<'src>>) -> Type<'src> {
    binary_types::normalize_union_plain(members)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeTypeCategory {
    Number,
    String,
    Bool,
    Null,
    Array,
    Function,
}

fn runtime_type_category(ty: &Type<'_>) -> Option<RuntimeTypeCategory> {
    match ty {
        Type::Int | Type::Float => Some(RuntimeTypeCategory::Number),
        Type::String => Some(RuntimeTypeCategory::String),
        Type::Bool => Some(RuntimeTypeCategory::Bool),
        Type::Null => Some(RuntimeTypeCategory::Null),
        Type::Array(_) => Some(RuntimeTypeCategory::Array),
        Type::Function(_) | Type::GenericFunction(_) => Some(RuntimeTypeCategory::Function),
        _ => None,
    }
}

fn is_js_value(ty: &Type<'_>) -> bool {
    matches!(ty, Type::TypeParameter("$js"))
}

fn is_js_index_type(ty: &Type<'_>) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::String) || is_js_value(ty)
}

fn is_js_value_or_nullable_js_value(ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter("$js") => true,
        Type::Nullable(inner) => is_js_value(inner),
        _ => false,
    }
}

fn validate_type_guard(
    value: &Type<'_>,
    target: &Type<'_>,
    span: Span,
) -> Result<(), SemanticError> {
    if matches!(target, Type::Union(_) | Type::Nullable(_)) {
        return Err(SemanticError::new(
            span,
            "an `is` guard target must be one concrete member type",
        ));
    }
    let target_category = runtime_type_category(target).ok_or_else(|| {
        SemanticError::new(
            span,
            format!("type `{target}` has no portable runtime type guard"),
        )
    })?;
    if is_js_value_or_nullable_js_value(value) {
        return if matches!(target, Type::Float | Type::String | Type::Bool) {
            Ok(())
        } else {
            Err(SemanticError::new(
                span,
                format!(
                    "a `JsValue` cannot be soundly narrowed to `{target}`; use `float` for JavaScript numbers, `.isArray()` for untyped arrays, or `.truthy()`/`.isObject()` without narrowing"
                ),
            ))
        };
    }
    let members = runtime_guard_members(value);
    if !members.iter().any(|member| member == target) {
        return Err(SemanticError::new(
            span,
            format!("type `{target}` is not a member of `{value}`"),
        ));
    }
    if members
        .iter()
        .any(|member| member != target && runtime_type_category(member) == Some(target_category))
    {
        return Err(SemanticError::new(
            span,
            format!("type guard `{target}` is runtime-ambiguous within `{value}`"),
        ));
    }
    Ok(())
}

fn runtime_guard_members<'src>(value: &Type<'src>) -> Vec<Type<'src>> {
    match value {
        Type::Union(members) => members.clone(),
        Type::Nullable(inner) => {
            let mut members = runtime_guard_members(inner);
            members.push(Type::Null);
            members
        }
        value => vec![value.clone()],
    }
}

fn subtract_guarded_type<'src>(value: &Type<'src>, target: &Type<'src>) -> Option<Type<'src>> {
    match value {
        Type::Union(members) => {
            let remaining = members
                .iter()
                .filter(|member| *member != target)
                .cloned()
                .collect::<Vec<_>>();
            (!remaining.is_empty()).then(|| normalize_union(remaining))
        }
        Type::Nullable(inner) if target == &Type::Null => Some(inner.as_ref().clone()),
        Type::Nullable(inner) if target == inner.as_ref() => Some(Type::Null),
        Type::Nullable(inner) => subtract_guarded_type(inner, target)
            .map(|remaining| Type::Nullable(Box::new(remaining))),
        _ => None,
    }
}

fn common_numeric_type<'src>(lhs: &Type<'src>, rhs: &Type<'src>) -> Type<'src> {
    if lhs == &Type::Float || rhs == &Type::Float {
        Type::Float
    } else {
        Type::Int
    }
}

fn equality_comparable(lhs: &Type<'_>, rhs: &Type<'_>) -> bool {
    binary_types::equality_comparable_plain(lhs, rhs)
}

fn index_key_type<'src>(ty: &Type<'src>) -> Option<Type<'src>> {
    match ty {
        Type::Array(_) | Type::String => Some(Type::Int),
        Type::Record(_) => Some(Type::String),
        ty if crate::typed_array::is_typed_array_type(ty) => Some(Type::Int),
        Type::Union(members) => {
            let mut keys = members.iter().map(index_key_type);
            let first = keys.next().flatten()?;
            keys.all(|key| key.as_ref() == Some(&first))
                .then_some(first)
        }
        _ => None,
    }
}

fn index_value_type<'src>(ty: &Type<'src>, writable: bool) -> Option<Type<'src>> {
    match ty {
        Type::TypeParameter("$js") => Some(Type::TypeParameter("$js")),
        Type::Array(element) => Some(element.as_ref().clone()),
        Type::Record(value) if writable => Some(value.as_ref().clone()),
        Type::Record(value) => Some(nullable_type(value.as_ref().clone())),
        Type::String if !writable => Some(Type::String),
        ty if crate::typed_array::is_typed_array_type(ty) => Some(
            crate::typed_array::TypedArrayKind::from_type(ty)
                .expect("typed array type")
                .index_value_type(),
        ),
        Type::Union(members) => {
            let values = members
                .iter()
                .map(|member| index_value_type(member, writable))
                .collect::<Option<Vec<_>>>()?;
            if writable {
                let first = values.first()?.clone();
                values.iter().all(|value| value == &first).then_some(first)
            } else {
                Some(normalize_union(values))
            }
        }
        _ => None,
    }
}

fn indexed_collection_has_length(ty: &Type<'_>) -> bool {
    match ty {
        Type::Array(_) | Type::String => true,
        ty if crate::typed_array::is_typed_array_type(ty) => true,
        Type::Union(members) => members.iter().all(indexed_collection_has_length),
        _ => false,
    }
}

fn is_stringable(ty: &Type<'_>) -> bool {
    binary_types::is_stringable_plain(ty)
}

fn binary_op_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::Xor => "^",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::UnsignedShiftRight => ">>>",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEq => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::Nullish => "??",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_type_only_exports_keep_their_namespace_and_check_value_reference_abis() {
        let arena = Bump::new();
        let source = parse_source(&arena,
            "export struct Box{int value;}export extern class Document{string title;}export extern Document document;export int first(Box box){return box.value;}").unwrap();
        let model = analyze(&source).unwrap();
        assert!(model.struct_info("Box").is_some());
        assert!(model.class_info("Document").is_some());
        assert!(!model
            .symbols()
            .iter()
            .any(|symbol| matches!(symbol.name, "Box" | "Document")));
        assert!(model
            .symbols()
            .iter()
            .find(|symbol| symbol.name == "document")
            .unwrap()
            .is_foreign());
        let error = check("export extern class Document{string title;}export void update(ref int value){value=1;}").unwrap_err();
        assert!(error
            .message
            .contains("public exports do not yet support mutable-reference"));
    }

    #[test]
    fn mutable_reference_source_contract_preserves_storage_and_value_transfers() {
        let arena = Bump::new();
        let source = parse_source(&arena, "struct Point{int x;int y;}void leaf(ref int value){value+=1;}void forward(ref Point point){leaf(ref point.x);}func()->int snapshot(ref Point point){Point copy=point;return ()=>copy.x;}Point p=Point{1,2};forward(ref p);auto read=snapshot(ref p);print(read());").unwrap();
        let model = analyze(&source).unwrap();
        let signature = model
            .symbols()
            .iter()
            .find(|symbol| symbol.name == "forward")
            .unwrap();
        let Type::Function(signature) = &signature.ty else {
            panic!("signature")
        };
        assert_eq!(
            signature.params[0],
            FunctionParameter {
                ty: Type::Struct(model.struct_type("Point").unwrap()),
                passing: ParameterPassing::MutableReference,
                default: None
            }
        );
        let Item::Function(forward) = &source.items[2] else {
            panic!("forward")
        };
        let Stmt::Expr(Expr {
            kind: ExprKind::Call { args, .. },
            ..
        }) = &forward.body[0]
        else {
            panic!("call")
        };
        assert_eq!(
            model.expression_type(args[0].expression.id),
            Some(&Type::Int)
        );
        assert!(matches!(
            model.expression_resolution(args[0].expression.id),
            ExpressionResolution::NominalMember(_)
        ));
        let ExprKind::Member { object, .. } = &args[0].expression.kind else {
            panic!("field")
        };
        assert_eq!(
            model.expression_type(object.id),
            Some(&Type::Struct(model.struct_type("Point").unwrap()))
        );
        assert!(matches!(
            model.expression_resolution(object.id),
            ExpressionResolution::Binding(_)
        ));
        assert!(model.symbol_is_assigned(
            model
                .identifier_symbol(forward.params[0].name.span)
                .unwrap()
        ));
    }

    #[test]
    fn mutable_reference_arguments_are_explicit_invariant_writable_places() {
        for (source, diagnostic) in [
            (
                "void f(ref int x){}int x=1;f(x);",
                "requires an explicit `ref`",
            ),
            ("void f(int x){}int x=1;f(ref x);", "value parameter cannot"),
            (
                "void f(ref int x){}int x=1;f(ref x+1);",
                "must name a writable lexical place",
            ),
            (
                "void f(ref float x){}int x=1;f(ref x);",
                "storage type must be exactly",
            ),
            (
                "struct P{int x;}void f(ref P p){}P? p=P{1};if(p!=null){f(ref p);}",
                "storage type must be exactly",
            ),
            (
                "struct P{int x;}void f(ref int x){}P? p=P{1};if(p!=null){f(ref p.x);}",
                "nonnullable, nongeneric",
            ),
            (
                "struct P<T>{T x;}void f(ref int x){}P<int> p=P{1};f(ref p.x);",
                "nonnullable, nongeneric",
            ),
            (
                "void f(ref int x){}int[] values=[1];f(ref values[0]);",
                "indexed or host-backed",
            ),
            (
                "void f(ref int x){}extern int value;f(ref value);",
                "not a foreign binding",
            ),
            ("void f(ref int x){}int x=f(ref x);", "own initializer"),
        ] {
            let error = check(source).unwrap_err();
            assert!(
                error.message.contains(diagnostic),
                "{source}: {}",
                error.message
            );
        }
    }

    #[test]
    fn mutable_reference_lifetimes_reject_capture_suspension_and_opaque_abis() {
        for (source, diagnostic) in [
            (
                "async void outer(){auto f=(ref int value)=>await Task.resolve(value);}",
                "cannot suspend in a callable",
            ),
            (
                "void f(ref int value){auto escaped=()=>value;}",
                "cannot be captured",
            ),
            (
                "void f(ref int value){auto escaped=()=>{value=2;};}",
                "cannot be captured",
            ),
            ("void f(ref int value=1){}", "cannot have defaults"),
            ("auto f=(ref int value=1)=>value;", "cannot have defaults"),
            ("async void f(ref int value){}", "async and generator"),
            (
                "generator int f(ref int value){yield value;}",
                "async and generator",
            ),
            ("extern void f(ref int value);", "foreign callable"),
            ("extern func(ref int)->void f;", "foreign bindings"),
            ("export void f(ref int value){}", "public exports"),
            (
                "class Box{init(ref int value){}}",
                "constructors do not support",
            ),
            (
                "class Box{init(int value){}}int value=1;Box b=new Box(ref value);",
                "constructors do not support",
            ),
            (
                "class A{init(int x){}}class B extends A{init(int x){super(ref x);}}",
                "super calls do not support",
            ),
            (
                "void use(ref int x,int y){}async void f(){int x=1;use(ref x,await Task.resolve(2));}",
                "cannot suspend after preparing",
            ),
            (
                "void use(ref int x,int y){}int id(int x){return x;}async void f(){int x=1;use(ref x,id(await Task.resolve(2)));}",
                "cannot suspend after preparing",
            ),
        ] {
            let error = check(source).unwrap_err();
            assert!(
                error.message.contains(diagnostic),
                "{source}: {}",
                error.message
            );
        }
        check(
            "void use(int y,ref int x){}async void f(){int x=1;use(await Task.resolve(2),ref x);}",
        )
        .unwrap();
        check("void use(ref int x,func()->int callback){x=callback();}void f(){int x=1;use(ref x,()=>2);}").unwrap();
        check("func()->int make(ref int value){int copy=value;return ()=>copy;}").unwrap();
    }

    #[test]
    fn mutable_reference_aliasing_forwarding_and_contextual_modes_remain_explicit() {
        check("struct P{int x;int y;}void bump(ref int x){x+=1;}void pair(ref P root,ref int field){root=P{7,8};field+=1;}void recurse(ref int x,int n){if(n>0){bump(ref x);recurse(ref x,n-1);}}P p=P{1,2};pair(ref p,ref p.x);recurse(ref p.x,2);").unwrap();
        check(
            "func(ref int)->int identity=(ref auto value)=>value;int x=1;print(identity(ref x));",
        )
        .unwrap();
        let error = check("func(ref int)->int identity=(int value)=>value;").unwrap_err();
        assert!(error.message.contains("callback parameter"));
        let error = check("func(int)->int identity=(ref int value)=>value;").unwrap_err();
        assert!(error.message.contains("callback parameter"));
        // The old identifier remains an ordinary type/function/local/parameter.
        check("struct ref{int x;}ref copy(ref ref){return ref;}int ref(int x){return x;}int result=ref (1);ref value=ref{2};ref another=copy(value);").unwrap();
    }

    #[test]
    fn parameter_records_keep_value_defaults_arity_and_diagnostics() {
        let arena = Bump::new();
        let source = parse_source(
            &arena,
            "int add(int value,int extra=3){return value+extra;}print(add(4));",
        )
        .unwrap();
        let model = analyze(&source).unwrap();
        let Type::Function(signature) = &model
            .symbols()
            .iter()
            .find(|symbol| symbol.name == "add")
            .unwrap()
            .ty
        else {
            panic!("function")
        };
        assert_eq!(
            signature.params,
            vec![
                FunctionParameter::value(Type::Int),
                FunctionParameter::defaulted(Type::Int, DefaultValue::Int(3))
            ]
        );
        assert_eq!(signature.validate_parameters(), Ok(()));
        assert_eq!(signature.required_params(), 1);
        assert!(!signature.accepts_arity(0));
        assert!(signature.accepts_arity(1) && signature.accepts_arity(2));
        assert!(!signature.accepts_arity(3));
        assert_eq!(
            Type::Function(signature.clone()).to_string(),
            "function(int, int) -> int"
        );
    }

    #[test]
    fn parameter_passing_is_invariant_and_survives_substitution_and_common_types() {
        let reference = FunctionType::new(FunctionSignature {
            params: vec![
                FunctionParameter {
                    ty: Type::TypeParameter("T"),
                    passing: ParameterPassing::MutableReference,
                    default: None,
                },
                FunctionParameter::defaulted(Type::Int, DefaultValue::Int(3)),
            ],
            return_type: Box::new(Type::TypeParameter("T")),
        });
        let mut substitutions = AHashMap::default();
        substitutions.insert("T", Type::Int);
        let Type::Function(reference) = substitute_type(&Type::Function(reference), &substitutions)
        else {
            panic!("function")
        };
        assert_eq!(reference.params[0].ty, Type::Int);
        assert_eq!(
            reference.params[0].passing,
            ParameterPassing::MutableReference
        );
        assert_eq!(reference.params[1].default, Some(DefaultValue::Int(3)));
        assert_eq!(*reference.return_type, Type::Int);
        let mut value = reference.clone();
        value.make_mut().params[0].passing = ParameterPassing::Value;
        let reference = Type::Function(reference);
        let value = Type::Function(value);
        assert!(!is_type_assignable(&reference, &value));
        assert!(!is_type_assignable(&value, &reference));
        let Some(Type::Union(members)) = common_type(&reference, &value) else {
            panic!("distinct modes remain distinct union alternatives")
        };
        assert_eq!(members.len(), 2);
        assert!(members.contains(&reference) && members.contains(&value));
        let wrapped = Type::Nullable(Box::new(Type::Array(Box::new(Type::GenericFunction(
            GenericFunctionType {
                type_params: vec!["T"],
                signature: FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(reference),
                }),
            },
        )))));
        assert!(wrapped.contains_mutable_reference_parameters());
        assert!(!value.contains_mutable_reference_parameters());
    }

    #[test]
    fn parameter_record_validation_rejects_reference_defaults_and_required_suffixes() {
        let mut signature = FunctionSignature {
            params: vec![FunctionParameter::defaulted(
                Type::Int,
                DefaultValue::Int(1),
            )],
            return_type: Box::new(Type::Void),
        };
        signature.params[0].passing = ParameterPassing::MutableReference;
        assert_eq!(
            signature.validate_parameters(),
            Err("mutable-reference parameters cannot have defaults")
        );
        signature.params[0].passing = ParameterPassing::Value;
        signature.params.push(FunctionParameter::value(Type::Int));
        assert_eq!(
            signature.validate_parameters(),
            Err("required parameters cannot follow defaulted parameters")
        );
        signature.params[0].default = None;
        signature.params[0].passing = ParameterPassing::MutableReference;
        assert_eq!(signature.validate_parameters(), Ok(()));
    }

    #[test]
    fn nominal_members_keep_declaring_identity_and_instantiated_receiver_types() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
            struct First { int value; }
            struct Second { int value; }
            class Base<T> {
                T value;
                init(T value) { this.value=value; }
                T read() { return this.value; }
            }
            class Derived extends Base<int> {
                int extra;
                init(int value,int extra) { super(value);this.extra=extra; }
                int total() { return this.value+this.extra; }
            }
            int inspect(First first,Second second,Derived derived) {
                first.value+=1;second.value+=2;
                return first.value+second.value+derived.value+derived.read()+derived.total();
            }
            Derived value=new Derived(3,4);
            print(inspect(First{1},Second{2},value));
        "#,
        )
        .unwrap();
        let model = analyze(&program).unwrap();
        assert_eq!(std::mem::size_of::<ExpressionResolution>(), 8);
        assert_eq!(std::mem::size_of::<Option<NominalId>>(), 4);
        assert_eq!(std::mem::size_of::<Option<NominalMemberId>>(), 4);
        let first = model.struct_info("First").unwrap().fields["value"].member;
        let second = model.struct_info("Second").unwrap().fields["value"].member;
        assert_ne!(first, second);
        let base = model.class_info("Base").unwrap();
        let derived = model.class_info("Derived").unwrap();
        assert_eq!(base.fields["value"].member, derived.fields["value"].member);
        assert_eq!(base.methods["read"].member, derived.methods["read"].member);
        assert_eq!(derived.fields["value"].ty, Type::Int);
        assert_eq!(
            derived.methods["read"].signature.return_type.as_ref(),
            &Type::Int
        );
        assert_eq!(derived.fields["extra"].index, 1);
        for index in 0..model.declarations.nominal_members.len() {
            assert!(model.nominal_member(NominalMemberId::new(index)).is_some());
        }
        let mut reads = 0;
        for source in &model.facts.source_info {
            let Some(expression) = source.expression else {
                continue;
            };
            if let ExpressionResolution::NominalMember(member) = source.resolution {
                let resolved = model.nominal_member(member).unwrap();
                match resolved {
                    NominalMember::Field { owner, field } => {
                        assert_eq!(field.member, member);
                        assert!(model.nominal_name(owner).is_some());
                    }
                    NominalMember::Method {
                        owner,
                        name: "read",
                        ..
                    } => {
                        assert_eq!(model.nominal_name(owner), Some("Base"));
                        assert!(
                            matches!(model.expression_type(expression.id), Some(Type::Function(signature))
                            if signature.return_type.as_ref()==&Type::Int)
                        );
                        reads += 1;
                    }
                    NominalMember::Method { .. } => {}
                }
            }
        }
        assert_eq!(reads, 1);
        crate::lower_to_control_flow(&program, &model).unwrap();
    }

    #[test]
    fn expression_identity_preserves_distinct_types_at_the_same_diagnostic_span() {
        let arena = Bump::new();
        let empty = parse_source(&arena, "").unwrap();
        let nodes = crate::ast::SourceNodes::default();
        let span = Span::empty(0);
        let integer = nodes.expression(ExprKind::Int(7, span));
        let string = nodes.expression(ExprKind::String("seven", span));
        let integer_id = integer.id;
        let string_id = string.id;
        let items = arena.alloc_slice_fill_iter([
            Item::Stmt(Stmt::Expr(integer)),
            Item::Stmt(Stmt::Expr(string)),
        ]);
        let program = empty.with_items(&nodes, items);
        let model = analyze(&program).unwrap();
        assert_eq!(model.expression_type(integer_id), Some(&Type::Int));
        assert_eq!(model.expression_type(string_id), Some(&Type::String));
        assert!(model.belongs_to(program.clone().source_identity()));

        // Equal numeric indices in an independently parsed program do not
        // grant that program access to another source state's checked facts.
        let other = parse_source(&arena, "\"seven\"; 7;").unwrap();
        assert!(!model.belongs_to(other.source_identity()));
        assert!(crate::lower::lower_to_control_flow(&other, &model).is_err());
        assert!(crate::interpreter::interpret_program(&other, &model).is_err());
        assert!(crate::codegen_js::JsEmitter::new(Default::default())
            .emit_checked_program(&other, &model)
            .is_err());
    }

    #[test]
    fn shared_callable_metadata_finalizes_in_each_checking_context() {
        use super::*;
        assert!(std::mem::size_of::<Type>() <= 48);
        assert_eq!(
            std::mem::size_of::<FunctionType>(),
            std::mem::size_of::<usize>()
        );
        let span = Span { start: 10, end: 11 };
        let nodes = crate::ast::SourceNodes::default();
        let expression = nodes.expression(ExprKind::Ident(Ident {
            name: "defaultValue",
            span,
        }));
        let pending = DefaultValue::PendingIdentifier {
            expression: expression.id,
            span,
        };
        let nested = FunctionType::new(FunctionSignature {
            params: vec![FunctionParameter::defaulted(Type::Int, pending.clone())],
            return_type: Box::new(Type::Int),
        });
        let original = FunctionType::new(FunctionSignature {
            params: vec![FunctionParameter::value(Type::Array(Box::new(
                Type::Function(nested),
            )))],
            return_type: Box::new(Type::Void),
        });
        let mut first = original.clone();
        let mut second = original.clone();
        let first_source = [SourceInfo {
            expression: Some(&expression),
            resolution: ExpressionResolution::Binding(SymbolId(41)),
        }];
        let second_source = [SourceInfo {
            expression: Some(&expression),
            resolution: ExpressionResolution::Binding(SymbolId(82)),
        }];
        finalize_default_bindings_in_signature(&mut first, &first_source, false).unwrap();
        finalize_default_bindings_in_signature(&mut second, &second_source, false).unwrap();
        fn default<'src>(signature: &FunctionType<'src>) -> DefaultValue<'src> {
            let Type::Array(element) = &signature.params[0].ty else {
                panic!("array parameter")
            };
            let Type::Function(nested) = element.as_ref() else {
                panic!("callable element")
            };
            nested.params[0].default.clone().unwrap()
        }
        assert_eq!(default(&first), DefaultValue::Symbol(SymbolId(41)));
        assert_eq!(default(&second), DefaultValue::Symbol(SymbolId(82)));
        assert_eq!(default(&original), pending);
        assert!(!signature_has_pending_bindings(&first));
        assert!(!signature_has_pending_bindings(&second));
        assert!(signature_has_pending_bindings(&original));
        let mut stripped = Type::Function(first.clone());
        strip_parameter_defaults_from_type(&mut stripped);
        let Type::Function(stripped) = stripped else {
            unreachable!()
        };
        let Type::Array(element) = &stripped.params[0].ty else {
            panic!("array parameter")
        };
        let Type::Function(nested) = element.as_ref() else {
            panic!("callable element")
        };
        assert!(nested
            .params
            .iter()
            .all(|parameter| parameter.default.is_none()));
        assert_eq!(default(&first), DefaultValue::Symbol(SymbolId(41)));
    }

    use bumpalo::Bump;

    use super::*;
    use crate::parser::parse_source;

    fn check(source: &str) -> Result<(), SemanticError> {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        analyze(&program).map(|_| ())
    }

    #[test]
    fn binary_worklist_checks_both_deep_tree_directions_and_records_source_facts() {
        // Construct the right-nested case directly: this exercises the checker
        // independently of the parser's separate parenthesis-depth behavior.
        for nested_on_left in [true, false] {
            let arena = Bump::new();
            let prefix = parse_source(&arena, "int seed=7;").unwrap();
            let nodes = crate::ast::SourceNodes::continuing(prefix.source_identity());
            let span = Span::empty(12);
            let mut expression = nodes.expression(ExprKind::Ident(Ident { name: "seed", span }));
            let binding_expression = expression.id;
            let mut checked_ids = vec![expression.id];
            for _ in 0..4096 {
                let literal = nodes.expression(ExprKind::Int(1, span));
                checked_ids.push(literal.id);
                let nested = arena.alloc(expression);
                let literal = arena.alloc(literal);
                let (lhs, rhs) = if nested_on_left {
                    (&*nested, &*literal)
                } else {
                    (&*literal, &*nested)
                };
                expression = nodes.expression(ExprKind::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                    span,
                });
                checked_ids.push(expression.id);
            }
            let mut items = prefix.items.to_vec();
            items.push(Item::Stmt(Stmt::Expr(expression)));
            let items = arena.alloc_slice_fill_iter(items);
            let program = prefix.with_items(&nodes, items);
            let model = analyze(&program).unwrap();
            assert!(model.belongs_to(program.source_identity()));
            for id in checked_ids {
                assert_eq!(model.expression_type(id), Some(&Type::Int));
                assert_eq!(
                    model.facts.source_info[id.index()].expression.unwrap().id,
                    id
                );
            }
            let seed = model
                .declarations
                .symbols
                .iter()
                .find(|symbol| symbol.name == "seed")
                .unwrap();
            assert_eq!(
                model.expression_resolution(binding_expression),
                ExpressionResolution::Binding(seed.id)
            );
            assert_eq!(model.identifier_symbol(span), Some(seed.id));
            assert!(model.identifier_index_is_consistent());
        }
    }

    #[test]
    fn binary_worklist_preserves_contextual_nullish_rhs_and_diagnostic_order() {
        check("int[]? first=null;int[]? second=null;int[] values=first??(second??[]);Map<string,int>? source=null;Map<string,int> valuesByName=source??new Map();").unwrap();
        check("struct Box<T>{T value;}Box<int>? source=null;Box<int> box=source??Box{7};").unwrap();
        // Preserve the existing rule: a literal-null LHS supplies Null as the
        // RHS context, so the declaration does not infer this empty array.
        let error = check("int[] values=null??(null??[]);").unwrap_err();
        assert!(
            error.message.contains("cannot infer the element type"),
            "{error}"
        );
        let source = format!(
            "int value=(missingLeft{})+(missingRight+1);",
            "+1".repeat(1024)
        );
        let error = check(&source).unwrap_err();
        assert_eq!(&source[error.span.start..error.span.end], "missingLeft");
        assert!(
            error.message.contains("unknown identifier `missingLeft`"),
            "{error}"
        );
        // A completed left subtree must fail before the right subtree is visited.
        let source = "int value=(1+true)+(missingRight+1);";
        let error = check(source).unwrap_err();
        assert!(error.message.contains("cannot be applied"), "{error}");
        assert_eq!(&source[error.span.start..error.span.end], "1+true");
    }

    #[test]
    fn binary_worklist_preserves_deep_short_circuit_narrowing_and_scope_boundaries() {
        for (guard, repeated, access) in [
            ("value!=null", "&&true", "&&value.length>0"),
            ("value==null", "||false", "||value.length>0"),
        ] {
            let condition = format!("{guard}{}{access}", repeated.repeat(512));
            check(&format!("bool ready(string? value){{return {condition};}}")).unwrap();
            // The RHS guard is not a fact about the following statement.
            let source =
                format!("int size(string? value){{bool ready={condition};return value.length;}}");
            let error = check(&source).unwrap_err();
            assert!(
                error
                    .message
                    .contains("type `string?` has no member `length`"),
                "{error}"
            );
            assert!(error.span.start > source.find("return").unwrap());
        }
    }

    #[test]
    fn registered_identity_counts_preserve_recursive_initializers_and_shadowing() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
            int outer=1;
            int f(int outer){int value=outer;return value;}
            func(int)->int recurse=(int n)=>{if(n==0){return 0;}return recurse(n-1);};
            print(f(outer));print(recurse(3));
        "#,
        )
        .unwrap();
        let model = analyze(&program).unwrap();
        let recursion = model
            .declarations
            .symbols
            .iter()
            .find(|symbol| symbol.name == "recurse")
            .unwrap();
        assert!(
            model.symbol_is_assigned(recursion.id),
            "the recursive initializer needs a stable cell"
        );
        let outers: Vec<_> = model
            .declarations
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "outer")
            .collect();
        assert_eq!(outers.len(), 2);
        assert_ne!(outers[0].id, outers[1].id);
        for symbol in outers {
            assert!(symbol.identifier_occurrences >= 2);
            assert!(
                !model.symbol_is_assigned(symbol.id),
                "later reads are not assignments"
            );
        }
        assert!(model.identifier_index_is_consistent());
    }

    #[test]
    fn identifier_registration_is_idempotent_and_tracks_rebinding_a_span() {
        let arena = Bump::new();
        let program = parse_source(&arena, "int a=1;int b=2;print(a);print(b);").unwrap();
        let mut model = analyze(&program).unwrap();
        let a = model
            .declarations
            .symbols
            .iter()
            .find(|symbol| symbol.name == "a")
            .unwrap()
            .id;
        let b = model
            .declarations
            .symbols
            .iter()
            .find(|symbol| symbol.name == "b")
            .unwrap()
            .id;
        let span = model.declarations.symbols[a.0 as usize].span;
        let count_a = model.declarations.symbols[a.0 as usize].identifier_occurrences;
        let count_b = model.declarations.symbols[b.0 as usize].identifier_occurrences;
        model.record_identifier(span, a);
        assert_eq!(
            model.declarations.symbols[a.0 as usize].identifier_occurrences,
            count_a
        );
        model.record_identifier(span, b);
        assert_eq!(model.identifier_symbol(span), Some(b));
        assert_eq!(
            model.declarations.symbols[a.0 as usize].identifier_occurrences,
            count_a - 1
        );
        assert_eq!(
            model.declarations.symbols[b.0 as usize].identifier_occurrences,
            count_b + 1
        );
        assert!(model.identifier_index_is_consistent());
    }

    #[test]
    fn nested_parameter_defaults_may_capture_outer_locals() {
        check("func(int)->int factory(int defaultSize){return (int size=defaultSize)=>size;}")
            .unwrap();
        let shadowed = check("int value=9;auto make=(int value=value)=>value;").unwrap_err();
        assert!(
            shadowed
                .message
                .contains("parameter defaults can only reference earlier parameters"),
            "{shadowed}"
        );
    }

    #[test]
    fn checks_exhaustive_enum_matches_without_pattern_false_positives() {
        check("enum Status{Draft,Active,Sold}string label(Status value){return match(value){Status.Draft=>\"draft\",Status.Active=>\"active\",Status.Sold=>\"sold\"};}").unwrap();
        check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Draft=>0,_=>1};}").unwrap();

        let missing = check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Draft=>0};}").unwrap_err();
        assert!(missing.message.contains("non-exhaustive"), "{missing}");

        let duplicate = check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Draft=>0,Status.Draft=>1,Status.Active=>2};}").unwrap_err();
        assert!(
            duplicate.message.contains("duplicate match arm"),
            "{duplicate}"
        );

        let wrong_enum = check("enum Status{Draft,Active}enum Other{Draft}int code(Status value){return match(value){Other.Draft=>0,Status.Active=>1};}").unwrap_err();
        assert!(
            wrong_enum.message.contains("expected `Status`"),
            "{wrong_enum}"
        );

        let unknown = check("enum Status{Draft,Active}int code(Status value){return match(value){Status.Missing=>0,Status.Active=>1};}").unwrap_err();
        assert!(
            unknown.message.contains("has no variant `Missing`"),
            "{unknown}"
        );

        let wildcard = check("enum Status{Draft,Active}int code(Status value){return match(value){_=>0,Status.Active=>1};}").unwrap_err();
        assert!(
            wildcard.message.contains("must appear once and last"),
            "{wildcard}"
        );
    }

    #[test]
    fn checks_expression_if_types_and_narrowing() {
        check("int choose(bool flag){return if(flag){1}else{2};}").unwrap();
        check("int choose(int? value){return if(value!=null){value}else{0};}").unwrap();

        let condition = check("int choose(int flag){return if(flag){1}else{2};}").unwrap_err();
        assert!(condition.message.contains("expected `bool`"), "{condition}");

        let arms =
            check("void choose(bool flag){auto value=if(flag){1}else{print(2)};}").unwrap_err();
        assert!(arms.message.contains("incompatible types"), "{arms}");
    }

    #[test]
    fn checks_scalar_literal_matches() {
        check("string label(int value){return match(value){-1=>\"negative\",0=>\"zero\",_=>\"positive\"};}").unwrap();
        check("int flag(bool value){return match(value){true=>1,false=>0};}").unwrap();
        check("int label(string value){return match(value){\"open\"=>1,_=>0};}").unwrap();

        let missing = check("int label(int value){return match(value){0=>1};}").unwrap_err();
        assert!(
            missing.message.contains("requires a final `_`"),
            "{missing}"
        );
        let duplicate =
            check("int label(string value){return match(value){\"x\"=>1,\"x\"=>2,_=>0};}")
                .unwrap_err();
        assert!(
            duplicate.message.contains("duplicate match arm"),
            "{duplicate}"
        );
        let wrong = check("int label(bool value){return match(value){0=>1,_=>0};}").unwrap_err();
        assert!(wrong.message.contains("cannot match `bool`"), "{wrong}");
    }

    #[test]
    fn checks_structural_record_presence_and_writes_soundly() {
        check("Record<int> values=record{alpha:1,beta:2};int first=values.alpha??0;values.gamma=3;int third=values[\"gamma\"]??0;").unwrap();

        let duplicate = check("auto values=record{alpha:1,alpha:2};").unwrap_err();
        assert!(
            duplicate.message.contains("duplicate record key"),
            "{duplicate}"
        );

        let escaped_duplicate =
            check(r#"auto values=record{alpha:1,"\u0061lpha":2};"#).unwrap_err();
        assert!(
            escaped_duplicate.message.contains("duplicate record key"),
            "{escaped_duplicate}"
        );

        let empty = check("auto values=record{};").unwrap_err();
        assert!(empty.message.contains("cannot infer"), "{empty}");

        let mixed = check("Record<int> values=record{alpha:1,beta:\"wrong\"};").unwrap_err();
        assert!(mixed.message.contains("expected `Record<int>`"), "{mixed}");

        let unsafe_update =
            check("Record<int> values=record{alpha:1};values.alpha+=1;").unwrap_err();
        assert!(
            unsafe_update.message.contains("only direct `=`"),
            "{unsafe_update}"
        );

        let missing_without_check =
            check("Record<int> values=record{alpha:1};int value=values.missing;").unwrap_err();
        assert!(
            missing_without_check.message.contains("expected `int`"),
            "{missing_without_check}"
        );
    }

    #[test]
    fn checks_portable_record_object_and_json_operations() {
        check(r#"Record<int> values=record{alpha:1};string[] keys=Object.keys(values);int[] entries=Object.values(values);bool has=Object.hasOwn(values,"alpha");Record<int> merged=Object.assign(values,record{beta:2});string json=JSON.stringify(merged);JsValue parsed=JSON.parse(json);bool object=parsed.isObject();"#).unwrap();

        let mismatch = check(
            "Record<int> target=record{a:1};Record<string> source=record{a:\"x\"};Object.assign(target,source);",
        )
        .unwrap_err();
        assert!(
            mismatch.message.contains("expected `Record<int>`"),
            "{mismatch}"
        );

        let float = check("float value=1.5;string json=JSON.stringify(value);").unwrap_err();
        assert!(
            float.message.contains("does not support `float` portably"),
            "{float}"
        );
    }

    #[test]
    fn checks_first_class_javascript_abi_operations() {
        check(
            r#"
                JsValue object=JS.object();
                JsValue array=JS.array();
                JS.set(object,"answer",42);
                JsValue answer=JS.get(object,"answer");
                bool present=JS.has(object,"answer");
                float length=JS.push(array,answer);
                JsValue popped=JS.pop(array);
                string text=JS.string(popped);
                float numeric=JS.number(popped);
                JsValue invoked=JS.invoke(object,"method",answer);
                bool missing=JS.isUndefined(JS.undefined());
                JS.delete(object,"answer");
            "#,
        )
        .unwrap();

        let unknown = check("JsValue value=JS.unknown();").unwrap_err();
        assert!(
            unknown.message.contains("unknown identifier `JS`"),
            "{unknown}"
        );
        let arity = check("JsValue value=JS.call();").unwrap_err();
        assert!(arity.message.contains("expects at least 2"), "{arity}");

        check("extern JsValue read();JsValue cb=read();JsValue result=cb(1,\"x\");").unwrap();
    }

    #[test]
    fn builtin_namespaces_do_not_override_shadowing_bindings() {
        check(
            r#"
                struct Api { int value; }
                int read(Api Object) { return Object.value; }
                int run(func(int)->int print) { return print(41); }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn checks_for_of_element_types_and_iterables() {
        check("int[] values=[1,2];for(int value of values){print(value);}Float32Array floats=new Float32Array(2);for(float value of floats){print(value);}").unwrap();

        let wrong = check("string[] values=[\"a\"];for(int value of values){}").unwrap_err();
        assert!(wrong.message.contains("expected `int`"), "{wrong}");

        let string = check("for(string value of \"text\"){}").unwrap_err();
        assert!(string.message.contains("array or typed array"), "{string}");
    }

    #[test]
    fn checks_inline_for_requires_const_list() {
        check("int total=0;inline for(int value of [1,2,3]){total+=value;}").unwrap();

        let runtime = check("int[] values=[1,2];inline for(int value of values){}").unwrap_err();
        assert!(
            runtime.message.contains("constant array literal"),
            "{runtime}"
        );

        let control = check("inline for(int value of [1,2]){break;}").unwrap_err();
        assert!(
            control.message.contains("`break` or `continue`"),
            "{control}"
        );
    }

    #[test]
    fn checks_array_and_record_spreads_without_widening_unsafely() {
        check("int[] base=[1,2];int[] values=[0,...base,3];Record<int> source=record{a:1};Record<int> merged=record{...source,b:2};").unwrap();

        let typed_array =
            check("Uint8Array bytes=new Uint8Array(2);int[] values=[...bytes];").unwrap_err();
        assert!(
            typed_array
                .message
                .contains("array spread requires an array"),
            "{typed_array}"
        );

        let array_mismatch =
            check("string[] words=[\"x\"];int[] values=[1,...words];").unwrap_err();
        assert!(
            array_mismatch.message.contains("expected `int`")
                || array_mismatch.message.contains("expected `int[]`"),
            "{array_mismatch}"
        );

        let record_mismatch = check(
            "Record<int> numbers=record{a:1};Record<string> words=record{a:\"x\"};Record<int> merged=record{...numbers,...words};",
        )
        .unwrap_err();
        assert!(
            record_mismatch.message.contains("expected `Record<int>`"),
            "{record_mismatch}"
        );

        let wrong_kind =
            check("int[] values=[1];Record<int> merged=record{...values};").unwrap_err();
        assert!(
            wrong_kind
                .message
                .contains("record spread requires a record"),
            "{wrong_kind}"
        );
    }

    #[test]
    fn contextual_record_literals_widen_fresh_values_without_alias_covariance() {
        check("struct Entry{int value;}Record<Entry> source=record{item:Entry{1}};Record<JsValue> direct=record{item:Entry{2}};Record<JsValue> spread=record{...source};").unwrap();

        let alias = check(
            "struct Entry{int value;}Record<Entry> source=record{item:Entry{1}};Record<JsValue> alias=source;",
        )
        .unwrap_err();
        assert!(
            alias
                .message
                .contains("expected `Record<JsValue>`, found `Record<Entry>`"),
            "{alias}"
        );
    }

    #[test]
    fn rejects_nominal_array_widening_to_js_value() {
        let error =
            check("struct Entry{int value;}Entry[] entries=[Entry{1}];JsValue[] erased=entries;")
                .unwrap_err();
        assert!(
            error
                .message
                .contains("expected `JsValue[]`, found `Entry[]`"),
            "{error}"
        );

        check("struct Entry{int value;}Entry[] entries=[Entry{1}];Entry[] exact=entries;JsValue[] contextual=[Entry{2}];").unwrap();
    }

    #[test]
    fn rejects_numeric_mutable_array_widening() {
        let error = check("int[] integers=[1,2];float[] widened=integers;").unwrap_err();
        assert!(
            error.message.contains("expected `float[]`, found `int[]`"),
            "{error}"
        );

        check("int[] integers=[1,2];int[] exact=integers;float[] contextual=[1,2];").unwrap();
    }

    #[test]
    fn rejects_writes_through_heterogeneous_collection_unions() {
        let error = check("void overwrite(int[]|string[] values){values[0]=1;}").unwrap_err();
        assert!(
            error
                .message
                .contains("cannot assign through an index on `int[] | string[]`"),
            "{error}"
        );

        let member = check("void resize(int[]|string values){values.length=0;}").unwrap_err();
        assert!(
            member
                .message
                .contains("cannot assign through member `length` on union"),
            "{member}"
        );

        check("void overwrite(int[]|Int32Array values){values[0]=1;}").unwrap();
    }

    #[test]
    fn requires_every_union_callable_member_to_accept_a_call() {
        let error = check(
            "int zero(){return 0;}int one(int value){return value;}(func()->int)|(func(int)->int) choose(bool first){if(first){return zero;}return one;}int value=choose(false)();",
        )
        .unwrap_err();
        assert!(
            error.message.contains("cannot call a value of type")
                && error.message.contains("with 0 arguments"),
            "{error}"
        );

        check(
            "int fromInt(int value){return value;}int fromFloat(float value){return 1;}(func(int)->int)|(func(float)->int) choose(bool first){if(first){return fromInt;}return fromFloat;}int value=choose(false)(1);",
        )
        .unwrap();

        let omitted_default = check(
            "int one(int value=1){return value;}int two(int value=2){return value;}auto choices=[one,two];auto chosen=choices[0];int value=chosen();",
        )
        .unwrap_err();
        assert!(
            omitted_default
                .message
                .contains("union calls require every argument explicitly"),
            "{omitted_default}"
        );

        check(
            "int one(int value=1){return value;}int two(int value=2){return value;}auto choices=[one,two];auto chosen=choices[0];int value=chosen(3);",
        )
        .unwrap();
    }

    #[test]
    fn revalidates_earlier_generic_arguments_after_inference_widens() {
        let call = check(
            "struct Entry{int value;}void copy<T>(T[] target,T[] source){target[0]=source[0];}Entry[] target=[Entry{1}];JsValue[] source=[\"bad\"];copy(target,source);",
        )
        .unwrap_err();
        assert!(
            call.message
                .contains("expected `JsValue[]`, found `Entry[]`"),
            "{call}"
        );

        let constructor = check(
            "struct Entry{int value;}class Pair<T>{T[] left;T[] right;init(T[] left,T[] right){this.left=left;this.right=right;}}Entry[] entries=[Entry{1}];JsValue[] values=[\"bad\"];auto pair=new Pair(entries,values);",
        )
        .unwrap_err();
        assert!(
            constructor
                .message
                .contains("expected `JsValue[]`, found `Entry[]`"),
            "{constructor}"
        );
    }

    #[test]
    fn requires_a_common_index_key_across_union_members() {
        check("void read(Record<int>|Record<string> values){auto value=values[\"key\"];} ")
            .unwrap();

        let wrong_key =
            check("void read(Record<int>|Record<string> values){auto value=values[0];}")
                .unwrap_err();
        assert!(
            wrong_key.message.contains("expected `string`"),
            "{wrong_key}"
        );

        let mixed =
            check("void read(int[]|Record<int> values){auto value=values[0];}").unwrap_err();
        assert!(
            mixed.message.contains("cannot index a value of type"),
            "{mixed}"
        );
    }

    #[test]
    fn infers_nullable_destructured_values_and_nonnullable_array_rest() {
        check("int[] values=[1,2];auto [first,,third,...rest]=values;int a=first??0;int b=third??0;int[] tail=rest;Record<int> source=record{x:1};auto {x,missing,...remaining}=source;int c=x??0;int d=missing??0;Record<int> others=remaining;").unwrap();

        let unsafe_value =
            check("int[] values=[1];auto [first]=values;int required=first;").unwrap_err();
        assert!(
            unsafe_value.message.contains("expected `int`"),
            "{unsafe_value}"
        );

        let non_array = check("auto [first]=\"text\";").unwrap_err();
        assert!(
            non_array
                .message
                .contains("array destructuring requires an array"),
            "{non_array}"
        );

        let duplicate =
            check("Record<int> source=record{a:1};auto {a,a:other}=source;").unwrap_err();
        assert!(
            duplicate.message.contains("duplicate record binding key"),
            "{duplicate}"
        );
    }

    #[test]
    fn infers_auto_and_checks_array_map() {
        check("int[] values=[1,2,3]; auto doubled=values.map((int value)=>value*2);").unwrap();
        check("int[] values=[1,2,3];values.map((int value)=>print(value));").unwrap();
    }

    #[test]
    fn validates_explicit_math_imul_calls() {
        check("int value=Math.imul(2147483647,2147483647);").unwrap();

        let arity = check("int value=Math.imul(1);").unwrap_err();
        assert!(arity.message.contains("expects two arguments"), "{arity}");

        let argument = check("int value=Math.imul(1,2.0);").unwrap_err();
        assert!(argument.message.contains("expected `int`"), "{argument}");
    }

    #[test]
    fn rejects_wrong_initializer_type() {
        let error = check("int value=\"no\";").unwrap_err();
        assert!(error.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn requires_numeric_assignable_update_targets() {
        let literal = check("int value=++1;").unwrap_err();
        assert!(
            literal
                .message
                .contains("expression is not an assignable location"),
            "{literal}"
        );

        let string = check("string value=\"ready\";value++;").unwrap_err();
        assert!(
            string
                .message
                .contains("operator `++` requires a numeric target"),
            "{string}"
        );
    }

    #[test]
    fn validates_nullable_assignments_calls_and_equality() {
        check(
            "T? maybe<T>(bool present,T value){if(present){return value;}return null;}int? value=maybe(true,7);bool present=value!=null;bool same=value==7;",
        )
        .unwrap();
        let error = check("int value=null;").unwrap_err();
        assert!(error.message.contains("expected `int`, found `null`"));
        let error = check("auto value=null;").unwrap_err();
        assert!(error.message.contains("explicit nullable type"));
        let error = check("int value=1;bool same=value==null;").unwrap_err();
        assert!(error.message.contains("cannot be applied"));
        let error = check(
            "struct Pair{int left;int right;}Pair? maybe=null;Pair pair=Pair{1,2};bool same=maybe==pair;",
        )
        .unwrap_err();
        assert!(error.message.contains("cannot be applied"));
    }

    #[test]
    fn validates_union_assignments_arrays_and_generic_inference() {
        check(
            r#"
                T choose<T>(T left,T right){return left;}
                string|int value=1;
                value="ready";
                int|string reordered=value;
                (string|int)[] values=[1,"two",3];
                string|int selected=choose(reordered,"fallback");
                bool same=selected=="ready";
            "#,
        )
        .unwrap();

        let error = check("string|int value=true;").unwrap_err();
        assert!(
            error
                .message
                .contains("expected `string | int`, found `bool`"),
            "{error}"
        );

        let error = check("bool same=1==true;").unwrap_err();
        assert!(error.message.contains("cannot be applied"), "{error}");
    }

    #[test]
    fn empty_record_assigns_to_js_value() {
        check("JsValue empty=record{};empty[\"k\"]=1;").unwrap();
    }

    #[test]
    fn allows_js_value_index_assignment() {
        check(
            r#"
                void set(JsValue options){
                    options["duration"]=0.0;
                    options["type"]="keyframes";
                    string key="delay";
                    options[key]=1.0;
                }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn narrows_through_logical_and() {
        check(
            r#"
                string label(JsValue? value){
                    if(value!=null && value is string){return value;}
                    return "none";
                }
                bool ready(string? name, JsValue? element){
                    if(name!=null && element!=null){return name.length>0 && element.truthy();}
                    return false;
                }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn narrows_nullable_js_value_with_portable_guards() {
        check(
            r#"
                bool isName(JsValue? value){
                    if(value is string){return value.startsWith("--");}
                    return false;
                }
                bool isZero(JsValue? value){
                    if(value is float){return value==0.0;}
                    return value==null;
                }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn narrows_portably_distinguishable_union_members() {
        check(
            r#"
                string describe(string|int value){
                    if(value is string){return value.toUpperCase();}
                    else{return "number-"+value;}
                }
                string invoke((func(int)->int)|string value){
                    if(value is func(int)->int){return "result-"+value(4);}
                    else{return value;}
                }
                string nested(string|int|bool value){
                    if(value is string){return value;}
                    else if(value is int){return "number-"+value;}
                    else if(value){return "yes";}
                    else{return "no";}
                }
                string nullableUnion((string|int)? value){
                    if(value==null){return "none";}
                    else if(value is string){return value.toUpperCase();}
                    else{return "number-"+value;}
                }
            "#,
        )
        .unwrap();

        let ambiguous = check("bool test(int|float value){return value is int;}").unwrap_err();
        assert!(
            ambiguous.message.contains("runtime-ambiguous"),
            "{ambiguous}"
        );
        let absent = check("bool test(string|int value){return value is bool;}").unwrap_err();
        assert!(absent.message.contains("is not a member"), "{absent}");
    }

    #[test]
    fn narrows_javascript_values_only_when_the_runtime_check_proves_the_type() {
        check(
            "bool inspect(JsValue value){if(value is string){print(value);}else if(value is float){print(value);}else if(value is bool){print(value);}return value.isArray()||value.isObject()||value.truthy();}",
        )
        .unwrap();

        for target in ["int", "string[]", "func(int)->int"] {
            let error = check(&format!(
                "bool inspect(JsValue value){{return value is {target};}}"
            ))
            .unwrap_err();
            assert!(
                error.message.contains("cannot be soundly narrowed"),
                "{error}"
            );
        }
    }

    #[test]
    fn propagates_narrowing_from_terminating_guard_branches() {
        check(
            r#"
                string nullable(string? value){
                    if(value==null){return "none";}
                    return value.toUpperCase();
                }
                string unionValue(string|int value){
                    if(value is string){return value;}
                    return "number-"+value;
                }
                string negated(string|int value){
                    if(!(value is string)){return "number-"+value;}
                    return value.toUpperCase();
                }
            "#,
        )
        .unwrap();

        let invalidated = check(
            r#"
                string invalid(string|int value){
                    if(value is string){value=1;}
                    else{return "number";}
                    return value.toUpperCase();
                }
            "#,
        )
        .unwrap_err();
        assert!(
            invalidated.message.contains("string | int"),
            "{invalidated}"
        );
    }

    #[test]
    fn validates_scalar_parameter_defaults_and_optional_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"int add(int value,int amount=2){return value+amount;}int direct=add(3);auto callable=add;int indirect=callable(4);"#,
        )
        .unwrap();

        check(
            r#"
                int sum(int[] values=[1,2,3]){return values.length;}
                int nested(int[][] values=[[1],[2,3]]){return values.length;}
                class Bag {
                    int[] values;
                    init(int[] values=[]){this.values=values;}
                }
                int first=sum();
                int second=nested();
                Bag bag=new Bag();

                int apply(int value,func(int)->int transform=(int current)=>current+1){
                    return transform(value);
                }
                int transformed=apply(4);

                struct Point { int x; int y; }
                int pointSum(Point point=Point{2,3}){return point.x+point.y;}
                class Box {
                    int value;
                    init(int value=4){this.value=value;}
                }
                int boxValue(Box box=new Box()){return box.value;}
            "#,
        )
        .unwrap();

        analyze(&program).unwrap();

        check(
            r#"
                T choose<T>(T current,T next,(func(T,T)->bool)? equals=null){
                    if(equals==null){if(current==next){return current;}return next;}
                    func(T,T)->bool compare=equals;
                    if(compare(current,next)){return current;}
                    return next;
                }
                int result=choose(1,2);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn rejects_invalid_parameter_defaults() {
        let arena = Bump::new();
        let wrong_type =
            parse_source(&arena, r#"int value(int input="no"){return input;}"#).unwrap();
        let error = analyze(&wrong_type).unwrap_err();
        assert!(error.message.contains("default value has type `string`"));

        let non_literal = parse_source(&arena, "int value(int input=1+2){return input;}").unwrap();
        let error = analyze(&non_literal).unwrap_err();
        assert!(error
            .message
            .contains("not a supported literal for parameter type `int`"));

        let arena = Bump::new();
        let wrong_array = parse_source(
            &arena,
            r#"int value(int[] input=["no"]){return input.length;}"#,
        )
        .unwrap();
        let error = analyze(&wrong_array).unwrap_err();
        assert!(error
            .message
            .contains("not a supported literal for parameter type `int[]`"));

        let arena = Bump::new();
        let wrong_callback = parse_source(
            &arena,
            r#"int value(func(int)->int transform=(int input)=>"no"){return transform(1);}"#,
        )
        .unwrap();
        let error = analyze(&wrong_callback).unwrap_err();
        assert!(error.message.contains("expected `function(int) -> int`"));

        let arena = Bump::new();
        let wrong_aggregate = parse_source(
            &arena,
            "struct Left{int value;}struct Right{int value;}int read(Left value=Right{1}){return value.value;}",
        )
        .unwrap();
        let error = analyze(&wrong_aggregate).unwrap_err();
        assert!(error
            .message
            .contains("not a supported literal for parameter type `Left`"));
    }

    #[test]
    fn js_undefined_default_requires_the_unshadowed_builtin() {
        check("JsValue read(JsValue JS,JsValue value=JS.undefined()){return value;}").unwrap();

        let error =
            check("auto read=(JsValue JS,JsValue value=JS.undefined())=>value;").unwrap_err();
        assert!(error.message.contains("undefined"), "{error}");
    }

    #[test]
    fn rejects_transported_defaults_that_depend_on_local_bindings() {
        let error = check("[1,2].map((int seed)=>(int value=seed)=>value)[1]();").unwrap_err();
        assert!(error.message.contains("local binding"), "{error}");

        check("int seed=7;int value=((int current=seed)=>current)();").unwrap();
        check("int value=((int first,int second=first)=>second)(7);").unwrap();
    }

    #[test]
    fn narrows_nullable_values_in_guarded_branches() {
        check(
            "class Box{int value;init(int value){this.value=value;}}int read(Box? box){if(box!=null){return box.value;}return -1;}int readElse(Box? box){if(box==null){return -1;}else{return box.value;}}",
        )
        .unwrap();

        let error = check(
            "class Box{int value;init(int value){this.value=value;}}void bad(Box? box){if(box!=null){box=null;print(box.value);}}",
        )
        .unwrap_err();
        assert!(error.message.contains("Box?"), "{error}");
    }

    #[test]
    fn rejects_out_of_range_integer_literals() {
        let arena = Bump::new();
        let program = parse_source(&arena, "int value=2147483648;").unwrap();
        assert!(analyze(&program)
            .unwrap_err()
            .message
            .contains("signed 32-bit"));
    }

    #[test]
    fn rejects_unimplemented_struct_equality_and_allows_function_equality() {
        let struct_error = check(
            "struct Pair{int left;int right;}Pair a=Pair{1,2};Pair b=Pair{1,2};bool same=a==b;",
        )
        .unwrap_err();
        assert!(struct_error.message.contains("cannot be applied"));

        check("auto a=(int value)=>value;auto b=(int value)=>value;bool same=a==b;").unwrap();
    }

    #[test]
    fn rejects_unknown_identifiers() {
        let error = check("int value=missing+1;").unwrap_err();
        assert!(error.message.contains("unknown identifier `missing`"));
    }

    #[test]
    fn validates_struct_fields_and_literals() {
        check("struct Point{int x;int y;} Point p=Point{10,20}; int x=p.x;").unwrap();
        let error = check("struct Point{int x;} Point p=Point{\"bad\"};").unwrap_err();
        assert!(error.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn resolves_generic_struct_literals_from_context() {
        check("struct Box<T>{T value;}Box<int> box=Box{7};int value=box.value;").unwrap();

        let missing = check("struct Box<T>{T value;}auto box=Box{7};").unwrap_err();
        assert!(
            missing
                .message
                .contains("requires a contextual `Box<...>` type"),
            "{missing}"
        );
    }

    #[test]
    fn validates_functions_and_returns() {
        check("int add(int a,int b){return a+b;} int result=add(1,2);").unwrap();
        let error = check("int bad(){return true;}").unwrap_err();
        assert!(error.message.contains("expected return type `int`"));
    }

    #[test]
    fn validates_class_construction_and_methods() {
        check(
            "class Vector{float x;float length(){return this.x;}} Vector v=new Vector(); float n=v.length();",
        )
        .unwrap();
    }

    #[test]
    fn validates_control_flow_mutation_and_templates() {
        check(
            "int sum=0;for(int i=0;i<3;i++){sum+=i;}if(sum==3){print(`sum=${sum}`);}else{print(\"bad\");}",
        )
        .unwrap();
    }

    #[test]
    fn validates_constructor_arguments() {
        check(
            "class Pair{int x;int y;init(int x,int y){this.x=x;this.y=y;}} Pair p=new Pair(1,2);",
        )
        .unwrap();
        let error = check("class Pair{init(int x){}} Pair p=new Pair(\"wrong\");").unwrap_err();
        assert!(error.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn rejects_break_outside_loop() {
        let error = check("break;").unwrap_err();
        assert!(error.message.contains("outside a loop"));
    }

    #[test]
    fn validates_standard_array_and_string_methods() {
        check(
            "int[] xs=[1,2,3];int[] ys=xs.filter((int x)=>x>1);int total=ys.reduce((int a,int x)=>a+x,0);int n=xs.push(4);int last=xs.pop();bool found=\"lilscript\".includes(\"pex\");",
        )
        .unwrap();
    }

    #[test]
    fn validates_compact_array_and_typed_array_bulk_methods() {
        check(
            "int[] values=[1,2,3];bool has=values.includes(2);bool tail=values.includes(1,-2);string text=values.join(\"-\");bool any=values.some((int value)=>value>2);bool all=values.every((int value)=>value>0);int found=values.findIndex((int value)=>value==2);int[] combined=values.concat([4,5]);values.copyWithin(1,0,2).reverse();Uint8Array a=new Uint8Array(8);Uint8Array b=new Uint8Array(2);a.fill(7,1,6);a.set(b,2);a.copyWithin(4,1,3);",
        )
        .unwrap();

        let join_error =
            check("class Box{}Box[] boxes=[];string text=boxes.join(\",\");").unwrap_err();
        assert!(
            join_error.message.contains("cannot be joined portably"),
            "{join_error}"
        );

        let float_join_error =
            check("float[] values=[0.1];string text=values.join(\",\");").unwrap_err();
        assert!(
            float_join_error
                .message
                .contains("cannot be joined portably"),
            "{float_join_error}"
        );

        let set_error = check(
            "Uint8Array bytes=new Uint8Array(2);Int16Array words=new Int16Array(2);bytes.set(words);",
        )
        .unwrap_err();
        assert!(
            set_error.message.contains("expected `Uint8Array`"),
            "{set_error}"
        );

        let predicate_error =
            check("int[] values=[1];bool found=values.some((int value)=>value);").unwrap_err();
        assert!(
            predicate_error.message.contains("must return `bool`"),
            "{predicate_error}"
        );
    }

    #[test]
    fn validates_compact_string_search_and_repeat_methods() {
        check(
            "string value=\"ababa\";int first=value.indexOf(\"ba\",1);int last=value.lastIndexOf(\"ba\");string repeated=value.repeat(2);",
        )
        .unwrap();

        let position_error = check("int value=\"abc\".indexOf(\"a\",true);").unwrap_err();
        assert!(
            position_error.message.contains("expected `int`"),
            "{position_error}"
        );
    }

    #[test]
    fn validates_javascript_regex_construction_testing_and_metadata() {
        check("Regex pattern=new Regex(\"sale\",\"gi\");bool found=pattern.test(\"SALE\");JsValue matched=pattern.exec(\"SALE\");string source=pattern.source;string flags=pattern.flags;bool global=pattern.global;bool insensitive=pattern.ignoreCase;float index=pattern.lastIndex;").unwrap();

        let arity = check("Regex pattern=new Regex();").unwrap_err();
        assert!(
            arity.message.contains("expects 1 or 2 arguments"),
            "{arity}"
        );

        let argument = check("Regex pattern=new Regex(1);").unwrap_err();
        assert!(argument.message.contains("expected `string`"), "{argument}");

        let unknown = check("Regex pattern=new Regex(\"x\");pattern.matches(\"x\");").unwrap_err();
        assert!(
            unknown.message.contains("has no member `matches`"),
            "{unknown}"
        );
    }

    #[test]
    fn validates_nullish_operators_without_truthiness_coercion() {
        check(
            "string? optional=null;string first=optional??\"fallback\";string second=null??\"literal\";optional??=\"stored\";int?[] values=[null];values[0]??=7;",
        )
        .unwrap();

        let non_nullable =
            check("string value=\"\";string result=value??\"fallback\";").unwrap_err();
        assert!(non_nullable.message.contains("nullable left operand"));

        let invalid_store = check("int? value=null;value??=\"wrong\";").unwrap_err();
        assert!(invalid_store.message.contains("expected `int?`"));
    }

    #[test]
    fn validates_optional_access_only_for_nullable_data() {
        check("int[]? values=null;int? length=values?.length;int? first=values?.[0];").unwrap();
        let non_nullable = check("int[] values=[1];int? first=values?.[0];").unwrap_err();
        assert!(non_nullable.message.contains("nullable receiver"));
        let method = check("int[]? values=null;auto mapped=values?.map;").unwrap_err();
        assert!(method.message.contains("must be called"), "{method}");
    }

    #[test]
    fn treats_number_as_non_wrapping_binary64() {
        check("number value=1;number next=value*3+0.5;number step(number input){return input+1;}")
            .unwrap();
        let bitwise = check("number value=1;number shifted=value<<1;").unwrap_err();
        assert!(bitwise.message.contains("cannot be applied"), "{bitwise}");
    }

    #[test]
    fn validates_collections_and_binary_memory() {
        check(
            "Map<string,int> values=new Map();values.set(\"x\",1);int? value=values.get(\"x\");Set<int> seen=new Set<int>();seen.add(1);ArrayBuffer buffer=new ArrayBuffer(4);Uint8Array bytes=new Uint8Array(buffer);bytes[0]=255;Uint8Array tail=bytes.subarray(1);",
        )
        .unwrap();

        let bad_key =
            check("struct Point{int x;}Map<Point,int> values=new Map<Point,int>();").unwrap_err();
        assert!(bad_key.message.contains("portable identity contract"));

        check("Map<JsValue,int> values=new Map<JsValue,int>();JsValue key=record{ok:1};values.set(key,1);").unwrap();

        let bad_buffer = check("ArrayBuffer buffer=new ArrayBuffer(\"four\");").unwrap_err();
        assert!(bad_buffer
            .message
            .contains("expected `int`, found `string`"));

        let bad_byte = check("Uint8Array bytes=new Uint8Array(4);bytes[0]=\"wrong\";").unwrap_err();
        assert!(bad_byte.message.contains("expected `int`, found `string`"));
    }

    #[test]
    fn requires_all_paths_to_return() {
        check("int choose(bool flag){if(flag){return 1;}else{return 2;}}").unwrap();
        let error = check("int choose(bool flag){if(flag){return 1;}}").unwrap_err();
        assert!(error.message.contains("must return a value"));
    }

    #[test]
    fn checks_explicit_callable_types() {
        check("func(int)->int twice=(int x)=>x*2;int answer=twice(21);").unwrap();
        let error = check("func(int)->bool bad=(int x)=>x+1;").unwrap_err();
        assert!(error.message.contains("expected `function(int) -> bool`"));
    }

    #[test]
    fn allows_explicit_function_bindings_to_recurse() {
        check("func(int)->int loop=(int n)=>loop(n);int value=loop(1);").unwrap();
        check("JsValue self=(JsValue _)=>self;JsValue value=self;").unwrap();
        let inferred = check("auto fact=(int n)=>fact(n);").unwrap_err();
        assert!(
            inferred.message.contains("unknown identifier `fact`"),
            "{inferred}"
        );
        let during_init = check("int boom=boom+1;").unwrap_err();
        assert!(
            during_init
                .message
                .contains("cannot read `boom` in its own initializer"),
            "{during_init}"
        );
        let shadowing_during_init = check("int value=1;{int value=value+1;}").unwrap_err();
        assert!(
            shadowing_during_init
                .message
                .contains("cannot read `value` in its own initializer"),
            "{shadowing_during_init}"
        );
    }

    #[test]
    fn checks_typed_javascript_adapter_callback_conventions() {
        check(
            "JsValue method0=JS.method0((JsValue self)=>self);JsValue method1=JS.method1((JsValue self,JsValue a)=>a);JsValue method2=JS.method2((JsValue self,JsValue a,JsValue b)=>b);JsValue method3=JS.method3((JsValue self,JsValue a,JsValue b,JsValue c)=>c);JsValue method4=JS.method4((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d)=>d);JsValue method5=JS.method5((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d,JsValue e)=>e);JsValue method6=JS.method6((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d,JsValue e,JsValue f)=>f);JsValue method7=JS.method7((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d,JsValue e,JsValue f,JsValue g)=>g);JsValue method8=JS.method8((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d,JsValue e,JsValue f,JsValue g,JsValue h)=>h);JsValue method9=JS.method9((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d,JsValue e,JsValue f,JsValue g,JsValue h,JsValue i)=>i);JsValue method10=JS.method10((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d,JsValue e,JsValue f,JsValue g,JsValue h,JsValue i,JsValue j)=>j);JsValue methodRest=JS.methodRest((JsValue self,JsValue args)=>args);JsValue staticRest=JS.staticRest((JsValue args)=>args);JsValue constructed=JS.construct(method0);",
        )
        .unwrap();

        let arity = check("JS.method0((JsValue self,JsValue extra)=>self);").unwrap_err();
        assert!(
            arity.message.contains("expects 1 parameters, found 2"),
            "{arity}"
        );

        let method3_arity =
            check("JS.method3((JsValue self,JsValue a,JsValue b)=>a);").unwrap_err();
        assert!(
            method3_arity
                .message
                .contains("expects 4 parameters, found 3"),
            "{method3_arity}"
        );

        let method10_arity = check(
            "JS.method10((JsValue self,JsValue a,JsValue b,JsValue c,JsValue d,JsValue e,JsValue f,JsValue g,JsValue h,JsValue i)=>i);",
        )
        .unwrap_err();
        assert!(
            method10_arity
                .message
                .contains("expects 11 parameters, found 10"),
            "{method10_arity}"
        );

        let parameter_type = check("JS.method1((int self,JsValue value)=>value);").unwrap_err();
        assert!(
            parameter_type
                .message
                .contains("callback parameter must be `JsValue`, found `int`"),
            "{parameter_type}"
        );

        let return_type = check("JS.staticRest((JsValue args)=>{});").unwrap_err();
        assert!(
            return_type
                .message
                .contains("expected `function(JsValue) -> JsValue`"),
            "{return_type}"
        );
    }

    #[test]
    fn allows_jsvalue_property_keys_on_jsvalue_bags() {
        check("extern JsValue obj;extern JsValue key;JsValue value=obj[key];obj[key]=value;")
            .unwrap();
        let error =
            check("extern JsValue obj;extern bool key;JsValue value=obj[key];").unwrap_err();
        assert!(
            error.message.contains("numeric, `string`, or `JsValue`"),
            "{error}"
        );
    }

    #[test]
    fn accepts_rebinding_captured_values() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int run(int seed){auto next=()=>{seed+=1;return seed;};return next();}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        assert!(semantics
            .symbols()
            .iter()
            .any(|symbol| { symbol.name == "seed" && semantics.symbol_is_assigned(symbol.id) }));
    }

    #[test]
    fn rejects_uninitialized_variables() {
        let arena = Bump::new();
        let program = parse_source(&arena, "int value;").unwrap();
        assert!(analyze(&program)
            .unwrap_err()
            .message
            .contains("require an initializer"));
    }

    #[test]
    fn checks_extern_call_signatures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int hostAdd(int a,int b);int value=hostAdd(1,2);",
        )
        .unwrap();
        analyze(&program).unwrap();

        let invalid = parse_source(
            &arena,
            "extern int hostAdd(int a,int b);int value=hostAdd(1);",
        )
        .unwrap();
        assert!(analyze(&invalid)
            .unwrap_err()
            .message
            .contains("2 arguments"));
    }

    #[test]
    fn checks_typed_host_object_members() {
        check(
            r#"
                extern class Element {
                    string textContent;
                    void setAttribute(string name, string value);
                }
                extern class Document {
                    Element createElement(string tag);
                    Element? querySelector(string selector);
                }
                extern Document document;
                Element element=document.createElement("div");
                element.textContent="ready";
                element.setAttribute("data-state","active");
                Element? existing=document.querySelector("main");
            "#,
        )
        .unwrap();

        let error = check(
            "extern class Document{string title;}extern Document document;document.missing();",
        )
        .unwrap_err();
        assert!(error.message.contains("has no member `missing`"));

        let error = check("extern class Document{}Document value=new Document();").unwrap_err();
        assert!(error.message.contains("cannot be constructed"));
    }

    #[test]
    fn accepts_closed_objects_and_merges_method_tables() {
        check("object Api{int add(int left,int right){return left+right;}}print(Api.add(1,2));")
            .unwrap();
        check(
            "object Api{int add(int left,int right){return left+right;}}object Api{int mul(int left,int right){return left*right;}}print(Api.add(1,2)+Api.mul(3,4));",
        )
        .unwrap();
        let constructed = check("object Api{int id(){return 1;}}Api value=new Api();").unwrap_err();
        assert!(
            constructed
                .message
                .contains("object `Api` cannot be constructed with `new`"),
            "{constructed}"
        );
        let duplicate = check(
            "object Api{int add(int left,int right){return left+right;}}object Api{int add(int left,int right){return left+right;}}",
        )
        .unwrap_err();
        assert!(
            duplicate.message.contains("duplicate member `add`"),
            "{duplicate}"
        );
        let clash = check("class Api{}object Api{int id(){return 1;}}").unwrap_err();
        assert!(
            clash.message.contains("duplicate type declaration `Api`"),
            "{clash}"
        );
    }

    #[test]
    fn infers_generic_functions_and_substitutes_class_members() {
        check(
            "T identity<T>(T value){return value;}int answer=identity(7);string text=identity(\"ok\");class Box<T>{T value;init(T value){this.value=value;}T get(){return this.value;}}Box<int> box=new Box(7);int value=box.get();",
        )
        .unwrap();
    }

    #[test]
    fn rejects_conflicting_generic_inferences() {
        let error =
            check("T choose<T>(T left,T right){return left;}int value=choose(1,\"wrong\");")
                .unwrap_err();
        assert!(error.message.contains("conflicting inferences"));
    }

    #[test]
    fn infers_zero_argument_generics_from_the_expected_return_type() {
        check("class Box<T>{}Box<T> make<T>(){return new Box<T>();}Box<int> box=make();").unwrap();
    }

    #[test]
    fn checks_async_and_exception_boundaries_without_narrowing_thrown_values() {
        check(
            "async int recover(){try{return await Task.reject(\"bad\");}catch(auto error){string text=error.message??\"caught\";return text.length;}finally{print(\"done\");}}",
        )
        .unwrap();
        check("try{throw null;}catch{}finally{print(1);}").unwrap();

        let error = check("try{throw 1;}catch(int error){print(error);}").unwrap_err();
        assert!(error.message.contains("`auto` or `JsValue`"), "{error}");
        let error = check("int value=await Task.resolve(1);").unwrap_err();
        assert!(
            error.message.contains("inside an async function"),
            "{error}"
        );
    }

    #[test]
    fn validates_flattened_generic_single_inheritance_without_unsound_overrides() {
        check(
            "class Base<T>{T value;init(T value){this.value=value;}T get(){return this.value;}}class Child extends Base<int>{int bonus;init(int value,int bonus){super(value);this.bonus=bonus;}int total(){return this.value+this.bonus;}}Child child=new Child(4,3);Base<int> base=child;int inherited=base.get();int total=child.total();",
        )
        .unwrap();

        let missing_super = check(
            "class Base{init(int value){print(value);}}class Child extends Base{init(){print(1);}}",
        )
        .unwrap_err();
        assert!(missing_super.message.contains("must begin with `super"));

        let override_error = check(
            "class Base{int value(){return 1;}}class Child extends Base{int value(){return 2;}}",
        )
        .unwrap_err();
        assert!(override_error
            .message
            .contains("cannot override inherited member"));

        let nested_super = check(
            "class Base{init(){}}class Child extends Base{init(){super();auto later=()=>{super();};}}",
        )
        .unwrap_err();
        assert!(nested_super.message.contains("derived class constructor"));

        check(
            "class Child extends Base<int>{}class Base<T>{T value;}Child child=new Child();child.value=7;Base<int> base=child;",
        )
        .unwrap();

        let cycle = check("class Left extends Right{}class Right extends Left{}").unwrap_err();
        assert!(cycle.message.contains("inheritance cycle"), "{cycle}");
    }

    #[test]
    fn validates_generator_boundaries_delegation_and_iteration() {
        check(
            "generator int range(int stop){for(int i=0;i<stop;i++){yield i;}}generator int values(){yield* [7,8];yield* range(2);}int sum=0;for(int value of values()){sum+=value;}",
        )
        .unwrap();

        let outside = check("yield 1;").unwrap_err();
        assert!(outside.message.contains("inside a generator"));
        let wrong = check("generator int values(){yield \"bad\";}").unwrap_err();
        assert!(wrong.message.contains("expected `int`"));
        let nested = check("generator int values(){auto bad=()=>{yield 1;};yield 2;}").unwrap_err();
        assert!(nested.message.contains("inside a generator"));
        let returned = check("generator int values(){return 1;}").unwrap_err();
        assert!(returned.message.contains("expected return type `void`"));
    }
}
