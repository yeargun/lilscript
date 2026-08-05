use bumpalo::Bump;
use std::path::Path;

use crate::codegen_ir_js::{
    emit_optimized_ir_js, emit_optimized_ir_js_module, emit_optimized_ir_js_module_with_options,
    emit_optimized_ir_js_with_options,
};
use crate::codegen_js::{compile_to_js, CompileError};
use crate::codegen_native::{compile_to_c, emit_native_c};
use crate::config::ProjectConfig;
use crate::lower::lower_to_control_flow;
use crate::module::{
    discover_modules, discover_modules_with_source, link_modules, locate_linked_span,
    parse_modules, ModuleError, ModuleSet,
};
use crate::optimizer::{
    optimize_control_flow, optimize_control_flow_for_module, optimize_control_flow_with_options,
    OptimizationReport,
};
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

pub fn compile_source_to_js_module(source: &str) -> Result<String, SourceCompileError> {
    let arena = Bump::new();
    let program = parse_source(&arena, source)?;
    compile_program_to_js_module(&program).map_err(Into::into)
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

pub fn compile_path(path: &Path) -> Result<String, ModuleError> {
    compile_path_js_inner(path, None)
}

pub fn compile_path_configured(path: &Path, config: &ProjectConfig) -> Result<String, ModuleError> {
    compile_path_js_configured_inner(path, None, config)
}

pub fn compile_path_to_c(path: &Path) -> Result<String, ModuleError> {
    let modules = discover_modules(path)?;
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_to_c(&linked).map_err(|error| module_compile_error(&modules, error))
}

pub fn compile_path_to_c_configured(
    path: &Path,
    config: &ProjectConfig,
) -> Result<String, ModuleError> {
    let modules = discover_modules(path)?;
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_to_c_configured(&linked, config)
        .map_err(|error| module_compile_error(&modules, error))
}

pub fn compile_path_to_js_module(path: &Path) -> Result<String, ModuleError> {
    compile_path_js_module_inner(path, None)
}

pub fn compile_path_to_js_module_configured(
    path: &Path,
    config: &ProjectConfig,
) -> Result<String, ModuleError> {
    compile_path_js_module_configured_inner(path, None, config)
}

pub fn compile_path_to_js_module_with_source(
    path: &Path,
    source: &str,
) -> Result<String, ModuleError> {
    compile_path_js_module_inner(path, Some(source))
}

pub fn compile_path_all(path: &Path) -> Result<CompilationArtifacts, ModuleError> {
    compile_path_all_inner(path, None)
}

pub fn compile_path_all_configured(
    path: &Path,
    config: &ProjectConfig,
) -> Result<CompilationArtifacts, ModuleError> {
    compile_path_all_configured_inner(path, None, config)
}

pub fn compile_path_with_source(path: &Path, source: &str) -> Result<String, ModuleError> {
    compile_path_js_inner(path, Some(source))
}

fn compile_path_js_inner(path: &Path, root_source: Option<&str>) -> Result<String, ModuleError> {
    let modules = match root_source {
        Some(source) => discover_modules_with_source(path, source)?,
        None => discover_modules(path)?,
    };
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_to_js(&linked).map_err(|error| module_compile_error(&modules, error))
}

fn compile_path_js_configured_inner(
    path: &Path,
    root_source: Option<&str>,
    config: &ProjectConfig,
) -> Result<String, ModuleError> {
    let modules = match root_source {
        Some(source) => discover_modules_with_source(path, source)?,
        None => discover_modules(path)?,
    };
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_to_js_configured(&linked, config)
        .map_err(|error| module_compile_error(&modules, error))
}

fn compile_path_js_module_inner(
    path: &Path,
    root_source: Option<&str>,
) -> Result<String, ModuleError> {
    let modules = match root_source {
        Some(source) => discover_modules_with_source(path, source)?,
        None => discover_modules(path)?,
    };
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_to_js_module(&linked).map_err(|error| module_compile_error(&modules, error))
}

fn compile_path_js_module_configured_inner(
    path: &Path,
    root_source: Option<&str>,
    config: &ProjectConfig,
) -> Result<String, ModuleError> {
    let modules = match root_source {
        Some(source) => discover_modules_with_source(path, source)?,
        None => discover_modules(path)?,
    };
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_to_js_module_configured(&linked, config)
        .map_err(|error| module_compile_error(&modules, error))
}

fn compile_path_all_inner(
    path: &Path,
    root_source: Option<&str>,
) -> Result<CompilationArtifacts, ModuleError> {
    let modules = match root_source {
        Some(source) => discover_modules_with_source(path, source)?,
        None => discover_modules(path)?,
    };
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_all(&linked).map_err(|error| module_compile_error(&modules, error))
}

fn compile_path_all_configured_inner(
    path: &Path,
    root_source: Option<&str>,
    config: &ProjectConfig,
) -> Result<CompilationArtifacts, ModuleError> {
    let modules = match root_source {
        Some(source) => discover_modules_with_source(path, source)?,
        None => discover_modules(path)?,
    };
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    compile_program_all_configured(&linked, config)
        .map_err(|error| module_compile_error(&modules, error))
}

fn compile_program_all<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
) -> Result<CompilationArtifacts, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    let optimization_reports = optimize_control_flow(&mut ir)?;
    let javascript = emit_optimized_ir_js(&ir)?;
    let c = emit_native_c(&ir)?;
    Ok(CompilationArtifacts {
        javascript,
        c,
        optimization_reports,
    })
}

