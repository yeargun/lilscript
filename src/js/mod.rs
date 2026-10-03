//! The JavaScript target tree. Formation from the program builds it; the
//! passes here edit it; naming, printing and delivery turn it into files.
//!
//! Expressions contain syntax, lexical cells have independent `BindingId`
//! handles. Their optional source symbol and each operation's `SourceOriginId`
//! retain provenance through target edits. Arena handles locate target
//! storage; source identities explain its origin.
//! There is deliberately no raw-code node or per-node rendered text.
//! Nonliteral expression handles own one syntax occurrence. Reusing an SSA
//! value requires a binding reference or an explicitly justified rematerialized
//! occurrence, never accidental duplication of a shared expression graph.

use crate::program::SourceOriginId;
pub(crate) use crate::catalog::{integer_intrinsic, original_int32_intrinsic};
use crate::catalog::{
    intrinsic_arity, intrinsic_form, intrinsic_recipe, native_constructor, IntrinsicForm,
};
use crate::check::SymbolId;
use crate::literal::StringValue;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::primitive::{IntBinary, Intrinsic};

pub(crate) mod authors;
mod admission;
mod calls;
mod cloning;
pub use crate::representation as choices;
pub use choices::{AltId, ChoiceFamily, ChoiceKey, ChoiceMap, ChoiceSite, SiteId};
mod declarations;
pub(crate) mod delivery;
mod families;
pub(crate) use families::HeadChoices;
pub use families::{
    ArrayPacking, Challenger, OutputFamilies, Spelling, StatementSpellings, TargetRules,
};
mod constants;
pub mod extract;
mod facts;
mod facts_values;
mod literal_output;
pub mod manifest;
mod mentions;
pub mod names;
mod scalar_objects;
pub mod tables;
mod typed;
pub use literal_output::LiteralOutput;
pub(crate) use literal_output::{LiteralAlternative, WeakLiteralObservation};
#[cfg(test)]
mod imports_tests;
mod inline;
mod journal;
mod private_calls;
mod reach;
mod renumber;
mod uses;
pub(crate) use journal::Journal;
#[cfg(test)]
mod literal_output_tests;
mod naming;
pub mod selection;
use naming::Names;
pub(crate) use naming::NamingProvenance;
mod blocks;
mod host_lowering;
mod initializers;
#[cfg(test)]
mod output_policy_tests;
mod placement;
mod pooling;
mod print;
pub(crate) use print::PlannedStructure;
pub(crate) use print::number_spelling_length;
pub(crate) mod rules;
mod simplify;
pub(crate) mod spellings;
mod statements;
pub(crate) use simplify::literal_array_projection;
#[cfg(test)]
mod tests;
mod verify;
pub(crate) use verify::MAX_NESTING;

// Zero-based arena positions and optional absence share one word. The encoded
// value is private: consumers use the index, not a second identity mapping.
// Source SymbolId/SourceOriginId provenance is independent and keeps its own encoding.
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
    /// `value instanceof constructor`: consults `Symbol.hasInstance`, so it
    /// can run code, and throws for a right side that is not callable.
    InstanceOf,
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

/// A host identifier and its catalog kind (M4.6): tests read the kind, never
/// the spelling.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Host {
    pub name: String,
    pub kind: crate::catalog::HostKind,
}

impl Host {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let kind = crate::catalog::host_kind(&name);
        Self { name, kind }
    }
}

impl From<&str> for Host {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl From<String> for Host {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Binding(BindingId),
    /// An external identifier, checked as an identifier rather than code,
    /// with the kind the catalog gives its name.
    Host(Host),
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
    /// `class name [extends base] { constructor(...) {...} method(...) {...} }`
    /// as a value: a class whose identity is observed (published, or a
    /// subclass of a host constructor). `name` is the class's observable
    /// `name`, independent of whichever binding holds it. Evaluating it reads
    /// `base`, which throws unless it is a constructor. The methods are
    /// non-enumerable prototype methods, in order. No constructor is
    /// JavaScript's implicit one (ECMA-262 ClassDefinitionEvaluation): for a
    /// root class, `constructor(){}`, whose `length` is 0 too.
    Class {
        name: String,
        base: Option<ExprId>,
        constructor: Option<FunctionId>,
        methods: Vec<(String, FunctionId)>,
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
    /// The functions this expression creates, each once: a function value,
    /// or a class value's constructor and methods.
    pub(crate) fn created_functions(&self) -> impl Iterator<Item = FunctionId> + '_ {
        let (first, methods) = match self {
            Self::Function(function) => (Some(*function), &[][..]),
            Self::Class {
                constructor,
                methods,
                ..
            } => (*constructor, methods.as_slice()),
            _ => (None, &[][..]),
        };
        first
            .into_iter()
            .chain(methods.iter().map(|(_, function)| *function))
    }
    /// Whether this expression creates a function (see `created_functions`).
    pub(crate) fn creates_function(&self) -> bool {
        matches!(self, Self::Function(_) | Self::Class { .. })
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
            Self::Class { base, .. } => {
                if let Some(base) = base {
                    *base = map(*base);
                }
            }
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
            Self::Class { base, .. } => {
                if let Some(base) = base {
                    visit(*base)?;
                }
            }
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

/// Typed operations the old route spelled as a host namespace function,
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

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
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
                value: Some(value), ..
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
            Self::ForIn { object, .. }
            | Self::ForOf {
                iterable: object, ..
            } => visit(*object),
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
            Self::ForIn { object, .. }
            | Self::ForOf {
                iterable: object, ..
            } => *object = map(*object),
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
    /// The final parameter receives a fresh array of trailing arguments.
    pub rest: bool,
    pub body: RegionId,
    pub arrow: bool,
    pub name: FunctionName,
    /// Printed with a `"use strict"` directive: a strict frame hides its
    /// caller and arguments from a sloppy host inside a classic script.
    pub strict: bool,
    /// The reflected `length` when it is shorter than the parameter count:
    /// JavaScript counts parameters before the first one with an
    /// initializer, so that one prints as `p=void 0` (or its own literal
    /// default) and the later ones need none. The body still applies the
    /// real default.
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
    /// What formation knows the binding always holds, from its source type
    /// (013-T1, an annotation of M5.2). A binding a pass creates, and every
    /// `JsValue`, is unknown (`None`).
    pub class: Option<ValueClass>,
    /// A parameter whose type excludes `undefined` (numbers, strings,
    /// booleans, enums, collections, class and struct instances, functions):
    /// a typed caller always passes a value, so only a host or erased caller
    /// could leave it to its default.
    pub defined: bool,
}

/// What evaluating an operation node does, from the program's effect
/// facts (M5.2's evaluation-behaviour column): whether it reads or writes
/// memory other code can change, throws, diverges, re-enters the program or
/// suspends. Operands are their own nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Behaviour {
    pub(crate) reads: bool,
    pub(crate) writes: bool,
    pub(crate) throws: bool,
    pub(crate) diverges: bool,
    pub(crate) reenters: bool,
    pub(crate) suspends: bool,
}

impl Behaviour {
    /// Changes and observes nothing another evaluation could, cannot throw,
    /// and ends: its value is its operands' function.
    pub(crate) fn quiet(self) -> bool {
        !(self.reads
            || self.writes
            || self.throws
            || self.diverges
            || self.reenters
            || self.suspends)
    }
}

/// One row of the behaviour column: the node it describes, as it stood.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BehaviourRow {
    expression: ExprId,
    node: Expr,
    behaviour: Behaviour,
}

impl BehaviourRow {
    /// The node the row describes.
    #[cfg(test)]
    pub(crate) fn node(&self) -> &Expr {
        &self.node
    }
}

/// A binding's value class, from the source type of what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueClass {
    /// A number in the int32 range (`int`).
    Int,
    /// A number (`float`).
    Number,
    String,
    Boolean,
    /// Always an object, never `null` or `undefined` once assigned: class
    /// and struct instances, arrays, maps, sets, records, functions, regexes.
    Object,
    /// An object or `null` (a nullable object type): truthy exactly when it
    /// is not `null` (or not yet assigned).
    NullableObject,
}

/// One authored named ESM import. Rows retain module-request order even when
/// the local binding is unused: linking and module initialization are effects.
/// The imported spelling is an IdentifierName, like the existing Export name;
/// only the local lexical BindingId participates in naming. No namespace read
/// or snapshot assignment implements this live binding.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub source: StringValue,
    /// Empty for a side-effect-only request; its unused binding identifies the edge.
    pub imported: String,
    pub binding: BindingId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    pub binding: BindingId,
    /// Stable public name; naming plans choose only the local binding spelling.
    pub name: String,
}

