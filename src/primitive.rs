//! Language operations whose meaning must survive target selection. These are
//! semantic operators, not recognizers for an emitted JavaScript spelling.

/// Invocation observations shared by language and target operations. A value
/// call cannot acquire a receiver or become direct eval after substitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invocation {
    Reference,
    Value,
    DirectEval,
}

/// Whether an omitted parameter is a caller evaluation or part of the called
/// operation's meaning. A checked default value alone cannot decide the actual
/// argument count observed by a replaceable JavaScript method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultConvention {
    MaterializeAtCaller,
    PreserveOmission,
}

/// A parameter's shared calling convention. Mutable references designate
/// caller storage; they cannot be lowered as copied values or copy-out results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParameterPassing {
    Value,
    MutableReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntBinary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    UnsignedShiftRight,
}

impl IntBinary {
    /// The v0.1 signed-i32 contract. Ordinary multiplication deliberately uses
    /// a binary64 product before normalization; it is not Math.imul.
    pub fn evaluate(self, left: i32, right: i32) -> i32 {
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Subtract => left.wrapping_sub(right),
            Self::Multiply => ((f64::from(left) * f64::from(right)) as i64) as i32,
            Self::Divide if right == 0 => 0,
            Self::Remainder if right == 0 => 0,
            Self::Divide => left.wrapping_div(right),
            Self::Remainder => left.wrapping_rem(right),
            Self::UnsignedShiftRight => ((left as u32) >> (right as u32 & 31)) as i32,
        }
    }
}

