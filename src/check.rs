//! The checker: names, types and narrowing over the parsed AST. A checked
//! module is what the program conversion (`crate::program`) reads.
use crate::ast::ExprKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::primitive::ParameterPassing;
use std::fmt;

use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};
use indexmap::IndexMap;

use crate::ast::{
    Argument, ArrayBinding, ArrayElement, ArrowBody, AssignmentOp, BinaryOp, ClassDecl,
    ClassMember, ConstructorDecl, DynamicBinaryOp, DynamicUnaryOp, Expr, ExternClassMember,
    ExternDecl, ForInitializer, FunctionDecl, Ident, Item, MatchPattern, Program, RecordElement,
    SourceNodeId, Stmt, StructDecl, TemplatePart, TypeKind, TypeRef, UnaryOp, UpdateOp, VarDecl,
};
use crate::span::Span;
use crate::typed_array::TypedArrayKind;

pub(crate) mod binary_types;
pub(crate) mod absence;
pub(crate) mod capabilities;
mod field_initialization;
mod shapes;
mod enums;
mod variants;
pub use shapes::ShapeTag;
mod modules;
mod struct_cycles;
pub use field_initialization::FieldInitializationFacts;
pub(crate) mod type_admission;
mod type_identity;
mod type_pool;
pub use type_pool::CheckedTypeId;
pub(crate) mod type_payload;
pub(crate) mod type_relation;
pub(crate) mod type_substitution;
pub(crate) use modules::{with_analyzed_modules, with_analyzed_modules_with_contract};
#[cfg(test)]
pub(crate) use modules::AdmittedModuleCheckError;
pub use modules::{
    analyze_modules, CheckedModules, InterfaceTarget, ModuleCheckError, ModuleExport, ModuleImport,
    ModuleInterface,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

/// The operation an arithmetic operator resolves to (M4.3): an int32
/// operation where its value is an `int` (or an enum's), otherwise the
/// operator itself, on numbers, strings or booleans. The checker decides it
/// once, for a binary expression by its result and for a compound
/// assignment or an update by its target; lowering reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedOperator {
    Int(crate::primitive::IntBinary),
    Plain(BinaryOp),
}

impl ResolvedOperator {
    fn of(op: BinaryOp, value: &Type<'_>) -> Self {
        use crate::primitive::IntBinary;
        let int = match op {
            BinaryOp::Add => Some(IntBinary::Add),
            BinaryOp::Sub => Some(IntBinary::Subtract),
            BinaryOp::Mul => Some(IntBinary::Multiply),
            BinaryOp::Div => Some(IntBinary::Divide),
            BinaryOp::Mod => Some(IntBinary::Remainder),
            BinaryOp::UnsignedShiftRight => Some(IntBinary::UnsignedShiftRight),
            _ => None,
        };
        match int {
            Some(int) if matches!(value, Type::Int | Type::Enum(_)) => Self::Int(int),
            _ => Self::Plain(op),
        }
    }
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
    /// `value instanceof constructor` (R12); no `JS.*` spelling.
    JsInstanceOf,
    /// `a - b`, `a * b`, `a / b` and `-a` with a `JsValue` operand (R12): the
    /// operators with JavaScript's meaning; no `JS.*` spelling.
    JsSubtract,
    JsMultiply,
    JsDivide,
    JsNegate,
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

/// A binder is identified by its declaring module and source occurrence.
/// Display spelling never participates in substitution or free-variable equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeParameterId {
    pub module: u32,
    pub declaration: SourceNodeId,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeParameter<'src> {
    pub identity: TypeParameterId,
    pub name: &'src str,
}
impl PartialEq for TypeParameter<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl Eq for TypeParameter<'_> {}
impl std::hash::Hash for TypeParameter<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}
impl fmt::Display for TypeParameter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}
#[cfg(test)]
impl<'src> TypeParameter<'src> {
    pub(crate) fn fixture(name: &'src str) -> Self {
        // Only hand-built type tests use this stable synthetic namespace.
        let id = name.bytes().fold(2166136261u32, |n, b| {
            (n ^ u32::from(b)).wrapping_mul(16777619)
        }) & 0x7fffffff;
        Self {
            identity: TypeParameterId {
                module: u32::MAX,
                declaration: SourceNodeId::detached(id),
            },
            name,
        }
    }
}

/// Absence has one semantic meaning; an ABI may require a particular spelling.
/// Keeping this on the nullable node preserves nested collection/callable pins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AbsencePin {
    #[default]
    Auto,
    Null,
    Undefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullableType<'src> {
    inner: Box<Type<'src>>,
    pub boundary: AbsencePin,
}

impl<'src> NullableType<'src> {
    pub fn new(inner: Box<Type<'src>>, boundary: AbsencePin) -> Self {
        Self { inner, boundary }
    }
    pub fn into_inner(self) -> Type<'src> { *self.inner }
}
impl<'src> std::ops::Deref for NullableType<'src> {
    type Target = Type<'src>;
    fn deref(&self) -> &Self::Target { &self.inner }
}
impl<'src> AsRef<Type<'src>> for NullableType<'src> {
    fn as_ref(&self) -> &Type<'src> { &self.inner }
}
impl fmt::Display for NullableType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.inner.fmt(f) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type<'src> {
    Int,
    Float,
    /// An enum by its checked identity; the spelling is display data.
    Enum(EnumType<'src>),
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
    Nullable(NullableType<'src>),
    Union(Vec<Type<'src>>),
    /// A declared shape view containing every component's fields.
    Intersection(Vec<Type<'src>>),
    Struct(StructType<'src>),
    /// A class or extern class by its checked identity.
    Class(NominalType<'src>),
    StructInstance {
        declaration: StructType<'src>,
        args: Vec<Type<'src>>,
    },
    ClassInstance {
        declaration: NominalType<'src>,
        args: Vec<Type<'src>>,
    },
    TypeParameter(TypeParameter<'src>),
    /// `JsValue`: the dynamic type, a JavaScript-only capability (R12,
    /// M4.2). Every host value that no declared type describes has it.
    Dynamic,
    /// `unknown`: dynamic too, but only the tests (`==`, `===`, `typeof`,
    /// `instanceof`, `is`) and the ways out (`as`, `as?`, the conversions)
    /// apply to it until it is narrowed (R12). It lowers as `JsValue`.
    Unknown,
    Function(FunctionType<'src>),
    GenericFunction(GenericFunctionType<'src>),
}

impl<'src> Type<'src> {
    pub fn nullable(inner: Box<Self>) -> Self {
        Self::pinned_nullable(inner, AbsencePin::Auto)
    }

    pub fn pinned_nullable(inner: Box<Self>, boundary: AbsencePin) -> Self {
        match *inner {
            Self::Nullable(mut nullable) => {
                if boundary != AbsencePin::Auto { nullable.boundary = boundary; }
                Self::Nullable(nullable)
            }
            Self::Null => Self::Null,
            _ => Self::Nullable(NullableType::new(inner, boundary)),
        }
    }

    /// The runtime calling convention is independent of generic binders.
    pub(crate) fn callable_signature(&self) -> Option<&FunctionType<'src>> {
        match self {
            Type::Function(signature) => Some(signature),
            Type::GenericFunction(function) => Some(&function.signature),
            _ => None,
        }
    }
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
    /// Whether `unknown` occurs in this type.
    pub fn mentions_unknown(&self) -> bool {
        match self {
            Self::Unknown => true,
            Self::Array(value)
            | Self::Record(value)
            | Self::Set(value)
            | Self::Task(value)
            | Self::Generator(value) => value.mentions_unknown(),
            Self::Nullable(value) => value.mentions_unknown(),
            Self::Map(key, value) => key.mentions_unknown() || value.mentions_unknown(),
            Self::Union(values) | Self::Intersection(values)
            | Self::StructInstance { args: values, .. }
            | Self::ClassInstance { args: values, .. } => values.iter().any(Self::mentions_unknown),
            Self::Function(signature) => signature_mentions_unknown(signature),
            Self::GenericFunction(function) => signature_mentions_unknown(&function.signature),
            _ => false,
        }
    }

    /// This type with `unknown` as `JsValue`: how it lowers (R12), since the
    /// two differ only in what the checker allows.
    pub fn without_unknown(&self) -> Self {
        let each = |values: &[Self]| values.iter().map(Self::without_unknown).collect::<Vec<_>>();
        match self {
            Self::Unknown => Self::Dynamic,
            Self::Array(value) => Self::Array(Box::new(value.without_unknown())),
            Self::Record(value) => Self::Record(Box::new(value.without_unknown())),
            Self::Set(value) => Self::Set(Box::new(value.without_unknown())),
            Self::Task(value) => Self::Task(Box::new(value.without_unknown())),
            Self::Generator(value) => Self::Generator(Box::new(value.without_unknown())),
            Self::Nullable(value) => Self::pinned_nullable(Box::new(value.without_unknown()), value.boundary),
            Self::Map(key, value) => Self::Map(
                Box::new(key.without_unknown()),
                Box::new(value.without_unknown()),
            ),
            Self::Union(values) => normalize_union(each(values)),
            Self::Intersection(values) => Self::Intersection(each(values)),
            Self::StructInstance { declaration, args } => Self::StructInstance {
                declaration: *declaration,
                args: each(args),
            },
            Self::ClassInstance { declaration, args } => Self::ClassInstance {
                declaration: *declaration,
                args: each(args),
            },
            Self::Function(signature) => Self::Function(signature_without_unknown(signature)),
            Self::GenericFunction(function) => Self::GenericFunction(GenericFunctionType {
                type_params: function.type_params.clone(),
                signature: signature_without_unknown(&function.signature),
            }),
            other => other.clone(),
        }
    }

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
                    | Type::Union(_) | Type::Intersection(_)
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
                | Self::Generator(value) => {
                    current = value;
                    continue;
                }
                Self::Nullable(value) => {
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
                Self::Union(values) | Self::Intersection(values)
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
            Self::Enum(declaration) => f.write_str(declaration.name),
            Self::String => f.write_str("string"),
            Self::Bool => f.write_str("bool"),
            Self::Null => f.write_str("null"),
            Self::Void => f.write_str("void"),
            Self::Array(element) => match element.as_ref() {
                Self::Union(_) | Self::Intersection(_) => write!(f, "({element})[]"),
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
            Self::Nullable(inner) if inner.boundary != AbsencePin::Auto => write!(f, "({inner} | {})", if inner.boundary == AbsencePin::Null { "null" } else { "undefined" }),
            Self::Nullable(inner) => match inner.as_ref() {
                Self::Union(_) | Self::Intersection(_) => write!(f, "({inner})?"),
                _ => write!(f, "{inner}?"),
            },
            Self::Union(members) | Self::Intersection(members) => {
                for (index, member) in members.iter().enumerate() {
                    if index != 0 {
                        f.write_str(if matches!(self, Self::Intersection(_)) { " & " } else { " | " })?;
                    }
                    write!(f, "{member}")?;
                }
                Ok(())
            }
            Self::Struct(declaration) | Self::Class(declaration) => f.write_str(declaration.name),
            Self::StructInstance {
                declaration: NominalType { name, .. },
                args,
            }
            | Self::ClassInstance {
                declaration: NominalType { name, .. },
                args,
            } => {
                write!(f, "{name}<")?;
                for (index, argument) in args.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str(">")
            }
            Self::Dynamic => f.write_str("JsValue"),
            Self::Unknown => f.write_str("unknown"),
            Self::TypeParameter(parameter) => f.write_str(parameter.name),
            Self::Function(signature) => {
                f.write_str("function(")?;
                for (index, parameter) in signature.params.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    if parameter.receiver {
                        f.write_str("this ")?;
                    }
                    if parameter.passing == ParameterPassing::MutableReference {
                        f.write_str("mutable-reference ")?;
                    }
                    match (&parameter.ty, parameter.rest) {
                        (Type::Array(element), true) => write!(f, "{element}...")?,
                        (ty, _) => write!(f, "{ty}")?,
                    }
                }
                write!(f, ") -> {}", signature.return_type)
            }
            Self::GenericFunction(function) => {
                f.write_str("function<")?;
                for (index, parameter) in function.type_params.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(parameter.name)?;
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

/// Callable type contract. Default expressions remain on source declarations;
/// only optionality participates in type identity and assignability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionParameter<'src> {
    /// The first physical parameter receives the invocation's receiver.
    pub receiver: bool,
    pub ty: Type<'src>,
    pub passing: ParameterPassing,
    pub optional: bool,
    /// A declared rest parameter (R7), the last: `ty` is the array `T[]` the
    /// body sees, and a call supplies it from its trailing arguments.
    pub rest: bool,
}
impl<'src> FunctionParameter<'src> {
    pub fn value(ty: Type<'src>) -> Self {
        Self {
            receiver: false,
            ty,
            passing: ParameterPassing::Value,
            optional: false,
            rest: false,
        }
    }
    pub fn optional(ty: Type<'src>) -> Self {
        Self {
            receiver: false,
            ty,
            passing: ParameterPassing::Value,
            optional: true,
            rest: false,
        }
    }
}

impl FunctionSignature<'_> {
    /// Structural parameter validity. Default expression types and binding
    /// identities remain the source checker's separate semantic obligation.
    pub fn validate_parameters(&self) -> Result<(), &'static str> {
        let mut optional = false;
        for (index, parameter) in self.params.iter().enumerate() {
            if parameter.receiver
                && (index != 0
                    || parameter.rest
                    || parameter.optional
                    || parameter.passing != ParameterPassing::Value)
            {
                return Err(
                    "a receiver is the first value parameter and has no default or rest marker",
                );
            }
            if parameter.rest {
                if index + 1 != self.params.len()
                    || parameter.optional
                    || parameter.passing != ParameterPassing::Value
                    || !matches!(parameter.ty, Type::Array(_))
                {
                    return Err("a rest parameter is the final array parameter, passed by value without a default");
                }
                continue;
            }
            if parameter.passing == ParameterPassing::MutableReference && parameter.optional {
                return Err("mutable-reference parameters cannot have defaults");
            }
            if parameter.optional {
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
            .position(|parameter| parameter.optional || parameter.rest)
            .unwrap_or(self.params.len())
    }

    /// The parameters a call supplies one by one: all but a rest parameter.
    pub fn fixed_params(&self) -> usize {
        self.params.len() - usize::from(self.has_rest())
    }

    pub fn has_receiver(&self) -> bool {
        self.params
            .first()
            .is_some_and(|parameter| parameter.receiver)
    }

    pub fn has_rest(&self) -> bool {
        self.params.last().is_some_and(|parameter| parameter.rest)
    }

    pub fn accepts_arity(&self, arity: usize) -> bool {
        arity >= self.required_params() && (self.has_rest() || arity <= self.params.len())
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
    /// A syntactic `JS.undefined()` candidate awaiting semantic builtin
    /// resolution. Its spelling alone is never accepted as value proof.
    PendingUndefined {
        expression: SourceNodeId,
        span: Span,
    },
    Array(Vec<DefaultValue<'src>>),
    Arrow(SourceNodeId),
    /// A struct literal default, by the identity its declaring scope
    /// resolved; the spelling is display data.
    Struct {
        declaration: NominalType<'src>,
        values: Vec<DefaultValue<'src>>,
    },
    /// A `new C(...)` default, by the class identity its declaring scope
    /// resolved. A caller in another module constructs that class, whatever
    /// its own scope names `C`.
    NewClass {
        declaration: NominalType<'src>,
        args: Vec<DefaultValue<'src>>,
    },
}

#[derive(Debug, Clone)]
pub struct GenericFunctionType<'src> {
    pub type_params: Vec<TypeParameter<'src>>,
    pub signature: FunctionType<'src>,
}

impl PartialEq for GenericFunctionType<'_> {
    fn eq(&self, other: &Self) -> bool {
        type_identity::generic_equal(self, other)
    }
}
impl Eq for GenericFunctionType<'_> {}

/// The registry a nominal declaration belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NominalKind {
    Struct,
    /// A class, an extern class or an `object`.
    Class,
    Enum,
}

/// A program-local declaration handle for every nominal kind. Each kind's
/// registry owns its definitions; the low two bits tag the registry. No
/// source spelling or diagnostic span is needed to dereference this handle,
/// and two modules' private declarations of one name are two identities
/// (Closure keys its type table by scope and compares resolved nominal
/// types by reference; esbuild and Rolldown key symbols by source index).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NominalId(std::num::NonZeroU32);

impl NominalId {
    pub(crate) fn new(index: usize, kind: NominalKind) -> Self {
        let tag = match kind {
            NominalKind::Struct => 0,
            NominalKind::Class => 1,
            NominalKind::Enum => 2,
        };
        let encoded = u32::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(4))
            .and_then(|index| index.checked_add(1 + tag))
            .and_then(std::num::NonZeroU32::new)
            .expect("nominal declaration capacity exceeded");
        Self(encoded)
    }

    /// The index in this identity's own registry.
    pub(crate) fn index(self) -> usize {
        ((self.0.get() - 1) / 4) as usize
    }
    pub fn kind(self) -> NominalKind {
        match (self.0.get() - 1) & 3 {
            0 => NominalKind::Struct,
            1 => NominalKind::Class,
            _ => NominalKind::Enum,
        }
    }
    pub fn is_struct(self) -> bool {
        self.kind() == NominalKind::Struct
    }
    pub fn is_class(self) -> bool {
        self.kind() == NominalKind::Class
    }
    pub fn is_enum(self) -> bool {
        self.kind() == NominalKind::Enum
    }
}

/// A canonical declaration reference within one checked declaration owner.
/// The spelling is diagnostic and display metadata; aliases never change
/// identity, and equal spellings never merge two identities.
#[derive(Debug, Clone, Copy)]
pub struct NominalType<'src> {
    pub identity: NominalId,
    pub name: &'src str,
}

/// A struct's nominal reference.
pub type StructType<'src> = NominalType<'src>;

/// A closed domain and its canonical ABI representation. The declaration
/// owns the values; primitive storage needs only this compact kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumType<'src> {
    pub declaration: NominalType<'src>,
    pub abi: crate::ast::EnumAbi,
}

impl<'src> std::ops::Deref for EnumType<'src> {
    type Target = NominalType<'src>;
    fn deref(&self) -> &Self::Target { &self.declaration }
}

impl EnumType<'_> {
    pub(crate) fn is_string(self) -> bool { self.abi == crate::ast::EnumAbi::String }
    pub(crate) fn is_flags(self) -> bool { self.abi == crate::ast::EnumAbi::Flags }
    pub(crate) fn primitive(self) -> Type<'static> {
        if self.is_string() { Type::String } else { Type::Int }
    }
}

/// Tests that build types without a checker: one identity per spelling, so
/// equal spellings are one declaration, as in a single scope.
#[cfg(test)]
pub(crate) fn test_nominal(kind: NominalKind, name: &str) -> NominalType<'_> {
    let index = name.bytes().fold(0x811c_9dc5u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    }) & 0x00ff_ffff;
    NominalType {
        identity: NominalId::new(index as usize, kind),
        name,
    }
}
/// Tests: the class a checked single source declares under `name`.
#[cfg(test)]
pub(crate) fn declared_class<'src>(
    model: &CheckedModule<'_, 'src>,
    name: &str,
) -> NominalType<'src> {
    model.class_info(name).unwrap().declaration
}
#[cfg(test)]
pub(crate) fn test_class(name: &str) -> NominalType<'_> {
    test_nominal(NominalKind::Class, name)
}
#[cfg(test)]
pub(crate) fn test_enum(name: &str) -> EnumType<'_> {
    EnumType { declaration: test_nominal(NominalKind::Enum, name), abi: crate::ast::EnumAbi::Ordinal }
}

impl PartialEq for NominalType<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl Eq for NominalType<'_> {}
impl std::hash::Hash for NominalType<'_> {
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
    pub accessor: bool,
    pub has_initializer: bool,
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
    pub type_params: Vec<TypeParameter<'src>>,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassInfo<'src> {
    pub sealed: bool,
    pub discriminant: Option<(usize, ShapeTag<'src>)>,
    /// Plain reference data with no constructor/prototype identity.
    pub shape: bool,
    /// The class's identity and display spelling.
    pub declaration: NominalType<'src>,
    /// The module whose scope declares it (none for a single source).
    pub module: Option<crate::module::ModuleId>,
    pub name: &'src str,
    pub type_params: Vec<TypeParameter<'src>>,
    pub base: Option<Type<'src>>,
    pub fields: IndexMap<&'src str, FieldInfo<'src>>,
    pub methods: IndexMap<&'src str, MethodInfo<'src>>,
    pub constructor: Option<FunctionType<'src>>,
    /// The binding the class's name declares as a value: its constructor
    /// (an internal class; only a class kept as a JavaScript class has one at
    /// run time).
    pub value: Option<SymbolId>,
    pub external: bool,
    /// A delivery entry exports the constructor, or a first-class use can
    /// expose it. Internal module visibility alone does not publish an ABI.
    pub published: bool,
    /// The class's identity is observable, so it stays a JavaScript class:
    /// it is published, has a host (extern) ancestor, or shares an internal
    /// inheritance chain with such a class. Every other class may dissolve.
    pub observed: bool,
    pub initialization: FieldInitializationFacts,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo<'src> {
    pub dispatch: crate::ast::MethodDispatch,
    pub member: NominalMemberId,
    /// The class that declares the method (a base, for an inherited one).
    pub owner: NominalId,
    pub type_params: Vec<TypeParameter<'src>>,
    pub signature: FunctionType<'src>,
    pub declared_pure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumInfo<'src> {
    pub declaration: EnumType<'src>,
    pub module: Option<crate::module::ModuleId>,
    pub name: &'src str,
    pub variants: IndexMap<&'src str, i64>,
    /// Declaration order, distinct from the externally observable ABI value.
    pub values: Vec<ShapeTag<'src>>,
    pub flag_mask: u32,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol<'src> {
    pub id: SymbolId,
    pub name: &'src str,
    pub ty: Type<'src>,
    pub span: Span,
    /// The declaring identifier, in its module's source (M4.4).
    pub node: SourceNodeId,
    /// What the declaration says of its calls (M4.3).
    pub attributes: Attributes,
    /// Classification belongs to the canonical declaration, including extern
    /// aliases shared by several checked modules.
    origin: DeclarationOrigin,
    /// Distinct identifier nodes currently registered to this identity,
    /// including its declaration. Maintained by ModuleFacts::record_identifier
    /// across source owners; this is source-binding knowledge, not a use count
    /// for a rewritten program.
    identifier_occurrences: usize,
}

/// A declaration's attributes (M4.3): `pure` (R15: its calls observe and
/// change nothing) and `debug` (its calls are logging, which `strip_debug`
/// drops). An extern's attributes are part of its contract, the same in
/// every module that declares it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes {
    pub pure: bool,
    pub debug: bool,
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
pub struct CheckError {
    pub span: Span,
    pub message: String,
}

impl CheckError {
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

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for CheckError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdmittedCheckError {
    Semantic(CheckError),
    Resources(AllocationError),
}

impl From<AllocationError> for AdmittedCheckError {
    fn from(error: AllocationError) -> Self {
        Self::Resources(error)
    }
}

impl From<CheckError> for AdmittedCheckError {
    fn from(error: CheckError) -> Self {
        Self::Semantic(error)
    }
}

impl AdmittedCheckError {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self::Semantic(CheckError::new(span, message))
    }
}

/// Whether one occurrence of a binding runs after the binding is initialized
/// (plan M4.3). `Definite` is the checker's proof; `NeedsCallGraph` is left
/// to the initialization owner (`program/initialization.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadInitialization {
    Definite,
    NeedsCallGraph,
}

/// Resolution belongs to the checked source occurrence, independently of its
/// diagnostic location or the target operation that eventually represents it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExpressionResolution {
    #[default]
    None,
    Binding(SymbolId),
    Enum { declaration: NominalId, operation: crate::primitive::EnumOperation },
    Builtin(BuiltinCall),
    /// Syntax on a `JsValue` that is the dynamic operation its `JS.*`
    /// spelling names (R12). Its operands are the node's own parts: the
    /// receiver and key of `v.k`, the callee and arguments of `f(a)`.
    Dynamic(BuiltinCall),
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
    /// The checked argument needs optional-value transport into a defaulted
    /// parameter. Native capability admission consumes the same call contract,
    /// including constructor and super calls.
    absent_default_argument: bool,
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
            .field("absent_default_argument", &self.absent_default_argument)
            .finish()
    }
}

/// Canonical declarations are owned once, including all local symbols.
#[derive(Debug, Clone, Default)]
struct DeclarationTables<'src> {
    source_contract: crate::config::LanguageConfig,
    types: type_pool::TypePool<'src>,
    /// Written after initialization: an assignment or a mutable reference.
    assigned_symbols: AHashSet<SymbolId>,
    /// Bindings some occurrence of which may run before the binding is
    /// initialized (`ReadInitialization::NeedsCallGraph`).
    observable_before_initialization: AHashSet<SymbolId>,
    /// Value bindings a module's top level declares (not its named
    /// functions, which exist from instantiation).
    module_bindings: AHashSet<SymbolId>,
    symbols: Vec<Symbol<'src>>,
    /// Each nominal registry, indexed by `NominalId::index` of its kind.
    structs: Vec<StructInfo<'src>>,
    classes: Vec<ClassInfo<'src>>,
    /// Classes an identity test names (`is`, `as?`; R13), which keep their
    /// identity once every body is checked (`mark_tested_classes`).
    tested_classes: AHashSet<NominalId>,
    /// The reflected set (R6, M10.14's checker half): nominals a crossing
    /// shows the host, seeded as the bodies are checked (through a shared
    /// borrow, hence the cell) and closed over fields, bases and type
    /// arguments once they are (`close_reflected`). Property names, key
    /// order and identity are observable only for these.
    reflected: std::cell::RefCell<AHashSet<NominalId>>,
    nominal_members: Vec<MemberDefinition>,
    enums: Vec<EnumInfo<'src>>,
    symbol_modules: Vec<Option<crate::module::ModuleId>>,
    foreign_symbols: AHashMap<&'src str, (SymbolId, bool)>,
}