/// Whether moving a root statement could be observed (plan M3.3, design
/// §6): a `Definition` creates only its own bindings and fresh objects,
/// cannot throw, diverge, run user code or read state that changes, so it
/// may be evaluated wherever its readers need it. Everything else is
/// `Anchored` to its module's evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anchor {
    Definition,
    Anchored,
}

/// What placement knows about one root statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootRow {
    /// Its source module. A statement a rule creates takes the module of
    /// the statement it stands beside, so rules that keep modules apart
    /// treat it as that module's; `origin` says what it really is.
    pub module: u32,
    pub anchor: Anchor,
    pub origin: RowOrigin,
    /// The program's root point of the last operation the statement was
    /// formed from (M6.5): code the program first runs later cannot run
    /// before the statement completes. None for a statement a rule created.
    pub point: Option<u32>,
    /// Finishes a fresh definition's public graph or object. Placement keeps
    /// it with that definition; it is not a free-standing observable store.
    pub completes: Option<BindingId>,
    /// A source-instantiated function in a separately delivered cycle. Its
    /// declaration must be hoisted and its exact name is local to this file.
    pub hoisted: bool,
}

/// Where a root statement comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowOrigin {
    /// Formed from its module's source.
    Source,
    /// Created by a rule: string pools, table decoders, generated helpers.
    Synthetic,
    /// Delivered host code, lowered into the tree.
    Host,
}

impl RootRow {
    pub const fn new(module: u32, anchor: Anchor) -> Self {
        Self {
            module,
            anchor,
            origin: RowOrigin::Source,
            point: None,
            completes: None,
            hoisted: false,
        }
    }
    /// The row formed at program root point `point`.
    pub const fn at(self, point: Option<u32>) -> Self {
        Self { point, ..self }
    }
    /// A statement a rule created that only defines, beside `module`.
    pub const fn synthetic(module: u32) -> Self {
        Self {
            module,
            anchor: Anchor::Definition,
            origin: RowOrigin::Synthetic,
            point: None,
            completes: None,
            hoisted: false,
        }
    }
    /// The row of a statement that now holds both statements' code:
    /// `Anchored` wins, and it completes at the later point (unknown when
    /// either is).
    pub fn fuse(self, other: Self) -> Self {
        Self {
            anchor: if self.anchor == Anchor::Anchored || other.anchor == Anchor::Anchored {
                Anchor::Anchored
            } else {
                Anchor::Definition
            },
            point: self
                .point
                .zip(other.point)
                .map(|(this, other)| this.max(other)),
            hoisted: self.hoisted && other.hoisted,
            completes: if self.completes == other.completes { self.completes } else { None },
            ..self
        }
    }
}

/// One entry's public exports (plan M3.3): its name and the positions in
/// `Module::exports` it publishes, in its source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPublic {
    pub name: String,
    pub exports: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub(crate) consumer_annotations: crate::config::ConsumerAnnotations,
    /// Source effects prove termination and no observable effects for every
    /// admitted argument. Body rewrites preserve that contract; new helpers
    /// receive no annotation without a semantic proof of their own.
    pub(crate) discardable_functions: Vec<FunctionId>,
    /// Local data ranking uses this objective's proxy and window. The full
    /// artifact, with its final names and shared helpers, is judged separately.
    pub(crate) const_freezers: Vec<(u32, BindingId)>,
    /// Checked deep-immutable source bindings, preserved across target copies.
    pub(crate) immutable_data: Vec<BindingId>,
    pub(crate) data_estimator: Option<(crate::config::CompressionCostModel, crate::compression::CodecSettings)>,
    pub expressions: Vec<Expr>,
    pub origins: Vec<Option<SourceOriginId>>,
    /// Source-authored string values admitted to shared storage regardless of seed.
    pub(crate) authored_pool: Vec<StringValue>,
    pub(crate) authored_pool_formed: bool,
    pub(crate) forming_choices: crate::representation::RegionalChoices,
    pub(crate) authored_expressions: Vec<crate::representation::RegionalChoices>,
    pub(crate) authored_regions: Vec<crate::representation::RegionalChoices>,
    pub(crate) authored_sites: Vec<crate::representation::RegionalChoices>,
    /// String literals the source only observes for truthiness or
    /// nullishness (an annotation of M5.2), sorted by expression: an
    /// artifact may spell them another way (`LiteralOutput::Observed`). The
    /// arena renumbers them with their expressions, and every pass leaves
    /// them where they are.
    pub(crate) observed_literals: Vec<LiteralAlternative>,
    /// What evaluating an operation node does, from the program's facts
    /// (M5.2's evaluation-behaviour column), sorted by node. A row answers
    /// only while the node at its id is still the one it describes.
    pub(crate) behaviours: Vec<BehaviourRow>,
    /// What the running rule changed (M5.2's journal): every edit of the
    /// tree goes through a helper that records it (`journal.rs`).
    pub(crate) journal: Journal,
    /// The program's initialization order on the tree (M6.5, carried by
    /// M5.2's binding and function columns), in the program's root points:
    /// by binding, the point that settles the module cell a formed binding
    /// stores; by function, the first point during which the unit a formed
    /// function runs may run. A binding or function a rule creates has
    /// neither initially; the target fact owner derives its physical initialization.
    pub(crate) settled: Vec<Option<u32>>,
    pub(crate) first_runs: Vec<Option<u32>>,
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
    /// The contract assumes member reads run no code (Terser's
    /// `pure_getters`): reads commute with reads.
    pub pure_property_reads: bool,
    /// The contract assumes code outside the program never constructs a
    /// function it receives nor reads its `prototype` (Terser's
    /// `unsafe_arrows`).
    pub unconstructed_callbacks: bool,
    /// The program-private property naming contract
    /// (`assume_private_underscore_properties`): present when it holds, with
    /// the preserved names it must not touch. The printer derives the map.
    pub private_names: Option<std::sync::Arc<[String]>>,
    /// One row per root statement, aligned with the root region (plan
    /// M3.3): its source module and its anchor. Formation writes the rows;
    /// every rule that inserts, removes, moves or fuses a root statement
    /// carries them. Placement reads them.
    pub root_rows: Vec<RootRow>,
    /// Host modules have been incorporated into this tree. This remains true
    /// when optimization removes every host statement.
    pub(crate) integrated_hosts: bool,
    /// Each entry's public exports, as positions in `exports`, when the
    /// program has several entries; empty for one (every export is its).
    pub entries: Vec<EntryPublic>,
    /// Where every root statement is delivered, when the output is more
    /// than one entry's one file (plan M3.3): decided once, before naming,
    /// and read by naming and printing.
    pub delivery: Option<delivery::DeliveryPlan>,
    /// Names delivered host code reads as globals from inside this module's
    /// scope; no binding of this module may take one.
    pub reserved: Vec<String>,
    /// Import sources the output carries instead of importing; a classic
    /// script can use these.
    pub carried: Vec<String>,
    /// Print `{let i=v;for(;c;u)b}` as `for(let i=v;c;u)b`: the
    /// `loop_heads` output family, written here by formation. Shorter, but
    /// shortening a loop can change repetition, so the codec judges it
    /// per artifact (a terminal challenger).
    pub loop_head_declarations: bool,
    /// Print `if(c)e;` as `c&&e;` (and `if(!c)e;` as `c||e;`) where neither
    /// side needs grouping: the `logical_statements` output family. Shorter,
    /// but changing repeated statement forms can grow compressed bytes,
    /// so the codec judges it per artifact as well.
    pub logical_statements: bool,
    /// Print `x=x+y` as `x+=y`: the `compound_assignments` output family,
    /// written here by formation (M8.3: a family of its own).
    pub compound_assignments: bool,
    /// Print a string in the quote it escapes least: the `quotes` output
    /// family, written here by formation (M8.3).
    pub quotes: bool,
    /// The `int32_hints` output family, written here by formation: an
    /// integer method's result prints its `|0` as the compiler printed it
    /// before R10 (only without pristine builtins).
    pub int32_hints: bool,
    /// The choice sites formation found on this tree (plan M9.1): what each
    /// offers, seeds and applied under the artifact's `ChoiceMap`. The
    /// terminal stage reads them to offer the other alternatives.
    pub choice_sites: Vec<ChoiceSite>,
    pub(crate) spelling_head: u8,
    pub(crate) spelling_regions: usize,
    pub(crate) spelling_functions: usize,
    pub(crate) spelling_node_count: usize,
    /// Persistent head-node identities; new tail nodes have no inherited site.
    pub(crate) spelling_nodes: Vec<Option<u32>>,
    pub(crate) print_forms: Option<spellings::PrintForms>,
}

