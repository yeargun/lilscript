use bumpalo::Bump;

use crate::codegen_ir_js::emit_optimized_ir_js;
use crate::codegen_js::{compile_to_js, CompileError};
use crate::codegen_native::{compile_to_c, emit_native_c};
use crate::lower::lower_to_control_flow;
use crate::optimizer::{optimize_control_flow, OptimizationReport};
use crate::parser::{parse_source, ParseError};
use crate::semantic::analyze;
use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct CompilationArtifacts {
    pub javascript: String,
    pub c: String,
    pub optimization_reports: Vec<OptimizationReport>,
}

#[derive(Debug)]
pub enum SourceCompileError {
    Parse(ParseError),
    Compile(CompileError),
}

impl SourceCompileError {
    pub const fn span(&self) -> Span {
        match self {
            Self::Parse(error) => error.span(),
            Self::Compile(error) => error.span(),
        }
    }
}

impl std::fmt::Display for SourceCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::Compile(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SourceCompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Compile(error) => Some(error),
        }
    }
}

impl From<ParseError> for SourceCompileError {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

impl From<CompileError> for SourceCompileError {
    fn from(value: CompileError) -> Self {
        Self::Compile(value)
    }
}

pub fn compile_source(source: &str) -> Result<String, SourceCompileError> {
    let arena = Bump::new();
    let program = parse_source(&arena, source)?;
    compile_to_js(&program).map_err(Into::into)
}

pub fn compile_source_to_c(source: &str) -> Result<String, SourceCompileError> {
    let arena = Bump::new();
    let program = parse_source(&arena, source)?;
    compile_to_c(&program).map_err(Into::into)
}

/// Compiles both backends from one parsed, checked, and optimized IR module.
///
/// Native object-code generation remains a CLI concern because it invokes the
/// host C toolchain, but the C text used for that executable is returned here.
pub fn compile_source_all(source: &str) -> Result<CompilationArtifacts, SourceCompileError> {
    let arena = Bump::new();
    let program = parse_source(&arena, source)?;
    let semantics = analyze(&program).map_err(CompileError::from)?;
    let mut ir = lower_to_control_flow(&program, &semantics).map_err(CompileError::from)?;
    let optimization_reports = optimize_control_flow(&mut ir).map_err(CompileError::from)?;
    let javascript = emit_optimized_ir_js(&ir).map_err(CompileError::from)?;
    let c = emit_native_c(&ir).map_err(CompileError::from)?;
    Ok(CompilationArtifacts {
        javascript,
        c,
        optimization_reports,
    })
}

pub fn render_diagnostic(
    path: &std::path::Path,
    source: &str,
    error: &SourceCompileError,
) -> String {
    let span = error.span();
    let start = span.start.min(source.len());
    let end = span.end.min(source.len()).max(start);
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |index| end + index);
    let line_number = source[..line_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let column = source[line_start..start].chars().count() + 1;
    let width = source[start..end].chars().count().max(1);
    let source_line = &source[line_start..line_end];
    let padding = " ".repeat(column.saturating_sub(1));
    let marker = "^".repeat(width);

    format!(
        "error: {error}\n --> {}:{line_number}:{column}\n  |\n{line_number:>2} | {source_line}\n  | {padding}{marker}",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_source_end_to_end() {
        assert_eq!(compile_source("print(40+2);").unwrap(), "console.log(42)");
    }

    #[test]
    fn renders_source_location() {
        let source = "int ok=1;\nint bad=\"x\";";
        let error = compile_source(source).unwrap_err();
        let rendered = render_diagnostic(std::path::Path::new("sample.lil"), source, &error);
        assert!(rendered.contains("sample.lil:2:9"));
        assert!(rendered.contains("int bad=\"x\";"));
    }

    #[test]
    fn compiles_v01_control_flow() {
        let source = "int sum=0;for(int i=0;i<3;i++){sum+=i;}print(`sum=${sum}`);";
        let output = compile_source(source).unwrap();
        assert!(output.contains("console.log"));
        assert!(output.contains("while("));
        assert!(!output.contains("switch("));
        assert!(output.contains("`sum=${"));
    }

    #[test]
    fn reconstructs_short_circuit_control_flow_without_a_state_machine() {
        let source = "extern bool read();bool value=read()&&read()||read();print(value);";
        let output = compile_source(source).unwrap();
        assert!(!output.contains("switch("));
        assert!(output.contains("console.log"));
    }

    #[test]
    fn compiles_source_to_native_c() {
        let output = compile_source_to_c("int value=40+2;print(value);").unwrap();
        assert!(output.contains("int main(void)"));
        assert!(output.contains("printf(\"%d\\n\""));
    }

    #[test]
    fn compiles_all_backends_from_one_optimized_module() {
        let output = compile_source_all("int value=40+2;print(value);").unwrap();
        assert_eq!(output.javascript, "console.log(42)");
        assert!(output.c.contains("int main(void)"));
        assert!(output.c.contains("printf(\"%d\\n\""));
        assert!(output
            .optimization_reports
            .iter()
            .any(|report| report.pass_name == "constant-propagation" && report.changed));
    }
}
