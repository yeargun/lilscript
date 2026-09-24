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
    /// A host value's `length` is an int32 Number, as it is for strings,
    /// arrays, typed arrays, `arguments` and functions: size-first's
    /// length-to-number decision.
    pub numeric_lengths: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptEffectPolicy {
    pub strip_console: bool,
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
}
