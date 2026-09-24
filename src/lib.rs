mod arena_budget;
pub mod ast;
pub mod build;
pub mod check;
pub mod compilation_contract;
pub mod compilation_policy;
#[cfg(test)]
mod compilation_policy_evidence_tests;
pub mod compression;
pub mod config;
pub mod diagnostics;
pub mod formatter;
mod host_modules;
pub mod interpreter;
pub mod js;
pub mod js_platform;
mod js_regex;
mod js_string;
pub mod js_syntax_target;
pub mod lexer;
pub mod lint;
pub mod literal;
pub mod module;
mod output_budget;
pub mod package;
pub mod parser;
pub mod primitive;
pub mod program;
pub(crate) mod scalar_transfer;
#[cfg(test)]
mod scalar_transfer_tests;
pub mod span;
mod stable_hash;
pub mod timing;
pub mod typed_array;

pub use build::{
    build_inputs, check_path, check_source, compile_path, compile_source, with_checked_path,
    with_checked_program, with_checked_source, BuildInputs, CheckedProgram, CheckedSourceSession,
    ChunkExtension, FinishedSourceSession, ServiceCompilation, ServiceError, ServiceJavaScript,
    ServiceJavaScriptBatch, ServiceOptions, ServiceTarget,
};
pub use check::{analyze, CheckError, CheckedModule, Type};
pub use compilation_contract::{
    JavaScriptAbiContract, JavaScriptCompilationContract, JavaScriptEffectPolicy,
    JavaScriptExecution, JavaScriptUnsafeAssumptions, JavaScriptWorld,
};
pub use diagnostics::{
    render_diagnostic, render_message_diagnostic, render_module_diagnostic, render_service_error,
    SourceDiagnostic,
};
pub use interpreter::{
    interpret_program, interpret_program_with_limits, InterpretError, InterpreterLimits,
};
pub use js::manifest::{
    javascript_bundle, JavaScriptBundle, JavaScriptBundleFile, JavaScriptBundleManifest,
    JavaScriptBundleManifestChunk, JavaScriptBundleObjectiveManifest, ManifestFile,
};
pub use lint::{
    lint_checked, lint_checked_with_providers, lint_path_with_providers, LintProviderDiagnostic,
    LintRuleContext, LintRuleProvider, WebRuleProvider,
};
pub use module::ModuleError;
pub use parser::{parse_source, ParseError, Parser};