fn compile_program_all_configured<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
    config: &ProjectConfig,
) -> Result<CompilationArtifacts, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    let optimization_reports =
        optimize_control_flow_with_options(&mut ir, &config.optimizer_options(), false)?;
    let javascript = emit_optimized_ir_js_with_options(&ir, &config.js_options())?;
    let c = emit_native_c(&ir)?;
    Ok(CompilationArtifacts {
        javascript,
        c,
        optimization_reports,
    })
}

fn compile_program_to_js<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow(&mut ir)?;
    emit_optimized_ir_js(&ir).map_err(Into::into)
}

fn compile_program_to_js_configured<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
    config: &ProjectConfig,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow_with_options(&mut ir, &config.optimizer_options(), false)?;
    emit_optimized_ir_js_with_options(&ir, &config.js_options()).map_err(Into::into)
}

fn compile_program_to_js_module<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow_for_module(&mut ir)?;
    emit_optimized_ir_js_module(&ir).map_err(Into::into)
}

fn compile_program_to_js_module_configured<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
    config: &ProjectConfig,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow_with_options(&mut ir, &config.optimizer_options(), true)?;
    emit_optimized_ir_js_module_with_options(&ir, &config.js_options()).map_err(Into::into)
}

fn compile_program_to_c<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow(&mut ir)?;
    emit_native_c(&ir).map_err(Into::into)
}

fn compile_program_to_c_configured<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
    config: &ProjectConfig,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    optimize_control_flow_with_options(&mut ir, &config.optimizer_options(), false)?;
    emit_native_c(&ir).map_err(Into::into)
}

fn module_compile_error(modules: &ModuleSet, error: CompileError) -> ModuleError {
    let span = error.span();
    let message = match &error {
        CompileError::Semantic(error) => error.message.clone(),
        CompileError::Lower(error) => error.message.clone(),
        CompileError::Optimize(error) => error.message.clone(),
        CompileError::Codegen(error) => error.message.clone(),
    };
    let (module, local_span) = locate_linked_span(modules, span);
    ModuleError::new(&module.path, &module.source, local_span, message)
}

pub fn render_module_diagnostic(error: &ModuleError) -> String {
    render_message_diagnostic(&error.path, &error.source, error.span, &error.message)
}

pub fn render_diagnostic(
    path: &std::path::Path,
    source: &str,
    error: &SourceCompileError,
) -> String {
    render_message_diagnostic(path, source, error.span(), &error.to_string())
}

