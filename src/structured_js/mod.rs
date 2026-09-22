//! Experimental, structured JavaScript boundary for the architecture comparison.
//!
//! This is not selected by the production compiler. Expressions contain syntax,
//! lexical cells have independent `BindingId` handles. Their optional source
//! symbol and each operation's `SourceNodeId` retain provenance through target edits.
//! Arena handles locate target storage; source identities explain its origin.
//! There is deliberately no raw-code node or per-node rendered text.
//! Nonliteral expression handles own one syntax occurrence. Reusing an SSA
//! value requires a binding reference or an explicitly justified rematerialized
//! occurrence, never accidental duplication of a shared expression graph.

use crate::ast::SourceNodeId;
use crate::literal::StringValue;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::primitive::{IntBinary, Intrinsic};
use crate::semantic::SymbolId;
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;

pub mod analysis;
mod compact;
mod constants;
pub(crate) mod delivery;
pub mod extract;
mod literal_output;
pub use literal_output::LiteralOutput;
pub(crate) use literal_output::{LiteralAlternative, WeakLiteralObservation};
mod flow;
#[cfg(test)]
mod imports_tests;
mod inline;
#[cfg(test)]
mod literal_output_tests;
mod naming;
pub mod selection;
use naming::Names;
pub(crate) use naming::NamingProvenance;
pub mod lower;
pub mod optimize;
#[cfg(test)]
mod output_policy_tests;
mod plan;
mod print;
mod rewrite;
mod simplify;
pub(crate) use rewrite::literal_array_projection;
#[cfg(test)]
mod string_recipe_tests;
#[cfg(test)]
mod tests;
mod verify;
pub(crate) use verify::MAX_NESTING;

// Zero-based arena positions and optional absence share one word. The encoded
// value is private: consumers use the index, not a second identity mapping.
// Source SymbolId/SourceNodeId provenance is independent and keeps its own encoding.
macro_rules! target_handles {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(std::num::NonZeroU32);

        impl $name {
            pub fn new(index: usize) -> Self {
                Self::try_new(index).expect("structured target arena capacity exceeded")
            }

            pub(crate) fn try_new(index: usize) -> Option<Self> {
                u32::try_from(index).ok()
                    .and_then(|index| index.checked_add(1))
                    .and_then(std::num::NonZeroU32::new)
                    .map(Self)
            }

            pub const fn index(self) -> usize {
                self.0.get() as usize - 1
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.debug_tuple(stringify!($name)).field(&self.index()).finish()
            }
        }
    )+};
}

// BindingId denotes a target lexical cell. Distinct cells can share source
// provenance after cloning; synthesized cells need no source declaration.
target_handles!(ExprId, RegionId, ScopeId, FunctionId, BindingId);

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    String(StringValue),
    Bool(bool),
    Null,
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unary {
    Negate,
    Not,
    BitNot,
    Plus,
    TypeOf,
    Void,
    /// `delete target`; the operand is a member expression.
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    StrictEqual,
    StrictNotEqual,
    /// `==`/`!=`: host code's `value == null`, which (unlike strict
    /// comparison) also treats `document.all` as nullish.
    Equal,
    NotEqual,
    /// `key in object`; the key converts to a property key first.
    In,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
    Nullish,
}

