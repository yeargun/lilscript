mod arena_budget;
pub mod artifact_memo;
pub mod ast;
pub mod codegen_ir_js;
pub mod codegen_js;
pub mod codegen_native;
pub mod compilation_contract;
pub mod compilation_policy;
#[cfg(test)]
mod compilation_policy_evidence_tests;
pub mod compiler;
pub mod compiler_service;
pub mod compress_passes;
pub mod compression;
pub mod config;
pub mod decision_registry;
pub mod for_of_family;
pub mod formatter;
pub mod interpreter;
pub mod ir;
mod js_externs;
pub mod js_peephole;
mod js_regex;
mod js_string;
pub mod js_syntax_target;
pub mod lexer;
pub mod lint;
pub mod literal;
pub mod lower;
pub mod module;
pub mod optimizer;
mod output_budget;
pub mod package;
pub mod parser;
pub mod primitive;
pub mod profile;
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
pub mod value_analysis;

pub use codegen_js::{CodegenError, CodegenOptions, CompileError, JsEmitter, compile_to_js};
pub use codegen_native::{NativeOptions, compile_to_c, emit_native_c, emit_native_c_with_options};
pub use compilation_contract::{
    JavaScriptAbiContract, JavaScriptAbiManifest, JavaScriptCompilationContract,
    JavaScriptEffectPolicy, JavaScriptExportAbi, JavaScriptExportKind, JavaScriptMethodAbi,
    JavaScriptOptimizationObjective, JavaScriptUnsafeAssumptions, JavaScriptWorld,
};
pub use compiler::{
    BundledCompilationArtifacts, CANONICAL_BROTLI_LIBRARY_VERSION,
    CANONICAL_BROTLI_PACKAGE_VERSION, CANONICAL_ZLIB_LIBRARY_VERSION,
    CANONICAL_ZLIB_PACKAGE_VERSION, CompilationArtifacts, JavaScriptBundle, JavaScriptBundleFile,
    JavaScriptBundleManifest, JavaScriptBundleManifestChunk, JavaScriptBundleObjectiveManifest,
    JavaScriptCompilation, JavaScriptSelectionMetrics, JavaScriptTransferSizes, SourceCompileError,
    canonical_brotli_version, canonical_zlib_version, compile_path, compile_path_all,
    compile_path_all_configured, compile_path_all_to_js_bundle_configured, compile_path_configured,
    compile_path_explained_configured, compile_path_to_c, compile_path_to_c_configured,
    compile_path_to_js_bundle_configured, compile_path_to_js_module,
    compile_path_to_js_module_configured, compile_path_to_js_module_explained_configured,
    compile_path_to_js_module_with_source, compile_path_with_source,
    compile_path_with_source_configured, compile_source, compile_source_all, compile_source_to_c,
    compile_source_to_js_module, measure_javascript_transfer_sizes,
    profile_template_path_configured, render_diagnostic, render_module_diagnostic,
};
pub use compiler_service::{
    CheckedSourceSession, FinishedSourceSession, ServiceCompilation, ServiceError,
    ServiceJavaScript, ServiceJavaScriptBatch, ServiceOptions, ServiceTarget,
    compile_path_semantic, compile_source_semantic, with_checked_path, with_checked_source,
};
pub use interpreter::{
    InterpretError, InterpreterLimits, interpret_program, interpret_program_with_limits,
};
pub use lint::{
    LintProviderDiagnostic, LintRuleContext, LintRuleProvider, WebRuleProvider,
    lint_path_with_providers,
};
pub use lower::{LowerError, lower_to_control_flow};
pub use module::ModuleError;
pub use parser::{ParseError, Parser, parse_source};
pub use profile::{JavaScriptPerformanceMetrics, OptimizationProfile};
pub use semantic::{SemanticError, SemanticModel, Type, analyze};
