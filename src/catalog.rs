//! The operation catalog (M4.6): what each builtin operation is, declared in
//! one place. Its first slice holds the intrinsics' effect classes, their
//! reliance on replaceable host builtins, and their JavaScript spellings:
//! form, arity and the int32 facts of their results. Checking, effects and
//! the JavaScript target read them here.
use crate::primitive::{Intrinsic, ResolvedIntrinsic};
use crate::typed_array::TypedArrayKind;

/// How an intrinsic touches its receiver and arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectClass {
    /// A computation over primitives, which convert their inputs.
    Pure {
        throws: bool,
        fresh: bool,
    },
    /// No input is converted and nothing is read or written.
    Inert {
        throws: bool,
    },
    /// Reads the receiver container.
    Read {
        fresh: bool,
    },
    /// Writes the receiver container.
    Write {
        throws: bool,
    },
    /// Creates a new object from primitive inputs.
    Construct {
        throws: bool,
    },
    /// Calls its first argument once per element of the receiver.
    Callback,
    Print,
    Unknown,
}

pub(crate) fn effect_class(operation: ResolvedIntrinsic) -> EffectClass {
    use Intrinsic as I;
    use EffectClass as Class;
    let intrinsic = match operation {
        ResolvedIntrinsic::Property(intrinsic)
        | ResolvedIntrinsic::Method(intrinsic)
        | ResolvedIntrinsic::Constructor(intrinsic) => intrinsic,
    };
    if let ResolvedIntrinsic::Constructor(intrinsic) = operation {
        return match intrinsic {
            I::MapNew | I::SetNew | I::SymbolNew => Class::Construct { throws: false },
            I::ArrayBufferNew | I::SharedArrayBufferNew | I::RegexNew => {
                Class::Construct { throws: true }
            }
            _ if typed_array_constructor(intrinsic) => Class::Construct { throws: true },
            _ => Class::Unknown,
        };
    }
    if callback_intrinsic(operation) {
        return Class::Callback;
    }
    match intrinsic {
        I::IntImul
        | I::IntToString
        | I::IntToUnsignedString
        | I::FloatAbs
        | I::FloatFloor
        | I::FloatCeil
        | I::FloatRound
        | I::FloatSqrt
        | I::FloatSin
        | I::FloatCos
        | I::FloatAcos
        | I::FloatExp
        | I::FloatLog
        | I::FloatTan
        | I::FloatAtan2
        | I::FloatHypot
        | I::FloatMin
        | I::FloatMax
        | I::FloatToInt
        | I::StringLength
        | I::StringCharCodeAt
        | I::StringCharAt
        | I::StringIncludes
        | I::StringIndexOf
        | I::StringLastIndexOf
        | I::StringStartsWith
        | I::StringEndsWith
        | I::StringToUpperCase
        | I::StringToLowerCase
        | I::StringTrim
        | I::StringTrimStart
        | I::StringTrimEnd
        | I::StringSlice
        | I::StringCodePointLength
        | I::JsMathPI => Class::Pure {
            throws: false,
            fresh: false,
        },
        I::StringSplit => Class::Pure {
            throws: false,
            fresh: true,
        },
        I::StringRepeat => Class::Pure {
            throws: true,
            fresh: false,
        },
        I::JsTruthy
        | I::JsTypeOf
        | I::JsIsNullish
        | I::JsIsFalse
        | I::JsIsUndefined
        | I::JsStrictEqual
        | I::JsStrictNotEqual => Class::Inert { throws: false },
        I::JsIsArray => Class::Inert { throws: true },
        I::ArrayLength
        | I::ArrayIndexOf
        | I::ArrayIncludes
        | I::MapSize
        | I::MapGet
        | I::MapHas
        | I::SetSize
        | I::SetHas
        | I::BufferByteLength
        | I::RegexSource
        | I::RegexFlags
        | I::RegexGlobal
        | I::RegexIgnoreCase
        | I::RegexMultiline
        | I::RegexDotAll
        | I::RegexSticky
        | I::RegexUnicode => Class::Read { fresh: false },
        I::ArraySlice | I::BufferSlice => Class::Read { fresh: true },
        I::ArrayPush
        | I::ArrayPop
        | I::ArraySplice
        | I::ArrayFill
        | I::ArrayCopyWithin
        | I::ArrayReverse
        | I::TypedArrayFill
        | I::TypedArrayCopyWithin
        | I::MapSet
        | I::MapDelete
        | I::MapClear
        | I::SetAdd
        | I::SetDelete
        | I::SetClear => Class::Write { throws: false },
        // An offset past the end throws a RangeError.
        I::TypedArraySet => Class::Write { throws: true },
        I::Print => Class::Print,
        _ => match typed_array_member(intrinsic) {
            Some(fresh) => Class::Read { fresh },
            None => Class::Unknown,
        },
    }
}

