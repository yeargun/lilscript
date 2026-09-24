mod arena_budget;
pub mod ast;
pub mod compilation_contract;
pub mod compilation_policy;
#[cfg(test)]
mod compilation_policy_evidence_tests;
pub mod compiler_service;
pub mod compression;
mod host_modules;
pub mod config;
pub mod diagnostics;
pub mod formatter;
pub mod interpreter;
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
pub(crate) mod scalar_transfer;
#[cfg(test)]
mod scalar_transfer_tests;
pub mod semantic;
pub mod semantic_program;
pub mod span;
mod stable_hash;
pub mod structured_js;
pub mod timing;
pub mod typed_array;

pub use compilation_contract::{
    JavaScriptAbiContract, JavaScriptCompilationContract, JavaScriptEffectPolicy,
    JavaScriptExecution, JavaScriptUnsafeAssumptions, JavaScriptWorld,
};
pub use diagnostics::{
    SourceDiagnostic, render_diagnostic, render_message_diagnostic, render_module_diagnostic,
    render_service_error,
};
pub use structured_js::manifest::{
    JavaScriptBundle, JavaScriptBundleFile, JavaScriptBundleManifest,
    JavaScriptBundleManifestChunk, JavaScriptBundleObjectiveManifest, ManifestFile,
    javascript_bundle,
};
pub use compiler_service::{
    BuildInputs, CheckedProgram, CheckedSourceSession, FinishedSourceSession, ServiceCompilation,
    ServiceError, ChunkExtension, ServiceJavaScript, ServiceJavaScriptBatch, ServiceOptions,
    ServiceTarget, build_inputs, check_path, check_source, compile_path_semantic,
    compile_source_semantic, with_checked_path, with_checked_program, with_checked_source,
};
pub use interpreter::{
    InterpretError, InterpreterLimits, interpret_program, interpret_program_with_limits,
};
pub use lint::{
    LintProviderDiagnostic, LintRuleContext, LintRuleProvider, WebRuleProvider, lint_checked,
    lint_checked_with_providers, lint_path_with_providers,
};
pub use module::ModuleError;
pub use parser::{ParseError, Parser, parse_source};
pub use semantic::{SemanticError, SemanticModel, Type, analyze};