/// Resolved language operations. IR and target modules use this same identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    Print,
    TaskResolve,
    TaskReject,
    TaskAll,
    IntImul,
    IntToString,
    IntToUnsignedString,
    UnwrapNullable,
    UnwrapUnion,
    ArrayLength,
    ArrayMap,
    ArrayFilter,
    ArrayReduce,
    ArrayForEach,
    ArrayPush,
    ArrayPop,
    ArrayIndexOf,
    ArrayIncludes,
    ArrayJoin,
    ArraySome,
    ArrayEvery,
    ArrayFindIndex,
    ArrayConcat,
    ArrayCopyWithin,
    ArrayReverse,
    ArraySlice,
    ArraySplice,
    ArrayFill,
    TypedArraySet,
    TypedArrayFill,
    TypedArrayCopyWithin,
    MapNew,
    MapSize,
    MapGet,
    MapSet,
    MapHas,
    MapDelete,
    MapClear,
    SetNew,
    SetSize,
    SetAdd,
    SetHas,
    SetDelete,
    SetClear,
    RecordKeys,
    RecordValues,
    RecordHasOwn,
    RecordAssign,
    JsonStringify,
    JsonParse,
    ArrayBufferNew,
    SharedArrayBufferNew,
    BufferByteLength,
    BufferSlice,
    Int8ArrayNew,
    Int8ArrayLength,
    Int8ArrayByteLength,
    Int8ArrayByteOffset,
    Int8ArrayBuffer,
    Int8ArraySlice,
    Int8ArraySubarray,
    Uint8ArrayNew,
    Uint8ArrayLength,
    Uint8ArrayByteLength,
    Uint8ArrayByteOffset,
    Uint8ArrayBuffer,
    Uint8ArraySlice,
    Uint8ArraySubarray,
    Uint8ClampedArrayNew,
    Uint8ClampedArrayLength,
    Uint8ClampedArrayByteLength,
    Uint8ClampedArrayByteOffset,
    Uint8ClampedArrayBuffer,
    Uint8ClampedArraySlice,
    Uint8ClampedArraySubarray,
    Int16ArrayNew,
    Int16ArrayLength,
    Int16ArrayByteLength,
    Int16ArrayByteOffset,
    Int16ArrayBuffer,
    Int16ArraySlice,
    Int16ArraySubarray,
    Uint16ArrayNew,
    Uint16ArrayLength,
    Uint16ArrayByteLength,
    Uint16ArrayByteOffset,
    Uint16ArrayBuffer,
    Uint16ArraySlice,
    Uint16ArraySubarray,
    Int32ArrayNew,
    Int32ArrayLength,
    Int32ArrayByteLength,
    Int32ArrayByteOffset,
    Int32ArrayBuffer,
    Int32ArraySlice,
    Int32ArraySubarray,
    Uint32ArrayNew,
    Uint32ArrayLength,
    Uint32ArrayByteLength,
    Uint32ArrayByteOffset,
    Uint32ArrayBuffer,
    Uint32ArraySlice,
    Uint32ArraySubarray,
    Float32ArrayNew,
    Float32ArrayLength,
    Float32ArrayByteLength,
    Float32ArrayByteOffset,
    Float32ArrayBuffer,
    Float32ArraySlice,
    Float32ArraySubarray,
    Float64ArrayNew,
    Float64ArrayLength,
    Float64ArrayByteLength,
    Float64ArrayByteOffset,
    Float64ArrayBuffer,
    Float64ArraySlice,
    Float64ArraySubarray,
    SymbolNew,
    RegexNew,
    RegexTest,
    RegexSource,
    RegexFlags,
    RegexGlobal,
    RegexIgnoreCase,
    RegexMultiline,
    RegexDotAll,
    RegexSticky,
    RegexUnicode,
    FloatAbs,
    FloatFloor,
    FloatCeil,
    FloatRound,
    FloatSqrt,
    FloatSin,
    FloatCos,
    FloatAcos,
    FloatExp,
    FloatLog,
    FloatTan,
    FloatAtan2,
    FloatHypot,
    FloatMin,
    FloatMax,
    FloatToInt,
    StringLength,
    StringCharCodeAt,
    StringCharAt,
    StringIncludes,
    StringIndexOf,
    StringLastIndexOf,
    StringRepeat,
    StringStartsWith,
    StringEndsWith,
    StringToUpperCase,
    StringToLowerCase,
    StringTrim,
    StringTrimStart,
    StringTrimEnd,
    StringSearch,
    StringSlice,
    StringReplace,
    StringSplit,
    StringCodePointLength,
    JsStringSlice,
    JsStringIndexOf,
    JsStringReplace,
    JsStringMatch,
    JsStringSplit,
    JsRegexExec,
    JsTruthy,
    JsIsArray,
    JsIsObject,
    JsPlainObject,
    JsUndefined,
    JsTypeOf,
    JsIsNullish,
    JsIsFalse,
    JsIsUndefined,
    JsStringify,
    JsDateNow,
    JsParseFloat,
    JsParseInt,
    JsIsFinite,
    JsEncodeURI,
    JsEncodeURIComponent,
    JsObjectCreate,
    JsGetPrototypeOf,
    JsMathPI,
    JsNullProtoObject,
    JsObjectConstructor,
    JsWindow,
    JsDocument,
    JsSetTimeout,
    JsClearTimeout,
    JsDomParserNew,
    JsXMLHttpRequestNew,
    JsNumber,
    JsAdd,
    JsMod,
    JsLessThan,
    JsLessThanOrEqual,
    JsGreaterThan,
    JsGreaterThanOrEqual,
    JsStrictEqual,
    JsStrictNotEqual,
    JsCall,
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
    JsGetProperty,
    JsDeleteProperty,
    JsHasProperty,
    JsInProperty,
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
    JsArrayFlat,
    JsConstruct,
    JsIsFunctionValue,
    JsIsWindowValue,
    JsDefineConfigurable,
    JsDefineIterator,
    JsArrayIterator,
    JsConsoleWarn,
    JsRequestAnimationFrameOrNull,
    JsForInKey,
    JsForInHasNext,
    JsForOfValue,
    JsForOfHasNext,
    GeneratorYield,
    GeneratorYieldDelegated,
}

/// The source checker retains how a checked primitive is used. Both source
/// lowerings consume this identity; target property or constructor text is not
/// a second source of semantic resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedIntrinsic {
    Property(Intrinsic),
    Method(Intrinsic),
    Constructor(Intrinsic),
}

impl ResolvedIntrinsic {
    /// Existing language calls preserve optional standard-call operands,
    /// except binary-memory range operations whose typed end is materialized.
    /// Both target consumers use this semantic identity, never a method name.
    pub fn call_default_convention(self) -> Option<DefaultConvention> {
        let operation = match self {
            Self::Method(operation) => operation,
            Self::Constructor(operation) if builtin_constructor(operation) => {
                return Some(DefaultConvention::PreserveOmission);
            }
            _ => return None,
        };
        Some(
            if operation == Intrinsic::BufferSlice
                || crate::typed_array::is_typed_array_range_intrinsic(operation)
            {
                DefaultConvention::MaterializeAtCaller
            } else {
                DefaultConvention::PreserveOmission
            },
        )
    }
}