/// Source-node and span indexes belong to exactly one original source.
#[derive(Debug, Clone)]
struct ModuleFacts<'ast, 'src> {
    source: crate::ast::SourceIdentity,
    expression_types: Vec<Option<CheckedTypeId>>,
    source_info: Vec<SourceInfo<'ast, 'src>>,
    call_instantiations: AHashMap<SourceNodeId, CheckedCallInstantiation<'src>>,
    /// This source's type scope: every nominal name it declares or imports,
    /// resolved once to its identity. Nothing else resolves a type by name.
    type_bindings: AHashMap<&'src str, NominalId>,
    // Facts about a source node, keyed by its id (plan M4.4).
    /// An optional member's or index's present type, by the expression.
    optional_present_types: AHashMap<SourceNodeId, CheckedTypeId>,
    /// A type test's (`is`, `as?`) target type, by the test's expression.
    type_check_types: AHashMap<SourceNodeId, CheckedTypeId>,
    /// A declaring identifier's binding.
    binding_types: AHashMap<SourceNodeId, BindingType>,
    /// An identifier's symbol: a declaration's and every reference's.
    identifier_symbols: AHashMap<SourceNodeId, SymbolId>,
    /// An enum variant's value, by the variant's identifier (in `E.V` and
    /// in a `match` pattern).
    enum_variant_values: AHashMap<SourceNodeId, i64>,
    /// A dynamic import's module, by the import expression.
    dynamic_import_modules: AHashMap<SourceNodeId, u32>,
    /// An arithmetic operator's resolution, by its binary, compound
    /// assignment or update expression (M4.3).
    resolved_operators: AHashMap<SourceNodeId, ResolvedOperator>,
    /// Direct module checking: a dynamically imported module's runtime
    /// exports, resolved through its interface rather than a merged scope.
    dynamic_export_symbols: AHashMap<(u32, &'src str), SymbolId>,
    used_dynamic_exports: AHashSet<(u32, &'src str)>,
    /// Occurrences (reads and writes) the checker cannot prove run after
    /// their binding's initialization.
    deferred_initialization: AHashSet<SourceNodeId>,
}

// Value declarations and import aliases share the canonical symbol's payload.
// Type-only bindings still need a type without introducing a value identity.
#[derive(Debug, Clone)]
enum BindingType {
    Symbol(SymbolId),
    Inline(CheckedTypeId),
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
pub struct CheckedModule<'ast, 'src> {
    declarations: DeclarationTables<'src>,
    facts: ModuleFacts<'ast, 'src>,
    /// Move the canonical one-module interface instead of reconstructing it
    /// from the dual type/value binding of a class declaration.
    exports: Vec<modules::ModuleExport<'src>>,
}

/// A read-only source qualification over the compilation's shared declarations.
#[derive(Debug, Clone, Copy)]
pub struct CheckedView<'view, 'ast, 'src> {
    declarations: &'view DeclarationTables<'src>,
    facts: &'view ModuleFacts<'ast, 'src>,
}

impl<'ast, 'src> ModuleFacts<'ast, 'src> {
    #[cfg(test)]
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
        expression_types: Vec<Option<CheckedTypeId>>,
        source_info: Vec<SourceInfo<'ast, 'src>>,
    ) -> Self {
        Self {
            source: source.clone(),
            expression_types,
            source_info,
            call_instantiations: AHashMap::default(),
            type_bindings: AHashMap::default(),
            optional_present_types: AHashMap::default(),
            type_check_types: AHashMap::default(),
            binding_types: AHashMap::default(),
            identifier_symbols: AHashMap::default(),
            enum_variant_values: AHashMap::default(),
            dynamic_import_modules: AHashMap::default(),
            resolved_operators: AHashMap::default(),
            dynamic_export_symbols: AHashMap::default(),
            used_dynamic_exports: AHashSet::default(),
            deferred_initialization: AHashSet::default(),
        }
    }

    fn record_identifier(
        &mut self,
        declarations: &mut DeclarationTables<'src>,
        ident: SourceNodeId,
        symbol: SymbolId,
    ) {
        match self.identifier_symbols.insert(ident, symbol) {
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
        let id =
            SymbolId(u32::try_from(self.symbols.len()).map_err(|_| AllocationError::Capacity)?);
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

    /// Marks the classes whose identity is observable (`ClassInfo::observed`)
    /// once every hierarchy is resolved: published classes and classes with a
    /// host ancestor, their internal ancestors, and every class that extends
    /// one of them. A published class that inherits must state its `init`.
    pub(super) fn mark_observed_classes(
        &mut self,
    ) -> Result<(), (Option<crate::module::ModuleId>, CheckError)> {
        let classes = &mut self.classes;
        let base_of = |classes: &[ClassInfo<'src>], index: usize| {
            classes[index]
                .base
                .as_ref()
                .and_then(class_type_identity)
                .map(NominalId::index)
        };
        for index in 0..classes.len() {
            let info = &classes[index];
            if info.external {
                continue;
            }
            if info.published && info.base.is_some() && info.constructor.is_none() {
                return Err((
                    info.module,
                    CheckError::new(
                        info.span,
                        format!(
                            "an inherited constructor export requires an explicit `init` with `super(...)` in class `{}`",
                            info.name
                        ),
                    ),
                ));
            }
            let mut host = false;
            let mut current = base_of(classes, index);
            while let Some(base) = current {
                if classes[base].external {
                    host = true;
                    break;
                }
                current = base_of(classes, base);
            }
            if info.published || host {
                // The seed and its internal ancestors.
                let mut current = Some(index);
                while let Some(class) = current {
                    if classes[class].external {
                        break;
                    }
                    classes[class].observed = true;
                    current = base_of(classes, class);
                }
            }
        }
        observe_descendants(classes);
        Ok(())
    }

    /// Resolve constructor observations once by checked value identity. An
    /// import/export name is only visibility, and `new C` names its type rather
    /// than reading its constructor binding. Actual first-class reads retain a
    /// conservative public prototype contract (including host escapes).
    fn mark_constructor_observations(
        &mut self,
        facts: &[ModuleFacts<'_, 'src>],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if self.classes.is_empty() {
            return Ok(());
        }
        let mut scope = budget.scope();
        let mut constructors = scope.filled(AllocationClass::Scratch, self.symbols.len(), None)?;
        for (index, class) in self.classes.iter().enumerate() {
            scope.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
            if let Some(symbol) = class.value {
                constructors[symbol.0 as usize] = Some(index);
            }
        }
        for module in facts {
            for info in &module.source_info {
                scope.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                if let ExpressionResolution::Binding(symbol) = info.resolution {
                    if let Some(index) = constructors[symbol.0 as usize] {
                        self.classes[index].published = true;
                    }
                }
            }
            for key in &module.used_dynamic_exports {
                scope.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                if let Some(symbol) = module.dynamic_export_symbols.get(key) {
                    if let Some(index) = constructors[symbol.0 as usize] {
                        self.classes[index].published = true;
                    }
                }
            }
        }
        Ok(())
    }

    /// Seeds the reflected set with every nominal `ty` names (R6).
    fn reflect(&self, ty: &Type<'_>) {
        let mut found = Vec::new();
        nominals_in(ty, &mut found);
        if !found.is_empty() {
            self.reflected.borrow_mut().extend(found);
        }
    }

    /// Closes the reflected set (R6) once every body is checked. Published
    /// classes and classes with a host ancestor are seeds; a reflected
    /// class's base and field types, a struct's field types, and the type
    /// arguments a seed names are reflected too.
    pub(super) fn close_reflected(&mut self) {
        let mut set = std::mem::take(self.reflected.get_mut());
        let mut descendants = vec![Vec::new(); self.classes.len()];
        for (index, class) in self.classes.iter().enumerate() {
            if let Some(base) = class_base_index(&self.classes, index) {
                descendants[base].push(class.declaration.identity);
            }
            let hosted = {
                let mut current = class_base_index(&self.classes, index);
                let mut hosted = false;
                while let Some(base) = current {
                    if self.classes[base].external {
                        hosted = true;
                        break;
                    }
                    current = class_base_index(&self.classes, base);
                }
                hosted
            };
            if !class.external && (class.published || hosted) {
                set.insert(class.declaration.identity);
            }
        }
        let mut pending = set.iter().copied().collect::<Vec<_>>();
        pending.sort_unstable_by_key(|id| (id.kind() as u8, id.index()));
        let mut found = Vec::new();
        while let Some(nominal) = pending.pop() {
            found.clear();
            match nominal.kind() {
                NominalKind::Class => {
                    let Some(class) = self.classes.get(nominal.index()) else {
                        continue;
                    };
                    if let Some(base) = &class.base {
                        nominals_in(base, &mut found);
                    }
                    // A value published as a base can hold any subclass.
                    // Enumeration and serialization see its additional fields.
                    found.extend_from_slice(&descendants[nominal.index()]);
                    for field in class.fields.values() {
                        nominals_in(&field.ty, &mut found);
                    }
                    for method in class.methods.values() {
                        nominals_in(&Type::Function(method.signature.clone()), &mut found);
                    }
                    if let Some(constructor) = &class.constructor {
                        nominals_in(&Type::Function(constructor.clone()), &mut found);
                    }
                }
                NominalKind::Struct => {
                    let Some(info) = self.structs.get(nominal.index()) else {
                        continue;
                    };
                    for field in info.fields.values() {
                        nominals_in(&field.ty, &mut found);
                    }
                }
                NominalKind::Enum => {}
            }
            for &reached in &found {
                if set.insert(reached) {
                    pending.push(reached);
                }
            }
        }
        *self.reflected.get_mut() = set;
    }

    /// Marks the classes an identity test names (R13) once every body is
    /// checked: `v is C` is `instanceof`, so `C`, its internal ancestors and
    /// every class extending it stay JavaScript classes.
    pub(super) fn mark_tested_classes(&mut self) {
        let mut tested = self
            .tested_classes
            .iter()
            .map(|class| class.index())
            .collect::<Vec<_>>();
        if tested.is_empty() {
            return;
        }
        tested.sort_unstable();
        let classes = &mut self.classes;
        for index in tested {
            let mut current = Some(index);
            while let Some(class) = current {
                if classes[class].external {
                    break;
                }
                classes[class].observed = true;
                current = class_base_index(classes, class);
            }
        }
        observe_descendants(classes);
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

impl<'ast, 'src> CheckedModule<'ast, 'src> {
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

    pub fn view(&self) -> CheckedView<'_, 'ast, 'src> {
        CheckedView {
            declarations: &self.declarations,
            facts: &self.facts,
        }
    }

    #[cfg(test)]
    fn record_identifier(&mut self, ident: SourceNodeId, symbol: SymbolId) {
        self.facts
            .record_identifier(&mut self.declarations, ident, symbol);
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

    pub fn binding_type(&self, node: SourceNodeId) -> Option<&Type<'src>> {
        self.view().binding_type(node)
    }

    pub(crate) fn builtin_call(&self, id: SourceNodeId) -> Option<BuiltinCall> {
        self.view().builtin_call(id)
    }

    pub(crate) fn dynamic_operation(&self, id: SourceNodeId) -> Option<BuiltinCall> {
        self.view().dynamic_operation(id)
    }

    pub(crate) fn resolved_operator(&self, node: SourceNodeId) -> Option<ResolvedOperator> {
        self.view().resolved_operator(node)
    }

    pub(crate) fn resolved_intrinsic(
        &self,
        id: SourceNodeId,
    ) -> Option<crate::primitive::ResolvedIntrinsic> {
        self.view().resolved_intrinsic(id)
    }

    pub fn identifier_symbol(&self, node: SourceNodeId) -> Option<SymbolId> {
        self.view().identifier_symbol(node)
    }

    pub(crate) fn symbol_is_reassigned(&self, symbol: SymbolId) -> bool {
        self.view().symbol_is_reassigned(symbol)
    }

    pub(crate) fn symbol_is_observable_before_initialization(&self, symbol: SymbolId) -> bool {
        self.view()
            .symbol_is_observable_before_initialization(symbol)
    }

    pub fn read_initialization(&self, node: SourceNodeId) -> ReadInitialization {
        self.view().read_initialization(node)
    }

    pub(crate) fn type_check_type(&self, node: SourceNodeId) -> Option<&Type<'src>> {
        self.view().type_check_type(node)
    }

    pub(crate) fn optional_present_type(&self, node: SourceNodeId) -> Option<&Type<'src>> {
        self.view().optional_present_type(node)
    }

    pub fn symbols(&self) -> &[Symbol<'src>] {
        self.view().symbols()
    }

    pub fn struct_type(&self, name: &str) -> Option<StructType<'src>> {
        self.view().struct_type(name)
    }

    /// Whether a nominal is in the reflected set (R6).
    pub fn is_reflected(&self, nominal: NominalId) -> bool {
        self.view().is_reflected(nominal)
    }

    pub(crate) fn exports(&self) -> &[modules::ModuleExport<'src>] {
        &self.exports
    }

    pub fn export_target(&self, local: SourceNodeId) -> Option<InterfaceTarget> {
        self.view().export_target(local)
    }

    pub fn struct_info(&self, name: &str) -> Option<&StructInfo<'src>> {
        self.view().struct_info(name)
    }

    /// The identity this source's scope gives a written type name.
    pub fn type_binding(&self, name: &str) -> Option<NominalId> {
        self.view().type_binding(name)
    }

    /// Test convenience: the class this source's scope names `name`.
    #[cfg(test)]
    pub fn class_info(&self, name: &str) -> Option<&ClassInfo<'src>> {
        self.nominal_class(self.type_binding(name)?)
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

    pub fn nominal_enum(&self, id: NominalId) -> Option<&EnumInfo<'src>> {
        self.view().nominal_enum(id)
    }

    pub fn nominal_member(&self, id: NominalMemberId) -> Option<NominalMember<'_, 'src>> {
        self.view().nominal_member(id)
    }

    pub fn resolved_member(&self, id: SourceNodeId) -> Option<NominalMember<'_, 'src>> {
        self.view().resolved_member(id)
    }

    pub(crate) fn base_class(&self, class: NominalId) -> Option<NominalId> {
        self.view().base_class(class)
    }

    pub(crate) fn base_constructor(
        &self,
        class: NominalId,
    ) -> Option<(NominalId, FunctionType<'src>)> {
        self.view().base_constructor(class)
    }

    pub(crate) fn enum_variant_value(&self, node: SourceNodeId) -> Option<i64> {
        self.view().enum_variant_value(node)
    }

    pub(crate) fn structs(&self) -> impl Iterator<Item = &StructInfo<'src>> {
        self.view().structs()
    }

    pub(crate) fn classes(&self) -> impl Iterator<Item = &ClassInfo<'src>> {
        self.view().classes()
    }

    pub(crate) fn dynamic_import_module(&self, node: SourceNodeId) -> Option<u32> {
        self.view().dynamic_import_module(node)
    }

    pub(crate) fn dynamic_export_used(&self, module: u32, name: &str) -> bool {
        self.view().dynamic_export_used(module, name)
    }
}

impl<'view, 'ast, 'src> CheckedView<'view, 'ast, 'src> {
    pub(crate) fn source_contract(&self) -> crate::config::LanguageConfig { self.declarations.source_contract }
    pub(crate) fn absence_abi(&self) -> bool { self.source_contract().unified_absence() || self.declarations.types.absence_pins }
    pub fn expression_type(&self, id: crate::ast::SourceNodeId) -> Option<&'view Type<'src>> {
        self.expression_type_id(id).map(|id| self.checked_type(id))
    }

    pub fn expression_type_id(&self, id: SourceNodeId) -> Option<CheckedTypeId> {
        self.facts
            .expression_types
            .get(id.index())
            .copied()
            .flatten()
    }
    pub fn checked_type(&self, id: CheckedTypeId) -> &'view Type<'src> {
        self.declarations.types.get(id)
    }
    pub(crate) fn type_count(&self) -> usize {
        self.declarations.types.len()
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

    pub fn binding_type(&self, node: SourceNodeId) -> Option<&'view Type<'src>> {
        Some(match self.facts.binding_types.get(&node)? {
            BindingType::Symbol(symbol) => &self.declarations.symbols[symbol.0 as usize].ty,
            BindingType::Inline(ty) => self.checked_type(*ty),
        })
    }

    pub(crate) fn builtin_call(&self, id: SourceNodeId) -> Option<BuiltinCall> {
        match self.expression_resolution(id) {
            ExpressionResolution::Builtin(builtin) => Some(builtin),
            _ => None,
        }
    }

    /// The dynamic operation a node's syntax is, if it is one (R12).
    pub(crate) fn dynamic_operation(&self, id: SourceNodeId) -> Option<BuiltinCall> {
        match self.expression_resolution(id) {
            ExpressionResolution::Dynamic(builtin) => Some(builtin),
            _ => None,
        }
    }

    /// The operation the arithmetic operator at `node` resolved to (M4.3).
    pub(crate) fn resolved_operator(&self, node: SourceNodeId) -> Option<ResolvedOperator> {
        self.facts.resolved_operators.get(&node).copied()
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

    pub fn identifier_symbol(&self, node: SourceNodeId) -> Option<SymbolId> {
        self.facts.identifier_symbols.get(&node).copied()
    }

    pub(crate) fn symbol_is_reassigned(&self, symbol: SymbolId) -> bool {
        self.declarations.assigned_symbols.contains(&symbol)
    }

    /// Whether a nominal is in the reflected set (R6): a crossing shows it
    /// to the host, so its property names, key order and identity are
    /// observable.
    pub fn is_reflected(&self, nominal: NominalId) -> bool {
        self.declarations.reflected.borrow().contains(&nominal)
    }

    /// Some occurrence of the binding may run before it is initialized: the
    /// checker's proof does not cover it (M4.3).
    pub(crate) fn symbol_is_observable_before_initialization(&self, symbol: SymbolId) -> bool {
        self.declarations
            .observable_before_initialization
            .contains(&symbol)
    }

    /// Whether the occurrence at `node` (a read or a write of a binding)
    /// provably runs after the binding's initialization.
    pub fn read_initialization(&self, node: SourceNodeId) -> ReadInitialization {
        if self.facts.deferred_initialization.contains(&node) {
            ReadInitialization::NeedsCallGraph
        } else {
            ReadInitialization::Definite
        }
    }

    pub(crate) fn type_check_type(&self, node: SourceNodeId) -> Option<&'view Type<'src>> {
        self.facts
            .type_check_types
            .get(&node)
            .map(|id| self.checked_type(*id))
    }

    pub(crate) fn optional_present_type(&self, node: SourceNodeId) -> Option<&'view Type<'src>> {
        self.facts
            .optional_present_types
            .get(&node)
            .map(|id| self.checked_type(*id))
    }

    pub fn symbols(&self) -> &'view [Symbol<'src>] {
        &self.declarations.symbols
    }

    /// The identity this source's scope gives a written type name: the one
    /// lexical step from a spelling to a nominal. Every later question is
    /// asked of the identity.
    pub fn type_binding(&self, name: &str) -> Option<NominalId> {
        self.facts.type_bindings.get(name).copied()
    }

    pub fn struct_type(&self, name: &str) -> Option<StructType<'src>> {
        self.nominal_struct(self.type_binding(name)?)
            .map(|info| info.declaration)
    }

    pub fn export_target(&self, local: SourceNodeId) -> Option<InterfaceTarget> {
        if let Some(symbol) = self.identifier_symbol(local) {
            return Some(InterfaceTarget::Value(symbol));
        }
        self.binding_type(local)
            .and_then(|ty| self.nominal_id(ty))
            .map(InterfaceTarget::Type)
    }

    pub fn struct_info(&self, name: &str) -> Option<&'view StructInfo<'src>> {
        self.nominal_struct(self.type_binding(name)?)
    }

    /// The identity a nominal type names. It is carried by the type itself.
    pub fn nominal_id(&self, ty: &Type<'src>) -> Option<NominalId> {
        match ty {
            Type::Struct(declaration)
            | Type::StructInstance { declaration, .. }
            | Type::Class(declaration)
            | Type::ClassInstance { declaration, .. } => Some(declaration.identity),
            Type::Enum(declaration) => Some(declaration.identity),
            _ => None,
        }
    }

    /// The type that names a nominal declaration (its bare, unapplied form).
    pub fn nominal_type(&self, id: NominalId) -> Option<Type<'src>> {
        let declaration = NominalType {
            identity: id,
            name: self.nominal_name(id)?,
        };
        Some(match id.kind() {
            NominalKind::Struct => Type::Struct(declaration),
            NominalKind::Class => Type::Class(declaration),
            NominalKind::Enum => Type::Enum(self.nominal_enum(id)?.declaration),
        })
    }

    pub fn nominal_name(&self, id: NominalId) -> Option<&'src str> {
        match id.kind() {
            NominalKind::Struct => self
                .declarations
                .structs
                .get(id.index())
                .map(|info| info.declaration.name),
            NominalKind::Class => self
                .declarations
                .classes
                .get(id.index())
                .map(|info| info.declaration.name),
            NominalKind::Enum => self
                .declarations
                .enums
                .get(id.index())
                .map(|info| info.declaration.name),
        }
    }

    pub fn nominal_struct(&self, id: NominalId) -> Option<&'view StructInfo<'src>> {
        id.is_struct()
            .then(|| self.declarations.structs.get(id.index()))
            .flatten()
    }

    pub fn nominal_class(&self, id: NominalId) -> Option<&'view ClassInfo<'src>> {
        id.is_class()
            .then(|| self.declarations.classes.get(id.index()))
            .flatten()
    }

    pub fn nominal_enum(&self, id: NominalId) -> Option<&'view EnumInfo<'src>> {
        id.is_enum()
            .then(|| self.declarations.enums.get(id.index()))
            .flatten()
    }

    pub fn nominal_member(&self, id: NominalMemberId) -> Option<NominalMember<'view, 'src>> {
        let MemberDefinition { owner, slot } =
            *self.declarations.nominal_members.get(id.index())?;
        let value = match owner.kind() {
            NominalKind::Class => {
                let class = self.declarations.classes.get(owner.index())?;
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
            }
            NominalKind::Struct => {
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
            }
            NominalKind::Enum => return None,
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

    /// The class a class extends, by identity.
    pub(crate) fn base_class(&self, class: NominalId) -> Option<NominalId> {
        self.nominal_class(class)?
            .base
            .as_ref()
            .and_then(class_type_identity)
    }

    pub(crate) fn base_constructor(
        &self,
        class: NominalId,
    ) -> Option<(NominalId, FunctionType<'src>)> {
        let class = self.nominal_class(class)?;
        let base_ty = class.base.as_ref()?;
        let (base_declaration, base_args) = class_type_parts(base_ty)?;
        let base = self.nominal_class(base_declaration.identity)?;
        let signature = base.constructor.clone()?;
        let substitutions = substitutions_for(&base.type_params, base_args);
        let Type::Function(signature) = substitute_type(&Type::Function(signature), &substitutions)
        else {
            unreachable!("constructor substitution preserves function type")
        };
        Some((base_declaration.identity, signature))
    }

    pub(crate) fn enum_variant_value(&self, node: SourceNodeId) -> Option<i64> {
        self.facts.enum_variant_values.get(&node).copied()
    }

    pub(crate) fn structs(&self) -> impl Iterator<Item = &'view StructInfo<'src>> + 'view {
        self.declarations.structs.iter()
    }

    pub(crate) fn classes(&self) -> impl Iterator<Item = &'view ClassInfo<'src>> + 'view {
        self.declarations.classes.iter()
    }

    pub(crate) fn dynamic_import_module(&self, node: SourceNodeId) -> Option<u32> {
        self.facts.dynamic_import_modules.get(&node).copied()
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
) -> Result<CheckedModule<'ast, 'src>, CheckError> {
    analyze_single_source(program, &mut AllocationBudget::new(None)).map_err(
        |failure| match failure {
            AdmittedCheckError::Semantic(error) => error,
            AdmittedCheckError::Resources(reason) => CheckError::new(
                program.span,
                format!("semantic checking allocation failed: {reason}"),
            ),
        },
    )
}