impl IntBinary {
    pub(crate) fn javascript(self) -> Binary {
        match self {
            Self::Add => Binary::Add,
            Self::Subtract => Binary::Subtract,
            Self::Multiply => Binary::Multiply,
            Self::Divide => Binary::Divide,
            Self::Remainder => Binary::Remainder,
            Self::UnsignedShiftRight => Binary::UnsignedShiftRight,
        }
    }
    pub(super) fn from_javascript(op: Binary) -> Option<Self> {
        Some(match op {
            Binary::Add => Self::Add,
            Binary::Subtract => Self::Subtract,
            Binary::Multiply => Self::Multiply,
            Binary::Divide => Self::Divide,
            Binary::Remainder => Self::Remainder,
            Binary::UnsignedShiftRight => Self::UnsignedShiftRight,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Property {
    Named(String),
    Computed(ExprId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    String(StringValue),
    Expression(ExprId),
}

pub use crate::primitive::Invocation;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Binding(BindingId),
    /// An external identifier, checked as an identifier rather than code.
    Host(String),
    This,
    Unary {
        op: Unary,
        value: ExprId,
    },
    /// JavaScript's signed 32-bit conversion. Language lowering introduces
    /// this operation where the source contract requires normalization;
    /// removing it requires a range proof, independent of print policy.
    ToInt32(ExprId),
    /// A language operation, retained until the target makes its arithmetic
    /// choice. The input program never needs to reconstruct it from `|0`.
    IntBinary {
        op: IntBinary,
        left: ExprId,
        right: ExprId,
    },
    IntNegate(ExprId),
    /// A typed language operation. Its receiver/operands are values, not an
    /// arbitrary property lookup. Host dispatch remains Member plus Call.
    Intrinsic {
        operation: Intrinsic,
        receiver: ExprId,
        arguments: Vec<ExprId>,
    },
    /// A checked language construction. Its implicit native constructor lookup
    /// remains observable in the target; it is not a pure allocation promise.
    ConstructIntrinsic {
        operation: Intrinsic,
        arguments: Vec<ExprId>,
    },
    Binary {
        op: Binary,
        left: ExprId,
        right: ExprId,
    },
    Member {
        object: ExprId,
        property: Property,
    },
    Call {
        callee: ExprId,
        arguments: Vec<ExprId>,
        invocation: Invocation,
    },
    Construct {
        callee: ExprId,
        arguments: Vec<ExprId>,
    },
    Conditional {
        condition: ExprId,
        yes: ExprId,
        no: ExprId,
    },
    Assign {
        target: ExprId,
        value: ExprId,
    },
    Sequence(Vec<ExprId>),
    /// Cooked text and substitutions in evaluation order. Each substitution
    /// undergoes ToString before the next expression; this is not binary +.
    Template(Vec<TemplatePart>),
    Array(Vec<ExprId>),
    /// Computed keys are explicit; in particular `__proto__` is never silently
    /// converted between a data property and the prototype-setting syntax.
    Object(Vec<(Property, ExprId)>),
    Function(FunctionId),
    /// `class name extends base { constructor(...) {...} }` as a value: a
    /// real subclass of a host constructor. `name` is the class's observable
    /// `name`, independent of whichever binding holds it. Evaluating it reads
    /// `base`, which throws unless it is a constructor.
    Class {
        name: String,
        base: ExprId,
        constructor: FunctionId,
    },
    /// `super(arguments)`, only in a class constructor: runs the base
    /// constructor, after which `this` is the new instance.
    SuperCall {
        arguments: Vec<ExprId>,
    },
    /// `...value`, only as an array-literal element: the value is iterated
    /// through its (possibly patched) iterator protocol.
    Spread(ExprId),
    /// `await value`, only in an async function. Any code may run while the
    /// function is suspended.
    Await(ExprId),
    /// `yield value` or `yield* value`, only in a generator. The consumer
    /// runs while the generator is suspended.
    Yield {
        value: ExprId,
        delegate: bool,
    },
    /// A regular-expression literal, its complete source text (`/p/f`). Each
    /// evaluation creates a fresh `RegExp`, so it is never copied or shared.
    Regex(String),
    /// `import()` of source module `module`: a promise of its namespace. A
    /// module delivered in its own lazy chunk loads that file; otherwise the
    /// namespace is an object of `members`, built a turn later, once every
    /// module has initialized. A failed load rejects with
    /// `{specifier, message}`.
    LoadModule {
        module: u32,
        /// The import's specifier, as a failed load reports it.
        specifier: String,
        /// Each member's export name and binding.
        members: Vec<(String, ExprId)>,
        /// The host `Promise` and `String`, as checked external references.
        promise: ExprId,
        string: ExprId,
    },
}

impl Expr {
    /// The function this expression creates: a function value, or the
    /// constructor of a class value.
    pub(crate) fn created_function(&self) -> Option<FunctionId> {
        match *self {
            Self::Function(function) | Self::Class { constructor: function, .. } => Some(function),
            _ => None,
        }
    }
    pub(super) fn remap_children(&mut self, mut map: impl FnMut(ExprId) -> ExprId) {
        let property = |key: &mut Property, map: &mut dyn FnMut(ExprId) -> ExprId| {
            if let Property::Computed(value) = key {
                *value = map(*value);
            }
        };
        match self {
            Self::Unary { value, .. }
            | Self::ToInt32(value)
            | Self::IntNegate(value)
            | Self::Spread(value)
            | Self::Await(value)
            | Self::Yield { value, .. } => *value = map(*value),
            Self::LoadModule {
                members,
                promise,
                string,
                ..
            } => {
                *promise = map(*promise);
                *string = map(*string);
                for (_, member) in members {
                    *member = map(*member);
                }
            }
            Self::Binary { left, right, .. } | Self::IntBinary { left, right, .. } => {
                *left = map(*left);
                *right = map(*right);
            }
            Self::Member {
                object,
                property: key,
            } => {
                *object = map(*object);
                property(key, &mut map);
            }
            Self::Call {
                callee, arguments, ..
            }
            | Self::Intrinsic {
                receiver: callee,
                arguments,
                ..
            }
            | Self::Construct { callee, arguments } => {
                *callee = map(*callee);
                for value in arguments {
                    *value = map(*value);
                }
            }
            Self::Class { base, .. } => *base = map(*base),
            Self::SuperCall { arguments } => {
                for value in arguments {
                    *value = map(*value);
                }
            }
            Self::Conditional { condition, yes, no } => {
                *condition = map(*condition);
                *yes = map(*yes);
                *no = map(*no);
            }
            Self::Assign { target, value } => {
                *target = map(*target);
                *value = map(*value);
            }
            Self::Sequence(values)
            | Self::Array(values)
            | Self::ConstructIntrinsic {
                arguments: values, ..
            } => {
                for value in values {
                    *value = map(*value);
                }
            }
            Self::Object(entries) => {
                for (key, value) in entries {
                    property(key, &mut map);
                    *value = map(*value);
                }
            }
            Self::Template(parts) => {
                for part in parts {
                    if let TemplatePart::Expression(value) = part {
                        *value = map(*value);
                    }
                }
            }
            Self::Literal(_)
            | Self::Binding(_)
            | Self::Host(_)
            | Self::Regex(_)
            | Self::This
            | Self::Function(_) => {}
        }
    }
    pub(super) fn visit_children<E>(
        &self,
        mut visit: impl FnMut(ExprId) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Literal(_)
            | Self::Binding(_)
            | Self::Host(_)
            | Self::Regex(_)
            | Self::This
            | Self::Function(_) => {}
            Self::Unary { value, .. }
            | Self::ToInt32(value)
            | Self::IntNegate(value)
            | Self::Spread(value)
            | Self::Await(value)
            | Self::Yield { value, .. } => visit(*value)?,
            Self::LoadModule {
                members,
                promise,
                string,
                ..
            } => {
                visit(*promise)?;
                visit(*string)?;
                for (_, member) in members {
                    visit(*member)?;
                }
            }
            Self::Binary { left, right, .. } | Self::IntBinary { left, right, .. } => {
                visit(*left)?;
                visit(*right)?;
            }
            Self::Member { object, property } => {
                visit(*object)?;
                if let Property::Computed(key) = property {
                    visit(*key)?;
                }
            }
            Self::Call {
                callee, arguments, ..
            }
            | Self::Intrinsic {
                receiver: callee,
                arguments,
                ..
            }
            | Self::Construct { callee, arguments } => {
                visit(*callee)?;
                for argument in arguments {
                    visit(*argument)?;
                }
            }
            Self::Class { base, .. } => visit(*base)?,
            Self::SuperCall { arguments } => {
                for argument in arguments {
                    visit(*argument)?;
                }
            }
            Self::Conditional { condition, yes, no } => {
                visit(*condition)?;
                visit(*yes)?;
                visit(*no)?;
            }
            Self::Assign { target, value } => {
                visit(*target)?;
                visit(*value)?;
            }
            Self::Sequence(values)
            | Self::Array(values)
            | Self::ConstructIntrinsic {
                arguments: values, ..
            } => {
                for value in values {
                    visit(*value)?;
                }
            }
            Self::Object(entries) => {
                for (key, value) in entries {
                    if let Property::Computed(key) = key {
                        visit(*key)?;
                    }
                    visit(*value)?;
                }
            }
            Self::Template(parts) => {
                for part in parts {
                    if let TemplatePart::Expression(value) = part {
                        visit(*value)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// The currently supported semantic subset. Both source lowering and target
/// verification use this boundary; unsupported operations never reach printing.
fn intrinsic_arity(operation: Intrinsic) -> Option<std::ops::RangeInclusive<usize>> {
    intrinsic_recipe(operation).map(|recipe| recipe.arguments)
}

/// Native spelling is a target choice. The semantic operation comes from the
/// source checker. Verification, naming and printing consume this one target
/// contract; the typed-array kind/name relationship has one existing owner.
struct NativeConstructor {
    name: &'static str,
    arity: usize,
}

fn native_constructor(operation: Intrinsic) -> Option<NativeConstructor> {
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
enum IntrinsicForm {
    Property(&'static str),
    Method(&'static str),
}

fn intrinsic_form(operation: Intrinsic) -> IntrinsicForm {
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
struct IntrinsicRecipe {
    form: IntrinsicForm,
    arguments: std::ops::RangeInclusive<usize>,
    normalizes_i32: bool,
}

fn intrinsic_recipe(operation: Intrinsic) -> Option<IntrinsicRecipe> {
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
        // Rows below mirror the legacy emitter's spelling. An `int` result
        // is normalized: a patched prototype method or getter may return any
        // value, and the typed contract does not assume pristine builtins.
        Intrinsic::StringIncludes => (Method("includes"), 1..=2, false),
        Intrinsic::StringStartsWith => (Method("startsWith"), 1..=2, false),
        Intrinsic::StringEndsWith => (Method("endsWith"), 1..=2, false),
        Intrinsic::StringLastIndexOf => (Method("lastIndexOf"), 1..=2, true),
        Intrinsic::StringSearch => (Method("search"), 1..=1, true),
        Intrinsic::IntToString | Intrinsic::IntToUnsignedString => (Method("toString"), 0..=1, false),
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

pub(crate) fn supports_intrinsic_method(operation: Intrinsic) -> bool {
    intrinsic_recipe(operation)
        .is_some_and(|recipe| matches!(recipe.form, IntrinsicForm::Method(_)))
}

/// `new Map()`, `new Uint8Array(n)`: a checked construction the target
/// spells with its native constructor at exactly that arity.
pub(crate) fn supports_intrinsic_construction(operation: Intrinsic, arguments: usize) -> bool {
    native_constructor(operation).is_some_and(|constructor| constructor.arity == arguments)
}

pub(crate) fn supports_intrinsic_property(operation: Intrinsic) -> bool {
    intrinsic_recipe(operation)
        .is_some_and(|recipe| matches!(recipe.form, IntrinsicForm::Property(_)))
}

/// Typed operations the legacy emitter spells as a host namespace function,
/// with the source receiver (if any) as the first argument. They lower to an
/// ordinary host member call; the namespace lookup stays observable.
pub(crate) fn intrinsic_host_function(
    operation: Intrinsic,
    edition: crate::js_syntax_target::EcmaScriptEdition,
) -> Option<&'static [&'static str]> {
    use crate::js_syntax_target::JsSyntaxFeature;
    Some(match operation {
        Intrinsic::FloatAbs => &["Math", "abs"],
        Intrinsic::FloatFloor => &["Math", "floor"],
        Intrinsic::FloatCeil => &["Math", "ceil"],
        Intrinsic::FloatRound => &["Math", "round"],
        Intrinsic::FloatSqrt => &["Math", "sqrt"],
        Intrinsic::FloatSin => &["Math", "sin"],
        Intrinsic::FloatCos => &["Math", "cos"],
        Intrinsic::FloatAcos => &["Math", "acos"],
        Intrinsic::FloatExp => &["Math", "exp"],
        Intrinsic::FloatLog => &["Math", "log"],
        Intrinsic::FloatTan => &["Math", "tan"],
        Intrinsic::FloatAtan2 => &["Math", "atan2"],
        Intrinsic::FloatHypot => &["Math", "hypot"],
        Intrinsic::FloatMin => &["Math", "min"],
        Intrinsic::FloatMax => &["Math", "max"],
        Intrinsic::RecordKeys => &["Object", "keys"],
        Intrinsic::RecordValues if edition.allows(JsSyntaxFeature::ObjectValues) => {
            &["Object", "values"]
        }
        Intrinsic::RecordHasOwn if edition.allows(JsSyntaxFeature::ObjectHasOwn) => {
            &["Object", "hasOwn"]
        }
        Intrinsic::RecordHasOwn => &["Object", "prototype", "hasOwnProperty", "call"],
        Intrinsic::RecordAssign => &["Object", "assign"],
        Intrinsic::JsonStringify => &["JSON", "stringify"],
        Intrinsic::JsonParse => &["JSON", "parse"],
        Intrinsic::TaskResolve => &["Promise", "resolve"],
        Intrinsic::TaskReject => &["Promise", "reject"],
        Intrinsic::TaskAll => &["Promise", "all"],
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Let {
        binding: BindingId,
        value: Option<ExprId>,
    },
    Evaluate(ExprId),
    Return(Option<ExprId>),
    Throw(ExprId),
    If {
        condition: ExprId,
        yes: RegionId,
        no: Option<RegionId>,
    },
    /// Initialization lives in the enclosing region and runs once. A loop
    /// owns its repeated test, body and optional update; continue reaches the
    /// update, while break/return/throw skip it. Body declarations create fresh
    /// cells each iteration; enclosing declarations retain their cell identity.
    Loop {
        condition: Option<ExprId>,
        update: Option<ExprId>,
        body: RegionId,
    },
    /// The finalizer executes for normal and abrupt completions, and can
    /// replace a pending return/throw/loop transfer with its own completion.
    Try {
        body: RegionId,
        catch: Option<Catch>,
        finally: Option<RegionId>,
    },
    Block(RegionId),
    /// `for (let binding in object) body`: the object is evaluated once; each
    /// iteration binds a fresh `binding` to the next enumerable string key,
    /// own or inherited, that still exists. break/continue target this loop.
    ForIn {
        binding: BindingId,
        object: ExprId,
        body: RegionId,
    },
    /// `for (let binding of iterable) body`: the iterable is evaluated once;
    /// each iteration binds a fresh `binding` to the next value its iterator
    /// produces. Leaving early closes the iterator. break/continue target it.
    ForOf {
        binding: BindingId,
        iterable: ExprId,
        body: RegionId,
    },
    Break,
    Continue,
    Function {
        binding: BindingId,
        function: FunctionId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Catch {
    pub binding: Option<BindingId>,
    pub body: RegionId,
}

impl Statement {
    /// Point the statement's own root expression at `value`.
    fn replace_root(&mut self, replacement: ExprId) {
        match self {
            Self::Return(Some(value))
            | Self::Evaluate(value)
            | Self::Throw(value)
            | Self::Let {
                value: Some(value),
                ..
            }
            | Self::If {
                condition: value, ..
            }
            | Self::ForIn { object: value, .. }
            | Self::ForOf {
                iterable: value, ..
            } => *value = replacement,
            _ => {}
        }
    }

    /// Immediate expression owners only. Child regions own their expressions;
    /// consumers that need evaluation order must handle control flow explicitly.
    pub(super) fn visit_expressions(&self, mut visit: impl FnMut(ExprId)) {
        match self {
            Self::Let { value, .. } | Self::Return(value) => {
                if let Some(value) = value {
                    visit(*value);
                }
            }
            Self::Evaluate(value) | Self::Throw(value) => visit(*value),
            Self::If { condition, .. } => visit(*condition),
            Self::ForIn { object, .. } | Self::ForOf { iterable: object, .. } => visit(*object),
            Self::Loop {
                condition, update, ..
            } => {
                if let Some(condition) = condition {
                    visit(*condition);
                }
                if let Some(update) = update {
                    visit(*update);
                }
            }
            _ => {}
        }
    }

    /// Immediate child regions; a declared function's body is not one.
    pub(super) fn visit_regions(&self, mut visit: impl FnMut(RegionId)) {
        match self {
            Self::If { yes, no, .. } => {
                visit(*yes);
                if let Some(no) = no {
                    visit(*no);
                }
            }
            Self::Loop { body, .. }
            | Self::ForIn { body, .. }
            | Self::ForOf { body, .. }
            | Self::Block(body) => visit(*body),
            Self::Try {
                body,
                catch,
                finally,
            } => {
                visit(*body);
                if let Some(catch) = catch {
                    visit(catch.body);
                }
                if let Some(finally) = finally {
                    visit(*finally);
                }
            }
            _ => {}
        }
    }

    pub(super) fn remap_expressions(&mut self, mut map: impl FnMut(ExprId) -> ExprId) {
        match self {
            Self::Let {
                value: Some(value), ..
            }
            | Self::Evaluate(value)
            | Self::Return(Some(value))
            | Self::Throw(value) => *value = map(*value),
            Self::If { condition, .. } => *condition = map(*condition),
            Self::ForIn { object, .. } | Self::ForOf { iterable: object, .. } => *object = map(*object),
            Self::Loop {
                condition, update, ..
            } => {
                *condition = condition.map(&mut map);
                *update = update.map(&mut map);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub scope: ScopeId,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionName {
    /// No name observation is required by this program's contract.
    Unobserved,
    /// The created value owns this name, including the empty anonymous name.
    /// It survives moves and removal of the binding that originally inferred it.
    Exact(StringValue),
}

impl FunctionName {
    fn exact(&self) -> Option<&StringValue> {
        match self {
            Self::Unobserved => None,
            Self::Exact(name) => Some(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub parameters: Vec<BindingId>,
    pub body: RegionId,
    pub arrow: bool,
    pub name: FunctionName,
    /// Printed with a `"use strict"` directive: a strict frame hides its
    /// caller and arguments from a sloppy host inside a classic script.
    pub strict: bool,
    /// The reflected `length` when it is shorter than the parameter count:
    /// JavaScript counts parameters before the first default, so each later
    /// one prints as `p=void 0`. The body still applies the real default.
    pub length: Option<usize>,
    /// `async function` / `function*`: only such a body may `await` or
    /// `yield`. A generator is never an arrow.
    pub suspension: Suspension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Suspension {
    #[default]
    None,
    Async,
    Generator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub source_symbol: Option<SymbolId>,
    pub scope: ScopeId,
    pub spelling: String,
    /// Exact observable lexical spelling. Callable reflection belongs to the
    /// function value, not every parameter or alias whose type is callable.
    pub pinned: bool,
}

/// One authored named ESM import. Rows retain module-request order even when
/// the local binding is unused: linking and module initialization are effects.
/// The imported spelling is an IdentifierName, like the existing Export name;
/// only the local lexical BindingId participates in naming. No namespace read
/// or snapshot assignment implements this live binding.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub source: StringValue,
    pub imported: String,
    pub binding: BindingId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    pub binding: BindingId,
    /// Stable public name; naming plans choose only the local binding spelling.
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub expressions: Vec<Expr>,
    pub origins: Vec<Option<SourceNodeId>>,
    pub regions: Vec<Region>,
    pub functions: Vec<Function>,
    pub scopes: Vec<Option<ScopeId>>,
    pub bindings: Vec<Binding>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub root: RegionId,
    /// The artifact's contract assumes unpatched builtins: an intrinsic whose
    /// specified result is always an int32 needs no `|0`.
    pub pristine_builtins: bool,
    /// The source module of each root statement, in order, when the producer
    /// records it; multi-file delivery groups statements by it.
    pub root_modules: Vec<u32>,
    /// Names delivered host code reads as globals from inside this module's
    /// scope; no binding of this module may take one.
    pub reserved: Vec<String>,
    /// Import sources the output carries instead of importing; a classic
    /// script can use these.
    pub carried: Vec<String>,
    /// Print `{let i=v;for(;c;u)b}` as `for(let i=v;c;u)b`. Shorter, but
    /// measured +125 Brotli on katexlil for -243 raw (neutral elsewhere), so
    /// it waits for 010 to score it per artifact.
    pub loop_head_declarations: bool,
    /// Print `if(c)e;` as `c&&e;` (and `if(!c)e;` as `c||e;`) where neither
    /// side needs grouping. Shorter, but measured +34 Brotli on zodlil and
    /// +38 on katexlil (-4 on markedlil), so it waits for 010 as well.
    pub logical_statements: bool,
}

impl Default for Module {
    fn default() -> Self {
        Self::new_in(&mut AllocationBudget::new(None)).expect("structured target allocation failed")
    }
}

/// Where a statement's first-evaluated leaf sits: a statement's own root
/// expression, or a child of another expression.
#[derive(Clone, Copy)]
enum Leaf {
    Root,
    Child(ExprId),
}

impl Module {
    /// `let x=void 0` is `let x` and `return void 0` is `return`: a `let`
    /// without a value still initializes to undefined each time it runs. A
    /// bare `return` ending a function body is where the body ends anyway.
    /// Returns the number of elided values and statements.
    pub(crate) fn elide_undefined(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let expressions = &self.expressions;
        let mut elided = 0;
        for region in &mut self.regions {
            budget.work(
                crate::compilation_policy::WorkKind::Analysis,
                1 + region.statements.len() as u64,
            )?;
            for statement in &mut region.statements {
                if let Statement::Let { value, .. } | Statement::Return(value) = statement {
                    if value.is_some_and(|value| {
                        matches!(
                            expressions[value.index()],
                            Expr::Literal(Literal::Undefined)
                        )
                    }) {
                        *value = None;
                        elided += 1;
                    }
                }
            }
        }
        budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            self.functions.len() as u64,
        )?;
        for function in &self.functions {
            let statements = &mut self.regions[function.body.index()].statements;
            if matches!(statements.last(), Some(Statement::Return(None))) {
                statements.pop();
                elided += 1;
            }
        }
        Ok(elided)
    }

    /// `let x;…;x=v` becomes `…;let x=v` when that assignment is the first
    /// code of the region to mention `x` and `v` does not: nothing before it
    /// can read `x`, and no hoisted declaration of the region mentions it, so
    /// no read meets the later declaration's temporal dead zone. With
    /// `prunes`, a bare statement whose value is only a literal, a function
    /// or a literal of those has no effect and goes. Returns the number of
    /// edits.
    pub(crate) fn merge_declarations(
        &mut self,
        prunes: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut edits = 0;
        for region in 0..self.regions.len() {
            budget.work(Analysis, 1 + self.regions[region].statements.len() as u64)?;
            let root = region == self.root.index();
            let mut index = 0;
            while prunes && index < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                if let Statement::Evaluate(value) = self.regions[region].statements[index] {
                    if self.inert_value(value, budget)? {
                        self.regions[region].statements.remove(index);
                        if root && index < self.root_modules.len() {
                            self.root_modules.remove(index);
                        }
                        edits += 1;
                        continue;
                    }
                }
                index += 1;
            }
            let declared: Vec<BindingId> = self.regions[region]
                .statements
                .iter()
                .filter_map(|statement| match statement {
                    Statement::Let {
                        binding,
                        value: None,
                    } => Some(*binding),
                    _ => None,
                })
                .collect();
            if declared.is_empty() {
                continue;
            }
            let first = self.first_mentions(RegionId::new(region), &declared, budget)?;
            let mut merges = Vec::new();
            for (index, statement) in self.regions[region].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                let Statement::Let {
                    binding,
                    value: None,
                } = *statement
                else {
                    continue;
                };
                let Some(&target) = first.get(&binding) else {
                    continue;
                };
                if target <= index
                    || (root && self.root_modules.get(index) != self.root_modules.get(target))
                {
                    continue;
                }
                let Statement::Evaluate(store) = self.regions[region].statements[target] else {
                    continue;
                };
                let Expr::Assign { target: place, value } = self.expressions[store.index()] else {
                    continue;
                };
                if matches!(self.expressions[place.index()], Expr::Binding(found) if found == binding)
                    && !self.mentions_within(value, binding, budget)?
                {
                    merges.push((index, target, binding, value));
                }
            }
            for &(_, target, binding, value) in &merges {
                self.regions[region].statements[target] = Statement::Let {
                    binding,
                    value: Some(value),
                };
            }
            for &(index, ..) in merges.iter().rev() {
                self.regions[region].statements.remove(index);
                if root && index < self.root_modules.len() {
                    self.root_modules.remove(index);
                }
            }
            edits += merges.len();
        }
        Ok(edits)
    }

    /// Whether evaluating `value` only creates literals and functions.
    fn inert_value(
        &self,
        value: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        Ok(match &self.expressions[value.index()] {
            Expr::Literal(_) | Expr::Function(_) => true,
            Expr::Array(items) => {
                let mut inert = true;
                for item in items {
                    inert = inert && self.inert_value(*item, budget)?;
                }
                inert
            }
            Expr::Object(entries) => {
                let mut inert = true;
                for (key, item) in entries {
                    inert = inert
                        && match key {
                            Property::Named(_) => true,
                            Property::Computed(key) => matches!(
                                self.expressions[key.index()],
                                Expr::Literal(Literal::String(_) | Literal::Number(_))
                            ),
                        }
                        && self.inert_value(*item, budget)?;
                }
                inert
            }
            _ => false,
        })
    }

    /// Number operations on literals become their results where the result
    /// is no longer: exact binary64 arithmetic, as in JavaScript, and the
    /// language's int32 contract for integer operations. String sums stay:
    /// computed, literal and shared spellings are the string family's choice
    /// for each codec. `protected` (ascending) lists literals with an
    /// observed alternative, left alone. Returns the number of folds.
    pub(crate) fn fold_literal_operations(
        &mut self,
        protected: &[ExprId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let free = |id: ExprId| protected.binary_search(&id).is_err();
        let number = |module: &Self, id: ExprId| match module.expressions[id.index()] {
            Expr::Literal(Literal::Number(value)) if free(id) => Some(value),
            _ => None,
        };
        let int32 = |value: f64| {
            (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
                .then(|| value as i32)
        };
        let spelled = |value: f64| print::number_spelling(value).len();
        let mut folds = 0;
        budget.work(Analysis, self.expressions.len() as u64)?;
        for index in 0..self.expressions.len() {
            let id = ExprId::new(index);
            let folded = match &self.expressions[index] {
                Expr::Binary {
                    op: Binary::Add,
                    left,
                    right,
                } => {
                    let (left, right) = (*left, *right);
                    match (&self.expressions[left.index()], &self.expressions[right.index()]) {
                        _ => match (number(self, left), number(self, right)) {
                            (Some(a), Some(b)) => Some(a + b)
                                .filter(|r| r.is_finite() && spelled(*r) <= spelled(a) + spelled(b) + 1)
                                .map(|r| Expr::Literal(Literal::Number(r))),
                            _ => None,
                        },
                    }
                }
                Expr::Binary { op, left, right }
                    if matches!(
                        op,
                        Binary::Subtract | Binary::Multiply | Binary::Divide | Binary::Remainder
                    ) =>
                {
                    match (number(self, *left), number(self, *right)) {
                        (Some(a), Some(b)) => Some(match op {
                            Binary::Subtract => a - b,
                            Binary::Multiply => a * b,
                            Binary::Divide => a / b,
                            _ => a % b,
                        })
                        .filter(|r| r.is_finite() && spelled(*r) <= spelled(a) + spelled(b) + 1)
                        .map(|r| Expr::Literal(Literal::Number(r))),
                        _ => None,
                    }
                }
                Expr::IntBinary { op, left, right } if *op != IntBinary::UnsignedShiftRight => {
                    match (
                        number(self, *left).and_then(int32),
                        number(self, *right).and_then(int32),
                    ) {
                        (Some(a), Some(b)) => Some(Expr::Literal(Literal::Number(f64::from(
                            op.evaluate(a, b),
                        )))),
                        _ => None,
                    }
                }
                Expr::ToInt32(value) => number(self, *value)
                    .and_then(int32)
                    .map(|a| Expr::Literal(Literal::Number(f64::from(a)))),
                Expr::Unary {
                    op: Unary::Plus,
                    value,
                } => number(self, *value).map(|a| Expr::Literal(Literal::Number(a))),
                _ => None,
            };
            if let Some(folded) = folded {
                self.expressions[id.index()] = folded;
                folds += 1;
            }
        }
        Ok(folds)
    }

    /// `!!x` is `x` where only its truth matters: a condition, an operand of
    /// `!`, a discarded value, or an operand of `&&`/`||` whose own truth is
    /// all that matters. Converting to a boolean runs no code. Returns the
    /// number of removed double negations.
    pub(crate) fn drop_double_negations(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let mut edits = 0;
        let mut regions = vec![self.root];
        let mut seen = vec![false; self.regions.len()];
        let mut pending: Vec<(ExprId, bool)> = Vec::new();
        while let Some(region) = regions.pop() {
            budget.work(Analysis, 1)?;
            if std::mem::replace(&mut seen[region.index()], true) {
                continue;
            }
            for index in 0..self.regions[region.index()].statements.len() {
                budget.work(Analysis, 1)?;
                let statement = &self.regions[region.index()].statements[index];
                match statement {
                    Statement::If { condition, .. } => pending.push((*condition, true)),
                    Statement::Loop {
                        condition, update, ..
                    } => {
                        if let Some(condition) = condition {
                            pending.push((*condition, true));
                        }
                        if let Some(update) = update {
                            pending.push((*update, true));
                        }
                    }
                    Statement::Evaluate(value) => pending.push((*value, true)),
                    other => other.visit_expressions(|root| pending.push((root, false))),
                }
                statement.visit_regions(|child| regions.push(child));
                if let Statement::Function { function, .. } = statement {
                    regions.push(self.functions[function.index()].body);
                }
                while let Some((id, truth)) = pending.pop() {
                    budget.work(Analysis, 1)?;
                    if truth {
                        while let Expr::Unary {
                            op: Unary::Not,
                            value: once,
                        } = self.expressions[id.index()]
                        {
                            let Expr::Unary {
                                op: Unary::Not,
                                value: twice,
                            } = self.expressions[once.index()]
                            else {
                                break;
                            };
                            self.expressions[id.index()] = self.expressions[twice.index()].clone();
                            edits += 1;
                        }
                    }
                    let expression = &self.expressions[id.index()];
                    if let Some(function) = expression.created_function() {
                        regions.push(self.functions[function.index()].body);
                    }
                    match *expression {
                        Expr::Unary {
                            op: Unary::Not,
                            value,
                        } => pending.push((value, true)),
                        Expr::Binary {
                            op: Binary::And | Binary::Or,
                            left,
                            right,
                        } => {
                            pending.push((left, truth));
                            pending.push((right, truth));
                        }
                        Expr::Conditional { condition, yes, no } => {
                            pending.push((condition, true));
                            pending.push((yes, truth));
                            pending.push((no, truth));
                        }
                        Expr::Sequence(ref items) => {
                            let last = items.len().saturating_sub(1);
                            for (position, item) in items.iter().enumerate() {
                                pending.push((*item, position != last || truth));
                            }
                        }
                        _ => {
                            let _ = expression.visit_children(|child| {
                                pending.push((child, false));
                                Ok::<_, ()>(())
                            });
                        }
                    }
                }
            }
        }
        Ok(edits)
    }

    /// A declaration that no code references goes when evaluating it has no
    /// effect: no value, or a value that only creates literals and functions
    /// (a function no code references is never called). A call spelled as the
    /// builtin it forwards to leaves such functions, and folds leave such
    /// declarations. Returns the number of dropped statements.
    pub(crate) fn drop_unreferenced_functions(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut dropped = 0;
        // Dropping one function can leave another unreferenced.
        for _ in 0..4 {
            let before = dropped;
            self.drop_unreferenced_functions_once(budget, &mut dropped)?;
            if dropped == before {
                break;
            }
        }
        Ok(dropped)
    }

    fn drop_unreferenced_functions_once(
        &mut self,
        budget: &mut AllocationBudget<'_>,
        dropped: &mut usize,
    ) -> Result<(), AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        // Only code that can run counts: edits leave unreachable nodes.
        let mut referenced = budget.filled(AllocationClass::Scratch, self.bindings.len(), false)?;
        self.walk(&mut vec![self.root], &mut Vec::new(), budget, |binding| {
            referenced[binding.index()] = true;
        })?;
        budget.work(Analysis, self.exports.len() as u64)?;
        for export in &self.exports {
            referenced[export.binding.index()] = true;
        }
        for region in 0..self.regions.len() {
            let root = region == self.root.index();
            let mut index = 0;
            while index < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                let unused = |binding: BindingId| {
                    !referenced[binding.index()] && !self.bindings[binding.index()].pinned
                };
                let drop = match &self.regions[region].statements[index] {
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => unused(*binding) && self.inert_value(*value, budget)?,
                    Statement::Let {
                        binding,
                        value: None,
                    } => unused(*binding),
                    Statement::Function { binding, .. } => unused(*binding),
                    _ => false,
                };
                if drop {
                    self.regions[region].statements.remove(index);
                    if root && index < self.root_modules.len() {
                        self.root_modules.remove(index);
                    }
                    *dropped += 1;
                } else {
                    index += 1;
                }
            }
        }
        Ok(())
    }

    /// `let o={…};o.k=v;o[1]=w` becomes `let o={…,k:v,1:w}`: the stores run
    /// right after the object exists, so each defines a data property of a
    /// fresh ordinary object. With pristine builtins no setter observes them
    /// (`__proto__` stays a store). The values evaluate in the same order and
    /// never mention `o`; nothing that could run first mentions it either: no
    /// earlier statement of its region and no hoisted declaration there.
    /// Returns the number of folded stores.
    pub(crate) fn fold_object_stores(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let depths = self.region_depths(budget)?;
        let mut folded = 0;
        for region in 0..self.regions.len() {
            let Some(region_depth) = depths[region] else {
                continue;
            };
            budget.work(Analysis, 1 + self.regions[region].statements.len() as u64)?;
            let candidates: Vec<BindingId> = self.regions[region]
                .statements
                .windows(2)
                .filter_map(|pair| match pair {
                    [Statement::Let {
                        binding,
                        value: Some(value),
                    }, Statement::Evaluate(store)]
                        if matches!(self.expressions[value.index()], Expr::Object(_))
                            && self.object_store(*store, *binding).is_some() =>
                    {
                        Some(*binding)
                    }
                    _ => None,
                })
                .collect();
            if candidates.is_empty() {
                continue;
            }
            let first = self.first_mentions(RegionId::new(region), &candidates, budget)?;
            let root = region == self.root.index();
            let mut index = 0;
            while index + 1 < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                let Statement::Let {
                    binding,
                    value: Some(object),
                } = self.regions[region].statements[index]
                else {
                    index += 1;
                    continue;
                };
                let Expr::Object(entries) = &self.expressions[object.index()] else {
                    index += 1;
                    continue;
                };
                if first
                    .get(&binding)
                    .is_none_or(|&mention| mention <= index)
                {
                    index += 1;
                    continue;
                }
                let mut entries = entries.clone();
                let mut end = index + 1;
                while let Some(Statement::Evaluate(store)) = self.regions[region].statements.get(end)
                {
                    budget.work(Analysis, 1)?;
                    if root && self.root_modules.get(end) != self.root_modules.get(index) {
                        break;
                    }
                    let Some((property, value)) = self.object_store(*store, binding) else {
                        break;
                    };
                    if self.mentions_within(value, binding, budget)? {
                        break;
                    }
                    // A store to a key the literal already has replaces that
                    // entry where it stands: the property keeps its first
                    // position either way. The old value must be inert (it no
                    // longer runs) and so must everything after it (the new
                    // value now runs first).
                    let replaced = match self.entry_position(&entries, &property) {
                        Some(position) if self.inert_value(entries[position].1, budget)? => {
                            let mut later = true;
                            for &(ref key, item) in &entries[position + 1..] {
                                later = later
                                    && !matches!(key, Property::Computed(key) if !matches!(self.expressions[key.index()], Expr::Literal(_)))
                                    && self.inert_value(item, budget)?;
                            }
                            later.then_some(position)
                        }
                        _ => None,
                    };
                    match replaced {
                        Some(position) => entries[position].1 = value,
                        None => entries.push((property, value)),
                    }
                    end += 1;
                }
                if end == index + 1 {
                    index += 1;
                    continue;
                }
                let origin = self.origins[object.index()];
                let id = self.expression_in(Expr::Object(entries), origin, budget)?;
                if region_depth + 1 + self.subtree_depth(id) > verify::MAX_NESTING {
                    self.expressions.pop();
                    self.origins.pop();
                    index += 1;
                    continue;
                }
                self.regions[region].statements[index].replace_root(id);
                folded += end - index - 1;
                self.regions[region].statements.drain(index + 1..end);
                if root {
                    let modules = end.min(self.root_modules.len());
                    if index + 1 < modules {
                        self.root_modules.drain(index + 1..modules);
                    }
                }
                index += 1;
            }
        }
        Ok(folded)
    }

    /// The entry of an object literal that defines the same key as `key`,
    /// for keys spelled as a name or a literal string.
    fn entry_position(&self, entries: &[(Property, ExprId)], key: &Property) -> Option<usize> {
        let text = |property: &Property| match property {
            Property::Named(name) => Some(StringValue::from(name.as_str())),
            Property::Computed(key) => match &self.expressions[key.index()] {
                Expr::Literal(Literal::String(value)) => Some(value.clone()),
                _ => None,
            },
        };
        let wanted = text(key)?;
        entries
            .iter()
            .position(|(property, _)| text(property).as_ref() == Some(&wanted))
    }

    /// `object.k=value` or `object["k"]=value` on `object`'s binding, as the
    /// object-literal entry it would define.
    fn object_store(&self, store: ExprId, object: BindingId) -> Option<(Property, ExprId)> {
        let Expr::Assign { target, value } = self.expressions[store.index()] else {
            return None;
        };
        let Expr::Member {
            object: receiver,
            property,
        } = &self.expressions[target.index()]
        else {
            return None;
        };
        if !matches!(self.expressions[receiver.index()], Expr::Binding(found) if found == object) {
            return None;
        }
        let entry = match property {
            Property::Named(name) if name != "__proto__" => Property::Named(name.clone()),
            Property::Computed(key) => match &self.expressions[key.index()] {
                Expr::Literal(Literal::String(name))
                    if name.as_unicode().is_some_and(|name| name != "__proto__") =>
                {
                    Property::Computed(*key)
                }
                Expr::Literal(Literal::Number(_)) => Property::Computed(*key),
                _ => return None,
            },
            _ => return None,
        };
        Some((entry, value))
    }

    /// For each binding, the index of the first statement of `region` whose
    /// code mentions it; a hoisted declaration's body counts as the first.
    pub(super) fn first_mentions(
        &self,
        region: RegionId,
        bindings: &[BindingId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<ahash::AHashMap<BindingId, usize>, AllocationError> {
        let mut first = ahash::AHashMap::<BindingId, usize>::default();
        for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
            let at = if matches!(statement, Statement::Function { .. }) {
                0
            } else {
                index
            };
            let mut regions = Vec::new();
            let mut expressions = Vec::new();
            statement.visit_expressions(|root| expressions.push(root));
            statement.visit_regions(|child| regions.push(child));
            if let Statement::Function { function, .. } = statement {
                regions.push(self.functions[function.index()].body);
            }
            self.walk(&mut regions, &mut expressions, budget, |binding| {
                if bindings.contains(&binding) {
                    first
                        .entry(binding)
                        .and_modify(|seen| *seen = (*seen).min(at))
                        .or_insert(at);
                }
            })?;
        }
        Ok(first)
    }

    /// Whether the code under `root` mentions `binding`.
    fn mentions_within(
        &self,
        root: ExprId,
        binding: BindingId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        let mut found = false;
        self.walk(&mut Vec::new(), &mut vec![root], budget, |seen| {
            found |= seen == binding;
        })?;
        Ok(found)
    }

    /// Every binding reference under the pending regions and expressions,
    /// function bodies included.
    fn walk(
        &self,
        regions: &mut Vec<RegionId>,
        expressions: &mut Vec<ExprId>,
        budget: &mut AllocationBudget<'_>,
        mut visit: impl FnMut(BindingId),
    ) -> Result<(), AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        loop {
            if let Some(id) = expressions.pop() {
                budget.work(Analysis, 1)?;
                let expression = &self.expressions[id.index()];
                if let Expr::Binding(binding) = expression {
                    visit(*binding);
                }
                if let Some(function) = expression.created_function() {
                    regions.push(self.functions[function.index()].body);
                }
                let _ = expression.visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, ()>(())
                });
                continue;
            }
            let Some(region) = regions.pop() else {
                return Ok(());
            };
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                statement.visit_expressions(|root| expressions.push(root));
                statement.visit_regions(|child| regions.push(child));
                if let Statement::Function { function, .. } = statement {
                    regions.push(self.functions[function.index()].body);
                }
            }
        }
    }

    /// `let x=v;S` becomes `S` with `v` in place of `x` when `x` is referenced
    /// exactly once, as the first thing `S` evaluates: the same evaluations
    /// in the same order, one binding fewer. A function or class value keeps
    /// its binding, which names it; a loop test repeats, so it never takes one.
    /// Root statements merge only within one source module. Returns the
    /// number of forwarded bindings.
    pub(crate) fn forward_single_uses(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut references =
            budget.filled(AllocationClass::Scratch, self.bindings.len(), 0u32)?;
        budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            self.exports.len() as u64,
        )?;
        // Only code that can run counts: edits leave unreachable nodes.
        self.walk(&mut vec![self.root], &mut Vec::new(), budget, |binding| {
            references[binding.index()] = references[binding.index()].saturating_add(1);
        })?;
        for export in &self.exports {
            references[export.binding.index()] = u32::MAX;
        }
        let depths = self.region_depths(budget)?;
        let mut forwarded = 0;
        for region in 0..self.regions.len() {
            let Some(region_depth) = depths[region] else {
                continue;
            };
            let root = region == self.root.index();
            let mut index = 0;
            while index + 1 < self.regions[region].statements.len() {
                budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                let Statement::Let {
                    binding,
                    value: Some(value),
                } = self.regions[region].statements[index]
                else {
                    index += 1;
                    continue;
                };
                let movable = references[binding.index()] == 1
                    && !self.bindings[binding.index()].pinned
                    && !matches!(
                        self.expressions[value.index()],
                        Expr::Function(_) | Expr::Class { .. }
                    )
                    && (!root
                        || self.root_modules.get(index) == self.root_modules.get(index + 1));
                let leaf = if movable {
                    self.first_leaf(&self.regions[region].statements[index + 1], binding)
                } else {
                    None
                };
                // An inert value (literals, and arrays or objects of them)
                // can be created later without any observer seeing it: it
                // may take its one reference anywhere the next statement
                // evaluates once.
                let leaf = match leaf {
                    Some(leaf) => Some(leaf),
                    None if movable && self.inert_value(value, budget)? => {
                        self.single_evaluation_reference(
                            &self.regions[region].statements[index + 1],
                            binding,
                        )
                    }
                    None => None,
                };
                // Children precede their parents in the arena, and the moved
                // value's deepest point must stay within the nesting limit.
                let fits = leaf.is_some_and(|(leaf, path)| {
                    let ordered = match leaf {
                        Leaf::Root => true,
                        Leaf::Child(parent) => value.index() < parent.index(),
                    };
                    ordered
                        && region_depth + 1 + path + self.subtree_depth(value)
                            <= verify::MAX_NESTING
                });
                budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                let Some((leaf, _)) = leaf.filter(|_| fits) else {
                    index += 1;
                    continue;
                };
                match leaf {
                    Leaf::Root => self.regions[region].statements[index + 1]
                        .replace_root(value),
                    Leaf::Child(parent) => {
                        let target = binding;
                        let expressions = &self.expressions;
                        let slot = {
                            let mut found = None;
                            let _ = expressions[parent.index()].visit_children(|child| {
                                if matches!(expressions[child.index()], Expr::Binding(b) if b == target)
                                {
                                    found.get_or_insert(child);
                                }
                                Ok::<_, ()>(())
                            });
                            found
                        };
                        let Some(slot) = slot else {
                            index += 1;
                            continue;
                        };
                        self.expressions[parent.index()]
                            .remap_children(|child| if child == slot { value } else { child });
                    }
                }
                self.regions[region].statements.remove(index);
                if root && index < self.root_modules.len() {
                    self.root_modules.remove(index);
                }
                references[binding.index()] = 0;
                forwarded += 1;
            }
        }
        Ok(forwarded)
    }

    /// Where `binding` is read in the expressions `statement` evaluates
    /// exactly once (not a loop's test or update, nor a nested function):
    /// the reading node's parent and depth below the statement root.
    fn single_evaluation_reference(
        &self,
        statement: &Statement,
        binding: BindingId,
    ) -> Option<(Leaf, usize)> {
        if matches!(statement, Statement::Loop { .. }) {
            return None;
        }
        let mut roots = Vec::new();
        statement.visit_expressions(|root| roots.push(root));
        let mut pending: Vec<(ExprId, Option<ExprId>, usize)> =
            roots.into_iter().map(|root| (root, None, 0)).collect();
        while let Some((id, parent, depth)) = pending.pop() {
            if matches!(self.expressions[id.index()], Expr::Binding(found) if found == binding) {
                return Some((parent.map_or(Leaf::Root, Leaf::Child), depth));
            }
            let _ = self.expressions[id.index()].visit_children(|child| {
                pending.push((child, Some(id), depth + 1));
                Ok::<_, ()>(())
            });
        }
        None
    }

    /// Each reachable region's nesting depth, as the verifier counts it.
    fn region_depths(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<Option<usize>>, AllocationError> {
        let mut depths = budget.filled(AllocationClass::Scratch, self.regions.len(), None)?;
        let mut regions = vec![(self.root, 0usize)];
        let mut expressions: Vec<(ExprId, usize)> = Vec::new();
        while let Some((region, depth)) = regions.pop() {
            budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
            if depths[region.index()].replace(depth).is_some() {
                continue;
            }
            for statement in &self.regions[region.index()].statements {
                statement.visit_expressions(|root| expressions.push((root, depth + 1)));
                match statement {
                    Statement::If { yes, no, .. } => {
                        regions.push((*yes, depth + 1));
                        if let Some(no) = no {
                            regions.push((*no, depth + 1));
                        }
                    }
                    Statement::Loop { body, .. }
                    | Statement::ForIn { body, .. }
                    | Statement::ForOf { body, .. }
                    | Statement::Block(body) => regions.push((*body, depth + 1)),
                    Statement::Try {
                        body,
                        catch,
                        finally,
                    } => {
                        regions.push((*body, depth + 1));
                        if let Some(catch) = catch {
                            regions.push((catch.body, depth + 1));
                        }
                        if let Some(finally) = finally {
                            regions.push((*finally, depth + 1));
                        }
                    }
                    Statement::Function { function, .. } => {
                        regions.push((self.functions[function.index()].body, depth + 2))
                    }
                    _ => {}
                }
                while let Some((id, at)) = expressions.pop() {
                    budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                    let expression = &self.expressions[id.index()];
                    if let Some(function) = expression.created_function() {
                        regions.push((self.functions[function.index()].body, at + 2));
                    }
                    let _ = expression.visit_children(|child| {
                        expressions.push((child, at + 1));
                        Ok::<_, ()>(())
                    });
                }
            }
        }
        Ok(depths)
    }

    /// The deepest point below `root`, relative to it, as the verifier counts
    /// depth, including regions of functions created there.
    fn subtree_depth(&self, root: ExprId) -> usize {
        let mut deepest = 0;
        let mut expressions = vec![(root, 0usize)];
        let mut regions: Vec<(RegionId, usize)> = Vec::new();
        loop {
            if let Some((id, at)) = expressions.pop() {
                deepest = deepest.max(at);
                let expression = &self.expressions[id.index()];
                if let Some(function) = expression.created_function() {
                    regions.push((self.functions[function.index()].body, at + 2));
                }
                let _ = expression.visit_children(|child| {
                    expressions.push((child, at + 1));
                    Ok::<_, ()>(())
                });
                continue;
            }
            let Some((region, depth)) = regions.pop() else {
                return deepest;
            };
            deepest = deepest.max(depth);
            for statement in &self.regions[region.index()].statements {
                statement.visit_expressions(|root| expressions.push((root, depth + 1)));
                match statement {
                    Statement::If { yes, no, .. } => {
                        regions.push((*yes, depth + 1));
                        if let Some(no) = no {
                            regions.push((*no, depth + 1));
                        }
                    }
                    Statement::Loop { body, .. }
                    | Statement::ForIn { body, .. }
                    | Statement::ForOf { body, .. }
                    | Statement::Block(body) => regions.push((*body, depth + 1)),
                    Statement::Try {
                        body,
                        catch,
                        finally,
                    } => {
                        regions.push((*body, depth + 1));
                        if let Some(catch) = catch {
                            regions.push((catch.body, depth + 1));
                        }
                        if let Some(finally) = finally {
                            regions.push((*finally, depth + 1));
                        }
                    }
                    Statement::Function { function, .. } => {
                        regions.push((self.functions[function.index()].body, depth + 2))
                    }
                    _ => {}
                }
            }
        }
    }

    /// The first leaf `statement` evaluates, when it is `binding` itself,
    /// with the number of expression steps from the statement's root to it.
    fn first_leaf(&self, statement: &Statement, binding: BindingId) -> Option<(Leaf, usize)> {
        let root = match statement {
            Statement::Return(Some(value))
            | Statement::Evaluate(value)
            | Statement::Throw(value)
            | Statement::Let {
                value: Some(value),
                ..
            }
            | Statement::If {
                condition: value, ..
            }
            | Statement::ForIn { object: value, .. }
            | Statement::ForOf {
                iterable: value, ..
            } => *value,
            _ => return None,
        };
        let mut current = root;
        let mut parent = Leaf::Root;
        let mut path = 0;
        loop {
            let next = match &self.expressions[current.index()] {
                Expr::Binding(found) => {
                    return (*found == binding).then_some((parent, path));
                }
                Expr::Unary { value, .. }
                | Expr::ToInt32(value)
                | Expr::IntNegate(value)
                | Expr::Spread(value)
                | Expr::Await(value)
                | Expr::Yield { value, .. } => *value,
                Expr::Binary { left, .. } | Expr::IntBinary { left, .. } => *left,
                Expr::Member { object, .. } => *object,
                Expr::Call { callee, .. } | Expr::Construct { callee, .. } => *callee,
                Expr::Intrinsic { receiver, .. } => *receiver,
                Expr::Conditional { condition, .. } => *condition,
                Expr::Assign { target, value } => match &self.expressions[target.index()] {
                    // Resolving a plain name observes nothing before the value.
                    Expr::Binding(_) => *value,
                    Expr::Member { object, .. } => {
                        parent = Leaf::Child(*target);
                        current = *object;
                        path += 2;
                        continue;
                    }
                    _ => return None,
                },
                Expr::Sequence(items) | Expr::Array(items) => *items.first()?,
                Expr::Template(parts) => match parts.iter().find_map(|part| match part {
                    TemplatePart::Expression(value) => Some(*value),
                    TemplatePart::String(_) => None,
                }) {
                    Some(value) => value,
                    None => return None,
                },
                Expr::Object(entries) => match entries.first()? {
                    (Property::Computed(key), _) => *key,
                    (Property::Named(_), value) => *value,
                },
                Expr::Class { base, .. } => *base,
                _ => return None,
            };
            parent = Leaf::Child(current);
            current = next;
            path += 1;
        }
    }

    /// Whether code under `regions` and `expressions` mentions `binding`;
    /// with `captured`, only inside a function created there.
    pub(crate) fn mentions(
        &self,
        regions: &[RegionId],
        expressions: &[ExprId],
        binding: BindingId,
        captured: bool,
    ) -> bool {
        enum Node {
            Region(RegionId, bool),
            Expr(ExprId, bool),
        }
        let mut stack: Vec<Node> = regions
            .iter()
            .map(|region| Node::Region(*region, false))
            .chain(expressions.iter().map(|expression| Node::Expr(*expression, false)))
            .collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Expr(id, inside) => {
                    let expression = &self.expressions[id.index()];
                    if matches!(expression, Expr::Binding(found) if *found == binding)
                        && (inside || !captured)
                    {
                        return true;
                    }
                    if let Some(function) = expression.created_function() {
                        stack.push(Node::Region(self.functions[function.index()].body, true));
                    }
                    let _ = expression.visit_children(|child| {
                        stack.push(Node::Expr(child, inside));
                        Ok::<_, ()>(())
                    });
                }
                Node::Region(region, inside) => {
                    for statement in &self.regions[region.index()].statements {
                        statement.visit_expressions(|root| stack.push(Node::Expr(root, inside)));
                        match statement {
                            Statement::If { yes, no, .. } => {
                                stack.push(Node::Region(*yes, inside));
                                if let Some(no) = no {
                                    stack.push(Node::Region(*no, inside));
                                }
                            }
                            Statement::Loop { body, .. }
                            | Statement::ForIn { body, .. }
                            | Statement::ForOf { body, .. }
                            | Statement::Block(body) => stack.push(Node::Region(*body, inside)),
                            Statement::Try {
                                body,
                                catch,
                                finally,
                            } => {
                                stack.push(Node::Region(*body, inside));
                                if let Some(catch) = catch {
                                    stack.push(Node::Region(catch.body, inside));
                                }
                                if let Some(finally) = finally {
                                    stack.push(Node::Region(*finally, inside));
                                }
                            }
                            Statement::Function { function, .. } => stack.push(Node::Region(
                                self.functions[function.index()].body,
                                true,
                            )),
                            _ => {}
                        }
                    }
                }
            }
        }
        false
    }

    pub(crate) fn new_in(budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        let scopes = budget.filled(AllocationClass::Retained, 1, None)?;
        let mut regions = budget.vector(AllocationClass::Retained, 1)?;
        regions.push(Region {
            scope: ScopeId::new(0),
            statements: Vec::new(),
        });
        Ok(Self {
            expressions: vec![],
            origins: vec![],
            functions: vec![],
            bindings: vec![],
            imports: vec![],
            exports: vec![],
            scopes,
            regions,
            root: RegionId::new(0),
            pristine_builtins: false,
            root_modules: vec![],
            reserved: vec![],
            carried: vec![],
            loop_head_declarations: false,
            logical_statements: false,
        })
    }

    pub fn binding(&mut self, binding: Binding) -> BindingId {
        self.binding_in(binding, &mut AllocationBudget::new(None))
            .expect("structured target allocation failed")
    }

    pub(crate) fn binding_in(
        &mut self,
        binding: Binding,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<BindingId, AllocationError> {
        let id = BindingId::try_new(self.bindings.len()).ok_or(AllocationError::Capacity)?;
        budget.push(AllocationClass::Retained, &mut self.bindings, binding)?;
        Ok(id)
    }

    /// Inspection construction delegates to the same admitted payload/row owner.
    /// Verification checks declarations and requested import-name syntax.
    pub fn import(&mut self, source: &str, imported: &str, binding: BindingId) {
        self.import_in(source, imported, binding, &mut AllocationBudget::new(None))
            .expect("structured target import allocation failed")
    }

    /// Copies payload only after admission, then commits one complete row.
    /// Failure drops temporary strings before releasing their reservations.
    pub(crate) fn import_in(
        &mut self,
        source: &str,
        imported: &str,
        binding: BindingId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let mut payload = budget.scope();
        let source = payload.string(AllocationClass::Retained, source)?;
        let imported = payload.string(AllocationClass::Retained, imported)?;
        let bytes = source
            .capacity()
            .checked_add(imported.capacity())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(AllocationError::Capacity)?;
        let row = Import {
            source: source.into(),
            imported,
            binding,
        };
        payload.finish_retained()?;
        if let Err(error) = budget.push(AllocationClass::Retained, &mut self.imports, row) {
            // push destroys the uncommitted row on failure. Existing vector
            // storage, if any, still belongs to the unchanged Module/parent.
            budget.release(AllocationClass::Retained, bytes)?;
            return Err(error);
        }
        Ok(())
    }

    pub fn expression(&mut self, expression: Expr, origin: Option<SourceNodeId>) -> ExprId {
        self.expression_in(expression, origin, &mut AllocationBudget::new(None))
            .expect("structured target allocation failed")
    }

    pub(crate) fn expression_in(
        &mut self,
        expression: Expr,
        origin: Option<SourceNodeId>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<ExprId, AllocationError> {
        let id = ExprId::try_new(self.expressions.len()).ok_or(AllocationError::Capacity)?;
        budget.reserve_vec(AllocationClass::Retained, &mut self.expressions, 1)?;
        budget.reserve_vec(AllocationClass::Retained, &mut self.origins, 1)?;
        self.expressions.push(expression);
        self.origins.push(origin);
        Ok(id)
    }

    pub fn region(&mut self, parent: ScopeId) -> RegionId {
        self.region_in(parent, &mut AllocationBudget::new(None))
            .expect("structured target allocation failed")
    }

    pub(crate) fn region_in(
        &mut self,
        parent: ScopeId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<RegionId, AllocationError> {
        let scope = ScopeId::try_new(self.scopes.len()).ok_or(AllocationError::Capacity)?;
        let id = RegionId::try_new(self.regions.len()).ok_or(AllocationError::Capacity)?;
        budget.reserve_vec(AllocationClass::Retained, &mut self.scopes, 1)?;
        budget.reserve_vec(AllocationClass::Retained, &mut self.regions, 1)?;
        self.scopes.push(Some(parent));
        self.regions.push(Region {
            scope,
            statements: vec![],
        });
        Ok(id)
    }

    pub fn verify(&self) -> Result<(), String> {
        verify::verify(self).map(|_| ())
    }

    /// Whether any expression names `name` as an external identifier,
    /// including a native constructor a checked construction looks up. A
    /// declaration with that required spelling would capture it.
    pub(crate) fn references_host(&self, name: &str) -> bool {
        self.expressions.iter().any(|expression| match expression {
            Expr::Host(host) => host == name,
            Expr::ConstructIntrinsic { operation, .. } => {
                native_constructor(*operation).is_some_and(|native| native.name == name)
            }
            _ => false,
        })
    }

    pub fn prepare_output(&self) -> Result<extract::Output<'_>, String> {
        extract::Output::from_module(self)
    }

    pub fn render(&self, policy: PrintPolicy) -> Result<String, String> {
        let names = Names::new(self, policy)?;
        Ok(print::render(self, &names, None))
    }
}

/// Explicit and immutable for a render. No thread-local or environment policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrintPolicy {
    pub mangle_bindings: bool,
}

pub(crate) fn identifier_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_' || b == b'$')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
}

pub(crate) fn identifier(name: &str) -> bool {
    identifier_name(name)
        && !matches!(
            name,
            "await"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "debugger"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "enum"
                | "export"
                | "extends"
                | "false"
                | "finally"
                | "for"
                | "function"
                | "if"
                | "implements"
                | "import"
                | "in"
                | "instanceof"
                | "interface"
                | "let"
                | "new"
                | "null"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "return"
                | "static"
                | "super"
                | "switch"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typeof"
                | "var"
                | "void"
                | "while"
                | "with"
                | "yield"
        )
}
