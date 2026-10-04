use crate::js_syntax_target::EcmaScriptEdition;

/// The set of consumers whose observations constrain JavaScript lowering.
/// Both variants still optimize the complete statically linked LilScript graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaScriptWorld {
    ClosedApplication,
    ReusableLibrary,
}

/// How the complete artifact must be loaded, independently of who consumes
/// its exports. Module output requires ECMAScript module execution even when
/// it exports nothing. Script output makes no strict-mode entry guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaScriptExecution {
    Module,
    Script,
}

impl JavaScriptExecution {
    pub fn guarantees_strict_execution(self) -> bool {
        matches!(self, Self::Module)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptAbiContract {
    pub preserve_root_exports: bool,
    /// Every function whose name some code could read keeps its exact source
    /// name, not only published exports.
    pub keep_function_names: bool,
    /// Published functions keep their exact source name (D2). A contract
    /// that publishes names but not `fn.name` turns it off.
    pub keep_published_function_names: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptUnsafeAssumptions {
    pub pristine_builtins: bool,
    pub pure_property_reads: bool,
    /// Code outside the program never constructs a function the program
    /// hands it, nor reads its `prototype` (Terser's `unsafe_arrows`).
    pub unconstructed_callbacks: bool,
    /// A property whose name matches `^_(?!_)` and is not preserved is private
    /// to the program wherever it occurs, so one program-wide map may rename
    /// it (Terser's `mangle.properties.regex`, as posthog-js builds with it).
    pub private_underscore_properties: bool,
    /// No value is `document.all`, the one object that is `== null`: a loose
    /// null test is exactly a test for `null` or `undefined` (Terser's compress
    /// folds `x === null || x === undefined` into `null == x`).
    pub no_document_all: bool,
    /// No object the program copies has an own `__proto__` property, so
    /// `Object.assign({}, a, {k: v})` is the object spread `{...a, k: v}`.
    pub no_proto_keys: bool,
    /// A host value's `length` is an int32 Number, as it is for strings,
    /// arrays, typed arrays, `arguments` and functions: size-first's
    /// length-to-number decision.
    pub numeric_lengths: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptEffectPolicy {
    /// Calls of the host `debugLog` extern are dropped (their arguments are
    /// still evaluated). `print` is a program effect and is never dropped.
    pub strip_debug: bool,
    /// Calls of the host `console` object's methods are dropped (their
    /// arguments are still evaluated): a declared relaxation of console output.
    pub strip_console_calls: bool,
}

/// Whether the program's preconditions are checked (language.md R1, R11, R12;
/// future-architecture's contract axis). `Development` emits a check at each
/// precondition that lands (an index read so far), which throws where
/// production gives an unspecified result. It is independent of effort: a
/// development build at level 13 checks everything, and level 0 in
/// production checks nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreconditionChecks {
    #[default]
    Production,
    Development,
}

impl PreconditionChecks {
    pub fn name(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Development => "development",
        }
    }
}

/// Immutable legality input for JavaScript compilation. This is intentionally
/// separate from profitability and search effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptCompilationContract {
    pub world: JavaScriptWorld,
    pub execution: JavaScriptExecution,
    pub ecmascript: EcmaScriptEdition,
    pub abi: JavaScriptAbiContract,
    pub assumptions: JavaScriptUnsafeAssumptions,
    pub effects: JavaScriptEffectPolicy,
    pub checks: PreconditionChecks,
}

impl JavaScriptCompilationContract {
    /// Whether host reflection over compiler-owned frames is outside the
    /// contract, so a frame may be elided or moved: strict code shows no
    /// caller to the code it calls, and in an application's world (owner
    /// answer Y5, 2026-09-29: the frames amendment) no code the program does
    /// not own may look at its frames. This is about reflection only; strict
    /// mode's own semantics (`this`, `delete`, failed writes, mapped
    /// `arguments`) follow `JavaScriptExecution::guarantees_strict_execution`.
    pub fn frames_hidden(&self) -> bool {
        self.execution.guarantees_strict_execution()
            || self.world == JavaScriptWorld::ClosedApplication
    }
}