fn render_message_diagnostic(path: &Path, source: &str, span: Span, message: &str) -> String {
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
        "error: {message}\n --> {}:{line_number}:{column}\n  |\n{line_number:>2} | {source_line}\n  | {padding}{marker}",
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
    fn compiles_nested_control_flow_after_cfg_inlining() {
        let output = compile_source(
            "int scan(int limit){for(int index=0;index<limit;index++){if(index==3){return index;}}return 0;}int outer(){int total=0;for(int index=0;index<5;index++){total+=scan(index);}return total;}print(outer());",
        )
        .unwrap();
        assert!(output.contains("console.log"));
        assert!(!output.contains("let ="));
        assert!(!output.contains("switch("));
    }

    #[test]
    fn does_not_fold_array_length_across_mutation() {
        let output =
            compile_source("int[] values=[];values.push(1);print(values.length);").unwrap();
        assert!(output.contains(".push(1)"));
        assert!(output.contains(".length"));
        assert!(!output.contains("console.log(0)"));
    }

    #[test]
    fn inlines_disjoint_top_level_control_flow_regions() {
        let output = compile_source(
            "int gcd(int a,int b){while(b!=0){int next=a%b;a=b;b=next;}return a;}int fib(int count){int a=0;int b=1;for(int i=0;i<count;i++){int next=a+b;a=b;b=next;}return a;}print(gcd(21,14));print(fib(8));",
        )
        .unwrap();
        assert!(!output.contains("function"));
        assert!(!output.contains("switch("));
        assert_eq!(output.matches("while(").count(), 2);
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

    #[test]
    fn repeated_compilation_is_byte_deterministic() {
        let source = r#"
            struct Zeta { int right; int left; }
            struct Alpha { int x; int y; }
            class Counter {
                int value;
                init(int value) { this.value = value; }
                int read() { return this.value; }
            }
            extern void consumeAlpha(Alpha value);
            extern void consumeZeta(Zeta value);
            extern void consumeCounter(Counter value);
            int calculate(int seed) {
                int first = seed + 1;
                int second = seed + 2;
                int third = first * second;
                if (third > 10) { return third - first; }
                return third + second;
            }
            Alpha alpha = Alpha{calculate(3), 2};
            Zeta zeta = Zeta{4, calculate(5)};
            Counter counter = new Counter(calculate(7));
            consumeAlpha(alpha);
            consumeZeta(zeta);
            consumeCounter(counter);
        "#;
        let expected = compile_source_all(source).unwrap();

        for _ in 0..16 {
            let actual = compile_source_all(source).unwrap();
            assert_eq!(actual.javascript, expected.javascript);
            assert_eq!(actual.c, expected.c);
        }
    }

    #[test]
    fn validates_declared_pure_functions() {
        let error = compile_source("pure int bad(int value){print(value);return value;}")
            .expect_err("printing from a pure function must fail");
        assert!(error.to_string().contains("declared `pure`"));

        assert_eq!(
            compile_source(
                "pure int square(int value){int copy=value;return copy*copy;}print(square(5));"
            )
            .unwrap(),
            "console.log(25)"
        );
    }

    #[test]
    fn compiles_and_tree_shakes_a_module_graph() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-module-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let math = directory.join("math.lil");
        let main = directory.join("main.lil");
        std::fs::write(
            &math,
            "export pure int square(int value){return value*value;}int unused(){return 99;}",
        )
        .unwrap();
        std::fs::write(
            directory.join("facade.lil"),
            "import {square} from \"./math\";export {square};",
        )
        .unwrap();
        std::fs::write(
            &main,
            "import {square as sq} from \"./facade\";print(sq(5));",
        )
        .unwrap();

        let artifacts = compile_path_all(&main).unwrap();
        assert_eq!(artifacts.javascript, "console.log(25)");
        assert!(!artifacts.javascript.contains("unused"));
        assert!(artifacts.c.contains("printf(\"%d\\n\""));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn keeps_private_module_bindings_isolated() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-module-scope-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("left.lil"),
            "int helper(){return 1;}export pure int left(){return helper();}",
        )
        .unwrap();
        std::fs::write(
            directory.join("right.lil"),
            "int helper(){return 2;}export pure int right(){return helper();}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import {left} from \"./left\";import {right} from \"./right\";print(left()+right());",
        )
        .unwrap();

        assert_eq!(compile_path(&main).unwrap(), "console.log(3)");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn emits_reusable_esm_with_mangled_live_exports() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-esm-module-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("library.lil"),
            "export pure int square(int value){return value*value;}pure int hidden(){return 99;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import {square as internalSquare} from \"./library\";export {internalSquare as square};export int answer=7;",
        )
        .unwrap();

        let executable = compile_path(&main).unwrap();
        assert!(!executable.contains("export{"));

        let module = compile_path_to_js_module(&main).unwrap();
        assert!(module.contains("export{"));
        assert!(module.contains(" as square"));
        assert!(module.contains(" as answer"));
        assert!(!module.contains("hidden"));
        assert!(!module.contains("internalSquare"));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn applies_fine_grained_optimizer_and_mangling_config() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1+2*3);").unwrap();
        let mut config = ProjectConfig::default();
        config.optimization.preset = crate::config::OptimizationPreset::None;
        config.optimization.constant_folding = Some(false);
        config.mangle.identifiers = false;
        let unoptimized = compile_program_to_js_configured(&program, &config).unwrap();
        assert_ne!(unoptimized, "console.log(7)");
        assert!(unoptimized.contains("Math.imul(2,3)"));

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int horizontal;int vertical;}extern void send(Point point);Point point=Point{1,2};send(point);",
        )
        .unwrap();
        config.mangle.properties = false;
        let preserved = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            preserved.contains("horizontal:") && preserved.contains("vertical:"),
            "{preserved}"
        );
        config.mangle.properties = true;
        let mangled = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            mangled.contains("{a:") && mangled.contains(",b:"),
            "{mangled}"
        );
    }

    #[test]
    fn can_mangle_public_esm_export_names() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export pure int descriptiveFunction(int descriptiveValue){return descriptiveValue+1;}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.exports = true;
        let output = compile_program_to_js_module_configured(&program, &config).unwrap();
        assert!(output.contains("export{"));
        assert!(!output.contains("descriptiveFunction"));

        config.mangle.identifiers = false;
        config.mangle.exports = false;
        let readable = compile_program_to_js_module_configured(&program, &config).unwrap();
        assert!(readable.contains("function descriptiveFunction(descriptiveValue)"));
        assert!(readable.contains("export{descriptiveFunction}"));
    }

    #[test]
    fn materializes_aggregate_abi_for_exported_functions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int x;int y;}export pure int sum(Point point){return point.x+point.y;}export pure Point origin(){return Point{0,0};}",
        )
        .unwrap();
        let output =
            compile_program_to_js_module_configured(&program, &ProjectConfig::default()).unwrap();

        assert!(output.contains(".x"));
        assert!(output.contains(".y"));
        assert!(output.contains("{x:0,y:0}"));
        assert!(output.contains(" as sum"));
        assert!(output.contains(" as origin"));
    }

    #[test]
    fn materializes_mutable_reads_before_later_writes() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int state=1;void run(){int previous=state;state=2;print(previous);}run();",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.optimization.preset = crate::config::OptimizationPreset::None;
        config.mangle.identifiers = false;
        let output = compile_program_to_js_configured(&program, &config).unwrap();

        let load = output
            .find("=state;")
            .unwrap_or_else(|| panic!("global read must be stored: {output}"));
        let store = output
            .find("state=2;")
            .unwrap_or_else(|| panic!("global write must remain: {output}"));
        let print = output
            .find("console.log(")
            .unwrap_or_else(|| panic!("saved value must be printed: {output}"));
        assert!(load < store && store < print, "{output}");
        assert!(
            !output[print..].starts_with("console.log(state)"),
            "{output}"
        );

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1];int length=values.length;values.push(2);print(length);",
        )
        .unwrap();
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        let load = output
            .find("=values.length;")
            .expect("array length read must be stored");
        let mutation = output
            .find("values.push(2)")
            .expect("array mutation must remain");
        assert!(load < mutation, "{output}");

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int state=1;void run(){bool wasInitial=state==1;state=2;print(wasInitial);}run();",
        )
        .unwrap();
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        let comparison = output
            .find("state==1")
            .unwrap_or_else(|| panic!("comparison must remain: {output}"));
        let store = output
            .find("state=2")
            .unwrap_or_else(|| panic!("write must remain: {output}"));
        assert!(comparison < store, "{output}");
    }

    #[test]
    fn compiles_nested_capturing_closures_after_inlining() {
        let source = "class Box{int value;init(int value){this.value=value;}void increment(){this.value+=1;}}extern void accept(func()->void callback);void run(){Box box=new Box(0);accept(()=>{accept(()=>box.increment());});}run();";
        let output = compile_source(source).unwrap();

        assert!(output.contains("accept("), "{output}");
    }

    #[test]
    fn materializes_call_results_when_captured_by_closures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int current();extern void retain(func()->int callback);void install(){int snapshot=current();retain(()=>snapshot);}install();",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.optimization.preset = crate::config::OptimizationPreset::None;
        config.mangle.identifiers = false;
        let output = compile_program_to_js_configured(&program, &config).unwrap();

        let snapshot = output
            .find("=current();")
            .unwrap_or_else(|| panic!("captured call result must be stored: {output}"));
        let retain = output
            .find("retain(")
            .unwrap_or_else(|| panic!("closure must be retained: {output}"));
        assert!(snapshot < retain, "{output}");
        assert!(!output.contains("=>current()"), "{output}");
    }

    #[test]
    fn preserves_effectful_calls_before_conditional_returns() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Box{int value;init(int value){this.value=value;}void increment(){this.value+=1;}}extern void retain(func()->int callback);void install(bool flag){Box box=new Box(0);retain(()=>{box.increment();if(flag){return box.value;}return 0;});}install(true);",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.optimization.preset = crate::config::OptimizationPreset::None;
        config.mangle.identifiers = false;
        let output = compile_program_to_js_configured(&program, &config).unwrap();

        assert_eq!(output.matches("Box$increment(").count(), 2, "{output}");
    }

    #[test]
    fn compiles_inferred_generics_to_optimized_javascript() {
        let source = "T identity<T>(T value){return value;}class Box<T>{T value;init(T value){this.value=value;}T get(){return this.value;}}Box<int> box=new Box(7);print(identity(box.get()));";
        let output = compile_source(source).unwrap();

        assert!(output.contains("console.log"), "{output}");
        assert!(!output.contains("identity"), "{output}");
    }

    #[test]
    fn compiles_union_values_and_heterogeneous_arrays() {
        let source = r#"
            bool flip=false;
            bool next(){flip=!flip;return flip;}
            string|int choose(bool text){if(text){return "hello";}return 42;}
            string|int first=choose(next());
            string|int second=choose(next());
            (string|int)[] values=[first,second];
            class Box { string|int value; }
            Box box=new Box();
            print(first);
            print(second);
            print(values[0]=="hello");
            print(values[1]==42);
            print(box.value=="");
        "#;
        let output = compile_source_all(source).unwrap();

        assert!(
            output.javascript.contains("console.log"),
            "{}",
            output.javascript
        );
        assert!(output.c.contains("LilScriptValue"));
        assert!(output.c.contains("lilscript_print_value"));
        assert!(output.c.contains("lilscript_value_eq"));
    }

    #[test]
    fn preserves_branch_local_shadowing_while_linking() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-module-shadow-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("library.lil"),
            "int helper(){return 7;}export int run(bool flag){if(flag)int helper=1;return helper();}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(&main, "import {run} from \"./library\";print(run(true));").unwrap();

        assert_eq!(compile_path(&main).unwrap(), "console.log(7)");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reports_missing_exports_and_module_cycles() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-module-error-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(directory.join("library.lil"), "int hidden(){return 1;}").unwrap();
        std::fs::write(&main, "import {hidden} from \"./library\";print(hidden());").unwrap();
        let missing = compile_path(&main).unwrap_err();
        assert!(missing.message.contains("does not export `hidden`"));
        assert_eq!(missing.path, main.canonicalize().unwrap());

        std::fs::write(
            directory.join("left.lil"),
            "import \"./right\";export int left(){return 1;}",
        )
        .unwrap();
        std::fs::write(
            directory.join("right.lil"),
            "import \"./left\";export int right(){return 2;}",
        )
        .unwrap();
        let cycle = compile_path(&directory.join("left.lil")).unwrap_err();
        assert!(cycle.message.contains("cyclic module import"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_conflicting_extern_contracts_across_modules() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-module-extern-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("left.lil"),
            "extern int hostValue(int value);",
        )
        .unwrap();
        std::fs::write(
            directory.join("right.lil"),
            "extern float hostValue(float value);",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(&main, "import \"./left\";import \"./right\";").unwrap();

        let error = compile_path(&main).unwrap_err();
        assert!(error.message.contains("conflicting declarations"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn attributes_purity_errors_to_the_dependency_module() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-module-purity-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let dependency = directory.join("effect.lil");
        std::fs::write(
            &dependency,
            "export pure int noisy(int value){print(value);return value;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(&main, "import {noisy} from \"./effect\";print(noisy(1));").unwrap();

        let error = compile_path(&main).unwrap_err();
        assert_eq!(error.path, dependency.canonicalize().unwrap());
        assert!(error.span.start > 0);
        assert!(error.message.contains("declared `pure`"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