/// Signatures for the call operations currently delivered through the
/// semantic core. The checker and verifier share these exact defaults; a
/// target cannot mistake a same-shaped user callable for a primitive contract.
pub(crate) struct IntrinsicCallContract {
    pub receiver: Option<crate::check::Type<'static>>,
    pub parameters: &'static [crate::check::Type<'static>],
    pub defaults: &'static [Option<crate::check::DefaultValue<'static>>],
    pub result: &'static crate::check::Type<'static>,
}

impl IntrinsicCallContract {
    pub fn required_params(&self) -> usize {
        self.defaults
            .iter()
            .position(Option::is_some)
            .unwrap_or(self.parameters.len())
    }

    pub fn accepts_arity(&self, arity: usize) -> bool {
        arity >= self.required_params() && arity <= self.parameters.len()
    }

    pub fn signature<'src>(&self) -> crate::check::FunctionType<'src> {
        // These immutable catalog slices describe Value-only primitives, not
        // an independently mutable function signature. Build the one shared
        // parameter record per slot without compatibility type/default views.
        debug_assert_eq!(self.parameters.len(), self.defaults.len());
        crate::check::FunctionType::new(crate::check::FunctionSignature {
            params: self
                .parameters
                .iter()
                .zip(self.defaults)
                .map(|(ty, default)| crate::check::FunctionParameter {
                    ty: ty.clone(),
                    passing: ParameterPassing::Value,
                    default: default.clone(),
                })
                .collect(),
            return_type: Box::new(self.result.clone()),
        })
    }

    pub fn matches(&self, ty: &crate::check::Type<'_>) -> bool {
        match self.matches_with(ty, &mut crate::check::type_relation::Unmetered) {
            Ok(matches) => matches,
            Err(never) => match never {},
        }
    }

