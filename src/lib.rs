pub mod ast;
pub mod codegen_ir_js;
pub mod codegen_js;
pub mod codegen_native;
pub mod compiler;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod optimizer;
pub mod parser;
pub mod semantic;
pub mod span;

pub use codegen_js::{compile_to_js, CodegenError, CodegenOptions, CompileError, JsEmitter};
pub use codegen_native::{compile_to_c, emit_native_c};
pub use compiler::{
    compile_source, compile_source_all, compile_source_to_c, render_diagnostic,
    CompilationArtifacts, SourceCompileError,
};
pub use lower::{lower_to_control_flow, LowerError};
pub use parser::{parse_source, ParseError, Parser};
pub use semantic::{analyze, SemanticError, SemanticModel, Type};