/// Make `index` a slot of an id-indexed column, charged as retained output.
fn grow_column(
    column: &mut Vec<Option<u32>>,
    index: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    if index >= column.len() {
        budget.reserve_vec(AllocationClass::Retained, column, index + 1 - column.len())?;
        column.resize(index + 1, None);
    }
    Ok(())
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

/// Where each region sits, for asking what has run before a statement.
struct Frames {
    /// The statement holding each region, as `region_parents` found it.
    parents: Vec<Option<(RegionId, usize)>>,
    /// The function each region is the body of.
    bodies: Vec<Option<FunctionId>>,
    /// Functions reading their own `arguments` object.
    arguments: Vec<bool>,
}

impl Module {
    /// The source module of root statement `index`, for the rules that keep
    /// statements of different modules apart.
    pub(crate) fn root_module(&self, index: usize) -> Option<u32> {
        self.root_rows.get(index).map(|row| row.module)
    }

    /// Root statements `from` were folded into root statement `into`, which
    /// stands before them: their rows join its row and leave.
    pub(crate) fn fuse_roots(&mut self, into: usize, from: std::ops::Range<usize>) {
        self.tables_mut();
        let fused = self.root_rows[from.clone()]
            .iter()
            .fold(self.root_rows[into], |row, other| row.fuse(*other));
        self.root_rows[into] = fused;
        self.root_rows.drain(from);
    }

    // The root-row helpers (plan M3.3, design §6; architecture §10.6): every
    // rule that inserts, removes, moves or fuses a statement of a region
    // that may be the root edits through these, so the rows placement reads
    // stay aligned with the root statements. A tree without rows (a test
    // tree, a producer that records none) keeps none.

    /// Whether `region` is the root and the tree records its rows.
    fn rows_of(&self, region: usize) -> bool {
        region == self.root.index() && !self.root_rows.is_empty()
    }

    /// Remove statement `index` of `region`, with its row at the root.
    pub(crate) fn remove_statement(&mut self, region: usize, index: usize) -> Statement {
        self.journal_region(region);
        let statement = self.regions[region].statements.remove(index);
        if self.rows_of(region) && index < self.root_rows.len() {
            self.root_rows.remove(index);
        }
        statement
    }

    /// Statement `from` of `region`'s code now also runs in statement `into`:
    /// at the root, `into`'s row joins `from`'s (Anchored wins).
    pub(crate) fn fuse_row(&mut self, region: usize, from: usize, into: usize) {
        if self.rows_of(region) && from < self.root_rows.len() && into < self.root_rows.len() {
            let fused = self.root_rows[into].fuse(self.root_rows[from]);
            if fused != self.root_rows[into] {
                self.tables_mut().root_rows[into] = fused;
            }
        }
    }

    /// Remove statement `index` of `region`, whose code now runs in
    /// statement `into` of the same region (`into` counted before the
    /// removal): at the root its row joins `into`'s.
    pub(crate) fn remove_statement_into(
        &mut self,
        region: usize,
        index: usize,
        into: usize,
    ) -> Statement {
        self.fuse_row(region, index, into);
        self.remove_statement(region, index)
    }

    /// Statements `from` of `region` were folded into statement `into`,
    /// which stands before them: they leave, and at the root their rows
    /// join `into`'s.
    pub(crate) fn drain_into(&mut self, region: usize, into: usize, from: std::ops::Range<usize>) {
        self.journal_region(region);
        self.regions[region].statements.drain(from.clone());
        if self.rows_of(region) {
            let end = from.end.min(self.root_rows.len());
            if from.start < end && into < from.start {
                self.fuse_roots(into, from.start..end);
            }
        }
    }

    /// Move statement `from` of `region` to `to` (a position after the
    /// removal), with its row at the root.
    pub(crate) fn move_statement(&mut self, region: usize, from: usize, to: usize) {
        self.journal_region(region);
        let moved = self.regions[region].statements.remove(from);
        self.regions[region].statements.insert(to, moved);
        if self.rows_of(region) && from < self.root_rows.len() {
            let row = self.root_rows.remove(from);
            let to = to.min(self.root_rows.len());
            self.root_rows.insert(to, row);
        }
    }

    /// Insert `statement` at `at` of `region`; at the root, with `row`.
    pub(crate) fn insert_statement(
        &mut self,
        region: usize,
        at: usize,
        statement: Statement,
        row: RootRow,
    ) {
        self.journal_region(region);
        self.regions[region].statements.insert(at, statement);
        if self.rows_of(region) && at <= self.root_rows.len() {
            self.root_rows.insert(at, row);
        }
    }

    /// Append `statement` to the root with `row`, both charged to `budget`.
    pub(crate) fn push_root_admitted(
        &mut self,
        statement: Statement,
        row: RootRow,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let root = self.root.index();
        let recorded = self.root_rows.len() == self.regions[root].statements.len();
        self.journal_region(root);
        budget.push(
            AllocationClass::Retained,
            &mut self.regions[root].statements,
            statement,
        )?;
        if recorded {
            budget.push(AllocationClass::Retained, &mut self.root_rows, row)?;
        }
        Ok(())
    }

    /// Put `statements` at the start of the root, each with its row.
    pub(crate) fn prepend_roots(
        &mut self,
        statements: Vec<Statement>,
        rows: impl IntoIterator<Item = RootRow>,
    ) {
        let root = self.root.index();
        let recorded = self.root_rows.len() == self.regions[root].statements.len();
        self.journal_region(root);
        self.regions[root].statements.splice(0..0, statements);
        if recorded {
            self.root_rows.splice(0..0, rows);
        }
    }

    /// Replace statements `range` of `region` by `statements`; at the root
    /// their rows become `rows` of the replaced rows.
    pub(crate) fn splice_statements(
        &mut self,
        region: usize,
        range: std::ops::Range<usize>,
        statements: Vec<Statement>,
        rows: impl FnOnce(&[RootRow]) -> Vec<RootRow>,
    ) {
        self.journal_region(region);
        self.regions[region]
            .statements
            .splice(range.clone(), statements);
        if self.rows_of(region) && range.end <= self.root_rows.len() {
            let replaced = rows(&self.root_rows[range.clone()]);
            self.root_rows.splice(range, replaced);
        }
    }

    /// Drop every statement of `region` from `from` on, with their rows.
    pub(crate) fn truncate_statements(&mut self, region: usize, from: usize) {
        self.journal_region(region);
        self.regions[region].statements.truncate(from);
        if self.rows_of(region) && from < self.root_rows.len() {
            self.root_rows.truncate(from);
        }
    }

    /// Whether the root's rows align with its statements (design §6): no
    /// rows at all, or one per root statement.
    pub(crate) fn root_rows_align(&self) -> bool {
        self.root_rows.is_empty()
            || self.root_rows.len() == self.regions[self.root.index()].statements.len()
    }

    /// `let x=void 0` is `let x` and `return void 0` is `return`: a `let`
    /// without a value still initializes to undefined each time it runs. A
    /// bare `return` ending a function body is where the body ends anyway.
    /// Returns the number of elided values and statements.
    pub(crate) fn elide_undefined(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut elided = 0;
        for region in 0..self.regions.len() {
            budget.work(
                crate::compilation_policy::WorkKind::Analysis,
                1 + self.regions[region].statements.len() as u64,
            )?;
            for index in 0..self.regions[region].statements.len() {
                let (Statement::Let {
                    value: Some(value), ..
                }
                | Statement::Return(Some(value))) = self.regions[region].statements[index]
                else {
                    continue;
                };
                if matches!(
                    self.expressions[value.index()],
                    Expr::Literal(Literal::Undefined)
                ) {
                    if let Statement::Let { value, .. } | Statement::Return(value) =
                        &mut self.statements_mut(region)[index]
                    {
                        *value = None;
                    }
                    elided += 1;
                }
            }
        }
        budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            self.functions.len() as u64,
        )?;
        for function in 0..self.functions.len() {
            let body = self.functions[function].body.index();
            if matches!(
                self.regions[body].statements.last(),
                Some(Statement::Return(None))
            ) {
                self.statements_mut(body).pop();
                elided += 1;
            }
        }
        Ok(elided)
    }

    /// Statements after a region's first `return`, `throw`, `break` or
    /// `continue` never run, and go; declarations stay, since a closure
    /// created earlier may name them and a function declaration is hoisted.
    /// Then `elide_undefined` drops a body's final `return;` that this left.
    /// Returns the number of dropped statements.
    pub(crate) fn drop_unreachable(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut dropped = 0;
        for region in 0..self.regions.len() {
            budget.work(
                crate::compilation_policy::WorkKind::Analysis,
                1 + self.regions[region].statements.len() as u64,
            )?;
            let Some(exit) = self.regions[region]
                .statements
                .iter()
                .position(|statement| {
                    matches!(
                        statement,
                        Statement::Return(_)
                            | Statement::Throw(_)
                            | Statement::Break
                            | Statement::Continue
                    )
                })
            else {
                continue;
            };
            let mut index = exit + 1;
            while index < self.regions[region].statements.len() {
                if matches!(
                    self.regions[region].statements[index],
                    Statement::Let { .. } | Statement::Function { .. }
                ) {
                    index += 1;
                    continue;
                }
                self.remove_statement(region, index);
                dropped += 1;
            }
        }
        Ok(dropped + self.elide_undefined(budget)?)
    }

    /// `let x;…;x=v` becomes `…;let x=v` when that assignment is the first
    /// code of the region to mention `x` and `v` does not: nothing before it
    /// can read `x`, and no hoisted declaration of the region mentions it, so
    /// no read meets the later declaration's temporal dead zone. With
    /// `prunes`, a bare statement whose value is only a literal, a function,
    /// a literal of those or (under pristine builtins) a standard global has
    /// no effect and goes. Returns the number of
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
                    // A read of a standard global (`Object;`, left by an unused
                    // alias) is as inert under pristine builtins, and so is a
                    // null-guarded property read when member reads run no code.
                    if self.inert_value(value, budget)?
                        || self.pristine_builtins && self.standard_member(value)
                        || self.pure_property_reads && self.guarded_read(value)
                    {
                        self.remove_statement(region, index);
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
                if target <= index || (root && self.root_module(index) != self.root_module(target))
                {
                    continue;
                }
                let Statement::Evaluate(store) = self.regions[region].statements[target] else {
                    continue;
                };
                let Expr::Assign {
                    target: place,
                    value,
                } = self.expressions[store.index()]
                else {
                    continue;
                };
                if matches!(self.expressions[place.index()], Expr::Binding(found) if found == binding)
                    && !self.mentions_within(value, binding, budget)?
                {
                    merges.push((index, target, binding, value));
                }
            }
            for &(_, target, binding, value) in &merges {
                self.set_statement(
                    region,
                    target,
                    Statement::Let {
                        binding,
                        value: Some(value),
                    },
                );
            }
            for &(index, target, ..) in &merges {
                self.fuse_row(region, index, target);
            }
            for &(index, ..) in merges.iter().rev() {
                self.remove_statement(region, index);
            }
            edits += merges.len();
        }
        Ok(edits)
    }

    /// `x==null?void 0:x.p` or `x!=null?x.p:void 0` for a binding `x`: the
    /// spelling of an optional read. With member reads assumed to run no code
    /// (`assume_pure_property_reads`), evaluating it does nothing, since the
    /// null test keeps the one read from throwing. A statement of it is what an
    /// unused binding's initializer leaves behind (`document = global?.document`).
    fn guarded_read(&self, value: ExprId) -> bool {
        let node = |id: ExprId| &self.expressions[id.index()];
        let Expr::Conditional { condition, yes, no } = node(value) else {
            return false;
        };
        let Expr::Binary { op, left, right } = node(*condition) else {
            return false;
        };
        let tested = match (node(*left), node(*right)) {
            (Expr::Binding(binding), Expr::Literal(Literal::Null | Literal::Undefined))
            | (Expr::Literal(Literal::Null | Literal::Undefined), Expr::Binding(binding)) => *binding,
            _ => return false,
        };
        let (absent, read) = match op {
            Binary::Equal => (*yes, *no),
            Binary::NotEqual => (*no, *yes),
            _ => return false,
        };
        matches!(node(absent), Expr::Literal(Literal::Undefined | Literal::Null))
            && matches!(node(read), Expr::Member { object, property: Property::Named(_) }
                if matches!(node(*object), Expr::Binding(binding) if *binding == tested))
    }

    /// Whether evaluating `value` only creates literals and functions.
    fn inert_value(
        &self,
        value: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        Ok(match &self.expressions[value.index()] {
            // A regular-expression literal creates a fresh object and cannot
            // throw: its pattern was validated when it became a literal
            // (M8.2 A2, diagnosis C9).
            Expr::Literal(_) | Expr::Function(_) | Expr::Regex(_) => true,
            // `-5`, `!0`, `typeof "a"`: an operator on a primitive literal
            // converts nothing that could run code.
            Expr::Unary { value, .. } => {
                matches!(self.expressions[value.index()], Expr::Literal(_))
            }
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
            // An empty Map or Set: under pristine builtins its construction
            // runs no code and yields a fresh object, like a literal.
            Expr::ConstructIntrinsic {
                operation: crate::primitive::Intrinsic::MapNew | crate::primitive::Intrinsic::SetNew,
                arguments,
            } => self.pristine_builtins && arguments.is_empty(),
            _ => false,
        })
    }

    /// Number operations on literals become their results where the result
    /// is no longer: exact binary64 arithmetic, as in JavaScript, and the
    /// language's int32 contract for integer operations. String sums stay:
    /// computed, literal and shared spellings are the string family's choice
    /// for each codec. An observed literal is left alone. Returns the
    /// number of folds.
    pub(crate) fn fold_literal_operations(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        let number = |module: &Self, id: ExprId| match module.expressions[id.index()] {
            Expr::Literal(Literal::Number(value)) if !module.observed(id) => Some(value),
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
                    match (
                        &self.expressions[left.index()],
                        &self.expressions[right.index()],
                    ) {
                        _ => match (number(self, left), number(self, right)) {
                            (Some(a), Some(b)) => Some(a + b)
                                .filter(|r| {
                                    r.is_finite() && spelled(*r) <= spelled(a) + spelled(b) + 1
                                })
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
                        (Some(a), Some(b)) => {
                            Some(Expr::Literal(Literal::Number(f64::from(op.evaluate(a, b)))))
                        }
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
                if self.set_expression(id, folded) {
                    folds += 1;
                }
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
        let mut phase = budget.scope();
        let budget = &mut phase;
        let mut edits = 0;
        let mut regions = budget.copy_slice(AllocationClass::Scratch, &[self.root])?;
        let mut seen = budget.filled(AllocationClass::Scratch, self.regions.len(), false)?;
        let mut pending: Vec<(ExprId, bool)> = Vec::new();
        while let Some(region) = regions.pop() {
            budget.work(Analysis, 1)?;
            if std::mem::replace(&mut seen[region.index()], true) {
                continue;
            }
            for index in 0..self.regions[region.index()].statements.len() {
                budget.work(Analysis, 1)?;
                let statement = &self.regions[region.index()].statements[index];
                let mut roots = 0;
                statement.visit_expressions(|_| roots += 1);
                budget.reserve_vec(AllocationClass::Scratch, &mut pending, roots)?;
                let mut children = 0;
                statement.visit_regions(|_| children += 1);
                children += usize::from(matches!(statement, Statement::Function { .. }));
                budget.reserve_vec(AllocationClass::Scratch, &mut regions, children)?;
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
                            let node = self.expressions[twice.index()].clone_in(budget)?;
                            self.set_expression(id, node);
                            edits += 1;
                        }
                    }
                    let expression = &self.expressions[id.index()];
                    for function in expression.created_functions() {
                        budget.push(AllocationClass::Scratch, &mut regions, self.functions[function.index()].body)?;
                    }
                    match *expression {
                        Expr::Unary {
                            op: Unary::Not,
                            value,
                        } => budget.push(AllocationClass::Scratch, &mut pending, (value, true))?,
                        Expr::Binary {
                            op: Binary::And | Binary::Or,
                            left,
                            right,
                        } => {
                            budget.push(AllocationClass::Scratch, &mut pending, (left, truth))?;
                            budget.push(AllocationClass::Scratch, &mut pending, (right, truth))?;
                        }
                        Expr::Conditional { condition, yes, no } => {
                            budget.push(AllocationClass::Scratch, &mut pending, (condition, true))?;
                            budget.push(AllocationClass::Scratch, &mut pending, (yes, truth))?;
                            budget.push(AllocationClass::Scratch, &mut pending, (no, truth))?;
                        }
                        Expr::Sequence(ref items) => {
                            let last = items.len().saturating_sub(1);
                            for (position, item) in items.iter().enumerate() {
                                budget.push(AllocationClass::Scratch, &mut pending, (*item, position != last || truth))?;
                            }
                        }
                        _ => {
                            expression.visit_children(|child|
                                budget.push(AllocationClass::Scratch, &mut pending, (child, false)))?;
                        }
                    }
                }
            }
        }
        drop((regions, seen, pending));
        phase.finish_retained()?;
        Ok(edits)
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
        new_keys: bool,
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
                if first.get(&binding).is_none_or(|&mention| mention <= index) {
                    index += 1;
                    continue;
                }
                let mut entries = entries.clone();
                let mut end = index + 1;
                while let Some(Statement::Evaluate(store)) =
                    self.regions[region].statements.get(end)
                {
                    budget.work(Analysis, 1)?;
                    if root && self.root_module(end) != self.root_module(index) {
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
                    // A store to a key the literal has updates an own data
                    // property, which shadows anything inherited; a new key
                    // could meet an inherited setter, which only pristine
                    // builtins rule out.
                    match replaced {
                        Some(position) => entries[position].1 = value,
                        None if new_keys => entries.push((property, value)),
                        None => break,
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
                    self.spelling_nodes.pop();
                    self.authored_expressions.pop();
                    index += 1;
                    continue;
                }
                self.statements_mut(region)[index].replace_root(id);
                folded += end - index - 1;
                self.drain_into(region, index, index + 1..end);
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

    /// Whether evaluating `root` creates a function (or class) anywhere.
    fn creates_function(&self, root: ExprId) -> bool {
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let expression = &self.expressions[id.index()];
            if expression.creates_function() {
                return true;
            }
            let _ = expression.visit_children(|child| {
                pending.push(child);
                Ok::<_, ()>(())
            });
        }
        false
    }

    /// `if(p===void 0)p=D`, or its expression form `p===void 0&&(p=D)`, for
    /// a literal `D`: the parameter and its default.
    pub(crate) fn default_check(&self, statement: &Statement) -> Option<(BindingId, ExprId)> {
        let (condition, assign) = match statement {
            Statement::If {
                condition,
                yes,
                no: None,
            } => match self.regions[yes.index()].statements[..] {
                [Statement::Evaluate(assign)] => (*condition, assign),
                _ => return None,
            },
            Statement::Evaluate(value) => match &self.expressions[value.index()] {
                Expr::Binary {
                    op: Binary::And,
                    left,
                    right,
                } => (*left, *right),
                _ => return None,
            },
            _ => return None,
        };
        let Expr::Binary {
            op: Binary::StrictEqual,
            left,
            right,
        } = &self.expressions[condition.index()]
        else {
            return None;
        };
        let tested = match (
            &self.expressions[left.index()],
            &self.expressions[right.index()],
        ) {
            (Expr::Binding(tested), Expr::Literal(Literal::Undefined))
            | (Expr::Literal(Literal::Undefined), Expr::Binding(tested)) => *tested,
            _ => return None,
        };
        let Expr::Assign { target, value } = &self.expressions[assign.index()] else {
            return None;
        };
        match (
            &self.expressions[target.index()],
            &self.expressions[value.index()],
        ) {
            (Expr::Binding(target), Expr::Literal(literal))
                if *target == tested && !matches!(literal, Literal::Undefined) =>
            {
                Some((tested, *value))
            }
            _ => None,
        }
    }

    /// Whether `function`'s body reads no frame of its own: no `this`,
    /// `arguments`, `super` or direct `eval`, in it or in the arrows it
    /// creates, which share its frame. Called, such a function is its body.
    pub(crate) fn frame_free(&self, function: FunctionId) -> bool {
        !self.frame_reads(function, |expression| match expression {
            Expr::This | Expr::SuperCall { .. } => true,
            Expr::Host(host) => matches!(
                host.kind,
                crate::catalog::HostKind::Arguments | crate::catalog::HostKind::Eval
            ),
            Expr::Call { invocation, .. } => *invocation == Invocation::DirectEval,
            _ => false,
        })
    }

    /// Whether `function` reads its own `arguments` object, in it or in the
    /// arrows it creates. In a sloppy frame that object aliases the
    /// parameters: writing `arguments[0]` assigns the first.
    fn reads_arguments(&self, function: FunctionId) -> bool {
        self.frame_reads(
            function,
            |expression| matches!(expression, Expr::Host(host) if host.kind == crate::catalog::HostKind::Arguments),
        )
    }

    /// Whether `function`'s frame reads no `arguments` object and calls no
    /// direct `eval`: a parameter list with defaults then changes nothing
    /// the body can see (it would unmap a sloppy frame's `arguments`).
    pub(crate) fn arguments_free(&self, function: FunctionId) -> bool {
        !self.frame_reads(function, |expression| match expression {
            Expr::Host(host) => matches!(
                host.kind,
                crate::catalog::HostKind::Arguments | crate::catalog::HostKind::Eval
            ),
            Expr::Call { invocation, .. } => *invocation == Invocation::DirectEval,
            _ => false,
        })
    }

    /// Whether an expression of `function`'s frame (its body and the arrows
    /// it creates, not the functions they declare) satisfies `found`.
    fn frame_reads(&self, function: FunctionId, found: impl Fn(&Expr) -> bool) -> bool {
        let mut expressions = Vec::new();
        let mut regions = vec![self.functions[function.index()].body];
        while let Some(region) = regions.pop() {
            for statement in &self.regions[region.index()].statements {
                statement.visit_expressions(|root| expressions.push(root));
                if !matches!(statement, Statement::Function { .. }) {
                    statement.visit_regions(|child| regions.push(child));
                }
            }
            while let Some(id) = expressions.pop() {
                let expression = &self.expressions[id.index()];
                if found(expression) {
                    return true;
                }
                if let Expr::Function(inner) = expression {
                    if self.functions[inner.index()].arrow {
                        regions.push(self.functions[inner.index()].body);
                    }
                }
                let _ = expression.visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, ()>(())
                });
            }
        }
        false
    }

    /// Whether the reference to `binding` under `leaf` is a callee.
    fn calls_reference(&self, leaf: Leaf, binding: BindingId) -> bool {
        let Leaf::Child(parent) = leaf else {
            return false;
        };
        match &self.expressions[parent.index()] {
            Expr::Call { callee, .. } | Expr::Construct { callee, .. } => {
                matches!(self.expressions[callee.index()], Expr::Binding(found) if found == binding)
            }
            _ => false,
        }
    }

    /// Whether `statement` mentions `binding` anywhere: its own expressions,
    /// nested blocks and the bodies of functions it creates or declares.
    fn statement_mentions(&self, statement: &Statement, binding: BindingId) -> bool {
        let mut expressions = Vec::new();
        statement.visit_expressions(|root| expressions.push(root));
        let mut regions = Vec::new();
        statement.visit_regions(|child| regions.push(child));
        if let Statement::Function { function, .. } = statement {
            regions.push(self.functions[function.index()].body);
        }
        self.mentions(&regions, &expressions, binding, false)
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
                for function in expression.created_functions() {
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

    /// Every binding mention in code that can run, with whether it is
    /// written: an assignment's target, a `for…in`/`for…of` binding.
    fn walk_mentions(
        &self,
        regions: &mut Vec<RegionId>,
        expressions: &mut Vec<ExprId>,
        budget: &mut AllocationBudget<'_>,
        mut visit: impl FnMut(BindingId, bool),
    ) -> Result<(), AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        loop {
            if let Some(id) = expressions.pop() {
                budget.work(Analysis, 1)?;
                let expression = &self.expressions[id.index()];
                match expression {
                    Expr::Binding(binding) => visit(*binding, false),
                    Expr::Assign { target, value } => {
                        match self.expressions[target.index()] {
                            // The target is written, not read: visit it once as
                            // a write and continue with the value.
                            Expr::Binding(binding) => {
                                visit(binding, true);
                                expressions.push(*value);
                                continue;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                for function in expression.created_functions() {
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
                if let Statement::ForIn { binding, .. } | Statement::ForOf { binding, .. } =
                    statement
                {
                    visit(*binding, true);
                }
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
    /// number of forwarded bindings, with expression postorder restored.
    pub(crate) fn forward_single_uses(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut references = budget.filled(AllocationClass::Scratch, self.bindings.len(), 0u32)?;
        budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            self.exports.len() as u64,
        )?;
        // Occurrence counts include shared nodes at each evaluation site;
        // physical writes come from the shared reachable binding owner below.
        self.walk_mentions(
            &mut vec![self.root],
            &mut Vec::new(),
            budget,
            |binding, _| {
                references[binding.index()] = references[binding.index()].saturating_add(1);
            },
        )?;
        for export in &self.exports {
            references[export.binding.index()] = u32::MAX;
        }
        let mut depths = self.region_depths(budget)?;
        let mut captured = self.with_reach(budget, |_, reach, budget| {
            budget.copy_slice(AllocationClass::Scratch, &reach.captured)
        })??;
        let mut frames = self.frames(budget)?;
        let mut order = self.order(&frames, budget)?;
        let mut forwarded = 0;
        let mut disordered = false;
        for region in 0..self.regions.len() {
            let Some(region_depth) = depths[region] else {
                continue;
            };
            let root = region == self.root.index();
            let mut mentions = mentions::StatementMentions::default();
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
                // A function whose name nothing observes is created the same
                // way wherever it stands; one with an exact name would print
                // a wrapper at a position that infers another name.
                let function = match self.expressions[value.index()] {
                    Expr::Function(function) => Some(function),
                    _ => None,
                };
                // Its one mention must be a read: a binding reachable code
                // assigns is not its initializer everywhere, and an
                // assignment's target is not a place a value can take.
                let movable = references[binding.index()] == 1
                    && !order.written(binding)
                    && !self.bindings[binding.index()].pinned
                    && !matches!(self.expressions[value.index()], Expr::Class { .. })
                    && function.is_none_or(|function| {
                        matches!(
                            self.functions[function.index()].name,
                            FunctionName::Unobserved
                        )
                    });
                let same_module =
                    |at: usize| !root || self.root_module(index) == self.root_module(at);
                let leaf = if movable && function.is_none() && same_module(index + 1) {
                    let next = &self.regions[region].statements[index + 1];
                    // A function created in a `for…in`/`for…of` head closes
                    // over the loop's binding (see the inert rule below).
                    let head = matches!(next, Statement::ForIn { .. } | Statement::ForOf { .. });
                    match self.first_leaf(next, binding) {
                        Some(leaf) => Some((leaf, index + 1)),
                        None if head && self.creates_function(value) => None,
                        // Past the shared effect/initialization proof for the statement.
                        None => self
                            .quiet_leaf(
                                RegionId::new(region),
                                index,
                                binding,
                                value,
                                &order,
                                &frames,
                                &captured,
                                budget,
                            )?
                            .map(|leaf| (leaf, index + 1)),
                    }
                } else {
                    None
                };
                // An inert value (literals and functions, and arrays or objects
                // of them) can be created later without any observer seeing
                // it: nothing else reads the binding, and creating it runs no
                // code. It may take its one reference in the first later
                // statement that mentions it, where that statement evaluates
                // it once (not a loop's test or update, nor a nested function:
                // those would create it again).
                // So can a value of literals, functions and reads of bindings
                // (an object or array of them): reading an initialized binding
                // runs nothing, cannot throw, and yields the same value while
                // nothing assigns it. Either no closure reaches the binding,
                // so only this code could and no statement up to the reference
                // does, or nothing assigns it at all (a root constant).
                let mut settled = None;
                if leaf.is_none() && movable && !self.inert_value(value, budget)? {
                    if let Some(reads) = self.settled_reads(value) {
                        let mut holds = true;
                        let at = RegionId::new(region);
                        for &read in &reads {
                            holds = holds
                                && if captured[read.index()] {
                                    self.constant_at(read, at, index, &order, &frames, budget)?
                                } else {
                                    self.initialized_at(read, at, index, &frames, budget)?
                                };
                        }
                        settled = holds.then_some(reads);
                    }
                }
                // The region the value lands in, when it is a branch of the
                // statement that mentions it rather than that statement.
                let mut nested: Option<RegionId> = None;
                // The root-level statement the value moves into.
                let mut receiver = index + 1;
                let leaf = match leaf {
                    Some(leaf) => Some(leaf),
                    None if movable
                        && (settled.is_some() || self.inert_value(value, budget)?) =>
                    {
                        let mut found = None;
                        for later in index + 1..self.regions[region].statements.len() {
                            budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                            let statement = &self.regions[region].statements[later];
                            if let Some(reads) = &settled {
                                if self.statement_assigns(statement, reads) {
                                    break;
                                }
                            }
                            if !mentions.contains(self, region, later, binding, budget)? {
                                continue;
                            }
                            // A value that creates a function keeps out of a
                            // `for (let k of …)` head, which runs with `k` in
                            // scope (and in its TDZ): a function created there
                            // closes over it. Nor does it become a callee: a
                            // call is the inliners' to take, not an IIFE.
                            let creates = self.creates_function(value);
                            let head = matches!(
                                statement,
                                Statement::ForIn { .. } | Statement::ForOf { .. }
                            );
                            if same_module(later) && !(creates && head) {
                                receiver = later;
                                found = self
                                    .single_evaluation_reference(statement, binding)
                                    .filter(|(leaf, _)| {
                                        !creates || !self.calls_reference(*leaf, binding)
                                    })
                                    .map(|leaf| (leaf, later));
                                // An inert value may also be created in the
                                // branch that reads it: an `if` arm, a block or
                                // a `try` part runs at most once when its
                                // statement does, and creating the value there
                                // instead runs nothing either way.
                                if found.is_none() && settled.is_none() {
                                    if let Some((inner, at, leaf)) =
                                        self.branch_reference(statement, binding, creates)
                                    {
                                        nested = Some(inner);
                                        found = Some((leaf, at));
                                    }
                                }
                            }
                            break;
                        }
                        found
                    }
                    None => None,
                };
                // The moved value's deepest point must stay within the nesting
                // limit.
                let base = match nested {
                    Some(inner) => depths[inner.index()],
                    None => Some(region_depth),
                };
                let fits = leaf.is_some_and(|((_, path), _)| {
                    base.is_some_and(|base| {
                        base + 1 + path + self.subtree_depth(value) <= verify::MAX_NESTING
                    })
                });
                budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                let Some(((leaf, _), target_statement)) = leaf.filter(|_| fits) else {
                    index += 1;
                    continue;
                };
                let target_region = nested.map_or(region, |inner| inner.index());
                match leaf {
                    Leaf::Root => {
                        self.statements_mut(target_region)[target_statement].replace_root(value)
                    }
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
                        // Children precede their parents in the arena: a value
                        // created after its new parent (a literal a store fold
                        // rebuilt) waits for the renumbering below.
                        disordered |= value.index() > parent.index();
                        self.expression_mut(parent).remap_children(|child| {
                            if child == slot {
                                value
                            } else {
                                child
                            }
                        });
                    }
                }
                // Functions the value creates now open in the branch's scope.
                if let Some(inner) = nested {
                    let scope = self.regions[inner.index()].scope;
                    let mut pending = vec![value];
                    let mut bodies = Vec::new();
                    while let Some(id) = pending.pop() {
                        budget.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                        let expression = &self.expressions[id.index()];
                        for function in expression.created_functions() {
                            bodies.push(self.functions[function.index()].body);
                        }
                        let _ = expression.visit_children(|child| {
                            pending.push(child);
                            Ok::<_, ()>(())
                        });
                    }
                    for body in bodies {
                        self.rescope(body, scope, budget)?;
                    }
                }
                // The receiver now evaluates the moved value.
                self.remove_statement_into(region, index, receiver);
                mentions.moved(index, receiver, budget)?;
                references[binding.index()] = 0;
                forwarded += 1;
                if nested.is_some() {
                    // The moved functions' bodies stand deeper, under another
                    // statement: the region facts are found again.
                    depths = self.region_depths(budget)?;
                    self.with_reach(budget, |_, reach, budget| {
                        budget.work(
                            crate::compilation_policy::WorkKind::Analysis,
                            captured.len() as u64,
                        )?;
                        captured.copy_from_slice(&reach.captured);
                        Ok::<_, AllocationError>(())
                    })??;
                    frames = self.frames(budget)?;
                    order = self.order(&frames, budget)?;
                }
            }
            mentions.discard(budget)?;
        }
        if disordered { self.renumber(budget)?; }
        Ok(forwarded)
    }

    fn frames(&self, budget: &mut AllocationBudget<'_>) -> Result<Frames, AllocationError> {
        let parents = self.region_parents(budget)?;
        let mut bodies = budget.filled(AllocationClass::Scratch, self.regions.len(), None)?;
        let mut arguments = budget.filled(AllocationClass::Scratch, self.functions.len(), false)?;
        // Each expression is in the frame of one function, its nearest that
        // is not an arrow, so the frames are walked once in all.
        budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            (self.expressions.len() + self.functions.len()) as u64,
        )?;
        for (id, function) in self.functions.iter().enumerate() {
            bodies[function.body.index()] = Some(FunctionId::new(id));
            arguments[id] = !function.arrow && self.reads_arguments(FunctionId::new(id));
        }
        Ok(Frames {
            parents,
            bodies,
            arguments,
        })
    }

    /// Whether `binding` holds its value when statement `index` of `region`
    /// runs: a statement before it in the region declared it, or one before
    /// the statement holding the region, and so on up to the function body;
    /// or it is that function's parameter (which no `arguments` object
    /// aliases), or the binding of a `for…in`, `for…of` or `catch` around it.
    fn initialized_at(
        &self,
        binding: BindingId,
        mut region: RegionId,
        mut index: usize,
        frames: &Frames,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        use crate::compilation_policy::WorkKind::Analysis;
        loop {
            budget.work(Analysis, 1 + index as u64)?;
            let declares = |statement: &Statement| {
                matches!(
                    statement,
                    Statement::Let { binding: declared, .. }
                        | Statement::Function { binding: declared, .. }
                        if *declared == binding
                )
            };
            if self.regions[region.index()].statements[..index]
                .iter()
                .any(declares)
            {
                return Ok(true);
            }
            if let Some(function) = frames.bodies[region.index()] {
                return Ok(!frames.arguments[function.index()]
                    && self.functions[function.index()]
                        .parameters
                        .contains(&binding));
            }
            let Some((parent, _)) = frames.parents[region.index()] else {
                return Ok(false);
            };
            // Where the holding statement is now: forwarding may have removed
            // declarations before it since the parents were found.
            let statements = &self.regions[parent.index()].statements;
            budget.work(Analysis, statements.len() as u64)?;
            let Some(at) = statements.iter().position(|statement| {
                let mut holds = false;
                statement.visit_regions(|child| holds |= child == region);
                holds
            }) else {
                return Ok(false);
            };
            match &statements[at] {
                Statement::ForIn {
                    binding: declared, ..
                }
                | Statement::ForOf {
                    binding: declared, ..
                } if *declared == binding => return Ok(true),
                Statement::Try {
                    catch:
                        Some(Catch {
                            binding: Some(declared),
                            body,
                        }),
                    ..
                } if *declared == binding && *body == region => return Ok(true),
                _ => {}
            }
            region = parent;
            index = at;
        }
    }

    /// The bindings a value reads, when it is only literals, functions and
    /// reads of bindings (and, under pristine builtins, of standard globals),
    /// or arrays and objects (with literal keys) of them.
    fn settled_reads(&self, value: ExprId) -> Option<Vec<BindingId>> {
        let mut reads = Vec::new();
        let mut pending = vec![value];
        while let Some(id) = pending.pop() {
            match &self.expressions[id.index()] {
                Expr::Literal(_) | Expr::Function(_) | Expr::Regex(_) => {}
                Expr::Binding(binding)
                    if self.pristine_builtins && self.standard_global(*binding) => {}
                Expr::Host(_) | Expr::Member { .. }
                    if self.pristine_builtins && self.standard_member(id) => {}
                Expr::Binding(binding) => reads.push(*binding),
                Expr::Array(items) => {
                    for item in items {
                        if matches!(self.expressions[item.index()], Expr::Spread(_)) {
                            return None;
                        }
                        pending.push(*item);
                    }
                }
                Expr::Object(entries) => {
                    for (key, item) in entries {
                        match key {
                            Property::Named(name) if name != "__proto__" => {}
                            Property::Computed(key)
                                if matches!(
                                    self.expressions[key.index()],
                                    Expr::Literal(Literal::String(_) | Literal::Number(_))
                                ) => {}
                            _ => return None,
                        }
                        pending.push(*item);
                    }
                }
                _ => return None,
            }
        }
        Some(reads)
    }

    /// Whether `statement` (its nested regions included; no closure reaches
    /// the bindings) assigns one of `bindings`.
    fn statement_assigns(&self, statement: &Statement, bindings: &[BindingId]) -> bool {
        let mut expressions = Vec::new();
        statement.visit_expressions(|root| expressions.push(root));
        let mut regions = Vec::new();
        statement.visit_regions(|child| regions.push(child));
        loop {
            if let Some(id) = expressions.pop() {
                if let Expr::Assign { target, .. } = &self.expressions[id.index()] {
                    if matches!(self.expressions[target.index()], Expr::Binding(b) if bindings.contains(&b))
                    {
                        return true;
                    }
                }
                let _ = self.expressions[id.index()].visit_children(|child| {
                    expressions.push(child);
                    Ok::<_, ()>(())
                });
                continue;
            }
            let Some(region) = regions.pop() else {
                return false;
            };
            for statement in &self.regions[region.index()].statements {
                if let Statement::ForIn { binding, .. } | Statement::ForOf { binding, .. } =
                    statement
                {
                    if bindings.contains(binding) {
                        return true;
                    }
                }
                statement.visit_expressions(|root| expressions.push(root));
                if !matches!(statement, Statement::Function { .. }) {
                    statement.visit_regions(|child| regions.push(child));
                }
            }
        }
    }

    /// Where `binding` is read once in a branch of `statement` that runs at
    /// most once each time the statement does: an `if` arm (its test not
    /// mentioning the binding), a block, or a `try` part. The branch's first
    /// statement that mentions it must read it in expressions it evaluates
    /// once, or hold such a branch itself. A value creating a function keeps
    /// out of a `for…in`/`for…of` head and off a callee. Returns the branch,
    /// the statement's index there and the reading node.
    fn branch_reference(
        &self,
        statement: &Statement,
        binding: BindingId,
        creates: bool,
    ) -> Option<(RegionId, usize, (Leaf, usize))> {
        let mut branches = Vec::new();
        match statement {
            Statement::If { condition, yes, no } => {
                if self.mentions(&[], &[*condition], binding, false) {
                    return None;
                }
                branches.push(*yes);
                branches.extend(*no);
            }
            Statement::Block(body) => branches.push(*body),
            Statement::Try {
                body,
                catch,
                finally,
            } => {
                branches.push(*body);
                branches.extend(catch.as_ref().map(|catch| catch.body));
                branches.extend(*finally);
            }
            _ => return None,
        }
        for branch in branches {
            for (index, inner) in self.regions[branch.index()].statements.iter().enumerate() {
                if !self.statement_mentions(inner, binding) {
                    continue;
                }
                let head = matches!(inner, Statement::ForIn { .. } | Statement::ForOf { .. });
                if creates && head {
                    return None;
                }
                return match self.single_evaluation_reference(inner, binding) {
                    Some((leaf, path)) => (!creates || !self.calls_reference(leaf, binding))
                        .then_some((branch, index, (leaf, path))),
                    None => self.branch_reference(inner, binding, creates),
                };
            }
        }
        None
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
                    for function in expression.created_functions() {
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
                for function in expression.created_functions() {
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
                value: Some(value), ..
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
                Expr::Class { base, .. } => (*base)?,
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
            .chain(
                expressions
                    .iter()
                    .map(|expression| Node::Expr(*expression, false)),
            )
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
                    for function in expression.created_functions() {
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
                            Statement::Function { function, .. } => stack
                                .push(Node::Region(self.functions[function.index()].body, true)),
                            _ => {}
                        }
                    }
                }
            }
        }
        false
    }

    /// Record a proved binary choice before editing its subject. All target
    /// families use the same stable binding identity and immutable assignment.
    pub(crate) fn binary_choice(
        &mut self,
        binding: BindingId,
        family: ChoiceFamily,
        seed: bool,
        alternative_name: &'static str,
        saving: i64,
        choices: &ChoiceMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        use crate::output_budget::AllocationClass::Retained;
        use crate::representation::ChoiceAlternative;
        let key = ChoiceKey {
            family,
            site: match self.bindings[binding.index()].source_symbol {
                Some(symbol) => SiteId::Symbol(symbol.0),
                None => SiteId::Formed(binding.index() as u32),
            },
        };
        let seed = AltId(u8::from(seed));
        let applied = choices
            .get(key)
            .filter(|choice| choice.0 <= 1)
            .unwrap_or(seed);
        budget.work(
            crate::compilation_policy::WorkKind::Analysis,
            self.choice_sites.len() as u64 + 1,
        )?;
        if !self.choice_sites.iter().any(|site| site.key == key) {
            let name = budget.string(Retained, &self.bindings[binding.index()].spelling)?;
            let alternatives = budget.copy_slice(
                Retained,
                &[
                    ChoiceAlternative {
                        alternative: AltId(0),
                        name: "retained",
                        saving: 0,
                    },
                    ChoiceAlternative {
                        alternative: AltId(1),
                        name: alternative_name,
                        saving,
                    },
                ],
            )?;
            budget.push(
                Retained,
                &mut self.choice_sites,
                ChoiceSite {
                    estimate_codec: crate::config::CompressionCostModel::Raw,
            pinned: false,
                    key,
                    name,
                    alternatives,
                    seed,
                    applied,
                },
            )?;
        }
        Ok(applied == AltId(1))
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
            const_freezers: Vec::new(),
            immutable_data: Vec::new(),
            data_estimator: None,
            origins: vec![],
            authored_pool: Vec::new(),
            authored_pool_formed: false,
            forming_choices: crate::representation::RegionalChoices::NONE,
            authored_expressions: Vec::new(),
            authored_regions: Vec::new(),
            authored_sites: Vec::new(),
            functions: vec![],
            bindings: vec![],
            imports: vec![],
            exports: vec![],
            scopes,
            regions,
            root: RegionId::new(0),
            pristine_builtins: false,
            pure_property_reads: false,
            unconstructed_callbacks: false,
            private_names: None,
            root_rows: vec![],
            integrated_hosts: false,
            entries: vec![],
            consumer_annotations: crate::config::ConsumerAnnotations::Off,
            discardable_functions: Vec::new(),
            delivery: None,
            reserved: vec![],
            carried: vec![],
            loop_head_declarations: false,
            logical_statements: false,
            compound_assignments: false,
            quotes: false,
            int32_hints: false,
            choice_sites: Vec::new(),
            spelling_head: 0,
            spelling_regions: 0,
            spelling_functions: 0,
            spelling_node_count: 0,
            spelling_nodes: Vec::new(),
            print_forms: None,
            observed_literals: Vec::new(),
            behaviours: Vec::new(),
            journal: Journal::default(),
            settled: Vec::new(),
            first_runs: Vec::new(),
        })
    }

    /// Record that `binding` stores a module cell the program settles at
    /// root point `point` (M6.5).
    pub(crate) fn settle_in(
        &mut self,
        binding: BindingId,
        point: u32,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        grow_column(&mut self.settled, binding.index(), budget)?;
        self.settled[binding.index()] = Some(point);
        Ok(())
    }

    /// Record that `function` runs a unit the program may first run during
    /// root point `point` (M6.5).
    pub(crate) fn first_run_in(
        &mut self,
        function: FunctionId,
        point: u32,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        grow_column(&mut self.first_runs, function.index(), budget)?;
        self.first_runs[function.index()] = Some(point);
        Ok(())
    }

    /// The behaviour recorded for the operation node at `id`, while that
    /// node is still the one it describes.
    pub(crate) fn behaviour(&self, id: ExprId) -> Option<Behaviour> {
        let row = self
            .behaviours
            .binary_search_by_key(&id, |row| row.expression)
            .ok()
            .map(|index| &self.behaviours[index])?;
        (self.expressions.get(id.index()) == Some(&row.node)).then_some(row.behaviour)
    }

    /// Record `behaviour` for the operation node at `id` as it stands now,
    /// replacing an earlier row for that id.
    pub(crate) fn record_behaviour_in(
        &mut self,
        id: ExprId,
        behaviour: Behaviour,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let mut phase = budget.scope();
        let node = self.expressions[id.index()].clone_in(&mut phase)?;
        let bytes = node.payload_bytes()?;
        let row = BehaviourRow {
            expression: id,
            node,
            behaviour,
        };
        phase.finish_retained()?;
        match self
            .behaviours
            .binary_search_by_key(&id, |row| row.expression)
        {
            Ok(index) => {
                let old = std::mem::replace(&mut self.behaviours[index], row);
                let bytes = old.node.payload_bytes()?;
                drop(old);
                budget.release(AllocationClass::Retained, bytes)?;
            }
            Err(index) => {
                if let Err(error) = budget.reserve_vec(AllocationClass::Retained, &mut self.behaviours, 1) {
                    drop(row);
                    budget.release(AllocationClass::Retained, bytes)?;
                    return Err(error);
                }
                self.behaviours.insert(index, row);
            }
        }
        Ok(())
    }

    /// Carry the row of `from` to `to`, a node that evaluates the same way
    /// (a copy); `to` loses its own row when `from` has none.
    pub(crate) fn copy_behaviour_in(
        &mut self,
        from: ExprId,
        to: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        self.copy_author_choices(from, to);
        match self.behaviour(from) {
            Some(behaviour) => self.record_behaviour_in(to, behaviour, budget),
            None => {
                if let Ok(index) = self
                    .behaviours
                    .binary_search_by_key(&to, |row| row.expression)
                {
                    let row = self.behaviours.remove(index);
                    let bytes = row.node.payload_bytes()?;
                    drop(row);
                    budget.release(AllocationClass::Retained, bytes)?;
                }
                Ok(())
            }
        }
    }

    /// Whether the literal at `id` is observed only for truthiness or
    /// nullishness, so no pass may rewrite it.
    pub(crate) fn observed(&self, id: ExprId) -> bool {
        self.observed_literals
            .binary_search_by_key(&id, |alternative| alternative.expression())
            .is_ok()
    }

    /// Record an observed literal formation creates. Formation appends its
    /// expressions, so the list stays sorted.
    pub(crate) fn observe_in(
        &mut self,
        alternative: LiteralAlternative,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        debug_assert!(self
            .observed_literals
            .last()
            .is_none_or(|last| last.expression() < alternative.expression()));
        budget.push(
            AllocationClass::Retained,
            &mut self.observed_literals,
            alternative,
        )
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

    pub fn expression(&mut self, expression: Expr, origin: Option<SourceOriginId>) -> ExprId {
        self.expression_in(expression, origin, &mut AllocationBudget::new(None))
            .expect("structured target allocation failed")
    }

    pub(crate) fn expression_in(
        &mut self,
        expression: Expr,
        origin: Option<SourceOriginId>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<ExprId, AllocationError> {
        let id = ExprId::try_new(self.expressions.len()).ok_or(AllocationError::Capacity)?;
        budget.reserve_vec(AllocationClass::Retained, &mut self.expressions, 1)?;
        budget.reserve_vec(AllocationClass::Retained, &mut self.origins, 1)?;
        if !self.spelling_nodes.is_empty() {
            budget.push(AllocationClass::Retained, &mut self.spelling_nodes, None)?;
        }
        if !self.forming_choices.is_empty() || !self.authored_expressions.is_empty() {
            let missing = id.index().saturating_sub(self.authored_expressions.len());
            budget.reserve_vec(AllocationClass::Retained, &mut self.authored_expressions, missing + 1)?;
            self.authored_expressions.resize(id.index(), crate::representation::RegionalChoices::NONE);
            self.authored_expressions.push(self.forming_choices);
        }
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
            Expr::Host(host) => host.name == name,
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
        Ok(print::render(self, &names))
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

/// A property key whose numeric literal names the same key: a canonical
/// integer (no sign unless `signed`, no leading zero, not `-0`) of at most
/// 2^53 − 1, whose value prints back as exactly these digits, so `{32:v}` and
/// `o[32]` are `{"32":v}` and `o["32"]` (architecture §12, L2). Closure's
/// `CodeGenerator.isSimpleNumber`/`getSimpleNumber` (`closure-compiler@0da58e1
/// CodeGenerator.java:1596-1622`) is the same test; Terser prints any
/// `""+ +key==key && key>=0` key as a number (`terser@8fa44c8
/// lib/output.js:2245`), esbuild and Oxc only int32 ones.
pub(crate) fn simple_number_key(key: &str, signed: bool) -> bool {
    let digits = match key.strip_prefix('-') {
        Some(rest) if signed => rest,
        Some(_) => return false,
        None => key,
    };
    let canonical = match digits.as_bytes() {
        [] => false,
        [b'0'] => digits.len() == key.len(),
        [first, ..] => *first != b'0' && digits.bytes().all(|b| b.is_ascii_digit()),
    };
    canonical && digits.len() <= 16 && digits.parse::<u64>().is_ok_and(|value| value < 1 << 53)
}