    pub(crate) fn matches_with<A: crate::check::type_relation::RelationAdmission>(
        &self,
        ty: &crate::check::Type<'_>,
        admission: &mut A,
    ) -> Result<bool, A::Error> {
        use crate::check::type_relation::{type_equal_with, RelationEvent};
        admission.admit(RelationEvent::TypeWork(1))?;
        let crate::check::Type::Function(signature) = ty else {
            return Ok(false);
        };
        if signature.params.len() != self.parameters.len()
            || signature.params.len() != self.defaults.len()
        {
            return Ok(false);
        }
        for (parameter, (ty, default)) in signature
            .params
            .iter()
            .zip(self.parameters.iter().zip(self.defaults))
        {
            admission.admit(RelationEvent::ParameterPair)?;
            if !type_equal_with(&parameter.ty, ty, admission)?
                || parameter.passing != ParameterPassing::Value
            {
                return Ok(false);
            }
            match (&parameter.default, default) {
                (None, None) => {}
                (Some(left), Some(right)) => {
                    admission.admit(RelationEvent::DefaultEquality { left, right })?;
                    if left != right {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        type_equal_with(&signature.return_type, self.result, admission)
    }
}

pub(crate) fn intrinsic_call_contract(
    operation: ResolvedIntrinsic,
) -> Option<IntrinsicCallContract> {
    use crate::check::{DefaultValue, Type};
    static INDEX_PARAMETERS: [Type<'static>; 1] = [Type::Int];
    static REQUIRED_SINGLE: [Option<DefaultValue<'static>>; 1] = [None];
    static SEARCH_PARAMETERS: [Type<'static>; 2] = [Type::String, Type::Int];
    static SEARCH_DEFAULTS: [Option<DefaultValue<'static>>; 2] = [None, Some(DefaultValue::Int(0))];
    static SLICE_PARAMETERS: [Type<'static>; 2] = [Type::Int, Type::Int];
    static SLICE_DEFAULTS: [Option<DefaultValue<'static>>; 2] =
        [None, Some(DefaultValue::Int(i32::MAX as i64))];
    static STRING_PARAMETERS: [Type<'static>; 1] = [Type::String];
    static REPLACE_PARAMETERS: [Type<'static>; 2] = [Type::Regex, Type::String];
    static REQUIRED_PAIR: [Option<DefaultValue<'static>>; 2] = [None, None];
    static REGEX_PARAMETERS: [Type<'static>; 2] = [Type::String, Type::String];
    static REGEX_DEFAULTS: [Option<DefaultValue<'static>>; 2] =
        [None, Some(DefaultValue::String(""))];
    // Type already owns aggregate children. Retain this immutable language
    // metadata once, rather than allocating a fresh Array merely to inspect a
    // method contract. Checked signatures still own their copied result type.
    static SPLIT_RESULT: std::sync::LazyLock<Type<'static>> =
        std::sync::LazyLock::new(|| Type::Array(Box::new(Type::String)));
    if operation == ResolvedIntrinsic::Constructor(Intrinsic::RegexNew) {
        return Some(IntrinsicCallContract {
            receiver: None,
            parameters: &REGEX_PARAMETERS,
            defaults: &REGEX_DEFAULTS,
            result: &Type::Regex,
        });
    }
    let ResolvedIntrinsic::Method(operation) = operation else {
        return None;
    };
    let (parameters, defaults, result) = match operation {
        Intrinsic::StringCharCodeAt => (&INDEX_PARAMETERS[..], &REQUIRED_SINGLE[..], &Type::Int),
        Intrinsic::StringCharAt => (&INDEX_PARAMETERS[..], &REQUIRED_SINGLE[..], &Type::String),
        Intrinsic::StringIndexOf => (&SEARCH_PARAMETERS[..], &SEARCH_DEFAULTS[..], &Type::Int),
        Intrinsic::StringSlice => (&SLICE_PARAMETERS[..], &SLICE_DEFAULTS[..], &Type::String),
        Intrinsic::StringSplit => (&STRING_PARAMETERS[..], &REQUIRED_SINGLE[..], &*SPLIT_RESULT),
        Intrinsic::StringRepeat => (&INDEX_PARAMETERS[..], &REQUIRED_SINGLE[..], &Type::String),
        Intrinsic::StringTrim
        | Intrinsic::StringTrimStart
        | Intrinsic::StringTrimEnd
        | Intrinsic::StringToUpperCase
        | Intrinsic::StringToLowerCase => (&[][..], &[][..], &Type::String),
        Intrinsic::StringReplace => (&REPLACE_PARAMETERS[..], &REQUIRED_PAIR[..], &Type::String),
        Intrinsic::RegexTest => (&STRING_PARAMETERS[..], &REQUIRED_SINGLE[..], &Type::Bool),
        Intrinsic::JsRegexExec => (
            &STRING_PARAMETERS[..],
            &REQUIRED_SINGLE[..],
            &Type::TypeParameter("$js"),
        ),
        _ => return None,
    };
    Some(IntrinsicCallContract {
        receiver: Some(
            if matches!(operation, Intrinsic::RegexTest | Intrinsic::JsRegexExec) {
                Type::Regex
            } else {
                Type::String
            },
        ),
        parameters,
        defaults,
        result,
    })
}

/// The runtime test for `value is T`, as the old route's emitter spelled it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeTypeTest {
    TypeOf(&'static str),
    IsArray,
}

pub(crate) fn runtime_type_test(target: &crate::check::Type<'_>) -> Option<RuntimeTypeTest> {
    use crate::check::Type;
    Some(match target {
        Type::Int | Type::Float => RuntimeTypeTest::TypeOf("number"),
        Type::String => RuntimeTypeTest::TypeOf("string"),
        Type::Bool => RuntimeTypeTest::TypeOf("boolean"),
        Type::Function(_) | Type::GenericFunction(_) => RuntimeTypeTest::TypeOf("function"),
        Type::Array(_) => RuntimeTypeTest::IsArray,
        _ => return None,
    })
}

/// Builtin constructors the checker admits (`new Map<K, V>()`, `new
/// Uint8Array(n)`, `new Symbol(description)`, `new Regex(source)`). None has
/// defaults; each call supplies exactly the arguments it was checked with.
pub(crate) fn builtin_constructor(operation: Intrinsic) -> bool {
    matches!(
        operation,
        Intrinsic::RegexNew
            | Intrinsic::MapNew
            | Intrinsic::SetNew
            | Intrinsic::ArrayBufferNew
            | Intrinsic::SharedArrayBufferNew
            | Intrinsic::SymbolNew
    ) || matches!(
        crate::typed_array::classify_typed_array_intrinsic(operation),
        Some((_, crate::typed_array::TypedArrayIntrinsic::New))
    )
}

/// The checked shape of a builtin construction whose result type the
/// checker chose (a generic `Map` or `Set` has no static result type).
/// This mirrors the checker's constructor rules exactly.
/// Every such constructor takes at most one argument, so the caller passes
/// the operand count and the first operand's type without collecting them.
pub(crate) fn constructor_accepts(
    operation: Intrinsic,
    count: usize,
    first: Option<&crate::check::Type<'_>>,
    result: &crate::check::Type<'_>,
) -> bool {
    use crate::check::Type;
    let one = |accepts: fn(&Type<'_>) -> bool| count == 1 && first.is_some_and(accepts);
    match operation {
        Intrinsic::MapNew => count == 0 && matches!(result, Type::Map(_, _)),
        Intrinsic::SetNew => count == 0 && matches!(result, Type::Set(_)),
        Intrinsic::ArrayBufferNew => {
            one(|ty| matches!(ty, Type::Int)) && matches!(result, Type::ArrayBuffer)
        }
        Intrinsic::SharedArrayBufferNew => {
            one(|ty| matches!(ty, Type::Int)) && matches!(result, Type::SharedArrayBuffer)
        }
        Intrinsic::SymbolNew => {
            (count == 0 || one(|ty| matches!(ty, Type::String))) && matches!(result, Type::Symbol)
        }
        operation => match crate::typed_array::classify_typed_array_intrinsic(operation) {
            Some((kind, crate::typed_array::TypedArrayIntrinsic::New)) => {
                one(|ty| matches!(ty, Type::Int | Type::ArrayBuffer | Type::SharedArrayBuffer))
                    && *result == kind.as_type()
            }
            _ => false,
        },
    }
}

/// `JS.*`, `Object.*`, `JSON.*`, `Task.*` and URI builtins: host operations
/// the JavaScript target spells directly. The checker owns their arities and
/// operand types; natively they are unsupported.
pub(crate) fn host_builtin(builtin: crate::check::BuiltinCall) -> bool {
    use crate::check::BuiltinCall as B;
    !matches!(builtin, B::Print | B::MathImul | B::JsOr | B::JsAnd)
}

/// Builtins whose checker branch does not visit a separate callee expression.
/// `argument: None` means Print's existing unrestricted single argument, not
/// missing callable metadata. Unsupported builtins have no contract here.
pub(crate) struct BuiltinCallContract {
    pub arity: usize,
    pub argument: Option<crate::check::Type<'static>>,
    pub result: crate::check::Type<'static>,
}

pub(crate) fn builtin_call_contract(
    builtin: crate::check::BuiltinCall,
) -> Option<BuiltinCallContract> {
    use crate::check::{BuiltinCall, Type};
    Some(match builtin {
        BuiltinCall::Print => BuiltinCallContract {
            arity: 1,
            argument: None,
            result: Type::Void,
        },
        BuiltinCall::MathImul => BuiltinCallContract {
            arity: 2,
            argument: Some(Type::Int),
            result: Type::Int,
        },
        _ => return None,
    })
}

pub(crate) fn resolve_member(
    receiver: &crate::check::Type<'_>,
    property: &str,
) -> Option<ResolvedIntrinsic> {
    use crate::check::Type;
    match (receiver, property) {
        (Type::String, "length") => Some(ResolvedIntrinsic::Property(Intrinsic::StringLength)),
        (Type::Array(_), "length") => Some(ResolvedIntrinsic::Property(Intrinsic::ArrayLength)),
        (Type::Map(_, _), "size") => Some(ResolvedIntrinsic::Property(Intrinsic::MapSize)),
        (Type::Set(_), "size") => Some(ResolvedIntrinsic::Property(Intrinsic::SetSize)),
        (Type::ArrayBuffer | Type::SharedArrayBuffer, "byteLength") => {
            Some(ResolvedIntrinsic::Property(Intrinsic::BufferByteLength))
        }
        (ty, property) if let Some(kind) = crate::typed_array::TypedArrayKind::from_type(ty) => {
            kind.property_intrinsic(property)
                .map(ResolvedIntrinsic::Property)
                .or_else(|| member_intrinsic(ty, property).map(ResolvedIntrinsic::Method))
        }
        _ => member_intrinsic(receiver, property).map(ResolvedIntrinsic::Method),
    }
}

/// Called by the constructor checker after establishing the builtin type and
/// its argument contract. Ordinary values of these types are not construction.
pub(crate) fn constructor_intrinsic(ty: &crate::check::Type<'_>) -> Option<Intrinsic> {
    use crate::check::Type;
    Some(match ty {
        Type::Map(_, _) => Intrinsic::MapNew,
        Type::Set(_) => Intrinsic::SetNew,
        Type::ArrayBuffer => Intrinsic::ArrayBufferNew,
        Type::SharedArrayBuffer => Intrinsic::SharedArrayBufferNew,
        Type::Symbol => Intrinsic::SymbolNew,
        Type::Regex => Intrinsic::RegexNew,
        ty => crate::typed_array::TypedArrayKind::from_type(ty)?.new_intrinsic(),
    })
}

fn member_intrinsic(receiver: &crate::check::Type<'_>, property: &str) -> Option<Intrinsic> {
    use crate::check::Type;
    use crate::typed_array::TypedArrayKind;
    match (receiver, property) {
        (Type::TypeParameter("$js"), "truthy") => Some(Intrinsic::JsTruthy),
        (Type::TypeParameter("$js"), "isArray") => Some(Intrinsic::JsIsArray),
        (Type::TypeParameter("$js"), "isObject") => Some(Intrinsic::JsIsObject),
        (Type::Array(_), "push") => Some(Intrinsic::ArrayPush),
        (Type::Array(_), "pop") => Some(Intrinsic::ArrayPop),
        (Type::Array(_), "indexOf") => Some(Intrinsic::ArrayIndexOf),
        (Type::Array(_), "includes") => Some(Intrinsic::ArrayIncludes),
        (Type::Array(_), "join") => Some(Intrinsic::ArrayJoin),
        (Type::Array(_), "concat") => Some(Intrinsic::ArrayConcat),
        (Type::Array(_), "copyWithin") => Some(Intrinsic::ArrayCopyWithin),
        (Type::Array(_), "reverse") => Some(Intrinsic::ArrayReverse),
        (Type::Array(_), "slice") => Some(Intrinsic::ArraySlice),
        (Type::Array(_), "splice") => Some(Intrinsic::ArraySplice),
        (Type::Array(_), "fill") => Some(Intrinsic::ArrayFill),
        (Type::Map(_, _), "get") => Some(Intrinsic::MapGet),
        (Type::Map(_, _), "set") => Some(Intrinsic::MapSet),
        (Type::Map(_, _), "has") => Some(Intrinsic::MapHas),
        (Type::Map(_, _), "delete") => Some(Intrinsic::MapDelete),
        (Type::Map(_, _), "clear") => Some(Intrinsic::MapClear),
        (Type::Set(_), "add") => Some(Intrinsic::SetAdd),
        (Type::Set(_), "has") => Some(Intrinsic::SetHas),
        (Type::Set(_), "delete") => Some(Intrinsic::SetDelete),
        (Type::Set(_), "clear") => Some(Intrinsic::SetClear),
        (Type::ArrayBuffer | Type::SharedArrayBuffer, "slice") => Some(Intrinsic::BufferSlice),
        (ty, "set") if TypedArrayKind::from_type(ty).is_some() => Some(Intrinsic::TypedArraySet),
        (ty, "fill") if TypedArrayKind::from_type(ty).is_some() => Some(Intrinsic::TypedArrayFill),
        (ty, "copyWithin") if TypedArrayKind::from_type(ty).is_some() => {
            Some(Intrinsic::TypedArrayCopyWithin)
        }
        (ty, method) if let Some(kind) = TypedArrayKind::from_type(ty) => {
            kind.method_intrinsic(method)
        }
        (Type::Float, "abs") => Some(Intrinsic::FloatAbs),
        (Type::Float, "floor") => Some(Intrinsic::FloatFloor),
        (Type::Float, "ceil") => Some(Intrinsic::FloatCeil),
        (Type::Float, "round") => Some(Intrinsic::FloatRound),
        (Type::Float, "sqrt") => Some(Intrinsic::FloatSqrt),
        (Type::Float, "sin") => Some(Intrinsic::FloatSin),
        (Type::Float, "cos") => Some(Intrinsic::FloatCos),
        (Type::Float, "acos") => Some(Intrinsic::FloatAcos),
        (Type::Float, "exp") => Some(Intrinsic::FloatExp),
        (Type::Float, "log") => Some(Intrinsic::FloatLog),
        (Type::Float, "tan") => Some(Intrinsic::FloatTan),
        (Type::Float, "atan2") => Some(Intrinsic::FloatAtan2),
        (Type::Float, "hypot") => Some(Intrinsic::FloatHypot),
        (Type::Float, "min") => Some(Intrinsic::FloatMin),
        (Type::Float, "max") => Some(Intrinsic::FloatMax),
        (Type::Float, "toInt") => Some(Intrinsic::FloatToInt),
        (Type::Int, "toString") => Some(Intrinsic::IntToString),
        (Type::Int, "toUnsignedString") => Some(Intrinsic::IntToUnsignedString),
        (Type::String, "includes") => Some(Intrinsic::StringIncludes),
        (Type::String, "indexOf") => Some(Intrinsic::StringIndexOf),
        (Type::String, "lastIndexOf") => Some(Intrinsic::StringLastIndexOf),
        (Type::String, "repeat") => Some(Intrinsic::StringRepeat),
        (Type::String, "charCodeAt") => Some(Intrinsic::StringCharCodeAt),
        (Type::String, "charAt") => Some(Intrinsic::StringCharAt),
        (Type::String, "startsWith") => Some(Intrinsic::StringStartsWith),
        (Type::String, "endsWith") => Some(Intrinsic::StringEndsWith),
        (Type::String, "toUpperCase") => Some(Intrinsic::StringToUpperCase),
        (Type::String, "toLowerCase") => Some(Intrinsic::StringToLowerCase),
        (Type::String, "trim") => Some(Intrinsic::StringTrim),
        (Type::String, "trimStart") => Some(Intrinsic::StringTrimStart),
        (Type::String, "trimEnd") => Some(Intrinsic::StringTrimEnd),
        (Type::String, "search") => Some(Intrinsic::StringSearch),
        (Type::String, "slice") => Some(Intrinsic::StringSlice),
        (Type::String, "replace") => Some(Intrinsic::StringReplace),
        (Type::String, "split") => Some(Intrinsic::StringSplit),
        (Type::String, "codePointLength") => Some(Intrinsic::StringCodePointLength),
        (Type::String, "truthy") => Some(Intrinsic::JsTruthy),
        (Type::Regex, "test") => Some(Intrinsic::RegexTest),
        (Type::Regex, "exec") => Some(Intrinsic::JsRegexExec),
        _ => None,
    }
}

#[cfg(test)]
mod parameter_contract_tests {
    use super::*;
    use crate::check::{DefaultValue, FunctionType, Type};

    #[test]
    fn primitive_signatures_retain_value_modes_defaults_and_reject_reference_substitution() {
        let contract =
            intrinsic_call_contract(ResolvedIntrinsic::Method(Intrinsic::StringIndexOf)).unwrap();
        let signature = contract.signature();
        assert_eq!(signature.params.len(), 2);
        assert_eq!(signature.params[0].ty, Type::String);
        assert_eq!(signature.params[1].ty, Type::Int);
        assert_eq!(signature.params[0].default, None);
        assert_eq!(signature.params[1].default, Some(DefaultValue::Int(0)));
        assert!(signature
            .params
            .iter()
            .all(|parameter| parameter.passing == ParameterPassing::Value));
        assert!(contract.matches(&Type::Function(signature.clone())));
        let mut edited = (*signature).clone();
        edited.params[0].passing = ParameterPassing::MutableReference;
        assert!(!contract.matches(&Type::Function(FunctionType::new(edited))));
        let mut edited = (*signature).clone();
        edited.params[1].default = None;
        assert!(!contract.matches(&Type::Function(FunctionType::new(edited))));
    }
}