/// Fixed source-node tables and canonical declaration vectors are admitted.
/// Nested types, maps, diagnostics and comprehensive traversal remain separate.
pub(crate) fn with_analyzed_source<'ast, 'src, R>(
    program: &Program<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
    client: impl FnOnce(&CheckedModule<'ast, 'src>, &mut AllocationBudget<'_>) -> R,
) -> Result<R, AdmittedCheckError> {
    with_analyzed_source_with_contract(program, crate::config::LanguageConfig::default(), budget, client)
}

pub(crate) fn with_analyzed_source_with_contract<'ast, 'src, R>(
    program: &Program<'ast, 'src>,
    contract: crate::config::LanguageConfig,
    budget: &mut AllocationBudget<'_>,
    client: impl FnOnce(&CheckedModule<'ast, 'src>, &mut AllocationBudget<'_>) -> R,
) -> Result<R, AdmittedCheckError> {
    let mut scope = budget.scope();
    let model = analyze_single_source_with_contract(program, contract, &mut scope)?;
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
#[path = "check/admission_tests.rs"]
mod admission_tests;

#[cfg(test)]
#[path = "check/declaration_admission_tests.rs"]
mod declaration_admission_tests;

#[cfg(test)]
#[path = "check/analyzer_admission_tests.rs"]
mod analyzer_admission_tests;

#[cfg(test)]
#[path = "check/binary_admission_tests.rs"]
mod binary_admission_tests;

pub(crate) mod assignments;

#[cfg(test)]
#[path = "check/narrowing_admission_tests.rs"]
mod narrowing_admission_tests;

#[cfg(test)]
#[path = "check/narrowing_input_tests.rs"]
mod narrowing_input_tests;

/// Test driver for the checker's own admission accounting: one Analyzer
/// through one source's phases (`Analyzer::analyze_program`), with no module
/// graph around it. Compilation checks through `analyze_single_source`.
#[cfg(test)]
pub(crate) fn with_single_analyzer<'ast, 'src, R>(
    program: &Program<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
    client: impl FnOnce(&CheckedModule<'ast, 'src>, &mut AllocationBudget<'_>) -> R,
) -> Result<R, AdmittedCheckError> {
    let mut scope = budget.scope();
    let facts = ModuleFacts::new_admitted(program.source_identity(), &mut scope)?;
    let mut model = CheckedModule {
        declarations: DeclarationTables::default(),
        facts,
        exports: Vec::new(),
    };
    let mut initialization = ModuleInitialization::default();
    Analyzer::new(
        &mut model.facts,
        &mut model.declarations,
        &mut initialization,
        None,
        &mut scope,
    )?
    .analyze_program(program)?;
    let model = AdmittedFactsOwner::new(model, 1);
    let output = client(&model, &mut scope);
    drop(model);
    scope
        .finish_retained()
        .expect("checked-source callback transfers within its allocation owner");
    Ok(output)
}

/// A single source is checked as a module graph of one module, through the
/// one checking entry (`modules::analyze_modules_in`): there is one phase
/// order. Only its refusal of imports is the single source's own.
fn analyze_single_source<'ast, 'src>(
    program: &Program<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<CheckedModule<'ast, 'src>, AdmittedCheckError> {
    analyze_single_source_with_contract(program, crate::config::LanguageConfig::default(), budget)
}

fn analyze_single_source_with_contract<'ast, 'src>(
    program: &Program<'ast, 'src>,
    contract: crate::config::LanguageConfig,
    budget: &mut AllocationBudget<'_>,
) -> Result<CheckedModule<'ast, 'src>, AdmittedCheckError> {
    if let Some(import) = program.imports.first() {
        return Err(AdmittedCheckError::new(
            import.span,
            "imports require file-based compilation so the module graph can be resolved",
        ));
    }
    let checked = modules::analyze_source_in(program, contract, budget).map_err(|failure| failure.error)?;
    let model = checked.into_single();
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
    type_parameter_scopes: Vec<AHashMap<&'src str, TypeParameter<'src>>>,
    loop_depth: usize,
    async_depth: usize,
    callable_depth: usize,
    /// The bodies being analyzed, outermost (the module) first, with the
    /// names each assigns: a narrowing holds only where no code the flow
    /// does not see can assign its binding (R1, `check/assignments.rs`).
    bodies: Vec<assignments::Assigned<'src>>,
    /// Per body in `bodies`, the first narrowing scope its own code opens:
    /// a narrowing in an earlier scope is inherited from an enclosing body.
    narrowing_bases: Vec<usize>,
    /// The body that declares each source binding, as an index into `bodies`.
    symbol_bodies: AHashMap<SymbolId, usize>,
    /// Locals declared without a value and not assigned on every path to
    /// here (R3): a read of one is refused.
    unassigned: Vec<SymbolId>,
    /// Depth of expression parts that may not run: the right operands of
    /// `&&`, `||` and `??`, a conditional's branches, a match's arms. An
    /// assignment there does not count as definite (R3).
    conditional_assignments: usize,
    reference_parameters: AHashMap<SymbolId, usize>,
    pending_references: bool,
    current_reference_formals: bool,
    initializing: Option<(SymbolId, usize)>,
    /// Bindings whose initializers are being analyzed, innermost last: an
    /// occurrence inside one (in a nested function) may run before it ends.
    initializing_symbols: Vec<SymbolId>,
    /// Inside a parameter default: its reads run at each call site.
    parameter_defaults: usize,
    /// A module binding declared before its module's items are analyzed,
    /// by its declaring identifier.
    module_binding_declarations: AHashMap<SourceNodeId, SymbolId>,
    constructor_classes: Vec<Option<NominalId>>,
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
        /// `&&`, `||`, `??`: the right operand may not run (R3).
        conditional: bool,
    },
}

/// A module-scoped binding: where it is declared, and the module that owns
/// it, whose top level may not read it before that declaration.
#[derive(Debug, Clone, Copy)]
struct ModuleBindingState {
    declaration: Span,
    owner: crate::module::ModuleId,
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
        /// The test expression.
        test: SourceNodeId,
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
            Some(NarrowingLeaf::TypeCheck {
                ident,
                test: condition.id,
                span: *span,
            })
        }
        ExprKind::Binary {
            op: op @ (BinaryOp::Eq | BinaryOp::NotEq),
            lhs,
            rhs,
            ..
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
            || matches!(
                expression.kind,
                ExprKind::Unary {
                    op: UnaryOp::Not,
                    ..
                }
            );
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
            bodies: Vec::new(),
            narrowing_bases: Vec::new(),
            symbol_bodies: AHashMap::default(),
            unassigned: Vec::new(),
            conditional_assignments: 0,
            reference_parameters: AHashMap::default(),
            pending_references: false,
            current_reference_formals: false,
            initializing: None,
            initializing_symbols: Vec::new(),
            parameter_defaults: 0,
            module_binding_declarations: AHashMap::default(),
            constructor_classes: Vec::new(),
            generator_contexts: Vec::new(),
        };
        analyzer.scopes = analyzer.budget.vector(AllocationClass::Scratch, 1)?;
        analyzer.narrowings = analyzer.budget.vector(AllocationClass::Scratch, 1)?;
        analyzer.push_scope()?;
        Ok(analyzer)
    }

    fn record_type(
        &mut self,
        node: SourceNodeId,
        ty: &Type<'src>,
    ) -> Result<(), AdmittedCheckError> {
        let id = self.declarations.types.intern(ty, self.budget)?;
        self.facts.expression_types[node.index()] = Some(id);
        Ok(())
    }
    fn record_type_binding(
        &mut self,
        node: SourceNodeId,
        ty: &Type<'src>,
    ) -> Result<(), AdmittedCheckError> {
        let id = self.declarations.types.intern(ty, self.budget)?;
        self.facts
            .binding_types
            .insert(node, BindingType::Inline(id));
        Ok(())
    }

    /// Test driver: one Analyzer through one source's phases, so admission
    /// tests can observe its frames. Compilation checks every source through
    /// the module-graph entry (`modules::analyze_modules_in`).
    #[cfg(test)]
    fn analyze_program(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedCheckError> {
        if let Some(import) = program.imports.first() {
            return Err(AdmittedCheckError::new(
                import.span,
                "imports require file-based compilation so the module graph can be resolved",
            ));
        }
        self.declare_nominal_types(program)?;
        self.define_enums(program)?;
        self.define_structs(program)?;
        struct_cycles::validate(&self.declarations.structs).map_err(|(_, error)| error)?;
        self.define_classes(program)?;
        self.define_extern_classes(program)?;
        self.resolve_class_hierarchies()?;
        self.declarations
            .mark_observed_classes()
            .map_err(|(_, error)| AdmittedCheckError::Semantic(error))?;
        self.declare_functions(program)?;

        self.analyze_items(program)?;
        self.declarations
            .mark_constructor_observations(std::slice::from_ref(self.facts), self.budget)?;
        self.declarations
            .mark_observed_classes()
            .map_err(|(_, error)| AdmittedCheckError::Semantic(error))?;
        self.declarations.mark_tested_classes();

        for export in program.exports {
            let target = if let Some(target) = self.view().export_target(export.local.id) {
                Some(target)
            } else {
                match (
                    self.scopes[0].get(export.local.name).copied(),
                    self.facts
                        .type_bindings
                        .get(export.local.name)
                        .copied()
                        .filter(|identity| identity.is_struct()),
                ) {
                    (Some(_), Some(_)) => {
                        return Err(AdmittedCheckError::new(
                            export.span,
                            "ambiguous export names both a value and a struct type; export the declaration directly",
                        ));
                    }
                    (Some(symbol), None) => Some(InterfaceTarget::Value(symbol)),
                    (None, Some(identity)) => Some(InterfaceTarget::Type(identity)),
                    (None, None) => None, // Existing enum/type-only and unresolved-export owners remain unchanged.
                }
            };
            match target {
                Some(InterfaceTarget::Value(symbol)) => {
                    if self.declarations.symbols[symbol.0 as usize]
                        .ty
                        .contains_mutable_reference_parameters()
                    {
                        return Err(AdmittedCheckError::new(
                            export.span,
                            "public exports do not yet support mutable-reference callable contracts",
                        ));
                    }
                    self.record_identifier(export.local.id, symbol);
                    // An export's value crosses to its consumer (R6).
                    self.declarations
                        .reflect(&self.declarations.symbols[symbol.0 as usize].ty);
                }
                Some(InterfaceTarget::Type(identity)) => {
                    if let Some(ty) = self.view().nominal_type(identity) {
                        self.record_type_binding(export.local.id, &ty)?;
                    }
                }
                None => {}
            }
        }
        self.declarations.close_reflected();
        if self.declarations.classes.iter().any(|class| class.shape) {
            CheckedView { declarations: self.declarations, facts: self.facts }
                .validate_shapes(self.module, self.budget)?;
        }
        Ok(())
    }

    fn analyze_items(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedCheckError> {
        self.enter_body(assignments::Assigned::module(program))?;
        let result = self.analyze_module_items(program);
        self.leave_body();
        result
    }

    fn analyze_module_items(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
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

    fn view(&self) -> CheckedView<'_, 'ast, 'src> {
        CheckedView {
            declarations: self.declarations,
            facts: self.facts,
        }
    }

    fn record_identifier(&mut self, ident: SourceNodeId, symbol: SymbolId) {
        self.facts
            .record_identifier(self.declarations, ident, symbol);
    }

    /// Classify one occurrence (a read or a write) of a binding: whether the
    /// checker proves it runs after the binding's initialization. What it
    /// cannot prove, the initialization owner decides from the call graph
    /// and module order (M4.3, M6.5):
    /// * an occurrence inside the binding's own initializer: only a nested
    ///   function reaches it there, and it may be called before the
    ///   initializer ends;
    /// * a parameter default: it is read at every call site;
    /// * a module binding read in a function, which may be called before the
    ///   declaration runs, or from another module, whose order the checker
    ///   verifies only for typed bindings read at its top level.
    ///
    /// Every other occurrence follows its declaration in the same function,
    /// or in a closure created after it: definite.
    fn record_read_initialization(&mut self, node: SourceNodeId, symbol: SymbolId) {
        let module_binding = self.declarations.module_bindings.contains(&symbol);
        let foreign_module = self
            .declarations
            .symbol_modules
            .get(symbol.0 as usize)
            .copied()
            .flatten()
            != self.module;
        if self.initializing_symbols.contains(&symbol)
            || self.parameter_defaults > 0
            || (module_binding && (self.callable_depth > 0 || foreign_module))
        {
            self.declarations
                .observable_before_initialization
                .insert(symbol);
            self.facts.deferred_initialization.insert(node);
        }
    }

    /// The declaration phase: every nominal this source declares gets its
    /// identity and a binding in this source's type scope. Two sources'
    /// declarations of one name are two identities; only one scope refuses
    /// a second declaration (Closure's per-scope type table, esbuild's
    /// per-source symbol arrays).
    fn declare_nominal_types(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        for item in program.items {
            let (name, type_params, span, kind, external) = match item {
                Item::Struct(decl) => (
                    decl.name,
                    decl.type_params,
                    decl.span,
                    NominalKind::Struct,
                    false,
                ),
                Item::Class(decl) => (
                    decl.name,
                    decl.type_params,
                    decl.span,
                    NominalKind::Class,
                    false,
                ),
                Item::ExternClass(decl) => (
                    decl.name,
                    decl.type_params,
                    decl.span,
                    NominalKind::Class,
                    true,
                ),
                Item::Enum(decl) => (decl.name, &[][..], decl.span, NominalKind::Enum, false),
                _ => continue,
            };
            let type_params = validate_type_params(self.module, type_params)?;

            if self.facts.type_bindings.contains_key(name.name) {
                return Err(AdmittedCheckError::new(
                    span,
                    format!("duplicate type declaration `{}`", name.name),
                ));
            }

            let identity = match kind {
                NominalKind::Struct => {
                    let identity = NominalId::new(self.declarations.structs.len(), kind);
                    let declaration = NominalType {
                        identity,
                        name: name.name,
                    };
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
                    self.record_type_binding(name.id, &Type::Struct(declaration))?;
                    identity
                }
                NominalKind::Class => {
                    let identity = NominalId::new(self.declarations.classes.len(), kind);
                    let declaration = NominalType {
                        identity,
                        name: name.name,
                    };
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut self.declarations.classes,
                        ClassInfo {
                            sealed: matches!(item, Item::Class(decl) if decl.sealed),
                            discriminant: None,
                            shape: matches!(item, Item::Class(decl) if decl.shape),
                            declaration,
                            module: self.module,
                            name: name.name,
                            type_params,
                            base: None,
                            fields: IndexMap::new(),
                            methods: IndexMap::new(),
                            constructor: None,
                            value: None,
                            external,
                            published: false,
                            observed: false,
                            initialization: FieldInitializationFacts::default(),
                            span,
                        },
                    )?;
                    self.record_type_binding(name.id, &Type::Class(declaration))?;
                    identity
                }
                NominalKind::Enum => {
                    let identity = NominalId::new(self.declarations.enums.len(), kind);
                    let declaration = EnumType {
                        declaration: NominalType { identity, name: name.name },
                        abi: match item { Item::Enum(declaration) => declaration.abi, _ => unreachable!() },
                    };
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut self.declarations.enums,
                        EnumInfo {
                            declaration,
                            module: self.module,
                            name: name.name,
                            variants: IndexMap::new(),
                            values: Vec::new(),
                            flag_mask: 0,
                            span,
                        },
                    )?;
                    self.record_type_binding(name.id, &Type::Enum(declaration))?;
                    identity
                }
            };
            self.facts.type_bindings.insert(name.name, identity);
        }
        Ok(())
    }

    fn define_structs(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedCheckError> {
        for item in program.items {
            let Item::Struct(decl) = item else {
                continue;
            };
            self.push_type_params(decl.type_params)?;

            let fields = self.resolve_fields(decl)?;
            self.pop_type_params();
            let identity = self.facts.type_bindings[decl.name.name];
            self.declarations.structs[identity.index()].fields = fields;
        }
        Ok(())
    }

    fn define_classes(&mut self, program: &Program<'ast, 'src>) -> Result<(), AdmittedCheckError> {
        for item in program.items {
            let Item::Class(decl) = item else {
                continue;
            };

            let owner = self.facts.type_bindings[decl.name.name];
            let declaration = self.declarations.classes[owner.index()].declaration;

            self.push_type_params(decl.type_params)?;

            let base = decl
                .base
                .map(|base| self.resolve_value_type(base, "base class"))
                .transpose()?;
            if base
                .as_ref()
                .is_some_and(|base| !matches!(base, Type::Class(_) | Type::ClassInstance { .. }))
            {
                return Err(AdmittedCheckError::new(
                    decl.base.expect("checked base").span,
                    "`extends` requires a class type",
                ));
            }

            if base.as_ref().is_some_and(|base| self.view().is_shape(base)) {
                return Err(AdmittedCheckError::new(decl.span, "a class cannot extend a plain shape"));
            }
            let mut fields = IndexMap::new();
            let mut methods = IndexMap::new();
            let mut constructor = None;
            let mut discriminant = None;
            for member in decl.members {
                match member {
                    ClassMember::Field(field) => {
                        if fields.contains_key(field.name.name)
                            || methods.contains_key(field.name.name)
                        {
                            return Err(AdmittedCheckError::new(
                                field.name.span,
                                format!(
                                    "duplicate member `{}` in class `{}`",
                                    field.name.name, decl.name.name
                                ),
                            ));
                        }
                        let ty = self.resolve_value_type(field.ty, "class field")?;
                        let index = fields.len();
                        if field.discriminant {
                            if discriminant.is_some() {
                                return Err(AdmittedCheckError::new(field.span, "a shape declares one discriminant"));
                            }
                            let tag = field.initializer.as_ref().and_then(shapes::ShapeTag::literal)
                                .ok_or_else(|| AdmittedCheckError::new(field.span, "a shape tag requires a string, int or bool literal initializer"))?;
                            if !tag.matches_type(&ty) {
                                return Err(AdmittedCheckError::new(field.span, "shape tag literal disagrees with its declared type"));
                            }
                            discriminant = Some((index, tag));
                        }
                        fields.insert(
                            field.name.name,
                            FieldInfo {
                                accessor: field.accessor,
                                has_initializer: field.initializer.is_some(),
                                member: self.declarations.declare_member(
                                    owner,
                                    MemberSlot::field(index),
                                    self.budget,
                                )?,
                                name: field.name.name,
                                ty,
                                index,
                                span: field.span,
                            },
                        );
                    }
                    ClassMember::Method(method) => {
                        use crate::ast::MethodDispatch;
                        if method.dispatch == MethodDispatch::Virtual && !decl.sealed {
                            return Err(AdmittedCheckError::new(method.span, "a virtual method must be introduced by a sealed class"));
                        }
                        if method.dispatch == MethodDispatch::Override && decl.base.is_none() {
                            return Err(AdmittedCheckError::new(method.span, "an override needs an inherited virtual method"));
                        }
                        if fields.contains_key(method.name.name)
                            || methods.contains_key(method.name.name)
                        {
                            return Err(AdmittedCheckError::new(
                                method.name.span,
                                format!(
                                    "duplicate member `{}` in class `{}`",
                                    method.name.name, decl.name.name
                                ),
                            ));
                        }
                        let signature = self.function_type(method)?;
                        methods.insert(
                            method.name.name,
                            MethodInfo {
                                dispatch: method.dispatch,
                                member: self.declarations.declare_member(
                                    owner,
                                    MemberSlot::method(methods.len()),
                                    self.budget,
                                )?,
                                owner,
                                type_params: validate_type_params(self.module, method.type_params)?,
                                signature,
                                declared_pure: method.declared_pure,
                            },
                        );
                    }
                    ClassMember::Constructor(constructor_decl) => {
                        if constructor.is_some() {
                            return Err(AdmittedCheckError::new(
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
                            return Err(AdmittedCheckError::new(
                                constructor_decl.span,
                                "constructors do not support mutable-reference parameters",
                            ));
                        }
                        mark_rest_parameter(constructor_decl.params, &mut params)?;
                        resolve_parameter_defaults(
                            constructor_decl.params,
                            &mut params,
                            &self.facts.type_bindings,
                        )?;
                        constructor = Some(FunctionType::new(FunctionSignature {
                            params,
                            return_type: Box::new(applied_class_type(
                                declaration,
                                &validate_type_params(self.module, decl.type_params)?,
                            )),
                        }));
                    }
                }
            }

            {
                let info = &mut self.declarations.classes[owner.index()];
                info.fields = fields;
                info.methods = methods;
                info.base = base;
                info.constructor = constructor.clone();
                info.discriminant = discriminant;
            }
            self.pop_type_params();

            if decl.shape { continue; }
            // The class's name as a value is its constructor: only a class
            // kept as a JavaScript class has one at run time, and LilScript
            // constructs with `new`, never by a call (conversion refuses both).
            let constructor_signature =
                constructor.unwrap_or(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(applied_class_type(
                        declaration,
                        &validate_type_params(self.module, decl.type_params)?,
                    )),
                }));
            let constructor = if decl.type_params.is_empty() {
                Type::Function(constructor_signature)
            } else {
                Type::GenericFunction(GenericFunctionType {
                    type_params: validate_type_params(self.module, decl.type_params)?,
                    signature: constructor_signature,
                })
            };
            let value = self.declare(decl.name, constructor)?;
            self.declarations.classes[owner.index()].value = Some(value);
        }
        Ok(())
    }

    pub(super) fn define_extern_classes(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        for item in program.items {
            let Item::ExternClass(decl) = item else {
                continue;
            };
            let owner = self.facts.type_bindings[decl.name.name];
            self.push_type_params(decl.type_params)?;
            let base = decl
                .base
                .map(|base| self.resolve_value_type(base, "base extern class"))
                .transpose()?;
            if base
                .as_ref()
                .is_some_and(|base| !matches!(base, Type::Class(_) | Type::ClassInstance { .. }))
            {
                return Err(AdmittedCheckError::new(
                    decl.base.expect("checked base").span,
                    "`extends` requires a class type",
                ));
            }
            if base.as_ref().is_some_and(|base| self.view().is_shape(base)) {
                return Err(AdmittedCheckError::new(decl.span, "an extern class cannot extend a plain shape"));
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
                        if let Some(param) = constructor
                            .params
                            .iter()
                            .find(|param| param.default.is_some())
                        {
                            return Err(AdmittedCheckError::new(
                                param.span,
                                "a host constructor signature cannot declare parameter defaults",
                            ));
                        }
                        let mut params = Vec::with_capacity(constructor.params.len());
                        for param in constructor.params {
                            params.push(self.resolve_parameter_type(
                                &param.parameter,
                                "host constructor parameter",
                            )?);
                        }
                        mark_rest_parameter(constructor.params, &mut params)?;
                        let signature = FunctionType::new(FunctionSignature {
                            params,
                            return_type: Box::new(Type::Void),
                        });
                        if Type::Function(signature.clone()).contains_mutable_reference_parameters()
                        {
                            return Err(AdmittedCheckError::new(
                                constructor.span,
                                "foreign callable contracts do not support mutable-reference parameters",
                            ));
                        }
                        host_constructor = Some(signature);
                    }
                    ExternClassMember::Field(field) => {
                        if let Some(initializer) = &field.initializer {
                            return Err(AdmittedCheckError::new(
                                initializer.span(),
                                "an extern class field is the host's: it takes no initializer",
                            ));
                        }
                        if fields.contains_key(field.name.name)
                            || methods.contains_key(field.name.name)
                        {
                            return Err(AdmittedCheckError::new(
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
                                accessor: true,
                                has_initializer: false,
                                member: self.declarations.declare_member(
                                    owner,
                                    MemberSlot::field(index),
                                    self.budget,
                                )?,
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
                            return Err(AdmittedCheckError::new(
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
                                dispatch: crate::ast::MethodDispatch::Static,
                                member: self.declarations.declare_member(
                                    owner,
                                    MemberSlot::method(methods.len()),
                                    self.budget,
                                )?,
                                owner,
                                type_params: validate_type_params(self.module, method.type_params)?,
                                signature: self.extern_type(method)?,
                                declared_pure: method.declared_pure,
                            },
                        );
                    }
                }
            }
            self.pop_type_params();
            let info = &mut self.declarations.classes[owner.index()];
            info.fields = fields;
            info.methods = methods;
            info.base = base;
            if host_constructor.is_some() {
                info.constructor = host_constructor;
            }
        }
        Ok(())
    }

    fn resolve_class_hierarchies(&mut self) -> Result<(), AdmittedCheckError> {
        let mut visiting = AHashSet::default();
        let mut complete = AHashSet::default();
        for index in 0..self.declarations.classes.len() {
            let class = NominalId::new(index, NominalKind::Class);
            self.resolve_class_hierarchy(class, &mut visiting, &mut complete)?;
        }
        Ok(())
    }

    fn resolve_class_hierarchy(
        &mut self,
        class: NominalId,
        visiting: &mut AHashSet<NominalId>,
        complete: &mut AHashSet<NominalId>,
    ) -> Result<(), AdmittedCheckError> {
        if complete.contains(&class) {
            return Ok(());
        }
        let info = &self.declarations.classes[class.index()];
        let name = info.name;
        let span = info.span;
        let external = info.external;
        if !visiting.insert(class) {
            return Err(AdmittedCheckError::new(
                span,
                format!("inheritance cycle involving class `{name}`"),
            ));
        }

        let Some(base_ty) = &info.base else {
            visiting.remove(&class);
            complete.insert(class);
            return Ok(());
        };
        let base_class = class_type_identity(base_ty)
            .expect("base classes were validated while defining classes");
        let base = &self.declarations.classes[base_class.index()];
        // An internal class may extend a host (`extern`) class: that is how a
        // typed class becomes a real `Error` subclass, with a native prototype
        // chain, `instanceof`, `stack` and `message`, instead of hand-written
        // `JsValue` prototype ceremony. The reverse is still meaningless: a
        // host interface cannot inherit an implementation the host never sees.
        if external && !base.external {
            return Err(AdmittedCheckError::new(
                span,
                "an extern class cannot extend an internal class",
            ));
        }
        self.resolve_class_hierarchy(base_class, visiting, complete)?;
        let info = &self.declarations.classes[class.index()];
        let (_, base_args) = class_type_parts(
            info.base
                .as_ref()
                .expect("derived class keeps its base type"),
        )
        .expect("base classes were validated while defining classes");
        let base = &self.declarations.classes[base_class.index()];
        let substitutions = substitutions_for(&base.type_params, base_args);
        let mut fields = IndexMap::new();
        for field in base.fields.values() {
            fields.insert(
                field.name,
                FieldInfo {
                    accessor: field.accessor,
                    has_initializer: field.has_initializer,
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
                    dispatch: method.dispatch,
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
                return Err(AdmittedCheckError::new(
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
        let mut next_method = methods.len();
        for (method_name, method) in &info.methods {
            use crate::ast::MethodDispatch;
            if let Some(inherited) = methods.get(method_name) {
                if inherited.dispatch == MethodDispatch::Static || method.dispatch != MethodDispatch::Override {
                    return Err(AdmittedCheckError::new(span,
                        format!("class `{name}` cannot override inherited member `{method_name}` without an explicit override of a virtual method")));
                }
                if !variants::override_signature(method, inherited)
                    || inherited.declared_pure && !method.declared_pure {
                    return Err(AdmittedCheckError::new(span, "a virtual override must preserve its parameter, result, ref/default/rest and purity contract"));
                }
                self.declarations.nominal_members[method.member.index()].slot =
                    self.declarations.nominal_members[inherited.member.index()].slot;
                continue;
            }
            if method.dispatch == MethodDispatch::Override {
                return Err(AdmittedCheckError::new(span, "an override needs an inherited virtual method"));
            }
            if fields.contains_key(method_name)
                || info.fields.contains_key(method_name)
            {
                return Err(AdmittedCheckError::new(
                    span,
                    format!("class `{name}` cannot override inherited member `{method_name}`"),
                ));
            }
            self.declarations.nominal_members[method.member.index()].slot =
                MemberSlot::method(next_method);
            next_method += 1;
        }
        let resolved = &mut self.declarations.classes[class.index()];
        for (_, mut field) in std::mem::take(&mut resolved.fields) {
            field.index = fields.len();
            fields.insert(field.name, field);
        }
        methods.extend(std::mem::take(&mut resolved.methods));
        resolved.fields = fields;
        resolved.methods = methods;
        visiting.remove(&class);
        complete.insert(class);
        Ok(())
    }

    fn resolve_fields(
        &mut self,
        decl: &'ast StructDecl<'ast, 'src>,
    ) -> Result<IndexMap<&'src str, FieldInfo<'src>>, AdmittedCheckError> {
        let owner = self
            .view()
            .struct_type(decl.name.name)
            .expect("struct declared before member definitions")
            .identity;
        let mut fields = IndexMap::new();
        for field in decl.fields {
            if fields.contains_key(field.name.name) {
                return Err(AdmittedCheckError::new(
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
                    accessor: false,
                    has_initializer: field.initializer.is_some(),
                    member: self.declarations.declare_member(
                        owner,
                        MemberSlot::field(index),
                        self.budget,
                    )?,
                    name: field.name.name,
                    ty,
                    index,
                    span: field.span,
                },
            );
        }
        Ok(fields)
    }

    fn declare_functions(
        &mut self,
        program: &Program<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        for item in program.items {
            match item {
                Item::Function(function) => {
                    let signature = self.function_type(function)?;
                    let ty = if function.type_params.is_empty() {
                        Type::Function(signature)
                    } else {
                        Type::GenericFunction(GenericFunctionType {
                            type_params: validate_type_params(self.module, function.type_params)?,
                            signature,
                        })
                    };
                    let id = self.declare(function.name, ty)?;
                    self.declarations.symbols[id.0 as usize].attributes = Attributes {
                        pure: function.declared_pure,
                        debug: function.declared_debug,
                    };
                    // A named function exists from instantiation.
                    self.declarations.module_bindings.remove(&id);
                }
                Item::Extern(extern_decl) => {
                    let signature = self.extern_type(extern_decl)?;
                    let ty = if extern_decl.type_params.is_empty() {
                        Type::Function(signature.clone())
                    } else {
                        Type::GenericFunction(GenericFunctionType {
                            type_params: validate_type_params(
                                self.module,
                                extern_decl.type_params,
                            )?,
                            signature: signature.clone(),
                        })
                    };
                    let repeated = self.module.is_some()
                        && self
                            .declarations
                            .foreign_symbols
                            .contains_key(extern_decl.name.name);
                    let symbol = self.declare_foreign(extern_decl.name, ty, true)?;
                    let attributes = Attributes {
                        pure: extern_decl.declared_pure,
                        debug: extern_decl.declared_debug,
                    };
                    let recorded = &mut self.declarations.symbols[symbol.0 as usize].attributes;
                    if repeated && *recorded != attributes {
                        return Err(AdmittedCheckError::new(
                            extern_decl.name.span,
                            format!(
                                "conflicting extern contracts for `{}`: another module declares it {}",
                                extern_decl.name.name,
                                match (recorded.pure, recorded.debug) {
                                    (true, _) => "pure",
                                    (_, true) => "debug",
                                    _ => "without attributes",
                                }
                            ),
                        ));
                    }
                    *recorded = attributes;
                    let mut names = AHashMap::default();
                    for (param, ty) in extern_decl.params.iter().zip(signature.params.iter()) {
                        if names.insert(param.name.name, param.name.span).is_some() {
                            return Err(AdmittedCheckError::new(
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
                        return Err(AdmittedCheckError::new(
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
    ) -> Result<FunctionType<'src>, AdmittedCheckError> {
        self.push_type_params(function.type_params)?;
        let signature = self.function_type_in_current_scope(function);
        self.pop_type_params();
        signature
    }

    fn function_type_in_current_scope(
        &self,
        function: &'ast FunctionDecl<'ast, 'src>,
    ) -> Result<FunctionType<'src>, AdmittedCheckError> {
        if (function.is_async || function.is_generator)
            && function
                .params
                .iter()
                .any(|parameter| parameter.parameter.passing == ParameterPassing::MutableReference)
        {
            return Err(AdmittedCheckError::new(
                function.span,
                "async and generator functions cannot have mutable-reference parameters",
            ));
        }
        let mut params = Vec::with_capacity(function.params.len());
        for param in function.params {
            params.push(self.resolve_parameter_type(&param.parameter, "parameter")?);
        }
        resolve_parameter_defaults(function.params, &mut params, &self.facts.type_bindings)?;
        mark_rest_parameter(function.params, &mut params)?;
        let declared_return = self.resolve_type(function.return_type, true, "return type")?;
        let return_type = if function.is_async {
            Type::Task(Box::new(declared_return))
        } else if function.is_generator {
            if declared_return == Type::Void {
                return Err(AdmittedCheckError::new(
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
    ) -> Result<FunctionType<'src>, AdmittedCheckError> {
        self.push_type_params(extern_decl.type_params)?;
        let mut params = Vec::with_capacity(extern_decl.params.len());
        for param in extern_decl.params {
            params.push(self.resolve_parameter_type(&param.parameter, "extern parameter")?);
        }
        resolve_parameter_defaults(extern_decl.params, &mut params, &self.facts.type_bindings)?;
        mark_rest_parameter(extern_decl.params, &mut params)?;
        let return_type = self.resolve_type(extern_decl.return_type, true, "extern return type")?;
        let signature = FunctionType::new(FunctionSignature {
            params,
            return_type: Box::new(return_type),
        });
        if Type::Function(signature.clone()).contains_mutable_reference_parameters() {
            return Err(AdmittedCheckError::new(
                extern_decl.span,
                "foreign callable contracts do not support mutable-reference parameters",
            ));
        }
        self.pop_type_params();
        Ok(signature)
    }

    fn analyze_class(
        &mut self,
        class: &'ast ClassDecl<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        self.push_type_params(class.type_params)?;
        let identity = self.facts.type_bindings[class.name.name];
        let requires_super = self
            .view()
            .base_class(identity)
            .and_then(|base| self.view().nominal_class(base))
            .is_some_and(|base| base.constructor.is_some());
        if requires_super
            && !class
                .members
                .iter()
                .any(|member| matches!(member, ClassMember::Constructor(_)))
        {
            self.pop_type_params();
            return Err(AdmittedCheckError::new(
                class.span,
                format!(
                    "class `{}` must declare `init` and call its base constructor",
                    class.name.name
                ),
            ));
        }
        for member in class.members {
            match member {
                ClassMember::Method(method) => self.analyze_function(method, Some(identity))?,
                ClassMember::Constructor(constructor) => {
                    self.analyze_constructor(constructor, identity)?
                }
                // A field's own initializer (R3): a value of its type, which
                // every construction evaluates before `init`, where `this` is
                // not yet readable.
                ClassMember::Field(field) => {
                    if let Some(initializer) = &field.initializer {
                        let ty = self
                            .view()
                            .nominal_class(identity)
                            .and_then(|class| class.fields.get(field.name.name))
                            .map(|field| field.ty.clone())
                            .ok_or_else(|| {
                                AdmittedCheckError::new(field.name.span, "a field without a type")
                            })?;
                        let actual = self.analyze_expr(initializer, Some(&ty))?;
                        self.require_assignable(&ty, &actual, initializer.span())?;
                    }
                }
            }
        }
        let initialization = if class.shape { FieldInitializationFacts::default() } else { field_initialization::analyze(
            class,
            CheckedView {
                declarations: self.declarations,
                facts: self.facts,
            },
            self.budget,
        )? };
        self.declarations.classes[identity.index()].initialization = initialization;
        self.pop_type_params();
        Ok(())
    }

    fn analyze_constructor(
        &mut self,
        constructor: &'ast ConstructorDecl<'ast, 'src>,
        class: NominalId,
    ) -> Result<(), AdmittedCheckError> {
        let super_calls = count_super_calls(constructor.body);
        match self.view().base_class(class) {
            None if super_calls != 0 => {
                return Err(AdmittedCheckError::new(
                    constructor.span,
                    "`super` is only valid in a derived class constructor",
                ));
            }
            Some(base) => {
                if super_calls > 1 {
                    return Err(AdmittedCheckError::new(
                        constructor.span,
                        "a derived constructor may call `super` only once",
                    ));
                }
                let base = &self.declarations.classes[base.index()];
                if base.constructor.is_some() && super_calls == 0 {
                    return Err(AdmittedCheckError::new(
                        constructor.span,
                        format!(
                            "derived constructor must begin with `super(...)` for `{}`",
                            base.name
                        ),
                    ));
                }
                if super_calls != 0
                    && !matches!(constructor.body.first(), Some(Stmt::SuperCall { .. }))
                {
                    return Err(AdmittedCheckError::new(
                        constructor.span,
                        "`super(...)` must be the first statement in a derived constructor",
                    ));
                }
            }
            None => {}
        }
        let mut parameters = constructor
            .params
            .iter()
            .map(|param| self.resolve_parameter_type(&param.parameter, "parameter"))
            .collect::<Result<Vec<_>, _>>()?;
        mark_rest_parameter(constructor.params, &mut parameters)?;
        self.callable_depth += 1;
        self.enter_body(assignments::Assigned::body(
            constructor.params,
            constructor.body,
        ))?;
        self.push_scope()?;
        let class_info = &self.declarations.classes[class.index()];
        let this = applied_class_type(class_info.declaration, &class_info.type_params);
        self.declare(constructor.this, this)?;
        for (param, parameter) in constructor.params.iter().zip(&parameters) {
            self.declare(param.name, parameter.ty.clone())?;
        }
        self.analyze_parameter_defaults(constructor.params, &parameters)?;
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
            Some(class),
        )?;
        self.budget
            .push(AllocationClass::Scratch, &mut self.generator_contexts, None)?;
        for statement in constructor.body {
            self.analyze_stmt(statement)?;
        }
        self.generator_contexts.pop();
        self.constructor_classes.pop();
        self.return_contexts.pop();
        self.pop_scope();
        self.leave_body();
        self.callable_depth -= 1;
        Ok(())
    }

    fn analyze_function(
        &mut self,
        function: &'ast FunctionDecl<'ast, 'src>,
        class: Option<NominalId>,
    ) -> Result<(), AdmittedCheckError> {
        self.push_type_params(function.type_params)?;
        let signature = self.function_type_in_current_scope(function)?;
        if class.is_some() {
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
        self.enter_body(assignments::Assigned::body(function.params, function.body))?;
        self.push_scope()?;

        if let Some(class) = class {
            let class_info = &self.declarations.classes[class.index()];
            let this = applied_class_type(class_info.declaration, &class_info.type_params);
            self.declare(function.this, this)?;
        }

        for (param, ty) in function.params.iter().zip(&signature.params) {
            let symbol = self.declare(param.name, ty.ty.clone())?;
            if ty.passing == ParameterPassing::MutableReference {
                self.reference_parameters
                    .insert(symbol, self.callable_depth);
            }
        }

        self.analyze_parameter_defaults(function.params, &signature.params)?;

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
        self.leave_body();
        self.callable_depth -= 1;
        self.pending_references = outer_pending;
        self.current_reference_formals = outer_formals;
        self.pop_type_params();

        if let ReturnContext::Declared { ty, .. } = context {
            if !ty.is_void() && !statements_guarantee_return(function.body) {
                return Err(AdmittedCheckError::new(
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
    ) -> Result<(), AdmittedCheckError> {
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
    ) -> Result<(), AdmittedCheckError> {
        self.push_type_params(class.type_params)?;
        for member in class.members {
            if let ExternClassMember::Method(method) = member {
                self.analyze_extern_defaults(method)?;
            }
        }
        self.pop_type_params();
        Ok(())
    }

    fn analyze_parameter_defaults(
        &mut self,
        params: &'ast [crate::ast::Param<'ast, 'src>],
        parameters: &[FunctionParameter<'src>],
    ) -> Result<(), AdmittedCheckError> {
        self.parameter_defaults += 1;
        let analyzed = self.analyze_parameter_default_expressions(params, parameters);
        self.parameter_defaults -= 1;
        analyzed
    }

    fn analyze_parameter_default_expressions(
        &mut self,
        params: &'ast [crate::ast::Param<'ast, 'src>],
        parameters: &[FunctionParameter<'src>],
    ) -> Result<(), AdmittedCheckError> {
        for (index, (param, parameter)) in params.iter().zip(parameters).enumerate() {
            let expected = &parameter.ty;
            let Some(expression) = &param.default else {
                continue;
            };
            let contextual = match expression {
                Expr {
                    kind: ExprKind::ArrayLiteral { .. },
                    ..
                } => expected_array_type(expected).unwrap_or(expected),
                _ => expected,
            };
            let actual = self.analyze_expr(expression, Some(contextual))?;
            self.require_assignable(expected, &actual, expression.span())?;
            let view = self.view();
            let mut invalid = None;
            crate::ast_walk::expression(expression, &mut |node| {
                if invalid.is_none() {
                    if let ExprKind::Ident(identifier) = &node.kind {
                        if let Some(symbol) = view.identifier_symbol(identifier.id) {
                            if params[index..]
                                .iter()
                                .any(|param| view.identifier_symbol(param.name.id) == Some(symbol))
                            {
                                invalid = Some(identifier.span);
                            }
                        }
                    }
                }
            });
            if let Some(span) = invalid {
                return Err(AdmittedCheckError::new(
                    span,
                    "parameter defaults can only reference earlier parameters",
                ));
            }
        }
        Ok(())
    }

    fn analyze_stmt(
        &mut self,
        statement: &'ast Stmt<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        match statement {
            Stmt::VarDecl(decl, ..) => self.analyze_var_decl(decl),
            Stmt::ArrayDestructure {
                bindings, value, ..
            } => {
                let actual = self.analyze_expr(value, None)?;
                let Type::Array(element) = actual else {
                    return Err(AdmittedCheckError::new(
                        value.span(),
                        format!("array destructuring requires an array, found `{actual}`"),
                    ));
                };
                for binding in *bindings {
                    match binding {
                        ArrayBinding::Hole(_) => {}
                        ArrayBinding::Name(name) => {
                            self.declare(*name, nullable_type(element.as_ref().clone()))?;
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
                    return Err(AdmittedCheckError::new(
                        value.span(),
                        format!("record destructuring requires a record, found `{actual}`"),
                    ));
                };
                let mut keys = AHashSet::default();
                for binding in *bindings {
                    if !keys.insert(decode_source_string(binding.key.name, binding.key.span)?) {
                        return Err(AdmittedCheckError::new(
                            binding.key.span,
                            format!("duplicate record binding key `{}`", binding.key.name),
                        ));
                    }
                    self.declare(binding.name, nullable_type(element.as_ref().clone()))?;
                }
                if let Some(rest) = rest {
                    self.declare(*rest, Type::Record(element.clone()))?;
                }
                Ok(())
            }
            Stmt::Expr(expr, ..) => {
                self.analyze_expr(expr, None)?;
                Ok(())
            }
            Stmt::Return { value, span, .. } => self.analyze_return(value.as_ref(), *span),
            Stmt::Throw { value, .. } => {
                let thrown = self.analyze_expr(value, None)?;
                // A thrown value can reach host code (R6).
                self.declarations.reflect(&thrown);
                if thrown == Type::Void {
                    return Err(AdmittedCheckError::new(
                        value.span(),
                        "cannot throw a `void` expression",
                    ));
                }
                Ok(())
            }
            Stmt::SuperCall { args, span, .. } => self.analyze_super_call(args, *span),
            Stmt::Yield {
                value,
                delegate,
                span,
                ..
            } => {
                self.analyze_yield(value, *delegate, *span)?;
                // Other code runs while the generator is suspended.
                self.invalidate_host_narrowings();
                self.invalidate_captured_narrowings();
                Ok(())
            }
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                // Definite assignment (R3): the catch and the finally may run
                // before any of the body's assignments.
                let before = self.unassigned.clone();
                self.push_scope()?;
                for statement in *body {
                    self.analyze_stmt(statement)?;
                }
                self.pop_scope();
                let body_leaves = body.last().is_some_and(statement_leaves);
                let after_body = std::mem::replace(&mut self.unassigned, before.clone());
                let mut catch_leaves = false;
                if let Some(clause) = catch {
                    self.push_scope()?;
                    if let Some(binding) = clause.binding {
                        let ty = if binding.ty.is_auto() {
                            Type::Dynamic
                        } else {
                            self.resolve_value_type(binding.ty, "catch binding")?
                        };
                        if !is_js_value(&ty) {
                            return Err(AdmittedCheckError::new(
                                binding.ty.span,
                                format!(
                                    "catch bindings must use `auto` or `JsValue`, found `{ty}`"
                                ),
                            ));
                        }
                        self.declare(binding.name, Type::Dynamic)?;
                    }
                    for statement in clause.body {
                        self.analyze_stmt(statement)?;
                    }
                    self.pop_scope();
                    catch_leaves = clause.body.last().is_some_and(statement_leaves);
                }
                let after_catch = std::mem::replace(&mut self.unassigned, before.clone());
                let mut joined = match (body_leaves, catch.is_some() && catch_leaves) {
                    (true, true) => before.clone(),
                    (true, false) if catch.is_some() => after_catch,
                    (false, true) => after_body,
                    _ => {
                        let mut joined = after_body;
                        if catch.is_some() {
                            for symbol in after_catch {
                                if !joined.contains(&symbol) {
                                    joined.push(symbol);
                                }
                            }
                        }
                        joined
                    }
                };
                if let Some(finally) = finally {
                    self.push_scope()?;
                    for statement in *finally {
                        self.analyze_stmt(statement)?;
                    }
                    self.pop_scope();
                    // The finally's own assignments are definite afterwards.
                    let after_finally = std::mem::take(&mut self.unassigned);
                    joined.retain(|symbol| {
                        !before.contains(symbol) || after_finally.contains(symbol)
                    });
                }
                self.unassigned = joined;
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
                // Definite assignment (R3): each branch starts here, and a
                // local stays unassigned after the `if` when some branch that
                // falls through leaves it so.
                let before = self.unassigned.clone();
                self.push_scope()?;
                self.apply_narrowing(then_narrowing.clone());
                self.analyze_stmt(then_branch)?;
                let then_survives = self.current_scope_preserves(&then_narrowing);
                self.pop_scope();
                let after_then = std::mem::replace(&mut self.unassigned, before.clone());
                let mut else_survives = else_branch.is_none() && !else_narrowing.is_empty();
                if let Some(else_branch) = else_branch {
                    self.push_scope()?;
                    self.apply_narrowing(else_narrowing.clone());
                    self.analyze_stmt(else_branch)?;
                    else_survives = self.current_scope_preserves(&else_narrowing);
                    self.pop_scope();
                }
                let after_else = std::mem::take(&mut self.unassigned);
                self.unassigned = match (
                    statement_leaves(then_branch),
                    else_branch.is_some_and(|branch| statement_leaves(branch)),
                ) {
                    (true, true) => before,
                    (true, false) => after_else,
                    (false, true) => after_then,
                    (false, false) => {
                        let mut joined = after_then;
                        for symbol in after_else {
                            if !joined.contains(&symbol) {
                                joined.push(symbol);
                            }
                        }
                        joined
                    }
                };
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
                // A loop body may not run: its assignments do not count (R3).
                let before = self.unassigned.clone();
                self.loop_depth += 1;
                self.push_scope()?;
                self.apply_narrowing(body_narrowing);
                self.analyze_stmt(body)?;
                self.pop_scope();
                self.loop_depth -= 1;
                self.unassigned = before;
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
                self.analyze_loop_body(body)?;
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
                    return Err(AdmittedCheckError::new(
                        key_type.span,
                        format!("for-in keys must have type `string`, found `{key_ty}`"),
                    ));
                }
                let object_ty = self.analyze_expr(object, None)?;
                if !is_js_value(&object_ty) && !matches!(object_ty, Type::Record(_)) {
                    return Err(AdmittedCheckError::new(
                        object.span(),
                        format!(
                            "for-in requires a `JsValue` or `Record<T>` object, found `{object_ty}`"
                        ),
                    ));
                }
                self.declare(*key, Type::String)?;
                self.loop_depth += 1;
                self.analyze_loop_body(body)?;
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(())
            }
            Stmt::ForOf {
                element_type,
                element,
                value,
                iterable,
                body,
                inline,
                ..
            } => {
                self.push_scope()?;
                let declared = self.resolve_value_type(*element_type, "for-of element")?;
                let iterable_type = self.analyze_expr(iterable, None)?;
                // `for (K k, V v of map)`: a map's entries, in insertion order,
                // as its key and its value (R14).
                match (&iterable_type, value) {
                    (Type::Map(key, entry_value), Some((value_type, value_name))) => {
                        let declared_value =
                            self.resolve_value_type(*value_type, "for-of map value")?;
                        self.require_assignable(&declared, key, element_type.span)?;
                        self.require_assignable(&declared_value, entry_value, value_type.span)?;
                        self.declare(*element, declared)?;
                        self.declare(*value_name, declared_value)?;
                        self.loop_depth += 1;
                        self.analyze_loop_body(body)?;
                        self.loop_depth -= 1;
                        self.pop_scope();
                        return Ok(());
                    }
                    (Type::Map(..), None) => {
                        return Err(AdmittedCheckError::new(
                            iterable.span(),
                            "a map iterates as a key and a value: `for (K k, V v of map)`",
                        ));
                    }
                    (_, Some((value_type, _))) => {
                        return Err(AdmittedCheckError::new(
                            value_type.span,
                            format!(
                                "only a map iterates as a key and a value, found `{iterable_type}`"
                            ),
                        ));
                    }
                    _ => {}
                }
                if *inline {
                    if iterable.const_list_literals().is_none() {
                        return Err(AdmittedCheckError::new(
                            iterable.span(),
                            "`inline for` requires a constant array literal of int, float, string, or bool values",
                        ));
                    }
                    if statement_contains_loop_control(body, false) {
                        return Err(AdmittedCheckError::new(
                            body.span(),
                            "`inline for` cannot contain `break` or `continue`",
                        ));
                    }
                }
                let actual = match iterable_type {
                    Type::Array(element) => *element,
                    Type::Generator(element) => *element,
                    // A set's elements, in insertion order (R14).
                    Type::Set(element) => *element,
                    // A `JsValue` iterates by JavaScript's iterator protocol
                    // (R14); its elements are `JsValue`s, never cast (Y1).
                    Type::Dynamic => Type::Dynamic,
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
                        return Err(AdmittedCheckError::new(
                            iterable.span(),
                            format!(
                                "for-of requires an array, a typed array, a Set<T>, a Generator<T> or a JsValue, found `{other}`"
                            ),
                        ));
                    }
                };
                self.require_assignable(&declared, &actual, element_type.span)?;
                self.declare(*element, declared)?;
                if *inline {
                    self.analyze_loop_body(body)?;
                } else {
                    self.loop_depth += 1;
                    self.analyze_loop_body(body)?;
                    self.loop_depth -= 1;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::Break(span, ..) | Stmt::Continue(span, ..) => {
                if self.loop_depth == 0 {
                    Err(AdmittedCheckError::new(
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
    ) -> Result<(), AdmittedCheckError> {
        self.require_value_arguments(
            args,
            "super calls do not support mutable-reference arguments",
        )?;
        let class = self
            .constructor_classes
            .last()
            .copied()
            .flatten()
            .ok_or_else(|| {
                AdmittedCheckError::new(
                    span,
                    "`super` is only valid in a derived class constructor",
                )
            })?;
        let class = &self.declarations.classes[class.index()];
        let base_ty = class.base.as_ref().ok_or_else(|| {
            AdmittedCheckError::new(span, "`super` is only valid in a derived class constructor")
        })?;
        let (base_declaration, base_args) =
            class_type_parts(base_ty).expect("derived class bases are class types");
        let base_name = base_declaration.name;
        let base = &self.declarations.classes[base_declaration.identity.index()];
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
                return Err(AdmittedCheckError::new(
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
    ) -> Result<(), AdmittedCheckError> {
        self.require_no_pending_reference(span)?;
        let expected = self
            .generator_contexts
            .last()
            .and_then(Clone::clone)
            .ok_or_else(|| {
                AdmittedCheckError::new(span, "`yield` is only valid inside a generator")
            })?;
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
                    return Err(AdmittedCheckError::new(
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

    fn analyze_var_decl(
        &mut self,
        decl: &'ast VarDecl<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        // A local may be declared without a value: every read must then be
        // definitely assigned (R3). A module's own bindings keep their
        // initializers until the initialization order proves reads (M6.5).
        if decl.initializer.is_none() && (self.callable_depth == 0 || decl.ty.is_auto()) {
            return Err(AdmittedCheckError::new(
                decl.span,
                if decl.ty.is_auto() {
                    "`auto` declarations require an initializer"
                } else {
                    "a module-level variable declaration requires an initializer"
                },
            ));
        }
        if decl.ty.is_auto() {
            let initializer = decl.initializer.as_ref().ok_or_else(|| {
                AdmittedCheckError::new(decl.span, "`auto` declarations require an initializer")
            })?;
            let inferred = self.analyze_expr(initializer, None)?;
            if inferred.is_void() {
                return Err(AdmittedCheckError::new(
                    initializer.span(),
                    "cannot infer a variable type from a void expression",
                ));
            }
            if inferred == Type::Null {
                return Err(AdmittedCheckError::new(
                    initializer.span(),
                    "cannot infer a variable type from `null`; add an explicit nullable type",
                ));
            }
            let ty = inferred;
            self.declare(decl.name, ty)?;
            return Ok(());
        }

        let declared = self.resolve_value_type(decl.ty, "variable")?;
        let binding_ty = declared.clone();
        let id = if let Some(id) = self.module_binding_declarations.get(&decl.name.id).copied() {
            id
        } else {
            self.declare(decl.name, binding_ty)?
        };
        let previous = self.initializing;
        self.initializing = Some((id, self.callable_depth));
        self.initializing_symbols.push(id);
        let analyzed = if let Some(initializer) = &decl.initializer {
            let actual = self.analyze_expr(initializer, Some(&declared));
            self.initializing = previous;
            let actual = actual?;
            self.require_assignable(&declared, &actual, initializer.span())
        } else {
            self.initializing = previous;
            Ok(())
        };
        self.initializing_symbols.pop();
        analyzed?;
        self.initialization.initialized.insert(id);
        if decl.initializer.is_none() {
            self.unassigned.push(id);
        }
        Ok(())
    }

    /// A loop body may not run, or runs again: its assignments do not
    /// count after it (R3).
    fn analyze_loop_body(
        &mut self,
        body: &'ast Stmt<'ast, 'src>,
    ) -> Result<(), AdmittedCheckError> {
        let before = self.unassigned.clone();
        let result = self.analyze_stmt(body);
        self.unassigned = before;
        result
    }

    /// A read of `symbol` must follow its assignment on every path (R3).
    fn require_assigned(
        &self,
        symbol: SymbolId,
        name: &str,
        span: Span,
    ) -> Result<(), AdmittedCheckError> {
        if self.unassigned.contains(&symbol) {
            return Err(AdmittedCheckError::new(
                span,
                format!("`{name}` is read before it is assigned on every path (R3)"),
            ));
        }
        Ok(())
    }

    /// `x = e` assigns `x` definitely unless the assignment may not run.
    fn assign_definitely(&mut self, target: &Expr<'ast, 'src>) {
        if self.conditional_assignments != 0 {
            return;
        }
        if let ExprKind::Ident(ident) = &target.kind {
            if let Some(&symbol) = self.facts.identifier_symbols.get(&ident.id) {
                self.unassigned.retain(|&candidate| candidate != symbol);
            }
        }
    }

    /// A compound assignment or update reads its target first.
    fn require_assigned_target(&self, target: &Expr<'ast, 'src>) -> Result<(), AdmittedCheckError> {
        if let ExprKind::Ident(ident) = &target.kind {
            if let Some(&symbol) = self.facts.identifier_symbols.get(&ident.id) {
                return self.require_assigned(symbol, ident.name, ident.span);
            }
        }
        Ok(())
    }

    /// Runs `analyze` where its assignments may not run.
    fn conditionally<T>(
        &mut self,
        analyze: impl FnOnce(&mut Self) -> Result<T, AdmittedCheckError>,
    ) -> Result<T, AdmittedCheckError> {
        self.conditional_assignments += 1;
        let result = analyze(self);
        self.conditional_assignments -= 1;
        result
    }

    fn analyze_return(
        &mut self,
        value: Option<&'ast Expr<'ast, 'src>>,
        span: Span,
    ) -> Result<(), AdmittedCheckError> {
        let expected = match self.return_contexts.last() {
            Some(ReturnContext::Declared { ty, .. }) => Some(ty.clone()),
            Some(ReturnContext::Inferred { ty, .. }) => ty.clone(),
            None => return Err(AdmittedCheckError::new(span, "`return` outside a function")),
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
                return Err(AdmittedCheckError::new(
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
                        return Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let actual = self.analyze_expr_value(expr, expected)?;
        // Contextual literals and return expressions do not all pass through
        // require_assignable. Preserve every implicit host-erased boundary,
        // including early-return expression paths, in this common owner.
        if expected.is_some_and(|expected| expected != &actual && contains_host_value(expected)) {
            self.declarations.reflect(&actual);
        }
        Ok(actual)
    }

    fn analyze_expr_value(
        &mut self,
        expr: &'ast Expr<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        self.facts.source_info[expr.id.index()] = SourceInfo {
            expression: Some(expr),
            resolution: ExpressionResolution::None,
            absent_default_argument: false,
        };
        let ty = match expr {
            Expr {
                kind: ExprKind::Int(value, span),
                ..
            } => {
                if i32::try_from(*value).is_err() {
                    return Err(AdmittedCheckError::new(
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
                    .get(&expr.id)
                    .copied()
                    .ok_or_else(|| {
                        AdmittedCheckError::new(
                            *span,
                            "dynamic imports require file-based compilation so their module interface can be resolved",
                        )
                    })?;
                Type::Task(Box::new(Type::ModuleNamespace(module)))
            }
            Expr {
                kind: ExprKind::Ident(ident),
                ..
            } if ident.name == "undefined" && self.builtin_namespace_is_unshadowed("undefined") => {
                // JavaScript's `undefined`, a `JsValue` (R12): `JS.undefined()`.
                self.resolve_dynamic(expr.id, BuiltinCall::JsUndefined);
                if self.declarations.source_contract.unified_absence()
                    && expected.is_some_and(|ty| matches!(ty, Type::Nullable(_) | Type::Null)) {
                    Type::Null
                } else { Type::Dynamic }
            }
            Expr {
                kind: ExprKind::Ident(ident),
                ..
            } => {
                let (id, ty) = self.analyze_binding_read(ident, expr.id)?;
                self.require_assigned(id, ident.name, ident.span)?;
                self.facts.source_info[expr.id.index()].resolution =
                    ExpressionResolution::Binding(id);
                ty
            }
            Expr {
                kind: ExprKind::ArrayLiteral { elements, span },
                ..
            } => {
                let expected_element = match expected {
                    Some(Type::Array(element)) => Some(element.as_ref()),
                    // Where a `JsValue` is expected, a literal is a JavaScript
                    // array of `JsValue`s, `[]` and `[1, v]` alike (R12).
                    Some(Type::Dynamic) => Some(&Type::Dynamic),
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
                                return Err(AdmittedCheckError::new(
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
                                AdmittedCheckError::new(
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
                    AdmittedCheckError::new(
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
                                return Err(AdmittedCheckError::new(
                                    entry.key.span,
                                    format!("duplicate object key `{}`", entry.key.name),
                                ));
                            }
                            self.analyze_dynamic_operand(&entry.value)?;
                        }
                        RecordElement::Spread { span, .. } => {
                            return Err(AdmittedCheckError::new(
                                *span,
                                "ordinary object spread is not supported yet",
                            ));
                        }
                    }
                }
                Type::Dynamic
            }
            Expr { kind: ExprKind::RecordLiteral { name, entries, span }, .. }
                if name.is_some() || expected.is_some_and(|ty| self.view().is_shape(ty)) => {
                    self.analyze_shape_literal(*name, entries, expected, *span)?
                }
            Expr { kind: ExprKind::StructLiteral { name, values, span }, .. }
                if values.is_empty() && self.shape_name(*name).is_some() => {
                    self.analyze_shape_literal(Some(*name), &[], expected, *span)?
                }
            Expr {
                kind: ExprKind::RecordLiteral { entries, span, .. },
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
                                    return Err(AdmittedCheckError::new(
                                        entry.key.span,
                                        format!("duplicate record key `{}`", entry.key.name),
                                    ));
                                }
                                self.analyze_expr(&entry.value, Some(&Type::Dynamic))?;
                            }
                            RecordElement::Spread { value, .. } => {
                                self.analyze_expr(value, Some(&Type::Dynamic))?;
                            }
                        }
                    }
                    Type::Dynamic
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
                                    return Err(AdmittedCheckError::new(
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
                                    return Err(AdmittedCheckError::new(
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
                                return Err(AdmittedCheckError::new(
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
                                    AdmittedCheckError::new(
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
                    AdmittedCheckError::new(
                        *span,
                        "cannot infer the value type of an empty record; add an explicit `Record<T>` type",
                    )
                })?;
                    Type::Record(Box::new(value_type))
                }
            }
            Expr {
                kind:
                    ExprKind::With {
                        value,
                        fields,
                        span,
                    },
                ..
            } => {
                let ty = self.analyze_expr(value, expected)?;
                if !matches!(ty, Type::Struct(_) | Type::StructInstance { .. }) {
                    return Err(AdmittedCheckError::new(
                        *span,
                        "`with` requires a struct value",
                    ));
                }
                let mut seen = AHashSet::default();
                for field in *fields {
                    self.budget
                        .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                    if !seen.insert(field.key.name) {
                        return Err(AdmittedCheckError::new(
                            field.key.span,
                            format!("duplicate update of field `{}`", field.key.name),
                        ));
                    }
                    let field_type =
                        self.analyze_member_type(ty.clone(), field.key, field.key.id, field.span)?;
                    let actual = self.analyze_expr(&field.value, Some(&field_type))?;
                    self.require_assignable(&field_type, &actual, field.value.span())?;
                    self.record_type(field.key.id, &field_type)?;
                }
                ty
            }
            Expr {
                kind: ExprKind::StructLiteral { name, values, span },
                ..
            } => {
                let info = self
                    .facts
                    .type_bindings
                    .get(name.name)
                    .and_then(|&identity| self.view().nominal_struct(identity))
                    .ok_or_else(|| {
                        AdmittedCheckError::new(
                            name.span,
                            format!("unknown struct `{}`", name.name),
                        )
                    })?;
                if values.len() != info.fields.len() {
                    return Err(AdmittedCheckError::new(
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
                    // A non-absent literal can initialize an optional value;
                    // its generic arguments come from the present member.
                    let expected = expected.map(|ty| match ty {
                        Type::Nullable(inner) => inner.as_ref(),
                        other => other,
                    });
                    let Some(Type::StructInstance {
                        declaration: expected_declaration,
                        args,
                    }) = expected
                    else {
                        return Err(AdmittedCheckError::new(
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
                        return Err(AdmittedCheckError::new(
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
                    let Some(identity) = self
                        .facts
                        .type_bindings
                        .get(class.name)
                        .copied()
                        .filter(|identity| identity.is_class())
                    else {
                        // `new C(a)` of a `JsValue` binding: `JS.construct(C, a)`.
                        return self.analyze_binding_construction(expr, class, type_args, args);
                    };
                    let info = &self.declarations.classes[identity.index()];
                    let declaration = info.declaration;
                    self.facts.source_info[expr.id.index()].resolution =
                        ExpressionResolution::NominalConstruction(identity);
                    if info.shape {
                        return Err(AdmittedCheckError::new(*span, "construct a shape with a keyed literal, not `new`"));
                    }
                    if info.external {
                        return Err(AdmittedCheckError::new(
                            *span,
                            format!("extern class `{}` cannot be constructed", class.name),
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
                        return Err(AdmittedCheckError::new(
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
                    let parameter_names = type_params
                        .iter()
                        .map(|parameter| parameter.identity)
                        .collect::<AHashSet<_>>();
                    let mut substitutions = AHashMap::default();
                    if !type_args.is_empty() {
                        let resolved = self.resolve_type_arguments(
                            class.name,
                            type_args,
                            type_params.len(),
                            *span,
                        )?;
                        substitutions.extend(
                            type_params
                                .iter()
                                .map(|parameter| parameter.identity)
                                .zip(resolved),
                        );
                    } else if let Some(Type::ClassInstance {
                        declaration: expected_declaration,
                        args: expected_args,
                    }) = expected
                    {
                        if *expected_declaration == declaration
                            && expected_args.len() == type_params.len()
                        {
                            substitutions.extend(
                                type_params
                                    .iter()
                                    .map(|parameter| parameter.identity)
                                    .zip(expected_args.iter().cloned()),
                            );
                        }
                    } else if type_params.is_empty() {
                        self.resolve_type_arguments(
                            class.name,
                            type_args,
                            type_params.len(),
                            *span,
                        )?;
                    }
                    let pattern = |index: usize, spread: bool| {
                        let fixed = constructor.as_ref().map_or(0, |sig| sig.fixed_params());
                        let parameter = &params[index.min(fixed)];
                        if parameter.rest && !spread {
                            let Type::Array(element) = &parameter.ty else {
                                unreachable!("checked rest")
                            };
                            element.as_ref()
                        } else {
                            &parameter.ty
                        }
                    };
                    let mut actual_args = Vec::with_capacity(args.len());
                    for (index, arg) in args.iter().enumerate() {
                        let pattern = pattern(index, arg.spread);
                        let optional = params[index.min(constructor.as_ref().unwrap().fixed_params())].optional
                            && self.declarations.source_contract.unified_absence();
                        let mut resolved = substitute_type(pattern, &substitutions);
                        if optional { resolved = Type::nullable(Box::new(resolved)); }
                        let expected = (!contains_type_parameter(&resolved, &parameter_names))
                            .then_some(&resolved);
                        let actual = if arg.spread {
                            if index < constructor.as_ref().unwrap().fixed_params() {
                                return Err(spread_refusal(arg.span));
                            }
                            self.analyze_expr(&arg.expression, expected)?
                        } else {
                            self.analyze_value_argument(arg, expected)?
                        };
                        let present = match &actual {
                            Type::Nullable(inner) if optional => inner.as_ref(),
                            _ => &actual,
                        };
                        if !optional || !matches!(present, Type::Null) {
                            infer_type_arguments(pattern, present, &parameter_names, &mut substitutions, arg.span)?;
                        }
                        let mut resolved = substitute_type(pattern, &substitutions);
                        if optional { resolved = Type::nullable(Box::new(resolved)); }
                        if !contains_type_parameter(&resolved, &parameter_names) {
                            self.require_assignable(&resolved, &actual, arg.span)?;
                        }
                        self.facts.source_info[arg.expression.id.index()].absent_default_argument =
                            optional && absence::optional(&actual);
                        actual_args.push(actual);
                    }
                    let resolved_args = type_params
                        .iter()
                        .map(|parameter| {
                            substitutions
                                .get(&parameter.identity)
                                .cloned()
                                .ok_or_else(|| {
                                    AdmittedCheckError::new(
                                        *span,
                                        format!("cannot infer type argument `{parameter}`"),
                                    )
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    // Inferred constructor arguments cross the same unknown
                    // generic-body boundary as explicitly written arguments.
                    for argument in &resolved_args {
                        self.declarations.reflect(argument);
                    }
                    for (index, (arg, actual)) in args.iter().zip(&actual_args).enumerate() {
                        let mut resolved = substitute_type(pattern(index, arg.spread), &substitutions);
                        if params[index.min(constructor.as_ref().unwrap().fixed_params())].optional
                            && self.declarations.source_contract.unified_absence() {
                            resolved = Type::nullable(Box::new(resolved));
                        }
                        self.require_assignable(&resolved, actual, arg.span)?;
                    }
                    if type_params.is_empty() {
                        Type::Class(declaration)
                    } else {
                        Type::ClassInstance {
                            declaration,
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
            } if self
                .facts
                .type_bindings
                .get(enum_name.name)
                .is_some_and(|identity| identity.is_enum())
                && self.builtin_namespace_is_unshadowed(enum_name.name) =>
            {
                let info =
                    &self.declarations.enums[self.facts.type_bindings[enum_name.name].index()];
                let declaration = info.declaration;
                let value = info.variants.get(property.name).copied().ok_or_else(|| {
                    AdmittedCheckError::new(
                        property.span,
                        format!(
                            "enum `{}` has no variant `{}`",
                            enum_name.name, property.name
                        ),
                    )
                })?;
                self.facts.enum_variant_values.insert(property.id, value);
                Type::Enum(declaration)
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
                    return Err(AdmittedCheckError::new(
                        object.span(),
                        format!(
                            "optional access requires a nullable receiver, found `{object_type}`"
                        ),
                    ));
                };
                let member = self.analyze_member_type(inner.into_inner(), *property, expr.id, *span)?;
                self.check_member_value(expr.id, *span)?;
                let id = self.declarations.types.intern(&member, self.budget)?;
                self.facts.optional_present_types.insert(expr.id, id);
                optional_result_type(member, *span)?
            }
            Expr {
                kind: ExprKind::Call { callee, args, span },
                ..
            } => {
                if let Some(result) = self.analyze_enum_from(callee, args, expr.id, *span)? {
                    result
                } else if let Some((builtin, result)) =
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
                    let contract = crate::catalog::builtin_call_contract(BuiltinCall::MathImul)
                        .expect("checked Math.imul contract");
                    if args.len() != contract.arity {
                        return Err(AdmittedCheckError::new(
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
                    let contract = crate::catalog::builtin_call_contract(BuiltinCall::Print)
                        .expect("checked print contract");
                    if args.len() != contract.arity {
                        return Err(AdmittedCheckError::new(
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
                kind: ExprKind::ArrowFunction { params, body, span },
                ..
            } => {
                validate_parameter_roles(params)?;
                self.analyze_arrow(params, body, expected)?
            }
            Expr {
                kind:
                    ExprKind::Unary {
                        op,
                        expr: operand_expr,
                        span,
                    },
                id,
            } => {
                let operand = self.analyze_expr(operand_expr, None)?;
                match op {
                    // `-v` with JavaScript's meaning (R12).
                    UnaryOp::Neg if is_js_value(&operand) => {
                        self.resolve_dynamic(*id, BuiltinCall::JsNegate);
                        Type::Dynamic
                    }
                    UnaryOp::Neg if operand.is_numeric() => operand,
                    UnaryOp::Not if operand == Type::Bool => Type::Bool,
                    UnaryOp::Neg => {
                        return Err(AdmittedCheckError::new(
                            *span,
                            format!("unary `-` requires a numeric operand, found `{operand}`"),
                        ));
                    }
                    UnaryOp::Not if is_js_value(&operand) => {
                        return Err(AdmittedCheckError::new(
                            *span,
                            "unary `!` requires a bool operand; a `JsValue`'s truthiness is explicit: `!bool(v)`",
                        ));
                    }
                    UnaryOp::Not => {
                        return Err(AdmittedCheckError::new(
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
                    return Err(AdmittedCheckError::new(
                        *span,
                        "`await` is only valid inside an async function or method",
                    ));
                }
                let expected_task = expected.map(|value| Type::Task(Box::new(value.clone())));
                let task_type = self.analyze_expr(task, expected_task.as_ref())?;
                let Type::Task(value) = task_type else {
                    return Err(AdmittedCheckError::new(
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
                if !self.class_guard(&value_type, &target_type, *span)? {
                    validate_type_guard(&value_type, &target_type, *span)?;
                }
                let id = self.declarations.types.intern(&target_type, self.budget)?;
                self.facts.type_check_types.insert(expr.id, id);
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
                    self.analyze_dynamic_key(index)?;
                } else {
                    let expected_index = index_key_type(&object_type).ok_or_else(|| {
                        AdmittedCheckError::new(
                            *span,
                            format!("cannot index a value of type `{object_type}`"),
                        )
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                }
                index_value_type(&object_type, false).ok_or_else(|| {
                    AdmittedCheckError::new(
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
                    return Err(AdmittedCheckError::new(
                        object.span(),
                        format!(
                            "optional indexing requires a nullable receiver, found `{object_type}`"
                        ),
                    ));
                };
                if is_js_value(&inner) {
                    let index_type = self.analyze_expr(index, None)?;
                    if !is_js_index_type(&index_type) {
                        return Err(AdmittedCheckError::new(
                            index.span(),
                            format!(
                                "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                            ),
                        ));
                    }
                } else {
                    let expected_index = index_key_type(&inner).ok_or_else(|| {
                        AdmittedCheckError::new(
                            *span,
                            format!("cannot index a value of type `{inner}`"),
                        )
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                }
                let element = index_value_type(&inner, false).ok_or_else(|| {
                    AdmittedCheckError::new(
                        *span,
                        format!("cannot index a value of type `{inner}`"),
                    )
                })?;
                let id = self.declarations.types.intern(&element, self.budget)?;
                self.facts.optional_present_types.insert(expr.id, id);
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
                let then_type =
                    self.conditionally(|this| this.analyze_expr(then_value, expected))?;
                self.pop_scope();
                self.push_scope()?;
                self.apply_narrowing(else_narrowing);
                let else_type =
                    self.conditionally(|this| this.analyze_expr(else_value, expected))?;
                self.pop_scope();
                common_type(&then_type, &else_type).ok_or_else(|| {
                    AdmittedCheckError::new(
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
            } => self.conditionally(|this| this.analyze_match(value, arms, expected, *span))?,
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
                    return Err(AdmittedCheckError::new(
                        target.span(),
                        "record entries currently support only direct `=` assignment; read with `??` before computing an update",
                    ));
                }
                let target_type = self.analyze_lvalue(target)?;
                if *op != AssignmentOp::Assign {
                    self.require_assigned_target(target)?;
                }
                if is_js_value(&target_type) && *op != AssignmentOp::Assign {
                    return self.analyze_dynamic_update(expr, *op, target, value, *span);
                }
                let value_expected = if *op == AssignmentOp::Nullish {
                    Some(nullish_present_type(&target_type).ok_or_else(|| {
                        AdmittedCheckError::new(
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
                    // The operation takes the target's type: it stores there.
                    self.facts
                        .resolved_operators
                        .insert(expr.id, ResolvedOperator::of(binary_op, &target_type));
                    target_type.clone()
                };
                self.invalidate_assigned_narrowing(target);
                if *op == AssignmentOp::Assign {
                    self.assign_definitely(target);
                }
                result_type
            }
            Expr {
                kind: ExprKind::Update {
                    target, op, span, ..
                },
                ..
            } => {
                if self.is_record_place(target)? {
                    return Err(AdmittedCheckError::new(
                        target.span(),
                        "record entries cannot be incremented directly because the key may be absent",
                    ));
                }
                let target_type = self.analyze_lvalue(target)?;
                self.require_assigned_target(target)?;
                if !target_type.is_numeric() {
                    return Err(AdmittedCheckError::new(
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
                let binary_op = match op {
                    UpdateOp::Increment => BinaryOp::Add,
                    UpdateOp::Decrement => BinaryOp::Sub,
                };
                self.facts
                    .resolved_operators
                    .insert(expr.id, ResolvedOperator::of(binary_op, &target_type));
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
                            return Err(AdmittedCheckError::new(
                                expression.span(),
                                format!("type `{ty}` cannot be interpolated into a string"),
                            ));
                        }
                    }
                }
                Type::String
            }
            Expr {
                kind:
                    ExprKind::Cast {
                        value,
                        target,
                        checked: true,
                        span,
                    },
                ..
            } => {
                // `v as? T`: a test, then the value as `T` or null (R12). A
                // `JsValue` narrows to the types `is` can test on it, and a
                // value to a class extending its type (R13).
                let source = self.analyze_expr(value, None)?;
                let target = self.resolve_value_type(*target, "`as?` target")?;
                if !self.class_guard(&source, &target, *span)? {
                    if !is_js_value_or_nullable_js_value(&source) {
                        return Err(AdmittedCheckError::new(
                            *span,
                            format!("`as?` narrows a `JsValue` or a class value, found `{source}`"),
                        ));
                    }
                    validate_type_guard(&source, &target, *span)?;
                }
                let id = self.declarations.types.intern(&target, self.budget)?;
                self.facts.type_check_types.insert(expr.id, id);
                nullable_type(target)
            }
            Expr {
                kind:
                    ExprKind::Cast {
                        value,
                        target,
                        checked: false,
                        span,
                    },
                ..
            } => {
                // `v as T`: a trusted view, `JS.assume(v)` (R12). `x as
                // JsValue` views any value as the dynamic type, the explicit
                // spelling of the widening a `JsValue` parameter makes.
                let source = self.analyze_expr(value, Some(&Type::Dynamic))?;
                let target = self.resolve_value_type(*target, "`as` target")?;
                if target.is_void() {
                    return Err(AdmittedCheckError::new(
                        *span,
                        "`as` cannot view a value as `void`",
                    ));
                }
                if source.is_void() {
                    return Err(AdmittedCheckError::new(
                        *span,
                        "`as` cannot view `void` as a value",
                    ));
                }
                if is_js_value_or_nullable_js_value(&target) {
                    // An explicit view as `JsValue` crosses to the host (R6).
                    self.declarations.reflect(&source);
                }
                if is_js_value_or_nullable_js_value(&source) {
                    self.declarations.reflect(&target);
                }
                if !is_js_value(&target) && !is_js_value_or_nullable_js_value(&source) {
                    return Err(AdmittedCheckError::new(
                        *span,
                        format!(
                            "`as` views a `JsValue` as a type, found `{source}`; view it as `JsValue` first: `x as JsValue as {target}`"
                        ),
                    ));
                }
                self.resolve_dynamic(expr.id, BuiltinCall::JsAssume);
                target
            }
            Expr {
                kind:
                    ExprKind::Convert {
                        target,
                        value,
                        span,
                    },
                ..
            } => {
                // `bool(v)`: JavaScript's truthiness, the `truthy()` intrinsic
                // on a `JsValue` (R12; a condition stays a `bool`).
                if matches!(target.kind, TypeKind::Bool) {
                    self.analyze_test_operand(value)?;
                    self.facts.source_info[expr.id.index()].resolution =
                        ExpressionResolution::Primitive(
                            crate::primitive::ResolvedIntrinsic::Method(
                                crate::primitive::Intrinsic::JsTruthy,
                            ),
                        );
                    self.record_type(expr.id, &Type::Bool)?;
                    return Ok(Type::Bool);
                }
                // `string(v)` is `JS.string(v)`; `float(v)` and `number(v)` are
                // `JS.number(v)` (R12).
                let (builtin, result) = match target.kind {
                    TypeKind::String => (BuiltinCall::JsString, Type::String),
                    TypeKind::Float => (BuiltinCall::JsNumber, Type::Float),
                    _ => {
                        return Err(AdmittedCheckError::new(
                            *span,
                            "a conversion is `string(v)`, `float(v)`, `number(v)` or `bool(v)`",
                        ))
                    }
                };
                self.analyze_test_operand(value)?;
                self.resolve_dynamic(expr.id, builtin);
                result
            }
            Expr {
                kind: ExprKind::Construct { callee, args, .. },
                ..
            } => {
                // `new f(a)` of a value: `JS.construct(f, a)` (R12).
                let constructor = self.analyze_expr(callee, None)?;
                if !is_js_value(&constructor) {
                    return Err(AdmittedCheckError::new(
                        callee.span(),
                        format!(
                            "`new` of a value needs a `JsValue` constructor, found `{constructor}`"
                        ),
                    ));
                }
                self.analyze_dynamic_arguments(args)?;
                self.resolve_dynamic(expr.id, BuiltinCall::JsConstruct);
                Type::Dynamic
            }
            Expr {
                kind: ExprKind::DynamicBinary { op, lhs, rhs, .. },
                ..
            } => {
                // `===`, `!==` and `instanceof` are tests, which `unknown`
                // takes; `in` reads a property, which it does not (R12).
                if matches!(op, DynamicBinaryOp::In) {
                    self.analyze_dynamic_operand(lhs)?;
                    self.analyze_dynamic_operand(rhs)?;
                } else {
                    self.analyze_test_operand(lhs)?;
                    self.analyze_test_operand(rhs)?;
                }
                self.resolve_dynamic(
                    expr.id,
                    match op {
                        DynamicBinaryOp::StrictEq => BuiltinCall::JsStrictEqual,
                        DynamicBinaryOp::StrictNotEq => BuiltinCall::JsStrictNotEqual,
                        DynamicBinaryOp::In => BuiltinCall::JsIn,
                        DynamicBinaryOp::InstanceOf => BuiltinCall::JsInstanceOf,
                    },
                );
                Type::Bool
            }
            Expr {
                kind:
                    ExprKind::DynamicUnary {
                        op: DynamicUnaryOp::TypeOf,
                        expr: value,
                        ..
                    },
                ..
            } => {
                self.analyze_test_operand(value)?;
                self.resolve_dynamic(expr.id, BuiltinCall::JsTypeOf);
                Type::String
            }
            Expr {
                kind:
                    ExprKind::DynamicUnary {
                        op: DynamicUnaryOp::Delete,
                        expr: target,
                        span,
                    },
                ..
            } => {
                // `delete v.k`, `delete v[k]`: `JS.delete(v, k)` (R12). The
                // property is an operand, not read.
                let (ExprKind::Member { object, .. } | ExprKind::Index { object, .. }) =
                    &target.kind
                else {
                    return Err(AdmittedCheckError::new(
                        *span,
                        "`delete` takes a `JsValue`'s property, `v.k` or `v[k]`",
                    ));
                };
                let object_type = self.analyze_expr(object, None)?;
                if !is_js_value(&object_type) {
                    return Err(AdmittedCheckError::new(
                        object.span(),
                        format!("`delete` takes a `JsValue`'s property, found `{object_type}`"),
                    ));
                }
                if let ExprKind::Index { index, .. } = &target.kind {
                    self.analyze_dynamic_key(index)?;
                }
                self.facts.source_info[target.id.index()] = SourceInfo {
                    expression: Some(target),
                    resolution: ExpressionResolution::None,
                    absent_default_argument: false,
                };
                self.record_type(target.id, &Type::Dynamic)?;
                self.resolve_dynamic(expr.id, BuiltinCall::JsDelete);
                Type::Void
            }
        };
        // A call, a construction or a suspension runs code that can assign a
        // host binding: its narrowings end here (R1).
        if matches!(
            expr.kind,
            ExprKind::Call { .. }
                | ExprKind::New { .. }
                | ExprKind::Construct { .. }
                | ExprKind::Await { .. }
        ) {
            self.invalidate_host_narrowings();
            if matches!(expr.kind, ExprKind::Await { .. }) {
                self.invalidate_captured_narrowings();
            }
        }

        self.record_type(expr.id, &ty.clone())?;
        Ok(ty)
    }

    /// Ends every narrowing of a host binding: code that ran may have
    /// assigned it.
    fn invalidate_host_narrowings(&mut self) {
        let symbols = &self.declarations.symbols;
        for scope in &mut self.narrowings {
            scope.retain(|symbol, _| !symbols[symbol.0 as usize].is_foreign());
        }
    }

    /// Binary trees are common even in flat source such as `a + b + c`.
    /// Keep their continuations on the heap rather than retaining the large
    /// expression-checker frame for every operator. Leaves still use the same
    /// contextual checker; no expression is prechecked in a different scope.
    fn analyze_binary_expression(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let original_scope_depth = self.scopes.len();
        let original_conditional = self.conditional_assignments;
        let mut pending = Vec::new();
        let result = (|| {
            let mut next = expression;
            let mut next_expected = expected.cloned();
            'visit: loop {
                self.budget
                    .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
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
                    absent_default_argument: false,
                    };
                    next = lhs;
                    continue;
                }

                let mut ty = self.analyze_expr(next, next_expected.as_ref())?;
                let mut narrowing = NarrowingInput::leaf(next);
                loop {
                    self.budget
                        .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
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
                                let (when_true, when_false) =
                                    self.narrowing_from_input(narrowing)?;
                                self.push_scope()?;
                                self.apply_narrowing(if *op == BinaryOp::And {
                                    when_true
                                } else {
                                    when_false
                                });
                                // `v || x` on a `JsValue` yields an operand.
                                Some(if is_js_value(&ty) {
                                    Type::Dynamic
                                } else {
                                    Type::Bool
                                })
                            } else if *op == BinaryOp::Nullish && is_js_value(&ty) {
                                // `v ?? x` on a `JsValue` yields either side.
                                Some(Type::Dynamic)
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
                                    conditional: matches!(
                                        op,
                                        BinaryOp::And | BinaryOp::Or | BinaryOp::Nullish
                                    ),
                                },
                            )?;
                            if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Nullish) {
                                self.conditional_assignments += 1;
                            }
                            next = rhs;
                            continue 'visit;
                        }
                        Some(BinaryContinuation::Right {
                            expression,
                            left,
                            left_narrowing,
                            narrowed_scope,
                            conditional,
                        }) => {
                            if narrowed_scope {
                                self.pop_scope();
                            }
                            if conditional {
                                self.conditional_assignments -= 1;
                            }
                            let ExprKind::Binary { op, span, .. } = &expression.kind else {
                                unreachable!("binary continuation owns a binary expression")
                            };
                            ty = match dynamic_binary(*op, &left, &ty) {
                                Some((builtin, result)) => {
                                    self.resolve_dynamic(expression.id, builtin);
                                    result
                                }
                                None => {
                                    let result = self.analyze_binary(*op, &left, &ty, *span)?;
                                    self.facts
                                        .resolved_operators
                                        .insert(expression.id, ResolvedOperator::of(*op, &result));
                                    result
                                }
                            };
                            narrowing = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                                left_narrowing.join(narrowing, expression, *op)
                            } else {
                                NarrowingInput::leaf(expression)
                            };
                            self.record_type(expression.id, &ty.clone())?;
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
        self.conditional_assignments = original_conditional;
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        self.analyze_place(expression, PlaceIntent::Write)
    }

    fn analyze_place(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
        intent: PlaceIntent,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        self.facts.source_info[expression.id.index()] = SourceInfo {
            expression: Some(expression),
            resolution: ExpressionResolution::None,
            absent_default_argument: false,
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
                    return Err(AdmittedCheckError::new(
                        ident.span,
                        "mutable-reference arguments require lexical storage, not a foreign binding",
                    ));
                }
                self.declarations.assigned_symbols.insert(id);
                self.record_read_initialization(expression.id, id);
                if intent == PlaceIntent::MutableArgument {
                    for narrowing in &mut self.narrowings {
                        narrowing.remove(&id);
                        narrowing
                            .retain(|symbol, _| !self.reference_parameters.contains_key(symbol));
                    }
                }
                self.record_identifier(ident.id, id);
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
                        return Err(AdmittedCheckError::new(
                            *span,
                            "mutable-reference field arguments require a nonnullable, nongeneric value-struct path rooted in a lexical cell",
                        ));
                    }
                    ty
                } else {
                    self.analyze_expr(object, None)?
                };
                if matches!(&object_type, Type::Union(_)) {
                    return Err(AdmittedCheckError::new(
                        *span,
                        format!(
                            "cannot assign through member `{}` on union `{object_type}`",
                            property.name
                        ),
                    ));
                }
                let ty = match object_type {
                    Type::Record(value) => *value,
                    // `v.k = x`: a property write with JavaScript's meaning.
                    Type::Dynamic
                        if intent != PlaceIntent::MutableArgument
                            && !typed_js_member(property.name) =>
                    {
                        Type::Dynamic
                    }
                    other => self.analyze_member_type(other, *property, expression.id, *span)?,
                };
                if let ExpressionResolution::NominalMember(member) = self.facts.source_info[expression.id.index()].resolution {
                    if let Some(NominalMember::Field { owner, field }) = self.view().nominal_member(member) {
                        if self.view().nominal_class(owner).is_some_and(|class| class.discriminant.is_some_and(|(slot,_)| slot == field.index)) {
                            return Err(AdmittedCheckError::new(*span, "a shape discriminant is immutable"));
                        }
                    }
                }
                ty
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
                    return Err(AdmittedCheckError::new(
                        *span,
                        "mutable-reference arguments do not yet support indexed or host-backed locations",
                    ));
                }
                let object_type = self.analyze_expr(object, None)?;
                if is_js_value(&object_type) {
                    self.analyze_dynamic_key(index)?;
                    Type::Dynamic
                } else {
                    let expected_index = index_key_type(&object_type).ok_or_else(|| {
                        AdmittedCheckError::new(
                            *span,
                            format!("cannot assign through an index on `{object_type}`"),
                        )
                    })?;
                    let index_type = self.analyze_expr(index, Some(&expected_index))?;
                    self.require_assignable(&expected_index, &index_type, index.span())?;
                    index_value_type(&object_type, true).ok_or_else(|| {
                        AdmittedCheckError::new(
                            *span,
                            format!("cannot assign through an index on `{object_type}`"),
                        )
                    })?
                }
            }
            _ => {
                return Err(AdmittedCheckError::new(
                    expression.span(),
                    if intent == PlaceIntent::MutableArgument {
                        "mutable-reference argument must name a writable lexical place"
                    } else {
                        "expression is not an assignable location"
                    },
                ));
            }
        };
        self.record_type(expression.id, &ty.clone())?;
        Ok(ty)
    }

    fn is_record_place(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
    ) -> Result<bool, AdmittedCheckError> {
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let object_type = self.analyze_expr(object, None)?;
        if is_js_value(&object_type) && !typed_js_member(property.name) {
            // `v.k`: a property read with JavaScript's meaning (R12).
            return Ok(Type::Dynamic);
        }
        let member = self.analyze_member_type(object_type, property, id, span)?;
        self.check_member_value(id, span)?;
        Ok(member)
    }

    /// Receiver calls have their own checking path. A standalone primitive
    /// method read cannot silently lose the receiver its checked signature
    /// requires. Nominal callable fields and explicit dynamic host reads keep
    /// their separate value semantics.
    fn check_member_value(&self, id: SourceNodeId, span: Span) -> Result<(), AdmittedCheckError> {
        if matches!(self.view().resolved_member(id), Some(NominalMember::Method { method, .. })
            if method.dispatch != crate::ast::MethodDispatch::Static) {
            return Err(AdmittedCheckError::new(span, "a virtual method must be called through its receiver; use a closure to pass it as a value"));
        }
        if matches!(
            self.facts.source_info[id.index()].resolution,
            ExpressionResolution::Primitive(crate::primitive::ResolvedIntrinsic::Method(_))
                | ExpressionResolution::Enum { operation: crate::primitive::EnumOperation::Has, .. }
        ) {
            return Err(AdmittedCheckError::new(span,
                "a primitive method must be called through its receiver; use a closure to pass it as a value"));
        }
        Ok(())
    }

    /// A read of the binding `ident` at `node`: its symbol and its type,
    /// narrowed where the read is.
    fn analyze_binding_read(
        &mut self,
        ident: &Ident<'src>,
        node: SourceNodeId,
    ) -> Result<(SymbolId, Type<'src>), AdmittedCheckError> {
        let (id, declared) = {
            let symbol = self.resolve(ident)?;
            (symbol.id, symbol.ty.clone())
        };
        if let Some((initializing, depth)) = self.initializing {
            if id == initializing && self.callable_depth == depth {
                return Err(AdmittedCheckError::new(
                    ident.span,
                    format!(
                        "cannot read `{}` in its own initializer; nest the reference in a function",
                        ident.name
                    ),
                ));
            }
        }
        if let Some(binding) = self.initialization.bindings.get(&id) {
            let from_owner = self.module == Some(binding.owner);
            if from_owner && ident.span.start < binding.declaration.start {
                return Err(AdmittedCheckError::new(
                    ident.span,
                    format!("cannot read `{}` before its declaration", ident.name),
                ));
            }
            if !self.initialization.initialized.contains(&id) && self.callable_depth == 0 {
                return Err(AdmittedCheckError::new(
                    ident.span,
                    format!(
                        "cannot eagerly read module binding `{}` before it is initialized",
                        ident.name
                    ),
                ));
            }
        }
        self.record_identifier(ident.id, id);
        self.record_read_initialization(node, id);
        Ok((id, self.narrowed_type(id).cloned().unwrap_or(declared)))
    }

    /// Records that `id` is the dynamic operation `builtin` (R12). Its syntax
    /// lowers exactly as its `JS.*` spelling does, so each dynamic operation
    /// has one IR form.
    fn resolve_dynamic(&mut self, id: SourceNodeId, builtin: BuiltinCall) {
        self.facts.source_info[id.index()].resolution = ExpressionResolution::Dynamic(builtin);
    }

    /// An operand of a dynamic operation: any value, viewed as a `JsValue`,
    /// as a `JS.*` builtin's argument is.
    fn analyze_dynamic_operand(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let ty = self.analyze_expr(expression, Some(&Type::Dynamic))?;
        if is_unknown(&ty) {
            return Err(AdmittedCheckError::new(
                expression.span(),
                "an `unknown` is narrowed before other operations: `v is T`, `v as? T` or `v as T` (R12)",
            ));
        }
        self.require_assignable(&Type::Dynamic, &ty, expression.span())?;
        Ok(ty)
    }

    /// An operand of a test or a conversion, which an `unknown` takes too
    /// (R12): `===`, `!==`, `instanceof`, `typeof`, `bool(v)`, `string(v)`,
    /// `float(v)`.
    fn analyze_test_operand(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let ty = self.analyze_expr(expression, Some(&Type::Dynamic))?;
        if !is_unknown(&ty) {
            self.require_assignable(&Type::Dynamic, &ty, expression.span())?;
        }
        Ok(ty)
    }

    fn analyze_dynamic_arguments(
        &mut self,
        args: &'ast [Argument<'ast, 'src>],
    ) -> Result<(), AdmittedCheckError> {
        for argument in args {
            if argument.spread {
                // `...xs` into a JavaScript call: an array's elements, or a
                // `JsValue`'s by the iterator protocol (R7).
                let spread = self.analyze_expr(&argument.expression, None)?;
                if !matches!(spread, Type::Array(_)) && !is_js_value(&spread) {
                    return Err(AdmittedCheckError::new(
                        argument.span,
                        format!("a spread argument is an array or a `JsValue`, found `{spread}`"),
                    ));
                }
                self.declarations.reflect(&spread);
                continue;
            }
            let actual = self.analyze_value_argument(argument, Some(&Type::Dynamic))?;
            self.require_assignable(&Type::Dynamic, &actual, argument.span)?;
        }
        Ok(())
    }

    /// `v.m(a)` on a `JsValue`: a method call with JavaScript's meaning (R12).
    /// `v.call(t, a)` and `v.apply(t, a)` are `JS.call(v, t, a)` and
    /// `JS.apply(v, t, a)`, which assume the standard `Function.prototype`
    /// methods, as those builtins always have.
    fn analyze_dynamic_method_call(
        &mut self,
        call_node: SourceNodeId,
        member: &'ast Expr<'ast, 'src>,
        property: Ident<'src>,
        args: &'ast [Argument<'ast, 'src>],
    ) -> Result<Type<'src>, AdmittedCheckError> {
        self.analyze_dynamic_arguments(args)?;
        match (property.name, args.len()) {
            ("call", 1..) => self.resolve_dynamic(call_node, BuiltinCall::JsCall),
            ("apply", 2) => self.resolve_dynamic(call_node, BuiltinCall::JsApply),
            // A method call: the method is read before the arguments are
            // evaluated, as a reference call models it.
            _ => {}
        }
        self.record_type(member.id, &Type::Dynamic)?;
        Ok(Type::Dynamic)
    }

    /// `new C(a)` where `C` is a `JsValue` binding rather than a class:
    /// `JS.construct(C, a)` (R12).
    fn analyze_binding_construction(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
        class: &Ident<'src>,
        type_args: &[TypeRef<'ast, 'src>],
        args: &'ast [Argument<'ast, 'src>],
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let unknown =
            || AdmittedCheckError::new(class.span, format!("unknown class `{}`", class.name));
        if self.resolve(class).is_err() {
            return Err(unknown());
        }
        let (_, ty) = self.analyze_binding_read(class, expression.id)?;
        if !is_js_value(&ty) {
            return Err(unknown());
        }
        if let Some(argument) = type_args.first() {
            return Err(AdmittedCheckError::new(
                argument.span,
                "a `JsValue` constructor takes no type arguments",
            ));
        }
        self.analyze_dynamic_arguments(args)?;
        self.resolve_dynamic(expression.id, BuiltinCall::JsConstruct);
        self.record_type(expression.id, &Type::Dynamic)?;
        Ok(Type::Dynamic)
    }

    /// `v += x` on a `JsValue` place is `+` with JavaScript's meaning,
    /// `JS.add` (R12); the place is read and written once each.
    fn analyze_dynamic_update(
        &mut self,
        expression: &'ast Expr<'ast, 'src>,
        op: AssignmentOp,
        target: &'ast Expr<'ast, 'src>,
        value: &'ast Expr<'ast, 'src>,
        span: Span,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        if op != AssignmentOp::Add {
            return Err(AdmittedCheckError::new(
                span,
                "a `JsValue` place takes `=` and `+=`; compute other updates explicitly",
            ));
        }
        // A `JsValue` key would be converted twice, where JavaScript converts
        // it once.
        if let ExprKind::Index { index, .. } = &target.kind {
            if self
                .view()
                .expression_type(index.id)
                .is_some_and(is_js_value)
            {
                return Err(AdmittedCheckError::new(
                    index.span(),
                    "`+=` on a `JsValue` property needs a `string` or numeric key",
                ));
            }
        }
        self.analyze_dynamic_operand(value)?;
        self.resolve_dynamic(expression.id, BuiltinCall::JsAdd);
        self.invalidate_assigned_narrowing(target);
        self.record_type(expression.id, &Type::Dynamic)?;
        Ok(Type::Dynamic)
    }

    /// The key of `v[k]` on a `JsValue`: a number, a `string` or a `JsValue`.
    fn analyze_dynamic_key(
        &mut self,
        index: &'ast Expr<'ast, 'src>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let index_type = self.analyze_expr(index, None)?;
        if !is_js_index_type(&index_type) {
            return Err(AdmittedCheckError::new(
                index.span(),
                format!(
                    "a `JsValue` index must be numeric, `string`, or `JsValue`, found `{index_type}`"
                ),
            ));
        }
        Ok(index_type)
    }

    fn analyze_match(
        &mut self,
        value: &'ast Expr<'ast, 'src>,
        arms: &'ast [crate::ast::MatchArm<'ast, 'src>],
        expected: Option<&Type<'src>>,
        span: Span,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let value_type = self.analyze_expr(value, None)?;
        if arms.iter().any(|arm| matches!(arm.pattern, MatchPattern::Payload { .. })) {
            return self.analyze_payload_match(&value_type, arms, expected, span);
        }
        if !matches!(
            value_type,
            Type::Enum(_) | Type::Int | Type::String | Type::Bool
        ) {
            return Err(AdmittedCheckError::new(
                value.span(),
                format!("match requires an enum, int, string, or bool value, found `{value_type}`"),
            ));
        }
        let variants = match value_type {
            Type::Enum(declaration) => Some((
                declaration.name,
                self.declarations.enums[declaration.identity.index()]
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
                return Err(AdmittedCheckError::new(
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
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("enum pattern cannot match `{value_type}`"),
                        ));
                    };
                    // The scrutinee's type fixes the enum; the pattern must
                    // name that identity in this scope, or spell its declared
                    // name where this scope does not bind it.
                    let names_scrutinee =
                        match (self.facts.type_bindings.get(pattern_enum.name), &value_type) {
                            (Some(identity), Type::Enum(declaration)) => {
                                *identity == declaration.identity
                            }
                            (None, _) => pattern_enum.name == *enum_name,
                            _ => false,
                        };
                    if !names_scrutinee {
                        return Err(AdmittedCheckError::new(
                            pattern_enum.span,
                            format!(
                                "match pattern uses enum `{}`, expected `{enum_name}`",
                                pattern_enum.name
                            ),
                        ));
                    }
                    let discriminant = variants.get(variant.name).copied().ok_or_else(|| {
                        AdmittedCheckError::new(
                            variant.span,
                            format!("enum `{enum_name}` has no variant `{}`", variant.name),
                        )
                    })?;
                    let key = format!("enum:{enum_name}:{}", variant.name);
                    if !covered.insert(key) {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("duplicate match arm for `{enum_name}.{}`", variant.name),
                        ));
                    }
                    self.facts
                        .enum_variant_values
                        .insert(variant.id, discriminant);
                }
                MatchPattern::Int(value, pattern_span) => {
                    if value_type != Type::Int {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("integer pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("int:{value}")) {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::String(value, pattern_span) => {
                    if value_type != Type::String {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("string pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("string:{value}")) {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::Bool(value, pattern_span) => {
                    if value_type != Type::Bool {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("boolean pattern cannot match `{value_type}`"),
                        ));
                    }
                    if !covered.insert(format!("bool:{value}")) {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            format!("duplicate match arm for `{value}`"),
                        ));
                    }
                }
                MatchPattern::Wildcard(pattern_span) => {
                    if wildcard || index + 1 != arms.len() {
                        return Err(AdmittedCheckError::new(
                            pattern_span,
                            "the `_` match arm must appear once and last",
                        ));
                    }
                    wildcard = true;
                }
                MatchPattern::Payload { .. } => unreachable!("payload matches have one checked owner"),
            }
            let arm_type = self.analyze_expr(&arm.value, expected)?;
            result = Some(match result {
                Some(previous) => common_type(&previous, &arm_type).ok_or_else(|| {
                    AdmittedCheckError::new(
                        arm.span,
                        format!("match arm has type `{arm_type}`, incompatible with `{previous}`"),
                    )
                })?,
                None => arm_type,
            });
        }
        if !wildcard {
            match (&value_type, variants) {
                (Type::Enum(declaration), _) if declaration.is_flags() => {
                    return Err(AdmittedCheckError::new(span, "a flag-set match requires a final `_` arm for combinations and the empty set"));
                }
                (Type::Enum(_), Some((enum_name, variants))) if covered.len() != variants.len() => {
                    let missing = variants
                        .keys()
                        .filter(|variant| !covered.contains(&format!("enum:{enum_name}:{variant}")))
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(AdmittedCheckError::new(
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
                        return Err(AdmittedCheckError::new(
                            span,
                            format!(
                                "non-exhaustive match on `bool`; missing {}",
                                missing.join(", ")
                            ),
                        ));
                    }
                }
                (Type::Int | Type::String, _) => {
                    return Err(AdmittedCheckError::new(
                        span,
                        format!("match on `{value_type}` requires a final `_` arm"),
                    ));
                }
                _ => {}
            }
        }
        result.ok_or_else(|| AdmittedCheckError::new(span, "match expression has no arms"))
    }

    fn analyze_member_type(
        &mut self,
        object_type: Type<'src>,
        property: Ident<'src>,
        id: SourceNodeId,
        span: Span,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        // The checked receiver owns resolution. Re-analysis replaces the fact;
        // later consumers do not recover it from a property spelling.
        let intrinsic = crate::primitive::resolve_member(&object_type, property.name).map(|operation| {
            use crate::primitive::{Intrinsic, ResolvedIntrinsic};
            if operation == ResolvedIntrinsic::Method(Intrinsic::StringCharCodeAt)
                && self.declarations.source_contract.char_code_at == crate::config::CharCodeAtContract::Number {
                ResolvedIntrinsic::Method(Intrinsic::StringCharCodeAtNumber)
            } else { operation }
        });
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
            Type::Enum(declaration) => self.enum_member(declaration, property, id),
            Type::Intersection(_) => {
                let fields = self.shape_fields(&object_type, span)?;
                let (_, field) = fields.into_iter().find(|(_, field)| field.name == property.name)
                    .ok_or_else(|| AdmittedCheckError::new(property.span, format!("shape has no field `{}`", property.name)))?;
                self.facts.source_info[id.index()].resolution = ExpressionResolution::NominalMember(field.member);
                Ok(field.ty)
            }
            Type::Struct(declaration) => {
                let name = declaration.name;
                let field = self
                    .declarations
                    .structs
                    .get(declaration.identity.index())
                    .and_then(|info| info.fields.get(property.name))
                    .ok_or_else(|| {
                        AdmittedCheckError::new(
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
                    AdmittedCheckError::new(
                        property.span,
                        format!("struct `{name}` has no field `{}`", property.name),
                    )
                })?;
                self.facts.source_info[id.index()].resolution =
                    ExpressionResolution::NominalMember(field.member);
                Ok(substitute_type(&field.ty, &substitutions))
            }
            Type::Class(declaration) => {
                let name = declaration.name;
                let class = &self.declarations.classes[declaration.identity.index()];
                if let Some(field) = class.fields.get(property.name) {
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(field.member);
                    return Ok(field.ty.clone());
                }
                if let Some(method) = class.methods.get(property.name) {
                    if method.dispatch != crate::ast::MethodDispatch::Static {
                        self.declarations.tested_classes.insert(declaration.identity);
                    }
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(method.member);
                    return Ok(method_callable_type(method, &AHashMap::default()));
                }
                Err(AdmittedCheckError::new(
                    property.span,
                    format!("class `{name}` has no member `{}`", property.name),
                ))
            }
            Type::ClassInstance { declaration, args } => {
                let name = declaration.name;
                let class = &self.declarations.classes[declaration.identity.index()];
                let substitutions = substitutions_for(&class.type_params, &args);
                if let Some(field) = class.fields.get(property.name) {
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(field.member);
                    return Ok(substitute_type(&field.ty, &substitutions));
                }
                if let Some(method) = class.methods.get(property.name) {
                    if method.dispatch != crate::ast::MethodDispatch::Static {
                        self.declarations.tested_classes.insert(declaration.identity);
                    }
                    self.facts.source_info[id.index()].resolution =
                        ExpressionResolution::NominalMember(method.member);
                    return Ok(method_callable_type(method, &substitutions));
                }
                Err(AdmittedCheckError::new(
                    property.span,
                    format!("class `{name}` has no member `{}`", property.name),
                ))
            }
            Type::Array(_) | Type::String if property.name == "length" => Ok(Type::Int),
            Type::Dynamic => match property.name {
                "length" => Ok(Type::Float),
                "message" | "specifier" => Ok(Type::nullable(Box::new(Type::String))),
                "truthy" | "isArray" | "isObject" => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: Vec::new(),
                        return_type: Box::new(Type::Bool),
                    })))
                }
                _ => Err(AdmittedCheckError::new(
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
                let symbol = self
                    .facts
                    .dynamic_export_symbols
                    .get(&(module, property.name))
                    .and_then(|symbol| self.declarations.symbols.get(symbol.0 as usize))
                    .ok_or_else(|| {
                        AdmittedCheckError::new(
                            property.span,
                            format!("dynamic module has no runtime export `{}`", property.name),
                        )
                    })?;
                let ty = symbol.ty.clone();
                self.facts
                    .used_dynamic_exports
                    .insert((module, property.name));
                Ok(ty)
            }
            Type::Task(_) if matches!(property.name, "then" | "catch" | "finally") => Err(
                AdmittedCheckError::new(span, format!("Task `{}` must be called", property.name)),
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
                _ => Err(AdmittedCheckError::new(
                    span,
                    format!("float has no member `{}`", property.name),
                )),
            },
            Type::Int => match property.name {
                "toString" | "toUnsignedString" => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::optional(Type::Int)],
                        return_type: Box::new(Type::String),
                    })))
                }
                _ => Err(AdmittedCheckError::new(
                    span,
                    format!("int has no member `{}`", property.name),
                )),
            },
            Type::Array(element) => match property.name {
                "get" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::Int)],
                    return_type: Box::new(nullable_type(*element)),
                }))),
                "map" | "filter" | "forEach" | "reduce" | "some" | "every" | "findIndex" => {
                    Err(AdmittedCheckError::new(
                        span,
                        format!("array `{}` must be called", property.name),
                    ))
                }
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
                        FunctionParameter::optional(Type::Int),
                    ],
                    return_type: Box::new(Type::Bool),
                }))),
                "join" if is_stringifiable_array_element(&element) => {
                    Ok(Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::optional(Type::String)],
                        return_type: Box::new(Type::String),
                    })))
                }
                "join" => Err(AdmittedCheckError::new(
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
                        FunctionParameter::optional(Type::Int),
                    ],
                    return_type: Box::new(Type::Array(element)),
                }))),
                "reverse" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: Vec::new(),
                    return_type: Box::new(Type::Array(element)),
                }))),
                "slice" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::optional(Type::Int),
                        FunctionParameter::optional(Type::Int),
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
                _ => Err(AdmittedCheckError::new(
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
                _ => Err(AdmittedCheckError::new(
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
                _ => Err(AdmittedCheckError::new(
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
                "get" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![FunctionParameter::value(Type::Int)],
                    return_type: Box::new(nullable_type(
                        crate::typed_array::TypedArrayKind::from_type(&ty)
                            .unwrap()
                            .index_value_type(),
                    )),
                }))),
                "slice" | "subarray" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::optional(Type::Int),
                    ],
                    return_type: Box::new(ty),
                }))),
                "set" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(ty.clone()),
                        FunctionParameter::optional(Type::Int),
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
                            FunctionParameter::optional(Type::Int),
                            FunctionParameter::optional(Type::Int),
                        ],
                        return_type: Box::new(ty),
                    })))
                }
                "copyWithin" => Ok(Type::Function(FunctionType::new(FunctionSignature {
                    params: vec![
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::value(Type::Int),
                        FunctionParameter::optional(Type::Int),
                    ],
                    return_type: Box::new(ty),
                }))),
                _ => Err(AdmittedCheckError::new(
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
                        FunctionParameter::optional(Type::Int),
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
                _ => Err(AdmittedCheckError::new(
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
                _ => Err(AdmittedCheckError::new(
                    span,
                    format!("Regex has no member `{}`", property.name),
                )),
            },
            other => Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        use crate::primitive::{Intrinsic, ResolvedIntrinsic};
        self.facts.source_info[member.id.index()] = SourceInfo {
            expression: Some(member),
            resolution: ExpressionResolution::None,
            absent_default_argument: false,
        };
        let receiver = self.analyze_expr(object, None)?;
        if let Type::Function(signature) = &receiver {
            if signature.has_receiver() && property.name == "call" {
                let mut explicit = signature.clone();
                explicit.make_mut().params[0].receiver = false;
                let explicit = Type::Function(explicit);
                self.record_type(member.id, &explicit)?;
                return self.analyze_call(&explicit, args, span, expected, Some(call_node));
            }
        }
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
                let callback = self
                    .view()
                    .expression_type(args[0].expression.id)
                    .cloned()
                    .ok_or_else(|| {
                        AdmittedCheckError::new(args[0].span, "task callback lost its checked type")
                    })?;
                self.record_type(
                    member.id,
                    &Type::Function(FunctionType::new(FunctionSignature {
                        params: vec![FunctionParameter::value(callback)],
                        return_type: Box::new(result.clone()),
                    })),
                )?;
                return Ok(result);
            }
            (Type::Dynamic, name)
                if crate::primitive::resolve_member(&Type::Dynamic, name).is_none() =>
            {
                return self.analyze_dynamic_method_call(call_node, member, property, args);
            }
            (receiver, _) => {
                let callee =
                    self.analyze_member_type(receiver.clone(), property, member.id, member.span())?;
                self.record_type(member.id, &callee)?;
                if let Type::Function(signature) = &callee {
                    if signature.has_receiver() {
                        self.require_assignable(&signature.params[0].ty, &receiver, object.span())?;
                        let mut explicit = signature.clone();
                        explicit.make_mut().params.remove(0);
                        return self.analyze_call(
                            &Type::Function(explicit),
                            args,
                            span,
                            expected,
                            Some(call_node),
                        );
                    }
                }
                return self.analyze_call(&callee, args, span, expected, Some(call_node));
            }
        };
        // These methods have no standalone value type, but each call was
        // checked against one contextual signature. Record it on the callee so
        // later owners read the checked contract instead of re-deriving it.
        let mut params = Vec::with_capacity(args.len());
        for argument in args {
            let ty = self
                .view()
                .expression_type(argument.expression.id)
                .cloned()
                .ok_or_else(|| {
                    AdmittedCheckError::new(
                        argument.span,
                        "array callback argument lost its checked type",
                    )
                })?;
            params.push(FunctionParameter::value(ty));
        }
        self.record_type(
            member.id,
            &Type::Function(FunctionType::new(FunctionSignature {
                params,
                return_type: Box::new(result.clone()),
            })),
        )?;
        self.facts.source_info[member.id.index()].resolution =
            ExpressionResolution::Primitive(ResolvedIntrinsic::Method(operation));
        Ok(result)
    }

    fn analyze_array_map(
        &mut self,
        element_type: Type<'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        if args.len() != 1 {
            return Err(AdmittedCheckError::new(
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
            return Err(AdmittedCheckError::new(
                args[0].span,
                format!("array `map` expects a function, found `{callback}`"),
            ));
        };
        self.require_value_parameters(&signature, args[0].span)?;
        if signature.params.len() != 1
            || !is_type_assignable(&signature.params[0].ty, &element_type)
            || !is_type_assignable(&element_type, &signature.params[0].ty)
        {
            return Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let signature = self.analyze_array_callback(args, &element_type, "filter", span)?;
        if signature.return_type.as_ref() != &Type::Bool {
            return Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let signature = self.analyze_array_callback(args, &element_type, method, span)?;
        if signature.return_type.as_ref() != &Type::Bool {
            return Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        if args.len() != 2 {
            return Err(AdmittedCheckError::new(
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
            return Err(AdmittedCheckError::new(
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
            return Err(AdmittedCheckError::new(
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
    ) -> Result<FunctionType<'src>, AdmittedCheckError> {
        if args.len() != 1 {
            return Err(AdmittedCheckError::new(
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
            return Err(AdmittedCheckError::new(
                args[0].span,
                format!("array `{method}` expects a function callback"),
            ));
        };
        self.require_value_parameters(&signature, args[0].span)?;
        if signature.params.len() != 1 || signature.params[0].ty != *element_type {
            return Err(AdmittedCheckError::new(
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
    ) -> Result<(), AdmittedCheckError> {
        signature
            .validate_parameters()
            .map_err(|message| AdmittedCheckError::new(span, message))?;
        if signature
            .params
            .iter()
            .any(|parameter| parameter.passing != ParameterPassing::Value)
        {
            return Err(AdmittedCheckError::new(
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
    ) -> Result<(), AdmittedCheckError> {
        if let Some(argument) = args
            .iter()
            .find(|argument| argument.passing != ParameterPassing::Value)
        {
            return Err(AdmittedCheckError::new(argument.span, message));
        }
        Ok(())
    }

    /// An argument for a declared rest parameter `T... name` (R7): a `T`, or
    /// `...xs` over an array of `T`s.
    fn analyze_rest_argument(
        &mut self,
        argument: &'ast Argument<'ast, 'src>,
        array: &Type<'src>,
    ) -> Result<(), AdmittedCheckError> {
        let Type::Array(element) = array else {
            unreachable!("a rest parameter is an array")
        };
        if argument.passing != ParameterPassing::Value {
            return Err(AdmittedCheckError::new(
                argument.span,
                "a rest parameter receives values",
            ));
        }
        if argument.spread {
            let actual = self.analyze_expr(&argument.expression, Some(array))?;
            self.require_assignable(array, &actual, argument.span)
        } else {
            let actual = self.analyze_value_argument(argument, Some(element))?;
            self.require_assignable(element, &actual, argument.span)
        }
    }

    fn analyze_value_argument(
        &mut self,
        argument: &'ast Argument<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        if argument.spread {
            return Err(spread_refusal(argument.span));
        }
        if argument.passing != ParameterPassing::Value {
            return Err(AdmittedCheckError::new(
                argument.span,
                "a value parameter cannot receive a mutable-reference argument",
            ));
        }
        self.analyze_expr(&argument.expression, expected)
    }

    fn require_no_pending_reference(&self, span: Span) -> Result<(), AdmittedCheckError> {
        if self.pending_references {
            return Err(AdmittedCheckError::new(
                span,
                "cannot suspend after preparing a mutable-reference argument",
            ));
        }
        if self.current_reference_formals {
            return Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        if is_js_value(callee) {
            self.analyze_dynamic_arguments(args)?;
            return Ok(Type::Dynamic);
        }
        if matches!(callee, Type::Function(signature) if signature.has_receiver()) {
            return Err(AdmittedCheckError::new(
                span,
                "a receiver function must be called as a member or with `.call(receiver, ...)`",
            ));
        }
        // A spread passes to a JavaScript function or to a declared rest
        // parameter (R7).
        let variadic = match callee {
            Type::Function(signature) => signature.has_rest(),
            Type::GenericFunction(function) => function.signature.has_rest(),
            _ => false,
        };
        if !variadic {
            if let Some(spread) = args.iter().find(|argument| argument.spread) {
                return Err(spread_refusal(spread.span));
            }
        }
        if let Type::GenericFunction(function) = callee {
            return self.analyze_generic_call(function, args, span, expected_return, call_node);
        }
        if let Type::Union(members) = callee {
            let mut signatures = Vec::new();
            for member in members {
                let Type::Function(signature) = member else {
                    return Err(AdmittedCheckError::new(
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
                return Err(AdmittedCheckError::new(
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
            return Err(AdmittedCheckError::new(
                span,
                format!("cannot call a value of type `{callee}`"),
            ));
        };
        signature
            .validate_parameters()
            .map_err(|message| AdmittedCheckError::new(span, message))?;
        if !signature.accepts_arity(args.len()) {
            return Err(AdmittedCheckError::new(
                span,
                format!(
                    "function expects {} to {} arguments, found {}",
                    signature.required_params(),
                    signature.params.len(),
                    args.len()
                ),
            ));
        }
        let outer_pending = self.pending_references;
        let result = (|| {
            let fixed = signature.fixed_params();
            for (index, arg) in args.iter().enumerate() {
                if index >= fixed {
                    self.analyze_rest_argument(arg, &signature.params[fixed].ty)?;
                    continue;
                }
                let parameter = &signature.params[index];
                if arg.passing != parameter.passing {
                    return Err(AdmittedCheckError::new(
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
                        return Err(AdmittedCheckError::new(
                            arg.span,
                            format!(
                                "mutable-reference storage type must be exactly `{}`, found `{actual}`",
                                parameter.ty
                            ),
                        ));
                    }
                    self.pending_references = true;
                } else {
                    let expected = if parameter.optional && self.declarations.source_contract.unified_absence() {
                        Type::nullable(Box::new(parameter.ty.clone()))
                    } else { parameter.ty.clone() };
                    let actual = self.analyze_value_argument(arg, Some(&expected))?;
                    self.require_assignable(&expected, &actual, arg.span)?;
                    self.facts.source_info[arg.expression.id.index()].absent_default_argument =
                        parameter.optional && self.declarations.source_contract.unified_absence()
                            && absence::optional(&actual);
                }
            }
            Ok(())
        })();
        self.pending_references = outer_pending;
        result?;
        Ok((*signature.return_type).clone())
    }

    fn analyze_static_namespace_call(
        &mut self,
        callee: &'ast Expr<'ast, 'src>,
        args: &'ast [Argument<'ast, 'src>],
        span: Span,
        expected: Option<&Type<'src>>,
    ) -> Result<Option<(BuiltinCall, Type<'src>)>, AdmittedCheckError> {
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
                    return Err(AdmittedCheckError::new(
                        span,
                        format!("`Object.{}` expects one record", property.name),
                    ));
                };
                let actual = self.analyze_value_argument(record, None)?;
                let Type::Record(value) = actual else {
                    return Err(AdmittedCheckError::new(
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
                    return Err(AdmittedCheckError::new(
                        span,
                        "`Object.hasOwn` expects a record and string key",
                    ));
                };
                let actual = self.analyze_value_argument(record, None)?;
                if !matches!(actual, Type::Record(_)) {
                    return Err(AdmittedCheckError::new(
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
                    return Err(AdmittedCheckError::new(
                        span,
                        "`Object.assign` expects two records",
                    ));
                };
                let target_type = self.analyze_value_argument(target, None)?;
                let Type::Record(_) = target_type else {
                    return Err(AdmittedCheckError::new(
                        target.span,
                        format!(
                            "`Object.assign` target must be a `Record<T>`, found `{target_type}`"
                        ),
                    ));
                };
                let source_type = self.analyze_value_argument(source, Some(&target_type))?;
                if source_type != target_type {
                    return Err(AdmittedCheckError::new(
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
                    return Err(AdmittedCheckError::new(
                        span,
                        "`JSON.stringify` expects one value",
                    ));
                };
                let actual = self.analyze_value_argument(value, None)?;
                if self.declarations.source_contract.unified_absence() && absence::json_observes(&actual, self.budget)? {
                    return Err(AdmittedCheckError::new(value.span,
                        "JSON serialization observes optional values outside optional object fields (R2); narrow before serializing"));
                }
                if !json_stringify_type_supported(&actual) {
                    return Err(AdmittedCheckError::new(
                        value.span,
                        format!("`JSON.stringify` does not support `{actual}` portably"),
                    ));
                }
                Ok(Some((BuiltinCall::JsonStringify, Type::String)))
            }
            ("JSON", "parse") => {
                let [value] = args else {
                    return Err(AdmittedCheckError::new(
                        span,
                        "`JSON.parse` expects one string",
                    ));
                };
                let actual = self.analyze_value_argument(value, Some(&Type::String))?;
                self.require_assignable(&Type::String, &actual, value.span)?;
                Ok(Some((BuiltinCall::JsonParse, Type::Dynamic)))
            }
            ("Task", "resolve") => {
                let [value] = args else {
                    return Err(AdmittedCheckError::new(
                        span,
                        "`Task.resolve` expects one value",
                    ));
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
                    return Err(AdmittedCheckError::new(
                        span,
                        "`Task.reject` expects one reason",
                    ));
                };
                let reason_type = self.analyze_value_argument(reason, None)?;
                if reason_type == Type::Void {
                    return Err(AdmittedCheckError::new(
                        reason.span,
                        "`Task.reject` reason cannot be `void`",
                    ));
                }
                let Some(Type::Task(value)) = expected else {
                    return Err(AdmittedCheckError::new(
                        span,
                        "cannot infer rejected task value type; provide an expected `Task<T>` type",
                    ));
                };
                Ok(Some((BuiltinCall::TaskReject, Type::Task(value.clone()))))
            }
            ("Task", "all") => {
                let [tasks] = args else {
                    return Err(AdmittedCheckError::new(
                        span,
                        "`Task.all` expects one task array",
                    ));
                };
                let tasks = self.analyze_value_argument(tasks, None)?;
                let Type::Array(ref task) = tasks else {
                    return Err(AdmittedCheckError::new(
                        args[0].span,
                        format!("`Task.all` requires a `Task<T>[]`, found `{tasks}`"),
                    ));
                };
                let Type::Task(value) = task.as_ref() else {
                    return Err(AdmittedCheckError::new(
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
    ) -> Result<Option<(BuiltinCall, Type<'src>)>, AdmittedCheckError> {
        let js = Type::Dynamic;
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
                Err(AdmittedCheckError::new(
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
                        return Err(AdmittedCheckError::new(
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
                    let result = if self.declarations.source_contract.unified_absence()
                        && expected.is_some_and(absence::optional) { Type::Null } else { js.clone() };
                    (BuiltinCall::JsUndefined, result, Vec::new())
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
                        AdmittedCheckError::new(
                            span,
                            "cannot infer `JS.assume` result type; provide an expected type",
                        )
                    })?;
                    if result.is_void() {
                        return Err(AdmittedCheckError::new(
                            span,
                            "`JS.assume` result cannot be `void`",
                        ));
                    }
                    // A trusted host view uses the declared public keys.
                    self.declarations.reflect(&result);
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        if args.len() != 1 {
            return Err(AdmittedCheckError::new(
                span,
                format!("Task `{method}` expects one callback, found {}", args.len()),
            ));
        }
        let parameters = match method {
            "then" => vec![FunctionParameter::value(value.clone())],
            "catch" => vec![FunctionParameter::value(Type::Dynamic)],
            "finally" => Vec::new(),
            _ => unreachable!("task call dispatch validates the method name"),
        };
        let expected = Type::Function(FunctionType::new(FunctionSignature {
            params: parameters,
            return_type: Box::new(Type::Void),
        }));
        let callback = self.analyze_value_argument(&args[0], Some(&expected))?;
        let Type::Function(signature) = callback else {
            return Err(AdmittedCheckError::new(
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
            return Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        self.require_value_parameters(&function.signature, span)?;
        if !function.signature.accepts_arity(args.len()) {
            return Err(AdmittedCheckError::new(
                span,
                format!(
                    "function expects {} to {} arguments, found {}",
                    function.signature.required_params(),
                    function.signature.params.len(),
                    args.len()
                ),
            ));
        }
        let parameters = function
            .type_params
            .iter()
            .map(|parameter| parameter.identity)
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
        let pattern = |index: usize, spread: bool| {
            let fixed = function.signature.fixed_params();
            let parameter = &function.signature.params[index.min(fixed)];
            if parameter.rest && !spread {
                let Type::Array(element) = &parameter.ty else {
                    unreachable!("checked rest type")
                };
                element.as_ref()
            } else {
                &parameter.ty
            }
        };
        for (index, arg) in args.iter().enumerate() {
            let pattern = pattern(index, arg.spread);
            let optional = function.signature.params[index.min(function.signature.fixed_params())].optional
                && self.declarations.source_contract.unified_absence();
            let mut partially_resolved = substitute_type(pattern, &substitutions);
            if optional { partially_resolved = Type::nullable(Box::new(partially_resolved)); }
            let expected = (!contains_type_parameter(&partially_resolved, &parameters))
                .then_some(&partially_resolved);
            let actual = if arg.spread {
                if index < function.signature.fixed_params() {
                    return Err(spread_refusal(arg.span));
                }
                self.analyze_expr(&arg.expression, expected)?
            } else {
                self.analyze_value_argument(arg, expected)?
            };
            let present = match &actual { Type::Nullable(inner) if optional => inner.as_ref(), _ => &actual };
            if !optional || !matches!(present, Type::Null) {
                infer_type_arguments(pattern, present, &parameters, &mut substitutions, arg.span)?;
            }
            let mut resolved = substitute_type(pattern, &substitutions);
            if optional { resolved = Type::nullable(Box::new(resolved)); }
            if !contains_type_parameter(&resolved, &parameters) {
                self.require_assignable(&resolved, &actual, arg.span)?;
            }
            self.facts.source_info[arg.expression.id.index()].absent_default_argument =
                optional && absence::optional(&actual);
            actual_args.push(actual);
        }
        for parameter in &function.type_params {
            if !substitutions.contains_key(&parameter.identity) {
                return Err(AdmittedCheckError::new(
                    span,
                    format!("cannot infer type argument `{parameter}`"),
                ));
            }
            // Until generic bodies publish an instantiated reflection summary,
            // a type parameter may be erased to JsValue in that body. Preserve
            // every concrete nominal supplied through this unknown boundary.
            self.declarations
                .reflect(&substitutions[&parameter.identity]);
        }
        for (index, (arg, actual)) in args.iter().zip(&actual_args).enumerate() {
            let mut resolved = substitute_type(pattern(index, arg.spread), &substitutions);
            if function.signature.params[index.min(function.signature.fixed_params())].optional
                && self.declarations.source_contract.unified_absence() {
                resolved = Type::nullable(Box::new(resolved));
            }
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
                        .map(|name| substitutions[&name.identity].clone())
                        .collect(),
                    signature: signature.clone(),
                },
            );
        }
        Ok((*signature.return_type).clone())
    }

    /// A lambda body is its own flow (R3): it reads an outer local only once
    /// that local is assigned, and its assignments to outer locals do not
    /// count after it, since it may run at any time or never.
    fn analyze_arrow(
        &mut self,
        params: &'ast [crate::ast::Param<'ast, 'src>],
        body: &'ast ArrowBody<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let outer = self.unassigned.clone();
        let conditional = std::mem::replace(&mut self.conditional_assignments, 0);
        self.enter_body(match body {
            ArrowBody::Expr(expression) => {
                assignments::Assigned::expression_body(params, expression)
            }
            ArrowBody::Block(statements) => assignments::Assigned::body(params, statements),
        })?;
        let result = self.analyze_arrow_body(params, body, expected);
        self.leave_body();
        self.unassigned = outer;
        self.conditional_assignments = conditional;
        result
    }

    fn analyze_arrow_body(
        &mut self,
        params: &'ast [crate::ast::Param<'ast, 'src>],
        body: &'ast ArrowBody<'ast, 'src>,
        expected: Option<&Type<'src>>,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        let expected_signature = match expected {
            Some(Type::Function(signature)) => Some(signature),
            _ => None,
        };
        if let Some(signature) = expected_signature {
            signature.validate_parameters().map_err(|message| {
                AdmittedCheckError::new(
                    params.first().map_or(Span::empty(0), |param| param.span),
                    message,
                )
            })?;
            if params.len() != signature.params.len() {
                return Err(AdmittedCheckError::new(
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
        self.budget
            .push(AllocationClass::Scratch, &mut self.generator_contexts, None)?;
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
                        AdmittedCheckError::new(
                            param.parameter.ty.span,
                            "`auto` arrow parameters require a contextual callback type",
                        )
                    })?
            } else {
                self.resolve_value_type(param.parameter.ty, "arrow parameter")?
            };
            let rest = param.role == crate::ast::ParamRole::Rest;
            let ty = if rest && !param.parameter.ty.is_auto() {
                Type::Array(Box::new(ty))
            } else {
                ty
            };
            if rest
                && (param.default.is_some() || param.parameter.passing != ParameterPassing::Value)
            {
                return Err(AdmittedCheckError::new(
                    param.span,
                    "a rest parameter passes by value and has no default",
                ));
            }
            if let Some(parameter) = expected_signature.and_then(|sig| sig.params.get(index)) {
                let expected = &parameter.ty;
                if parameter.receiver != (param.role == crate::ast::ParamRole::Receiver)
                    || parameter.rest != rest
                    || parameter.passing != param.parameter.passing
                    || !is_type_assignable(expected, &ty)
                    || !is_type_assignable(&ty, expected)
                {
                    return Err(AdmittedCheckError::new(
                        param.span,
                        format!("callback parameter must be `{expected}`, found `{ty}`"),
                    ));
                }
            }
            if param.parameter.passing == ParameterPassing::MutableReference
                && param.default.is_some()
            {
                return Err(AdmittedCheckError::new(
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
                receiver: param.role == crate::ast::ParamRole::Receiver,
                ty,
                passing: param.parameter.passing,
                optional: false,
                rest,
            });
        }

        resolve_parameter_defaults(params, &mut parameters, &self.facts.type_bindings)?;
        self.analyze_parameter_defaults(params, &parameters)?;
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
        self.pending_references = outer_pending;
        self.current_reference_formals = outer_formals;
        self.constructor_classes.pop();
        self.generator_contexts.pop();
        self.pop_scope();

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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        checked_binary_type(op, lhs, rhs, span).map_err(Into::into)
    }

    fn resolve_value_type(
        &self,
        ty: TypeRef<'ast, 'src>,
        context: &str,
    ) -> Result<Type<'src>, AdmittedCheckError> {
        self.resolve_type(ty, false, context)
    }

    fn resolve_parameter_type(
        &self,
        parameter: &crate::ast::ParameterType<'ast, 'src>,
        context: &'static str,
    ) -> Result<FunctionParameter<'src>, AdmittedCheckError> {
        Ok(FunctionParameter {
            receiver: false,
            ty: self.resolve_value_type(parameter.ty, context)?,
            passing: parameter.passing,
            optional: false,
            rest: false,
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
    ) -> Result<Option<Type<'src>>, AdmittedCheckError> {
        let ty = match class.name {
            "Map" => {
                if !args.is_empty() {
                    return Err(AdmittedCheckError::new(
                        span,
                        format!(
                            "`Map` constructor expects 0 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if type_args.is_empty() {
                    let Some(Type::Map(key, value)) = expected else {
                        return Err(AdmittedCheckError::new(
                            span,
                            "cannot infer `Map` type arguments; write `new Map<K, V>()`",
                        ));
                    };
                    Type::Map(key.clone(), value.clone())
                } else {
                    let [key, value]: [Type<'src>; 2] = self
                        .resolve_type_arguments("Map", type_args, 2, span)?
                        .try_into()
                        .expect("Map arity was checked");
                    validate_collection_key(&key, span, "Map key")?;
                    Type::Map(Box::new(key), Box::new(value))
                }
            }
            "Set" => {
                if !args.is_empty() {
                    return Err(AdmittedCheckError::new(
                        span,
                        format!(
                            "`Set` constructor expects 0 arguments, found {}",
                            args.len()
                        ),
                    ));
                }
                if type_args.is_empty() {
                    let Some(Type::Set(element)) = expected else {
                        return Err(AdmittedCheckError::new(
                            span,
                            "cannot infer `Set` type argument; write `new Set<T>()`",
                        ));
                    };
                    Type::Set(element.clone())
                } else {
                    let [element]: [Type<'src>; 1] = self
                        .resolve_type_arguments("Set", type_args, 1, span)?
                        .try_into()
                        .expect("Set arity was checked");
                    validate_collection_key(&element, span, "Set element")?;
                    Type::Set(Box::new(element))
                }
            }
            "ArrayBuffer" | "SharedArrayBuffer" => {
                self.resolve_type_arguments(class.name, type_args, 0, span)?;
                if args.len() != 1 {
                    return Err(AdmittedCheckError::new(
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
                self.resolve_type_arguments(class.name, type_args, 0, span)?;
                if args.len() != 1 {
                    return Err(AdmittedCheckError::new(
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
                    return Err(AdmittedCheckError::new(
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
                self.resolve_type_arguments(class.name, type_args, 0, span)?;
                if args.len() > 1 {
                    return Err(AdmittedCheckError::new(
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
                self.resolve_type_arguments(class.name, type_args, 0, span)?;
                let contract = crate::primitive::intrinsic_call_contract(
                    crate::primitive::ResolvedIntrinsic::Constructor(
                        crate::primitive::Intrinsic::RegexNew,
                    ),
                )
                .expect("checked Regex constructor signature");
                if !contract.accepts_arity(args.len()) {
                    return Err(AdmittedCheckError::new(
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
    ) -> Result<Type<'src>, AdmittedCheckError> {
        match ty.kind {
            TypeKind::Int => Ok(Type::Int),
            TypeKind::Float => Ok(Type::Float),
            TypeKind::String => Ok(Type::String),
            TypeKind::Bool => Ok(Type::Bool),
            TypeKind::Null | TypeKind::Undefined => Err(AdmittedCheckError::new(
                ty.span, "an absence boundary spelling must accompany a present type: `T | null` or `T | undefined` (R2)")),
            TypeKind::Void if allow_void => Ok(Type::Void),
            TypeKind::Void => Err(AdmittedCheckError::new(
                ty.span,
                format!("{context} cannot have type `void`"),
            )),
            TypeKind::Auto => Err(AdmittedCheckError::new(
                ty.span,
                format!("`auto` is not allowed as a {context} type"),
            )),
            TypeKind::Named { name, args }
                if self
                    .type_parameter_scopes
                    .iter()
                    .rev()
                    .any(|scope| scope.contains_key(name)) =>
            {
                if !args.is_empty() {
                    return Err(AdmittedCheckError::new(
                        ty.span,
                        format!("type parameter `{name}` does not accept type arguments"),
                    ));
                }
                Ok(Type::TypeParameter(
                    *self
                        .type_parameter_scopes
                        .iter()
                        .rev()
                        .find_map(|scope| scope.get(name))
                        .expect("resolved type parameter"),
                ))
            }
            TypeKind::Named { name: "Map", args } => {
                let [key, value]: [Type<'src>; 2] = self
                    .resolve_type_arguments("Map", args, 2, ty.span)?
                    .try_into()
                    .expect("Map arity was checked");
                validate_collection_key(&key, ty.span, "Map key")?;
                Ok(Type::Map(Box::new(key), Box::new(value)))
            }
            TypeKind::Named { name: "Set", args } => {
                let [element]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Set", args, 1, ty.span)?
                    .try_into()
                    .expect("Set arity was checked");
                validate_collection_key(&element, ty.span, "Set element")?;
                Ok(Type::Set(Box::new(element)))
            }
            TypeKind::Named { name: "Task", args } if !self.binds_aggregate_type("Task") => {
                let [value]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Task", args, 1, ty.span)?
                    .try_into()
                    .expect("Task arity was checked");
                Ok(Type::Task(Box::new(value)))
            }
            TypeKind::Named {
                name: "Generator",
                args,
            } if !self.binds_aggregate_type("Generator") => {
                let [value]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Generator", args, 1, ty.span)?
                    .try_into()
                    .expect("Generator arity was checked");
                Ok(Type::Generator(Box::new(value)))
            }
            TypeKind::Named {
                name: "ArrayBuffer",
                args,
            } => {
                self.resolve_type_arguments("ArrayBuffer", args, 0, ty.span)?;
                Ok(Type::ArrayBuffer)
            }
            TypeKind::Named {
                name: "SharedArrayBuffer",
                args,
            } => {
                self.resolve_type_arguments("SharedArrayBuffer", args, 0, ty.span)?;
                Ok(Type::SharedArrayBuffer)
            }
            TypeKind::Named { name, args }
                if let Some(kind) = crate::typed_array::TypedArrayKind::from_name(name) =>
            {
                self.resolve_type_arguments(name, args, 0, ty.span)?;
                Ok(kind.as_type())
            }
            TypeKind::Named {
                name: "Symbol",
                args,
            } => {
                self.resolve_type_arguments("Symbol", args, 0, ty.span)?;
                Ok(Type::Symbol)
            }
            TypeKind::Named {
                name: "Regex",
                args,
            } => {
                self.resolve_type_arguments("Regex", args, 0, ty.span)?;
                Ok(Type::Regex)
            }
            TypeKind::Named {
                name: "JsValue",
                args,
            } => {
                self.resolve_type_arguments("JsValue", args, 0, ty.span)?;
                Ok(Type::Dynamic)
            }
            TypeKind::Named {
                name: "unknown",
                args,
            } => {
                self.resolve_type_arguments("unknown", args, 0, ty.span)?;
                Ok(Type::Unknown)
            }
            TypeKind::Named {
                name: "Record",
                args,
            } => {
                let [value]: [Type<'src>; 1] = self
                    .resolve_type_arguments("Record", args, 1, ty.span)?
                    .try_into()
                    .expect("Record arity was checked");
                Ok(Type::Record(Box::new(value)))
            }
            TypeKind::Named { name, args } if self.facts.type_bindings.contains_key(name) => {
                let identity = self.facts.type_bindings[name];
                let declaration = NominalType { identity, name };
                match identity.kind() {
                    NominalKind::Enum => {
                        self.resolve_type_arguments(name, args, 0, ty.span)?;
                        Ok(Type::Enum(
                            self.declarations.enums[identity.index()].declaration,
                        ))
                    }
                    NominalKind::Struct => {
                        let info = &self.declarations.structs[identity.index()];
                        let declaration = info.declaration;
                        let parameters = info.type_params.clone();
                        let arguments =
                            self.resolve_type_arguments(name, args, parameters.len(), ty.span)?;
                        if parameters.is_empty() {
                            Ok(Type::Struct(declaration))
                        } else {
                            Ok(Type::StructInstance {
                                declaration,
                                args: arguments,
                            })
                        }
                    }
                    NominalKind::Class => {
                        let info = &self.declarations.classes[identity.index()];
                        let declaration = NominalType {
                            name: info.declaration.name,
                            ..declaration
                        };
                        let parameters = info.type_params.clone();
                        let arguments =
                            self.resolve_type_arguments(name, args, parameters.len(), ty.span)?;
                        if parameters.is_empty() {
                            Ok(Type::Class(declaration))
                        } else {
                            Ok(Type::ClassInstance {
                                declaration,
                                args: arguments,
                            })
                        }
                    }
                }
            }
            TypeKind::Named { name, .. } => Err(AdmittedCheckError::new(
                ty.span,
                format!("unknown type `{name}`"),
            )),
            TypeKind::Array(element) => {
                let element = self.resolve_value_type(*element, "array element")?;
                Ok(Type::Array(Box::new(element)))
            }
            TypeKind::Nullable(inner) => {
                let inner = self.resolve_value_type(*inner, "nullable value")?;
                Ok(Type::nullable(Box::new(inner)))
            }
            TypeKind::Intersection(members) => {
                let mut types = Vec::with_capacity(members.len());
                for member in members {
                    let member = self.resolve_value_type(*member, "shape intersection")?;
                    if !self.view().is_shape(&member) {
                        return Err(AdmittedCheckError::new(ty.span, "intersections require declared shapes"));
                    }
                    match member {
                        Type::Intersection(members) => types.extend(members),
                        member => if !types.contains(&member) { types.push(member); },
                    }
                }
                // A joined view must address the same keys through every component.
                // Preserve this common schema until the naming owner joins its slots.
                let result = Type::Intersection(types);
                self.declarations.reflect(&result);
                Ok(result)
            }
            TypeKind::Union(members) => {
                let mut resolved = Vec::with_capacity(members.len());
                let mut pin = AbsencePin::Auto;
                for member in members {
                    let spelling = match member.kind {
                        TypeKind::Null => Some(AbsencePin::Null),
                        TypeKind::Undefined => Some(AbsencePin::Undefined),
                        _ => None,
                    };
                    if let Some(spelling) = spelling {
                        if pin != AbsencePin::Auto && pin != spelling {
                            return Err(AdmittedCheckError::new(member.span,
                                "a boundary must choose one absent spelling; use `T?` internally (R2)"));
                        }
                        pin = spelling;
                    } else {
                        resolved.push(self.resolve_value_type(*member, "union member")?);
                    }
                }
                if resolved.is_empty() {
                    return Err(AdmittedCheckError::new(ty.span, "an absence boundary needs a present type (R2)"));
                }
                let result = normalize_union(resolved);
                Ok(if pin == AbsencePin::Auto { result } else { Type::pinned_nullable(Box::new(result), pin) })
            }
            TypeKind::Function {
                params,
                return_type,
            } => {
                let mut resolved_params = Vec::with_capacity(params.len());
                for param in params {
                    resolved_params.push(FunctionParameter {
                        receiver: param.receiver,
                        ty: {
                            let ty = self.resolve_value_type(param.ty, "function parameter")?;
                            if param.rest {
                                Type::Array(Box::new(ty))
                            } else {
                                ty
                            }
                        },
                        passing: param.passing,
                        optional: false,
                        rest: param.rest,
                    });
                }
                let return_type = self.resolve_type(*return_type, true, "function return")?;
                let signature = FunctionSignature {
                    params: resolved_params,
                    return_type: Box::new(return_type),
                };
                signature
                    .validate_parameters()
                    .map_err(|message| AdmittedCheckError::new(ty.span, message))?;
                Ok(Type::Function(FunctionType::new(signature)))
            }
        }
    }

    fn resolve_type_arguments(
        &self,
        name: &str,
        args: &'ast [TypeRef<'ast, 'src>],
        parameter_count: usize,
        span: Span,
    ) -> Result<Vec<Type<'src>>, AdmittedCheckError> {
        if args.len() != parameter_count {
            return Err(AdmittedCheckError::new(
                span,
                format!(
                    "type `{name}` expects {} type arguments, found {}",
                    parameter_count,
                    args.len()
                ),
            ));
        }
        let resolved: Vec<_> = args
            .iter()
            .map(|argument| self.resolve_value_type(*argument, "type argument"))
            .collect::<Result<_, _>>()?;
        if self.binds_aggregate_type(name) {
            for argument in &resolved {
                self.declarations.reflect(argument);
            }
        }
        Ok(resolved)
    }

    /// Whether this scope binds `name` to a struct or class, which shadows a
    /// built-in generic of that name.
    fn binds_aggregate_type(&self, name: &str) -> bool {
        self.facts
            .type_bindings
            .get(name)
            .is_some_and(|identity| !identity.is_enum())
    }

    fn push_type_params(&mut self, params: &[Ident<'src>]) -> Result<(), AdmittedCheckError> {
        let names = validate_type_params(self.module, params)?;
        for parameter in params {
            if self
                .type_parameter_scopes
                .iter()
                .any(|scope| scope.contains_key(parameter.name))
            {
                return Err(AdmittedCheckError::new(
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
            names
                .into_iter()
                .map(|parameter| (parameter.name, parameter))
                .collect(),
        )?;
        Ok(())
    }

    fn pop_type_params(&mut self) {
        self.type_parameter_scopes
            .pop()
            .expect("type parameter scope was pushed before it was popped");
    }

    /// `v is C` and `v as? C` on a class (R13): `v` is a `JsValue`, or has
    /// a member type that `C` is or extends. The test is `instanceof`, so an
    /// internal class keeps its identity (`mark_tested_classes`). Returns
    /// whether `target` is a class.
    fn class_guard(
        &mut self,
        value: &Type<'src>,
        target: &Type<'src>,
        span: Span,
    ) -> Result<bool, AdmittedCheckError> {
        let Some((declaration, args)) = class_type_parts(target) else {
            return Ok(false);
        };
        let info = &self.declarations.classes[declaration.identity.index()];
        if info.shape {
            if info.discriminant.is_none() {
                return Err(AdmittedCheckError::new(span, "a shape identity test requires a declared discriminant"));
            }
            if !args.is_empty() || !info.type_params.is_empty() {
                return Err(AdmittedCheckError::new(span, "a shape tag cannot test erased type arguments"));
            }
            if !is_js_value_or_nullable_js_value(value) && !runtime_guard_members(value).iter()
                .all(|member| self.view().is_shape(member) || matches!(member, Type::Null)) {
                return Err(AdmittedCheckError::new(span, "shape tag tests require a shape view or JsValue"));
            }
            self.declarations.reflect(target);
            self.declarations.reflect(value);
            return Ok(true);
        }
        let (external, generic) = (
            info.external,
            !args.is_empty() || !info.type_params.is_empty(),
        );
        if generic {
            return Err(AdmittedCheckError::new(
                span,
                format!("an identity test on the generic class `{target}` is not supported"),
            ));
        }
        let fits = is_js_value_or_nullable_js_value(value)
            || runtime_guard_members(value)
                .iter()
                .any(|member| self.is_assignable(member, target));
        if !fits {
            return Err(AdmittedCheckError::new(
                span,
                format!("`{target}` is neither a `JsValue` test nor a class extending a member of `{value}`"),
            ));
        }
        if !external {
            self.declarations
                .tested_classes
                .insert(declaration.identity);
        }
        if is_js_value_or_nullable_js_value(value) {
            self.declarations.reflect(target);
        }
        Ok(true)
    }

    fn require_assignable(
        &self,
        expected: &Type<'src>,
        actual: &Type<'src>,
        span: Span,
    ) -> Result<(), AdmittedCheckError> {
        // Erasure can occur inside a covariant result or callback as well as
        // at the outer type. Equal typed contracts introduce no new crossing.
        if expected != actual && contains_host_value(expected) {
            self.declarations.reflect(actual);
        }
        if self.is_assignable(expected, actual) {
            Ok(())
        } else {
            let mut message = format!("expected `{expected}`, found `{actual}`");
            let declaration = |ty: &Type<'src>| match ty {
                Type::Struct(declaration)
                | Type::StructInstance { declaration, .. }
                | Type::Class(declaration)
                | Type::ClassInstance { declaration, .. } => Some(*declaration),
                Type::Enum(declaration) => Some(declaration.declaration),
                _ => None,
            };
            if let (Some(expected), Some(actual)) = (declaration(expected), declaration(actual)) {
                if expected.name == actual.name
                    && expected.identity != actual.identity
                    && expected.identity.kind() == actual.identity.kind()
                {
                    // Two declarations of one spelling: say where each is.
                    let place = |identity: NominalId| match identity.kind() {
                        NominalKind::Struct => {
                            let info = &self.declarations.structs[identity.index()];
                            (info.module, info.span)
                        }
                        NominalKind::Class => {
                            let info = &self.declarations.classes[identity.index()];
                            (info.module, info.span)
                        }
                        NominalKind::Enum => {
                            let info = &self.declarations.enums[identity.index()];
                            (info.module, info.span)
                        }
                    };
                    let kind = match expected.identity.kind() {
                        NominalKind::Struct => "struct",
                        NominalKind::Class => "class",
                        NominalKind::Enum => "enum",
                    };
                    let (expected_module, expected_span) = place(expected.identity);
                    let (actual_module, actual_span) = place(actual.identity);
                    use std::fmt::Write;
                    write!(message, " (distinct {kind} declarations: expected module {:?} at {}..{}, found module {:?} at {}..{})",
                        expected_module, expected_span.start, expected_span.end,
                        actual_module, actual_span.start, actual_span.end).expect("String formatting");
                }
            }
            Err(AdmittedCheckError::new(span, message))
        }
    }

    fn is_assignable(&self, expected: &Type<'src>, actual: &Type<'src>) -> bool {
        if is_type_assignable(expected, actual) {
            return true;
        }
        match (expected, actual) {
            (Type::Intersection(members), actual) => members.iter().all(|expected| self.is_assignable(expected, actual)),
            (expected, Type::Intersection(members)) => members.iter().any(|actual| self.is_assignable(expected, actual)),
            (Type::Array(expected), Type::Array(actual)) => {
                absence::same_storage_pin(expected, actual)
                    && self.is_assignable(expected, actual) && self.is_assignable(actual, expected)
            }
            (Type::Task(expected), Type::Task(actual))
            | (Type::Generator(expected), Type::Generator(actual)) => {
                self.is_assignable(expected, actual)
            }
            (Type::Nullable(expected), Type::Nullable(actual)) => {
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
                while let Some((declaration, args)) = class_type_parts(&current) {
                    let Some(info) = self.declarations.classes.get(declaration.identity.index())
                    else {
                        return false;
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
    ) -> Result<(Narrowing<'src>, Narrowing<'src>), AdmittedCheckError> {
        let Some(expression) = input.expression else {
            return Ok((empty_narrowing(), empty_narrowing()));
        };
        let (when_true, when_false) = self.condition_narrowing(expression)?;
        Ok((
            if input.when_true {
                when_true
            } else {
                empty_narrowing()
            },
            if input.when_false {
                when_false
            } else {
                empty_narrowing()
            },
        ))
    }

    fn condition_narrowing(
        &mut self,
        condition: &'ast Expr<'ast, 'src>,
    ) -> Result<(Narrowing<'src>, Narrowing<'src>), AdmittedCheckError> {
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
            self.budget
                .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
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
                self.budget
                    .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
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
                            self.budget.push(
                                AllocationClass::Scratch,
                                &mut pending,
                                NarrowingStep::Not,
                            )?;
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
                            self.budget
                                .push(AllocationClass::Scratch, &mut answers, answer)?;
                        }
                    },
                    NarrowingStep::Not => {
                        let (when_true, when_false) =
                            answers.pop().expect("analyzed negated guard");
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
                        self.budget
                            .push(AllocationClass::Scratch, &mut answers, answer)?;
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
    ) -> Result<(Narrowing<'src>, Narrowing<'src>), AdmittedCheckError> {
        let Some(leaf) = narrowing_leaf(condition) else {
            return Ok((empty_narrowing(), empty_narrowing()));
        };
        if let NarrowingLeaf::TypeCheck { ident, test, span } = leaf {
            let symbol = self.resolve(ident)?;
            let target = self.view().type_check_type(test).cloned().ok_or_else(|| {
                AdmittedCheckError::new(span, "type guard was not analyzed before narrowing")
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
        let NarrowingLeaf::NullComparison {
            ident,
            present_when_true,
        } = leaf
        else {
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
        let narrowing = narrowing
            .into_iter()
            .filter(|(symbol, _)| self.narrowable(*symbol))
            .collect::<Vec<_>>();
        let scope = self
            .narrowings
            .last_mut()
            .expect("semantic analyzer always has a narrowing scope");
        for (symbol, ty) in narrowing {
            scope.insert(symbol, ty);
        }
    }

    /// The body that declares `symbol`, as an index into `bodies`: `Ok(None)`
    /// for a host binding, `Err(())` for another module's binding.
    fn declaring_body(&self, symbol: SymbolId) -> Result<Option<usize>, ()> {
        match self.symbol_bodies.get(&symbol) {
            Some(&declared) => Ok(Some(declared)),
            None if self
                .declarations
                .symbol_modules
                .get(symbol.0 as usize)
                .copied()
                .flatten()
                .is_some_and(|module| Some(module) != self.module) =>
            {
                Err(())
            }
            // A module's own binding may be declared before its body is
            // entered (the module's bindings are declared first): its body
            // is the module's, the outermost.
            None if self.declarations.module_bindings.contains(&symbol) => Ok(Some(0)),
            None => Ok(None),
        }
    }

    /// Whether a test may narrow `symbol` (R1: a narrowed value inhabits its
    /// narrowed type): no function nested in its declaring body assigns it,
    /// since a call may run that function between the test and a use, and it
    /// is not another module's. A host binding is narrowed until code runs
    /// (`invalidate_host_narrowings`).
    fn narrowable(&self, symbol: SymbolId) -> bool {
        match self.declaring_body(symbol) {
            Err(()) => false,
            Ok(None) => true,
            Ok(Some(declared)) => self.bodies.get(declared).is_none_or(|body| {
                !body
                    .nested
                    .contains(self.declarations.symbols[symbol.0 as usize].name)
            }),
        }
    }

    /// Whether the declaring body of `symbol` assigns it in its own code:
    /// a nested function that captures it sees the value that code last
    /// stored, whenever it runs.
    fn captured_and_assigned(&self, symbol: SymbolId) -> Option<usize> {
        let declared = self.declaring_body(symbol).ok()??;
        let body = self.bodies.get(declared)?;
        body.own
            .contains(self.declarations.symbols[symbol.0 as usize].name)
            .then_some(declared)
    }

    /// Enters a function's, lambda's or module's body.
    fn enter_body(
        &mut self,
        assigned: assignments::Assigned<'src>,
    ) -> Result<(), AdmittedCheckError> {
        self.budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            (assigned.own.len() + assigned.nested.len()) as u64 + 1,
        )?;
        // The module's narrowings start in the analyzer's first scope.
        let base = if self.bodies.is_empty() {
            0
        } else {
            self.narrowings.len()
        };
        self.bodies.push(assigned);
        self.narrowing_bases.push(base);
        Ok(())
    }

    fn leave_body(&mut self) {
        self.bodies.pop();
        self.narrowing_bases.pop();
    }

    /// At a suspension, other code runs: a narrowing this body made of a
    /// binding its declaring body assigns ends here.
    fn invalidate_captured_narrowings(&mut self) {
        let Some(current) = self.bodies.len().checked_sub(1) else {
            return;
        };
        let base = self.narrowing_bases[current].min(self.narrowings.len());
        let mut ended = Vec::new();
        for scope in &self.narrowings[base..] {
            for &symbol in scope.keys() {
                if self
                    .captured_and_assigned(symbol)
                    .is_some_and(|declared| declared < current)
                {
                    ended.push(symbol);
                }
            }
        }
        for scope in &mut self.narrowings[base..] {
            for symbol in &ended {
                scope.remove(symbol);
            }
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

    /// The narrowed type of `symbol` here. A narrowing an enclosing body
    /// made does not hold inside a nested one for a binding the enclosing
    /// code assigns: the nested function runs after that code, at any time.
    fn narrowed_type(&self, symbol: SymbolId) -> Option<&Type<'src>> {
        if !self.narrowable(symbol) {
            return None;
        }
        let (scope, ty) = self
            .narrowings
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, scope)| scope.get(&symbol).map(|ty| (index, ty)))?;
        // Outside every body (an analyzer driven expression by expression)
        // nothing nests: the narrowing holds as made.
        let Some(current) = self.bodies.len().checked_sub(1) else {
            return Some(ty);
        };
        let made_in = self
            .narrowing_bases
            .iter()
            .rposition(|&base| base <= scope)
            .unwrap_or(0);
        if made_in < current
            && self
                .captured_and_assigned(symbol)
                .is_some_and(|declared| declared < current)
        {
            return None;
        }
        Some(ty)
    }

    fn invalidate_assigned_narrowing(&mut self, target: &'ast Expr<'ast, 'src>) {
        let Expr {
            kind: ExprKind::Ident(ident),
            ..
        } = target
        else {
            return;
        };
        let Some(symbol) = self.facts.identifier_symbols.get(&ident.id).copied() else {
            return;
        };
        for scope in &mut self.narrowings {
            scope.remove(&symbol);
        }
    }

    fn declare_foreign(
        &mut self,
        ident: Ident<'src>,
        ty: Type<'src>,
        callable: bool,
    ) -> Result<SymbolId, AdmittedCheckError> {
        // A host binding's parameters, result or value cross (R6).
        self.declarations.reflect(&ty);
        if self.module.is_none() {
            let symbol = self.declare(ident, ty)?;
            self.declarations.symbols[symbol.0 as usize].origin = DeclarationOrigin::Foreign;
            return Ok(symbol);
        }
        if self.scopes[0].contains_key(ident.name) {
            return Err(AdmittedCheckError::new(
                ident.span,
                format!("duplicate binding `{}`", ident.name),
            ));
        }
        if let Some(&(symbol, prior_callable)) = self.declarations.foreign_symbols.get(ident.name) {
            if prior_callable != callable || self.declarations.symbols[symbol.0 as usize].ty != ty {
                return Err(AdmittedCheckError::new(
                    ident.span,
                    format!("conflicting extern contracts for `{}`", ident.name),
                ));
            }
            self.scopes[0].insert(ident.name, symbol);
            self.facts
                .binding_types
                .insert(ident.id, BindingType::Symbol(symbol));
            self.record_identifier(ident.id, symbol);
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
    ) -> Result<SymbolId, AdmittedCheckError> {
        let scope = self
            .scopes
            .last_mut()
            .expect("semantic analyzer always has a scope");
        if scope.contains_key(ident.name) {
            return Err(AdmittedCheckError::new(
                ident.span,
                format!("duplicate binding `{}`", ident.name),
            ));
        }

        let id = SymbolId(
            u32::try_from(self.declarations.symbols.len())
                .map_err(|_| AllocationError::Capacity)?,
        );
        let symbol = Symbol {
            id,
            name: ident.name,
            ty,
            span: ident.span,
            node: ident.id,
            attributes: Attributes::default(),
            origin: DeclarationOrigin::Source,
            identifier_occurrences: 0,
        };
        self.declarations
            .add_symbol(symbol, self.module, self.budget)?;
        scope.insert(ident.name, id);
        if self.scopes.len() == 1 && self.callable_depth == 0 {
            self.declarations.module_bindings.insert(id);
        }
        if let Some(body) = self.bodies.len().checked_sub(1) {
            self.symbol_bodies.insert(id, body);
        }
        self.facts
            .binding_types
            .insert(ident.id, BindingType::Symbol(id));
        self.record_identifier(ident.id, id);
        Ok(id)
    }

    fn record_detached(
        &mut self,
        ident: Ident<'src>,
        ty: Type<'src>,
    ) -> Result<SymbolId, AdmittedCheckError> {
        let id = SymbolId(
            u32::try_from(self.declarations.symbols.len())
                .map_err(|_| AllocationError::Capacity)?,
        );
        let symbol = Symbol {
            id,
            name: ident.name,
            ty,
            span: ident.span,
            node: ident.id,
            attributes: Attributes::default(),
            origin: DeclarationOrigin::Source,
            identifier_occurrences: 0,
        };
        self.declarations
            .add_symbol(symbol, self.module, self.budget)?;
        self.facts
            .binding_types
            .insert(ident.id, BindingType::Symbol(id));
        self.record_identifier(ident.id, id);
        Ok(id)
    }

    fn resolve(&self, ident: &Ident<'src>) -> Result<&Symbol<'src>, AdmittedCheckError> {
        self.resolve_with_scope(ident).map(|(_, symbol)| symbol)
    }

    fn resolve_with_scope(
        &self,
        ident: &Ident<'src>,
    ) -> Result<(usize, &Symbol<'src>), AdmittedCheckError> {
        let (scope, id) = self
            .scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, scope)| scope.get(ident.name).map(|id| (index, id)))
            .ok_or_else(|| {
                AdmittedCheckError::new(ident.span, format!("unknown identifier `{}`", ident.name))
            })?;
        if self
            .reference_parameters
            .get(id)
            .is_some_and(|owner| *owner != self.callable_depth)
        {
            return Err(AdmittedCheckError::new(
                ident.span,
                "mutable-reference parameters cannot be captured; copy the value into a local first",
            ));
        }
        Ok((scope, &self.declarations.symbols[id.0 as usize]))
    }

    fn push_scope(&mut self) -> Result<(), AllocationError> {
        self.budget
            .reserve_vec(AllocationClass::Scratch, &mut self.scopes, 1)?;
        self.budget
            .reserve_vec(AllocationClass::Scratch, &mut self.narrowings, 1)?;
        self.budget
            .work(crate::compilation_policy::WorkKind::Render, 2)?;
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

fn validate_type_params<'src>(
    module: Option<usize>,
    params: &[Ident<'src>],
) -> Result<Vec<TypeParameter<'src>>, CheckError> {
    let module = u32::try_from(module.unwrap_or(0)).map_err(|_| {
        CheckError::new(
            params
                .first()
                .map_or(Span { start: 0, end: 0 }, |parameter| parameter.span),
            "source module identity capacity",
        )
    })?;
    let mut names = Vec::with_capacity(params.len());
    let mut seen = AHashSet::default();
    for parameter in params {
        if !seen.insert(parameter.name) {
            return Err(CheckError::new(
                parameter.span,
                format!("duplicate type parameter `{}`", parameter.name),
            ));
        }
        names.push(TypeParameter {
            identity: TypeParameterId {
                module,
                declaration: parameter.id,
            },
            name: parameter.name,
        });
    }
    Ok(names)
}

/// `scope` is the declaring source's type scope: a nominal default names
/// the identity its declaration sees, wherever it is later evaluated.
fn resolve_parameter_defaults<'ast, 'src>(
    params: &[crate::ast::Param<'ast, 'src>],
    parameters: &mut [FunctionParameter<'src>],
    _scope: &AHashMap<&'src str, NominalId>,
) -> Result<(), CheckError> {
    for (param, parameter) in params.iter().zip(parameters) {
        parameter.optional = param.default.is_some();
        if parameter.optional
            && (parameter.passing == ParameterPassing::MutableReference
                || parameter.rest
                || parameter.receiver)
        {
            return Err(CheckError::new(
                param.span,
                "a reference, rest or receiver parameter cannot have a default",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "check/default_ownership_tests.rs"]
mod default_ownership_tests;

#[cfg(test)]
#[path = "check/binding_ownership_tests.rs"]
mod binding_ownership_tests;

#[cfg(test)]
#[path = "check/class_ownership_tests.rs"]
mod class_ownership_tests;

#[cfg(test)]
#[path = "check/constructor_ownership_tests.rs"]
mod constructor_ownership_tests;

#[cfg(test)]
#[path = "check/type_resolution_tests.rs"]
mod type_resolution_tests;

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

fn applied_class_type<'src>(
    declaration: NominalType<'src>,
    parameters: &[TypeParameter<'src>],
) -> Type<'src> {
    if parameters.is_empty() {
        Type::Class(declaration)
    } else {
        Type::ClassInstance {
            declaration,
            args: parameters
                .iter()
                .map(|parameter| Type::TypeParameter(*parameter))
                .collect(),
        }
    }
}

fn substitute_type<'src>(
    ty: &Type<'src>,
    substitutions: &AHashMap<TypeParameterId, Type<'src>>,
) -> Type<'src> {
    match type_substitution::substitute_type_with(
        ty,
        &mut |name, _: &mut type_relation::Unmetered| Ok(substitutions.get(&name)),
        &mut type_relation::Unmetered,
    ) {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

fn substitutions_for<'src>(
    parameters: &[TypeParameter<'src>],
    arguments: &[Type<'src>],
) -> AHashMap<TypeParameterId, Type<'src>> {
    parameters
        .iter()
        .map(|parameter| parameter.identity)
        .zip(arguments.iter().cloned())
        .collect()
}

fn method_callable_type<'src>(
    method: &MethodInfo<'src>,
    substitutions: &AHashMap<TypeParameterId, Type<'src>>,
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

fn contains_type_parameter(ty: &Type<'_>, parameters: &AHashSet<TypeParameterId>) -> bool {
    match ty {
        Type::TypeParameter(name) => parameters.contains(&name.identity),
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
    parameters: &AHashSet<TypeParameterId>,
    substitutions: &mut AHashMap<TypeParameterId, Type<'src>>,
    span: Span,
) -> Result<(), CheckError> {
    match (pattern, actual) {
        (Type::TypeParameter(name), actual) if parameters.contains(&name.identity) => {
            if let Some(previous) = substitutions.get(&name.identity) {
                if is_type_assignable(previous, actual) {
                    return Ok(());
                }
                if is_type_assignable(actual, previous) {
                    substitutions.insert(name.identity, actual.clone());
                } else {
                    return Err(CheckError::new(
                        span,
                        format!("conflicting inferences for `{name}`: `{previous}` and `{actual}`"),
                    ));
                }
            } else {
                substitutions.insert(name.identity, actual.clone());
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
                    return Err(CheckError::new(
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
                declaration: pattern,
                args: pattern_args,
            },
            Type::ClassInstance {
                declaration: actual,
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
) -> Result<Type<'src>, CheckError> {
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

/// Whether a type contains a host-erased position. This does not inspect a
/// nominal's fields: their exposure belongs to the reflected-set closure.
fn contains_host_value(ty: &Type<'_>) -> bool {
    match ty {
        Type::Dynamic | Type::Unknown => true,
        Type::Array(inner)
        | Type::Record(inner)
        | Type::Set(inner)
        | Type::Task(inner)
        | Type::Generator(inner) => contains_host_value(inner),
        Type::Nullable(inner) => contains_host_value(inner),
        Type::Map(key, value) => contains_host_value(key) || contains_host_value(value),
        Type::Union(types) | Type::Intersection(types)
        | Type::ClassInstance { args: types, .. }
        | Type::StructInstance { args: types, .. } => types.iter().any(contains_host_value),
        Type::Function(signature) => {
            signature.params.iter().any(|p| contains_host_value(&p.ty))
                || contains_host_value(&signature.return_type)
        }
        Type::GenericFunction(function) => {
            function
                .signature
                .params
                .iter()
                .any(|p| contains_host_value(&p.ty))
                || contains_host_value(&function.signature.return_type)
        }
        _ => false,
    }
}

/// Every nominal a type names, syntactically: classes, structs and enums,
/// their type arguments, and the types a function value's crossing carries.
fn nominals_in(ty: &Type<'_>, out: &mut Vec<NominalId>) {
    match ty {
        Type::Class(declaration) | Type::Struct(declaration) => {
            out.push(declaration.identity)
        }
        Type::Enum(declaration) => out.push(declaration.identity),
        Type::ClassInstance { declaration, args } | Type::StructInstance { declaration, args } => {
            out.push(declaration.identity);
            for argument in args {
                nominals_in(argument, out);
            }
        }
        Type::Array(value)
        | Type::Record(value)
        | Type::Set(value)
        | Type::Task(value)
        | Type::Generator(value) => nominals_in(value, out),
        Type::Nullable(value) => nominals_in(value, out),
        Type::Map(key, value) => {
            nominals_in(key, out);
            nominals_in(value, out);
        }
        Type::Union(members) | Type::Intersection(members) => {
            for member in members {
                nominals_in(member, out);
            }
        }
        Type::Function(signature) => {
            for parameter in &signature.params {
                nominals_in(&parameter.ty, out);
            }
            nominals_in(&signature.return_type, out);
        }
        Type::GenericFunction(function) => {
            for parameter in &function.signature.params {
                nominals_in(&parameter.ty, out);
            }
            nominals_in(&function.signature.return_type, out);
        }
        _ => {}
    }
}

/// The index of a class's base in the class table, if it has one.
fn class_base_index(classes: &[ClassInfo<'_>], index: usize) -> Option<usize> {
    classes[index]
        .base
        .as_ref()
        .and_then(class_type_identity)
        .map(NominalId::index)
}

/// Every class extending an observed class is observed, to a fixed point.
fn observe_descendants(classes: &mut [ClassInfo<'_>]) {
    loop {
        let mut changed = false;
        for index in 0..classes.len() {
            if classes[index].observed || classes[index].external {
                continue;
            }
            if class_base_index(classes, index).is_some_and(|base| classes[base].observed) {
                classes[index].observed = true;
                changed = true;
            }
        }
        if !changed {
            return;
        }
    }
}

fn class_type_identity(ty: &Type<'_>) -> Option<NominalId> {
    class_type_parts(ty).map(|(declaration, _)| declaration.identity)
}

fn class_type_parts<'ty, 'src>(
    ty: &'ty Type<'src>,
) -> Option<(NominalType<'src>, &'ty [Type<'src>])> {
    match ty {
        Type::Class(declaration) => Some((*declaration, &[])),
        Type::ClassInstance { declaration, args } => Some((*declaration, args)),
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
        Stmt::Break(_, ..) | Stmt::Continue(_, ..) => !inside_loop,
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
        Stmt::Break(_, ..) => true,
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
        Type::Union(members) => Type::nullable(Box::new(Type::Union(members))),
        ty => Type::nullable(Box::new(ty)),
    }
}

fn validate_collection_key(ty: &Type<'_>, span: Span, context: &str) -> Result<(), CheckError> {
    let supported = match ty {
        Type::Dynamic => true,
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
        Err(CheckError::new(
            span,
            format!("{context} type `{ty}` has no portable identity contract"),
        ))
    }
}

fn buffer_member<'src>(
    property: Ident<'src>,
    span: Span,
    return_type: Type<'src>,
) -> Result<Type<'src>, CheckError> {
    match property.name {
        "slice" => Ok(Type::Function(FunctionType::new(FunctionSignature {
            params: vec![
                FunctionParameter::value(Type::Int),
                FunctionParameter::optional(Type::Int),
            ],
            return_type: Box::new(return_type),
        }))),
        _ => Err(CheckError::new(
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
) -> Result<crate::literal::StringValue, CheckError> {
    crate::literal::StringValue::decode_source(value)
        .map_err(|error| CheckError::new(span, format!("invalid string escape: {error:?}")))
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

fn optional_result_type<'src>(ty: Type<'src>, span: Span) -> Result<Type<'src>, CheckError> {
    match ty {
        Type::Void => Err(CheckError::new(
            span,
            "optional access cannot materialize a nullable `void` value",
        )),
        Type::Function(_) | Type::GenericFunction(_) => Err(CheckError::new(
            span,
            "optional method calls are not yet supported; coalesce the receiver before calling",
        )),
        Type::Nullable(_) => Ok(ty),
        ty => Ok(Type::nullable(Box::new(ty))),
    }
}

fn signature_mentions_unknown(signature: &FunctionType<'_>) -> bool {
    signature.return_type.mentions_unknown()
        || signature
            .params
            .iter()
            .any(|parameter| parameter.ty.mentions_unknown())
}

fn signature_without_unknown<'src>(signature: &FunctionType<'src>) -> FunctionType<'src> {
    FunctionType::new(FunctionSignature {
        params: signature
            .params
            .iter()
            .map(|parameter| FunctionParameter {
                ty: parameter.ty.without_unknown(),
                ..parameter.clone()
            })
            .collect(),
        return_type: Box::new(signature.return_type.without_unknown()),
    })
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
    matches!(ty, Type::Dynamic)
}

/// A typed operator applied to a `JsValue` operand (R12), as the dynamic
/// operation its `JS.*` spelling names. `==` and `!=` stay loose binary
/// operations (M1.9); `+` with a `string` operand stays concatenation, which
/// is what JavaScript does with it.
fn dynamic_binary<'src>(
    op: BinaryOp,
    left: &Type<'src>,
    right: &Type<'src>,
) -> Option<(BuiltinCall, Type<'src>)> {
    if !(is_js_value(left) || is_js_value(right)) || left.is_void() || right.is_void() {
        return None;
    }
    Some(match op {
        BinaryOp::Add if *left != Type::String && *right != Type::String => {
            (BuiltinCall::JsAdd, Type::Dynamic)
        }
        BinaryOp::Mod => (BuiltinCall::JsMod, Type::Dynamic),
        BinaryOp::Sub => (BuiltinCall::JsSubtract, Type::Dynamic),
        BinaryOp::Mul => (BuiltinCall::JsMultiply, Type::Dynamic),
        BinaryOp::Div => (BuiltinCall::JsDivide, Type::Dynamic),
        BinaryOp::Less => (BuiltinCall::JsLessThan, Type::Bool),
        BinaryOp::LessEq => (BuiltinCall::JsLessThanOrEqual, Type::Bool),
        BinaryOp::Greater => (BuiltinCall::JsGreaterThan, Type::Bool),
        BinaryOp::GreaterEq => (BuiltinCall::JsGreaterThanOrEqual, Type::Bool),
        BinaryOp::And => (BuiltinCall::JsAnd, Type::Dynamic),
        BinaryOp::Or => (BuiltinCall::JsOr, Type::Dynamic),
        _ => return None,
    })
}

/// A spread passes to a JavaScript function only: a declared parameter list
/// takes its arguments one by one.
fn spread_refusal(span: Span) -> AdmittedCheckError {
    AdmittedCheckError::new(
        span,
        "a spread argument passes to a JavaScript function; this callee's parameters are declared",
    )
}

/// Whether a value of `ty` crossing between JavaScript and typed code needs a
/// conversion: a struct is a value the program lays out itself.
fn crosses_by_conversion(ty: &Type<'_>) -> bool {
    match ty {
        Type::Struct(_) | Type::StructInstance { .. } => true,
        Type::Nullable(inner) => crosses_by_conversion(inner),
        Type::Array(inner) => crosses_by_conversion(inner),
        Type::Union(members) => members.iter().any(crosses_by_conversion),
        _ => false,
    }
}

/// Whether a statement always leaves its block (R3's joins): `return`,
/// `throw`, `break` or `continue`, a block ending in one, or an `if` whose two
/// branches leave.
fn statement_leaves(statement: &Stmt<'_, '_>) -> bool {
    match statement {
        Stmt::Return { .. } | Stmt::Throw { .. } | Stmt::Break(_, ..) | Stmt::Continue(_, ..) => {
            true
        }
        Stmt::Block { body, .. } => body.last().is_some_and(statement_leaves),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => statement_leaves(then_branch) && statement_leaves(else_branch),
        _ => false,
    }
}

/// A declared rest parameter (R7): `T... name`, the last, is `T[]` in the
/// body and takes a call's trailing arguments. The parameters before it take
/// no defaults, and it passes by value.
fn mark_rest_parameter<'src>(
    params: &[crate::ast::Param<'_, 'src>],
    resolved: &mut [FunctionParameter<'src>],
) -> Result<(), AdmittedCheckError> {
    let Some(last) = params
        .last()
        .filter(|param| param.role == crate::ast::ParamRole::Rest)
    else {
        return Ok(());
    };
    if last.parameter.passing != ParameterPassing::Value {
        return Err(AdmittedCheckError::new(
            last.span,
            "a rest parameter passes by value",
        ));
    }
    let parameter = resolved
        .last_mut()
        .expect("one resolved parameter per declared parameter");
    parameter.ty = Type::Array(Box::new(std::mem::replace(&mut parameter.ty, Type::Void)));
    parameter.rest = true;
    Ok(())
}

/// Receiver and rest positions are syntax contracts, independent of any
/// JavaScript adapter arity or target representation.
fn validate_parameter_roles(params: &[crate::ast::Param<'_, '_>]) -> Result<(), CheckError> {
    use crate::ast::ParamRole;
    for (index, param) in params.iter().enumerate() {
        if (param.role == ParamRole::Receiver
            && (index != 0
                || param.default.is_some()
                || param.parameter.passing != ParameterPassing::Value))
            || (param.role == ParamRole::Rest && index + 1 != params.len())
        {
            return Err(CheckError::new(
                param.span,
                "the receiver is the first value parameter without a default, and the rest is last",
            ));
        }
    }
    Ok(())
}

/// The names `new` constructs as builtin types before any binding:
/// `analyze_builtin_constructor`'s cases.
pub(crate) fn builtin_constructor_name(name: &str) -> bool {
    matches!(name, "Map" | "Set" | "Symbol" | "Regex")
        || crate::typed_array::TypedArrayKind::from_name(name).is_some()
}

/// The members a `JsValue` has with a declared type (v0.1): `length`,
/// `message`, `specifier`, `truthy()`, `isArray()` and `isObject()`. Every
/// other member is a dynamic property (R12). They retire in S2.
fn typed_js_member(name: &str) -> bool {
    matches!(
        name,
        "length" | "message" | "specifier" | "truthy" | "isArray" | "isObject"
    )
}

fn is_js_index_type(ty: &Type<'_>) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::String) || is_js_value(ty)
}

/// `unknown`, or a nullable one.
fn is_unknown(ty: &Type<'_>) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Nullable(inner) => **inner == Type::Unknown,
        _ => false,
    }
}

/// A value the tests and the ways out take (`is`, `as`, `as?`; R12): a
/// `JsValue` or an `unknown`, or a nullable one.
fn is_js_value_or_nullable_js_value(ty: &Type<'_>) -> bool {
    if is_unknown(ty) {
        return true;
    }
    match ty {
        Type::Dynamic => true,
        Type::Nullable(inner) => is_js_value(inner),
        _ => false,
    }
}

fn validate_type_guard(value: &Type<'_>, target: &Type<'_>, span: Span) -> Result<(), CheckError> {
    if matches!(target, Type::Union(_) | Type::Nullable(_)) {
        return Err(CheckError::new(
            span,
            "an `is` guard target must be one concrete member type",
        ));
    }
    let target_category = runtime_type_category(target).ok_or_else(|| {
        CheckError::new(
            span,
            format!("type `{target}` has no portable runtime type guard"),
        )
    })?;
    if is_js_value_or_nullable_js_value(value) {
        return if matches!(target, Type::Float | Type::String | Type::Bool) {
            Ok(())
        } else {
            Err(CheckError::new(
                span,
                format!(
                    "a `JsValue` cannot be soundly narrowed to `{target}`; use `float` for JavaScript numbers, `.isArray()` for untyped arrays, or `.truthy()`/`.isObject()` without narrowing"
                ),
            ))
        };
    }
    let members = runtime_guard_members(value);
    if !members.iter().any(|member| member == target) {
        return Err(CheckError::new(
            span,
            format!("type `{target}` is not a member of `{value}`"),
        ));
    }
    if members
        .iter()
        .any(|member| member != target && runtime_type_category(member) == Some(target_category))
    {
        return Err(CheckError::new(
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
            .map(|remaining| Type::nullable(Box::new(remaining))),
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
        Type::Dynamic => Some(Type::Dynamic),
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
                receiver: false,
                ty: Type::Struct(model.struct_type("Point").unwrap()),
                passing: ParameterPassing::MutableReference,
                optional: false,
                rest: false,
            }
        );
        let Item::Function(forward) = &source.items[2] else {
            panic!("forward")
        };
        let Stmt::Expr(
            Expr {
                kind: ExprKind::Call { args, .. },
                ..
            },
            ..,
        ) = &forward.body[0]
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
        assert!(
            model.symbol_is_reassigned(model.identifier_symbol(forward.params[0].name.id).unwrap())
        );
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
                FunctionParameter::optional(Type::Int)
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
                    receiver: false,
                    ty: Type::TypeParameter(crate::check::TypeParameter::fixture("T")),
                    passing: ParameterPassing::MutableReference,
                    optional: false,
                    rest: false,
                },
                FunctionParameter::optional(Type::Int),
            ],
            return_type: Box::new(Type::TypeParameter(crate::check::TypeParameter::fixture(
                "T",
            ))),
        });
        let mut substitutions = AHashMap::default();
        substitutions.insert(TypeParameter::fixture("T").identity, Type::Int);
        let Type::Function(reference) = substitute_type(&Type::Function(reference), &substitutions)
        else {
            panic!("function")
        };
        assert_eq!(reference.params[0].ty, Type::Int);
        assert_eq!(
            reference.params[0].passing,
            ParameterPassing::MutableReference
        );
        assert!(reference.params[1].optional);
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
        let wrapped = Type::nullable(Box::new(Type::Array(Box::new(Type::GenericFunction(
            GenericFunctionType {
                type_params: vec![crate::check::TypeParameter::fixture("T")],
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
            params: vec![FunctionParameter::optional(Type::Int)],
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
        signature.params[0].optional = false;
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
            Item::Stmt(Stmt::Expr(integer, nodes.node())),
            Item::Stmt(Stmt::Expr(string, nodes.node())),
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
        assert!(crate::interpreter::interpret_program(&other, &model).is_err());
        assert!(crate::program::from_checked_source(&other, &model).is_err());
    }

    use bumpalo::Bump;

    use super::*;
    use crate::parser::parse_source;

    fn check(source: &str) -> Result<(), CheckError> {
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
            let seed_ident = nodes.ident("seed", span);
            let mut expression = nodes.expression(ExprKind::Ident(seed_ident));
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
            items.push(Item::Stmt(Stmt::Expr(expression, nodes.node())));
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
            assert_eq!(model.identifier_symbol(seed_ident.id), Some(seed.id));
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
            model.symbol_is_observable_before_initialization(recursion.id),
            "the recursive initializer reads its binding from a nested function"
        );
        assert!(
            !model.symbol_is_reassigned(recursion.id),
            "a read in its own initializer is not an assignment"
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
                !model.symbol_is_reassigned(symbol.id),
                "later reads are not assignments"
            );
        }
        assert!(model.identifier_index_is_consistent());
    }

    #[test]
    fn identifier_registration_is_idempotent_and_tracks_rebinding_a_node() {
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
        // `a`'s declaring identifier (M4.4: facts are keyed by node).
        let Item::Stmt(Stmt::VarDecl(declaration, ..)) = &program.items[0] else {
            unreachable!("the program starts with `int a=1;`")
        };
        let node = declaration.name.id;
        let count_a = model.declarations.symbols[a.0 as usize].identifier_occurrences;
        let count_b = model.declarations.symbols[b.0 as usize].identifier_occurrences;
        model.record_identifier(node, a);
        assert_eq!(
            model.declarations.symbols[a.0 as usize].identifier_occurrences,
            count_a
        );
        model.record_identifier(node, b);
        assert_eq!(model.identifier_symbol(node), Some(b));
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
        assert!(
            string.message.contains("a typed array, a Set<T>"),
            "{string}"
        );
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
    fn s4_parameter_defaults_use_the_expression_checker_and_callee_scope() {
        for source in [
            r#"int value(int input="no"){return input;}"#,
            r#"int value(int[] input=["no"]){return input.length;}"#,
            r#"int value(func(int)->int transform=(int input)=>"no"){return transform(1);}"#,
            "struct Left{int value;}struct Right{int value;}int read(Left value=Right{1}){return value.value;}",
            "int f(int first=second+1,int second=2){return first;}",
            "auto f=(int first=first+1)=>first;",
        ] { assert!(check(source).is_err(),"{source}"); }
        check("int value(int input=1+2){return input;}").unwrap();
        check("JsValue read(JsValue JS,JsValue value=JS.undefined()){return value;}").unwrap();
        check("auto read=(JsValue JS,JsValue value=JS.undefined())=>value;").unwrap();
    }

    #[test]
    fn accepts_callee_owned_defaults_that_depend_on_captured_bindings() {
        check("[1,2].map((int seed)=>(int value=seed)=>value)[1]();").unwrap();

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
        // R11: a bitwise operator takes a float operand through ToInt32 and
        // gives an `int`; `%` of floats is a float.
        check("number value=1.5;int shifted=value<<1;number rest=value%1;").unwrap();
        let narrowed = check("number value=1.5;int rest=value%1;").unwrap_err();
        assert!(narrowed.message.contains("expected `int`"), "{narrowed}");
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
            .any(|symbol| { symbol.name == "seed" && semantics.symbol_is_reassigned(symbol.id) }));
    }

    #[test]
    fn rejects_uninitialized_variables() {
        // A module's own binding keeps its initializer (R3): only a
        // function's local may be declared bare.
        let arena = Bump::new();
        let program = parse_source(&arena, "int value;").unwrap();
        assert!(analyze(&program)
            .unwrap_err()
            .message
            .contains("requires an initializer"));
    }

    /// A narrowing holds only where no code the flow does not see can assign
    /// its binding (R1): the facts trust a narrowed type.
    #[test]
    fn narrowing_holds_only_where_no_unseen_code_assigns_the_binding() {
        let accepted = [
            // Captured, never assigned again: the lambda sees the narrowing.
            "string? v = \"a\"; if (v != null) { auto f = () => v.length; print(f()); }",
            // Assigned only by the flow that narrows it.
            "int f(string? s) { string? v = s; if (v != null) { int n = v.length; v = null; return n; } return 0; } print(f(\"ab\"));",
            // A lambda's own local, narrowed inside it.
            "auto f = (string? s) => { string? v = s; if (v != null) { return v.length; } return 0; }; print(f(\"ab\"));",
            // A host binding until code runs.
            "extern string? host; if (host != null) { print(host.length); }",
            // A lambda's own test of a captured binding its function assigns
            // (motionlil's stagger and animate consumer): nothing assigns it
            // between that test and its use.
            "float f(bool b) { JsValue from = 0.0; if (b) { from = 1.5; } auto g = () => { float x = if (from is float) { from } else { 0.0 }; return x; }; return g(); } print(f(true));",
            "string? scope = null; scope = \"s\"; auto use = () => { if (scope != null) { string active = scope; print(active); } }; use();",
        ];
        for source in accepted {
            let arena = Bump::new();
            let program = parse_source(&arena, source).unwrap();
            analyze(&program).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        }
        let refused = [
            // Captured and assigned after the lambda's creation.
            "func()->int make(){string? value=\"ok\"; if(value!=null){auto read=()=>{return value.length;};value=null;return read;} return ()=>0;} print(make()());",
            "int? value=1;if(value!=null){auto read=()=>value+2;value=null;print(read());}",
            // Assigned by a lambda: a call may run it between test and use.
            "string? v = \"a\"; auto reset = () => { v = null; }; if (v != null) { reset(); print(v.length); }",
            // A module binding a function assigns.
            "string? g = \"a\"; void clear() { g = null; } if (g != null) { clear(); print(g.length); }",
            // A host binding after a call: the call may have assigned it.
            "extern string? host; extern void tick(); if (host != null) { tick(); print(host.length); }",
        ];
        for source in refused {
            let arena = Bump::new();
            let program = parse_source(&arena, source).unwrap();
            assert!(analyze(&program).is_err(), "{source}");
        }
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
    /// The reflected set (R6): a crossing reflects its operand's nominal and
    /// every nominal its fields reach; a class that never crosses is not
    /// reflected.
    #[test]
    fn crossings_reflect_their_nominals_and_what_they_reach() {
        let arena = Bump::new();
        let source = parse_source(
            &arena,
            "extern void show(JsValue value); class Inner { int x; init(int x) { this.x = x; } } class Outer { Inner inner; init() { this.inner = new Inner(1); } } class Private { int y; init() { this.y = 2; } } class Thrown { string why; init() { this.why = \"no\"; } } export void f() { Outer o = new Outer(); show(o); Private p = new Private(); show(p.y); } export void g() { throw new Thrown(); }",
        )
        .unwrap();
        let model = analyze(&source).unwrap();
        let class = |name| model.type_binding(name).unwrap();
        assert!(model.is_reflected(class("Outer")));
        assert!(model.is_reflected(class("Inner")));
        assert!(model.is_reflected(class("Thrown")));
        assert!(!model.is_reflected(class("Private")));
    }

    #[test]
    fn reflection_closes_over_subclasses_and_their_payloads() {
        let arena = Bump::new();
        let source = parse_source(
            &arena,
            r#"
            class Leaf{int payload;}
            class Base{int inherited;}
            class Child extends Base{Leaf? childOnly;}
            class Grandchild extends Child{int extra;}
            class Unrelated{int secret;}
            extern void expose(Base value);
        "#,
        )
        .unwrap();
        let model = analyze(&source).unwrap();
        for name in ["Base", "Child", "Grandchild", "Leaf"] {
            assert!(
                model.is_reflected(model.type_binding(name).unwrap()),
                "{name}"
            );
        }
        assert!(!model.is_reflected(model.type_binding("Unrelated").unwrap()));
    }

    #[test]
    fn reflection_preserves_trusted_host_views_and_guards() {
        let arena = Bump::new();
        let source = parse_source(
            &arena,
            r#"
            extern JsValue opaque;
            class Assumed{int publicField;}
            class Viewed{int publicField;}
            class Guarded{int publicField;}
            class Private{int privateField;}
            Assumed a=JS.assume(opaque);
            Viewed b=opaque as Viewed;
            if(opaque is Guarded){print(opaque.publicField);}
        "#,
        )
        .unwrap();
        let model = analyze(&source).unwrap();
        for name in ["Assumed", "Viewed", "Guarded"] {
            assert!(
                model.is_reflected(model.type_binding(name).unwrap()),
                "{name}"
            );
        }
        assert!(!model.is_reflected(model.type_binding("Private").unwrap()));
    }

    #[test]
    fn reflection_covers_nested_erasure_and_unqualified_generic_bodies() {
        let arena = Bump::new();
        let source = parse_source(
            &arena,
            r#"
            class Yielded{int key;}
            class Returned{int key;}
            class Generic{int key;}
            class Held{int key;}
            class Explicit{int key;}
            class Private{int key;}
            generator Yielded values(){yield new Yielded();}
            Generator<JsValue> erased=values();
            func()->Returned owned=()=>new Returned();
            func()->JsValue callback=owned;
            T identity<T>(T value){return value;}
            Generic generic=identity(new Generic());
            class Holder<T>{T value;init(T value){this.value=value;}JsValue expose(){return this.value;}}
            auto inferredHolder=new Holder(new Held());
            Holder<Explicit> explicitHolder=new Holder<Explicit>(new Explicit());
            Private local=new Private();
        "#,
        )
        .unwrap();
        let model = analyze(&source).unwrap();
        for name in ["Yielded", "Returned", "Generic", "Held", "Explicit"] {
            assert!(
                model.is_reflected(model.type_binding(name).unwrap()),
                "{name}"
            );
        }
        assert!(!model.is_reflected(model.type_binding("Private").unwrap()));
    }
}