/// Whether an intrinsic dispatches through a builtin the host can replace: a
/// prototype method or accessor, or a global constructor or namespace. Its
/// language meaning is what the `pure` contract judges; that the running
/// builtin still has it is an assumption about the environment, so it is an
/// undischarged obligation for removal. The primitive string and array
/// `length`, and the language operators, dispatch through nothing.
pub(crate) fn host_replaceable(operation: ResolvedIntrinsic) -> bool {
    let (ResolvedIntrinsic::Property(intrinsic)
    | ResolvedIntrinsic::Method(intrinsic)
    | ResolvedIntrinsic::Constructor(intrinsic)) = operation;
    !matches!(
        intrinsic,
        Intrinsic::StringLength
            | Intrinsic::ArrayLength
            | Intrinsic::JsTruthy
            | Intrinsic::JsTypeOf
            | Intrinsic::JsIsNullish
            | Intrinsic::JsIsFalse
            | Intrinsic::JsIsUndefined
            | Intrinsic::JsStrictEqual
            | Intrinsic::JsStrictNotEqual
    )
}

/// The currently supported semantic subset. Both source lowering and target
/// verification use this boundary; unsupported operations never reach printing.
pub(crate) fn intrinsic_arity(operation: Intrinsic) -> Option<std::ops::RangeInclusive<usize>> {
    intrinsic_recipe(operation).map(|recipe| recipe.arguments)
}

/// Native spelling is a target choice. The semantic operation comes from the
/// source checker. Verification, naming and printing consume this one target
/// contract; the typed-array kind/name relationship has one existing owner.
pub(crate) struct NativeConstructor {
    pub(crate) name: &'static str,
    pub(crate) arity: usize,
}

pub(crate) fn native_constructor(operation: Intrinsic) -> Option<NativeConstructor> {
    let (name, arity) = match operation {
        Intrinsic::MapNew => ("Map", 0),
        Intrinsic::SetNew => ("Set", 0),
        Intrinsic::ArrayBufferNew => ("ArrayBuffer", 1),
        Intrinsic::SharedArrayBufferNew => ("SharedArrayBuffer", 1),
        operation => {
            let (kind, use_) = crate::typed_array::classify_typed_array_intrinsic(operation)?;
            if use_ != crate::typed_array::TypedArrayIntrinsic::New {
                return None;
            }
            (kind.name(), 1)
        }
    };
    Some(NativeConstructor { name, arity })
}

