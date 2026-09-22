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
}

impl Default for Module {
    fn default() -> Self {
        Self::new_in(&mut AllocationBudget::new(None)).expect("structured target allocation failed")
    }
}

impl Module {
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
