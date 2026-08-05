pub mod ast;
pub mod codegen_ir_js;
pub mod codegen_js;
pub mod codegen_native;
pub mod compiler;
pub mod config;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod module;
pub mod optimizer;
pub mod parser;
pub mod semantic;
pub mod span;

pub use codegen_js::{compile_to_js, CodegenError, CodegenOptions, CompileError, JsEmitter};
pub use codegen_native::{compile_to_c, emit_native_c};
pub use compiler::{
    compile_path, compile_path_all, compile_path_all_configured, compile_path_configured,
    compile_path_to_c, compile_path_to_c_configured, compile_path_to_js_bundle_configured,
    compile_path_to_js_module, compile_path_to_js_module_configured,
    compile_path_to_js_module_with_source, compile_path_with_source, compile_source,
    compile_source_all, compile_source_to_c, compile_source_to_js_module, render_diagnostic,
    render_module_diagnostic, CompilationArtifacts, JavaScriptBundle, JavaScriptBundleFile,
    JavaScriptBundleManifest, JavaScriptBundleManifestChunk, SourceCompileError,
};
pub use lower::{lower_to_control_flow, LowerError};
pub use module::ModuleError;
pub use parser::{parse_source, ParseError, Parser};
pub use semantic::{analyze, SemanticError, SemanticModel, Type};