/// The selected JavaScript implementation. A checked language operation can
/// use a mutable prototype method; its raw target result/effects must then be
/// proved independently of its source signature.
#[derive(Clone, Copy)]
pub(crate) enum IntrinsicForm {
    Property(&'static str),
    Method(&'static str),
}

pub(crate) fn intrinsic_form(operation: Intrinsic) -> IntrinsicForm {
    intrinsic_recipe(operation)
        .expect("verified intrinsic subset")
        .form
}

pub(crate) fn integer_intrinsic(operation: Intrinsic) -> bool {
    intrinsic_recipe(operation).is_some_and(|recipe| recipe.normalizes_i32)
}

/// Unpatched, these results are always int32: string lengths and positions
/// stay below 2^31 on every engine, as do collection sizes. A typed array's
/// byte counts can exceed that, and `charCodeAt` past the end is NaN.
pub(crate) fn pristine_int32_intrinsic(operation: Intrinsic) -> bool {
    matches!(
        operation,
        Intrinsic::StringLength
            | Intrinsic::StringIndexOf
            | Intrinsic::StringLastIndexOf
            | Intrinsic::StringSearch
            | Intrinsic::ArrayLength
            | Intrinsic::ArrayPush
            | Intrinsic::ArrayIndexOf
            | Intrinsic::ArrayFindIndex
            | Intrinsic::MapSize
            | Intrinsic::SetSize
    )
}

/// One target recipe owns spelling, emitted argument arity and normalization.
/// Source and native signatures stay with the language primitive contract.
pub(crate) struct IntrinsicRecipe {
    pub(crate) form: IntrinsicForm,
    pub(crate) arguments: std::ops::RangeInclusive<usize>,
    pub(crate) normalizes_i32: bool,
}

pub(crate) fn intrinsic_recipe(operation: Intrinsic) -> Option<IntrinsicRecipe> {
    use IntrinsicForm::{Method, Property};
    let (form, arguments, normalizes_i32) = match operation {
        Intrinsic::StringLength | Intrinsic::ArrayLength => (Property("length"), 0..=0, true),
        Intrinsic::StringCharCodeAt => (Method("charCodeAt"), 1..=1, true),
        Intrinsic::StringCharAt => (Method("charAt"), 1..=1, false),
        Intrinsic::StringIndexOf => (Method("indexOf"), 1..=2, true),
        Intrinsic::StringSlice => (Method("slice"), 1..=2, false),
        Intrinsic::StringSplit => (Method("split"), 1..=1, false),
        Intrinsic::StringRepeat => (Method("repeat"), 1..=1, false),
        Intrinsic::StringTrim => (Method("trim"), 0..=0, false),
        Intrinsic::StringTrimStart => (Method("trimStart"), 0..=0, false),
        Intrinsic::StringTrimEnd => (Method("trimEnd"), 0..=0, false),
        Intrinsic::StringToUpperCase => (Method("toUpperCase"), 0..=0, false),
        Intrinsic::StringToLowerCase => (Method("toLowerCase"), 0..=0, false),
        Intrinsic::StringReplace => (Method("replace"), 2..=2, false),
        Intrinsic::RegexTest => (Method("test"), 1..=1, false),
        Intrinsic::JsRegexExec => (Method("exec"), 1..=1, false),
        // Rows below mirror the old route's spelling. An `int` result
        // is normalized: a patched prototype method or getter may return any
        // value, and the typed contract does not assume pristine builtins.
        Intrinsic::StringIncludes => (Method("includes"), 1..=2, false),
        Intrinsic::StringStartsWith => (Method("startsWith"), 1..=2, false),
        Intrinsic::StringEndsWith => (Method("endsWith"), 1..=2, false),
        Intrinsic::StringLastIndexOf => (Method("lastIndexOf"), 1..=2, true),
        Intrinsic::StringSearch => (Method("search"), 1..=1, true),
        Intrinsic::IntToString | Intrinsic::IntToUnsignedString => {
            (Method("toString"), 0..=1, false)
        }
        Intrinsic::ArrayPush => (Method("push"), 1..=1, true),
        Intrinsic::ArrayIndexOf => (Method("indexOf"), 1..=1, true),
        Intrinsic::ArrayIncludes => (Method("includes"), 1..=2, false),
        Intrinsic::ArrayJoin => (Method("join"), 0..=1, false),
        Intrinsic::ArrayConcat => (Method("concat"), 1..=1, false),
        Intrinsic::ArrayCopyWithin | Intrinsic::TypedArrayCopyWithin => {
            (Method("copyWithin"), 2..=3, false)
        }
        Intrinsic::ArrayReverse => (Method("reverse"), 0..=0, false),
        Intrinsic::ArraySlice | Intrinsic::BufferSlice => (Method("slice"), 0..=2, false),
        Intrinsic::ArraySplice => (Method("splice"), 2..=2, false),
        Intrinsic::ArrayFill => (Method("fill"), 1..=1, false),
        Intrinsic::ArrayMap => (Method("map"), 1..=1, false),
        Intrinsic::ArrayFilter => (Method("filter"), 1..=1, false),
        Intrinsic::ArrayForEach => (Method("forEach"), 1..=1, false),
        Intrinsic::ArrayReduce => (Method("reduce"), 2..=2, false),
        Intrinsic::ArraySome => (Method("some"), 1..=1, false),
        Intrinsic::ArrayEvery => (Method("every"), 1..=1, false),
        Intrinsic::ArrayFindIndex => (Method("findIndex"), 1..=1, true),
        Intrinsic::MapSize | Intrinsic::SetSize => (Property("size"), 0..=0, true),
        Intrinsic::MapGet => (Method("get"), 1..=1, false),
        Intrinsic::MapSet => (Method("set"), 2..=2, false),
        Intrinsic::MapHas | Intrinsic::SetHas => (Method("has"), 1..=1, false),
        Intrinsic::MapDelete | Intrinsic::SetDelete => (Method("delete"), 1..=1, false),
        Intrinsic::MapClear | Intrinsic::SetClear => (Method("clear"), 0..=0, false),
        Intrinsic::SetAdd => (Method("add"), 1..=1, false),
        Intrinsic::BufferByteLength => (Property("byteLength"), 0..=0, true),
        Intrinsic::TypedArraySet => (Method("set"), 1..=2, false),
        Intrinsic::TypedArrayFill => (Method("fill"), 1..=3, false),
        Intrinsic::RegexSource => (Property("source"), 0..=0, false),
        Intrinsic::RegexFlags => (Property("flags"), 0..=0, false),
        Intrinsic::RegexGlobal => (Property("global"), 0..=0, false),
        Intrinsic::RegexIgnoreCase => (Property("ignoreCase"), 0..=0, false),
        Intrinsic::RegexMultiline => (Property("multiline"), 0..=0, false),
        Intrinsic::RegexDotAll => (Property("dotAll"), 0..=0, false),
        Intrinsic::RegexSticky => (Property("sticky"), 0..=0, false),
        Intrinsic::RegexUnicode => (Property("unicode"), 0..=0, false),
        operation => {
            use crate::typed_array::TypedArrayIntrinsic as Typed;
            let (_, use_) = crate::typed_array::classify_typed_array_intrinsic(operation)?;
            match use_ {
                Typed::Length => (Property("length"), 0..=0, true),
                Typed::ByteLength => (Property("byteLength"), 0..=0, true),
                Typed::ByteOffset => (Property("byteOffset"), 0..=0, true),
                Typed::Buffer => (Property("buffer"), 0..=0, false),
                Typed::Slice => (Method("slice"), 1..=2, false),
                Typed::Subarray => (Method("subarray"), 1..=2, false),
                _ => return None,
            }
        }
    };
    Some(IntrinsicRecipe {
        form,
        arguments,
        normalizes_i32,
    })
}

/// Intrinsics that call their first argument once per element and return.
pub(crate) fn callback_intrinsic(operation: ResolvedIntrinsic) -> bool {
    matches!(
        operation,
        ResolvedIntrinsic::Method(
            Intrinsic::ArrayMap
                | Intrinsic::ArrayFilter
                | Intrinsic::ArrayReduce
                | Intrinsic::ArrayForEach
                | Intrinsic::ArraySome
                | Intrinsic::ArrayEvery
                | Intrinsic::ArrayFindIndex
        )
    )
}

fn typed_array_constructor(intrinsic: Intrinsic) -> bool {
    TypedArrayKind::ALL
        .iter()
        .any(|kind| kind.new_intrinsic() == intrinsic)
}

/// A typed array property or view method: `Some(creates a view)`.
fn typed_array_member(intrinsic: Intrinsic) -> Option<bool> {
    TypedArrayKind::ALL.iter().find_map(|kind| {
        if [
            kind.length_intrinsic(),
            kind.byte_length_intrinsic(),
            kind.byte_offset_intrinsic(),
            kind.buffer_intrinsic(),
        ]
        .contains(&intrinsic)
        {
            Some(false)
        } else if [kind.slice_intrinsic(), kind.subarray_intrinsic()].contains(&intrinsic) {
            Some(true)
        } else {
            None
        }
    })
}

