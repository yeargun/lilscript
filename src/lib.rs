pub mod ast;
pub mod codegen_ir_js;
pub mod codegen_js;
pub mod codegen_native;
pub mod compilation_contract;
pub mod compiler;
pub mod compress_passes;
pub mod config;
pub mod decision_registry;
pub mod for_of_family;
pub mod formatter;
pub mod interpreter;
pub mod ir;
pub mod js_peephole;
mod js_regex;
pub mod js_syntax_target;
pub mod lexer;
pub mod lint;
pub mod lower;
pub mod module;
pub mod optimizer;
pub mod package;
pub mod parser;
pub mod profile;
pub mod semantic;
pub mod span;
mod stable_hash;
pub mod typed_array;
pub mod value_analysis;

pub use codegen_js::{compile_to_js, CodegenError, CodegenOptions, CompileError, JsEmitter};
pub use codegen_native::{compile_to_c, emit_native_c, emit_native_c_with_options, NativeOptions};
pub use compilation_contract::{
    JavaScriptAbiContract, JavaScriptAbiManifest, JavaScriptCompilationContract,
    JavaScriptEffectPolicy, JavaScriptExportAbi, JavaScriptExportKind, JavaScriptMethodAbi,
    JavaScriptOptimizationObjective, JavaScriptUnsafeAssumptions, JavaScriptWorld,
};
pub use compiler::{
    canonical_brotli_version, canonical_zlib_version, compile_path, compile_path_all,
    compile_path_all_configured, compile_path_all_to_js_bundle_configured, compile_path_configured,
    compile_path_explained_configured, compile_path_to_c, compile_path_to_c_configured,
    compile_path_to_js_bundle_configured, compile_path_to_js_module,
    compile_path_to_js_module_configured, compile_path_to_js_module_explained_configured,
    compile_path_to_js_module_with_source, compile_path_with_source,
    compile_path_with_source_configured, compile_source, compile_source_all, compile_source_to_c,
    compile_source_to_js_module, measure_javascript_transfer_sizes,
    profile_template_path_configured, render_diagnostic, render_module_diagnostic,
    BundledCompilationArtifacts, CompilationArtifacts, JavaScriptBundle, JavaScriptBundleFile,
    JavaScriptBundleManifest, JavaScriptBundleManifestChunk, JavaScriptBundleObjectiveManifest,
    JavaScriptCompilation, JavaScriptSelectionMetrics, JavaScriptTransferSizes, SourceCompileError,
    CANONICAL_BROTLI_LIBRARY_VERSION, CANONICAL_BROTLI_PACKAGE_VERSION,
    CANONICAL_ZLIB_LIBRARY_VERSION, CANONICAL_ZLIB_PACKAGE_VERSION,
};
pub use interpreter::{
    interpret_program, interpret_program_with_limits, InterpretError, InterpreterLimits,
};
pub use lint::{
    lint_path_with_providers, LintProviderDiagnostic, LintRuleContext, LintRuleProvider,
    WebRuleProvider,
};
pub use lower::{lower_to_control_flow, LowerError};
pub use module::ModuleError;
pub use parser::{parse_source, ParseError, Parser};
pub use profile::{JavaScriptPerformanceMetrics, OptimizationProfile};
pub use semantic::{analyze, SemanticError, SemanticModel, Type};
