use ahash::AHashMap;
use bumpalo::Bump;
use serde::Serialize;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use crate::codegen_ir_js::{
    emit_optimized_ir_js, emit_optimized_ir_js_chunks_with_options, emit_optimized_ir_js_module,
    emit_optimized_ir_js_module_with_options_and_analysis,
    emit_optimized_ir_js_with_options_and_analysis, ir_function_can_move_to_chunk, IrJsChunkPlan,
    IrJsChunkSpec,
};
use crate::codegen_js::{compile_to_js, CompileError};
use crate::codegen_native::{compile_to_c, emit_native_c};
use crate::config::{BundleMode, CompressionCostModel, ProjectConfig};
use crate::ir::{ControlFlowModule, FunctionId};
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
use crate::value_analysis::{analyze_integer_values, IntegerValueAnalysis};

#[derive(Debug, Clone, PartialEq)]
pub struct CompilationArtifacts {
    pub javascript: String,
    pub c: String,
    pub optimization_reports: Vec<OptimizationReport>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JavaScriptCompilation {
    pub javascript: String,
    pub optimization_reports: Vec<OptimizationReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaScriptBundle {
    pub files: Vec<JavaScriptBundleFile>,
    pub manifest: JavaScriptBundleManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaScriptBundleFile {
    pub file_name: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptBundleManifest {
    pub version: u32,
    pub mode: String,
    pub entry: String,
    pub chunks: Vec<JavaScriptBundleManifestChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptBundleManifestChunk {
    pub file: String,
    pub modules: Vec<String>,
    pub bytes: usize,
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

pub fn compile_path_explained_configured(
    path: &Path,
    config: &ProjectConfig,
) -> Result<JavaScriptCompilation, ModuleError> {
    let arena = Bump::new();
    let modules = discover_modules(path)?;
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    let semantics = analyze(&linked)
        .map_err(|error| module_compile_error(&modules, CompileError::Semantic(error)))?;
    let ir = lower_to_control_flow(&linked, &semantics)
        .map_err(|error| module_compile_error(&modules, CompileError::Lower(error)))?;
    let selected = optimize_and_select_javascript(ir, config, false)
        .map_err(|error| module_compile_error(&modules, error))?;
    Ok(JavaScriptCompilation {
        javascript: selected.javascript,
        optimization_reports: selected.optimization_reports,
    })
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

pub fn compile_path_to_js_bundle_configured(
    path: &Path,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<JavaScriptBundle, ModuleError> {
    let modules = discover_modules(path)?;
    if entry_file.is_empty()
        || Path::new(entry_file)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(entry_file)
    {
        return Err(ModuleError::new(
            path,
            modules.modules[modules.root].source.clone(),
            Span::empty(0),
            "bundle entry file must be a file name without directory components",
        ));
    }
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    let semantics = analyze(&linked)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    let mut ir = lower_to_control_flow(&linked, &semantics)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    optimize_control_flow_with_options(&mut ir, &config.js_optimizer_options(), true)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;

    let specs = plan_javascript_chunks(&ir, &modules, config, entry_file)?;
    let plan = IrJsChunkPlan {
        entry_file: entry_file.to_string(),
        chunks: specs
            .iter()
            .map(|spec| IrJsChunkSpec {
                file_name: spec.file_name.clone(),
                functions: spec.functions.clone(),
            })
            .collect(),
    };
    let emitted = emit_optimized_ir_js_chunks_with_options(&ir, &config.js_options(), &plan)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    let files = emitted
        .into_iter()
        .map(|chunk| JavaScriptBundleFile {
            file_name: chunk.file_name,
            code: chunk.code,
        })
        .collect::<Vec<_>>();
    let chunks = specs
        .iter()
        .map(|spec| JavaScriptBundleManifestChunk {
            file: spec.file_name.clone(),
            modules: vec![relative_module_name(&modules, spec.module)],
            bytes: files
                .iter()
                .find(|file| file.file_name == spec.file_name)
                .map_or(0, |file| file.code.len()),
        })
        .collect();
    Ok(JavaScriptBundle {
        files,
        manifest: JavaScriptBundleManifest {
            version: 1,
            mode: bundle_mode_name(config.bundle.mode).to_string(),
            entry: entry_file.to_string(),
            chunks,
        },
    })
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

#[derive(Debug, Clone)]
struct PlannedChunk {
    module: usize,
    file_name: String,
    functions: Vec<FunctionId>,
}

fn plan_javascript_chunks(
    ir: &ControlFlowModule<'_>,
    modules: &ModuleSet,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<Vec<PlannedChunk>, ModuleError> {
    if config.bundle.mode == BundleMode::Single {
        return Ok(Vec::new());
    }
    let mut by_module = AHashMap::<usize, Vec<FunctionId>>::new();
    for function in &ir.functions {
        if !ir_function_can_move_to_chunk(ir, function.id) {
            continue;
        }
        let module = linked_module_for_offset(modules, function.span.start);
        if module != modules.root {
            by_module.entry(module).or_default().push(function.id);
        }
    }
    let mut candidates = by_module
        .into_iter()
        .map(|(module, mut functions)| {
            functions.sort_unstable_by_key(|function| function.0);
            PlannedChunk {
                module,
                file_name: module_chunk_file(modules, module, entry_file),
                functions,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|chunk| chunk.module);
    if config.bundle.mode == BundleMode::PreserveModules {
        return Ok(candidates);
    }

    let mut importer_counts = vec![0usize; modules.modules.len()];
    for module in &modules.modules {
        let mut unique = module.dependencies.clone();
        unique.sort_unstable();
        unique.dedup();
        for dependency in unique {
            importer_counts[dependency] += 1;
        }
    }
    candidates.retain(|chunk| importer_counts[chunk.module] >= config.bundle.shared_min_imports);
    if candidates.is_empty() {
        return Ok(candidates);
    }

    let provisional_plan = IrJsChunkPlan {
        entry_file: entry_file.to_string(),
        chunks: candidates
            .iter()
            .map(|chunk| IrJsChunkSpec {
                file_name: chunk.file_name.clone(),
                functions: chunk.functions.clone(),
            })
            .collect(),
    };
    let provisional =
        emit_optimized_ir_js_chunks_with_options(ir, &config.js_options(), &provisional_plan)
            .map_err(CompileError::from)
            .map_err(|error| module_compile_error(modules, error))?;
    let sizes = provisional
        .into_iter()
        .map(|chunk| (chunk.file_name, chunk.code.len()))
        .collect::<AHashMap<_, _>>();
    candidates.retain(|chunk| {
        sizes.get(&chunk.file_name).copied().unwrap_or(0) >= config.bundle.min_chunk_bytes
    });
    candidates.sort_unstable_by(|left, right| {
        sizes[&right.file_name]
            .cmp(&sizes[&left.file_name])
            .then_with(|| left.module.cmp(&right.module))
    });
    candidates.truncate(config.bundle.max_chunks);
    candidates.sort_unstable_by_key(|chunk| chunk.module);
    Ok(candidates)
}

fn linked_module_for_offset(modules: &ModuleSet, offset: usize) -> usize {
    modules
        .modules
        .iter()
        .enumerate()
        .rev()
        .find(|(_, module)| offset >= module.offset)
        .map_or(modules.root, |(module, _)| module)
}

fn module_chunk_file(modules: &ModuleSet, module: usize, entry_file: &str) -> String {
    let stem = modules.modules[module]
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("module");
    let sanitized = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let extension = Path::new(entry_file)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("js");
    let candidate = format!("chunk-{module}-{sanitized}.{extension}");
    if candidate == entry_file {
        format!("lil-{candidate}")
    } else {
        candidate
    }
}

fn relative_module_name(modules: &ModuleSet, module: usize) -> String {
    let root_directory = modules.modules[modules.root]
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    modules.modules[module]
        .path
        .strip_prefix(root_directory)
        .unwrap_or(&modules.modules[module].path)
        .to_string_lossy()
        .replace('\\', "/")
}

const fn bundle_mode_name(mode: BundleMode) -> &'static str {
    match mode {
        BundleMode::Single => "single",
        BundleMode::Split => "split",
        BundleMode::PreserveModules => "preserve-modules",
    }
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
    let javascript_ir = lower_to_control_flow(program, &semantics)?;
    let mut native_ir = javascript_ir.clone();
    let selected = optimize_and_select_javascript(javascript_ir, config, false)?;
    optimize_control_flow_with_options(&mut native_ir, &config.optimizer_options(), false)?;
    let c = emit_native_c(&native_ir)?;
    Ok(CompilationArtifacts {
        javascript: selected.javascript,
        c,
        optimization_reports: selected.optimization_reports,
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
    let ir = lower_to_control_flow(program, &semantics)?;
    optimize_and_select_javascript(ir, config, false).map(|selected| selected.javascript)
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
    let ir = lower_to_control_flow(program, &semantics)?;
    optimize_and_select_javascript(ir, config, true).map(|selected| selected.javascript)
}

struct OptimizedJavascriptCandidate {
    javascript: String,
    optimization_reports: Vec<OptimizationReport>,
}

fn optimize_and_select_javascript<'src>(
    ir: ControlFlowModule<'src>,
    config: &ProjectConfig,
    preserve_exports: bool,
) -> Result<OptimizedJavascriptCandidate, CompileError> {
    let configured = config.js_optimizer_options();
    let mut optimizer_options = vec![configured];
    if configured.inlining && config.ir_inlining_variants_enabled() {
        let mut no_inlining = configured;
        no_inlining.inlining = false;
        no_inlining.inline_instruction_limit = 0;
        no_inlining.inline_control_flow_limit = 0;
        no_inlining.inline_growth_limit = Some(0);
        optimizer_options.push(no_inlining);
    }

    let mut candidates = Vec::with_capacity(optimizer_options.len());
    for options in optimizer_options {
        let mut candidate_ir = ir.clone();
        let optimization_reports =
            optimize_control_flow_with_options(&mut candidate_ir, &options, preserve_exports)?;
        let javascript = select_javascript_candidate(&candidate_ir, config, preserve_exports)?;
        let cost = compressed_size(javascript.as_bytes(), config.javascript.cost_model)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
        if candidates
            .iter()
            .any(|candidate: &(usize, usize, OptimizedJavascriptCandidate)| {
                candidate.2.javascript == javascript
            })
        {
            continue;
        }
        candidates.push((
            cost,
            javascript.len(),
            OptimizedJavascriptCandidate {
                javascript,
                optimization_reports,
            },
        ));
    }
    candidates.sort_by(|left, right| {
        (left.0, left.1, &left.2.javascript).cmp(&(right.0, right.1, &right.2.javascript))
    });
    candidates
        .into_iter()
        .next()
        .map(|(_, _, candidate)| candidate)
        .ok_or_else(|| {
            crate::codegen_js::CodegenError::new(
                Span::empty(0),
                "optimizer-level candidate search produced no JavaScript output",
            )
            .into()
        })
}

fn select_javascript_candidate(
    ir: &ControlFlowModule<'_>,
    config: &ProjectConfig,
    module_output: bool,
) -> Result<String, CompileError> {
    let configured = config.js_options();
    let integer_analysis = Arc::new(analyze_integer_values(ir));
    if !config.javascript.candidate_search_enabled() {
        return emit_javascript_candidate(
            ir,
            module_output,
            configured,
            Arc::clone(&integer_analysis),
        )
        .map_err(Into::into);
    }
    let mut options = Vec::new();
    let phi_affinity_modes = match configured.phi_affinity_mode {
        crate::codegen_ir_js::PhiAffinityMode::Conservative => [
            crate::codegen_ir_js::PhiAffinityMode::Conservative,
            crate::codegen_ir_js::PhiAffinityMode::Conservative,
            crate::codegen_ir_js::PhiAffinityMode::Conservative,
        ],
        crate::codegen_ir_js::PhiAffinityMode::Direct => [
            crate::codegen_ir_js::PhiAffinityMode::Direct,
            crate::codegen_ir_js::PhiAffinityMode::Conservative,
            crate::codegen_ir_js::PhiAffinityMode::Conservative,
        ],
        crate::codegen_ir_js::PhiAffinityMode::Grouped => [
            crate::codegen_ir_js::PhiAffinityMode::Grouped,
            crate::codegen_ir_js::PhiAffinityMode::Direct,
            crate::codegen_ir_js::PhiAffinityMode::Conservative,
        ],
    };
    for pool_strings in [configured.pool_strings, false] {
        for elide_safe_integer_coercions in [configured.elide_safe_integer_coercions, false] {
            for compact_boolean_literals in [configured.compact_boolean_literals, false] {
                for inline_structured_closures in [configured.inline_structured_closures, false] {
                    for pack_string_arrays in [configured.pack_string_arrays, false] {
                        for scalar_phi_copies in [configured.scalar_phi_copies, false] {
                            for phi_affinity_mode in phi_affinity_modes {
                                let candidate = crate::codegen_ir_js::IrJsOptions {
                                    pool_strings,
                                    elide_safe_integer_coercions,
                                    compact_boolean_literals,
                                    inline_structured_closures,
                                    pack_string_arrays,
                                    scalar_phi_copies,
                                    phi_affinity_mode,
                                    ..configured
                                };
                                if !options.contains(&candidate) {
                                    options.push(candidate);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let mut candidates = Vec::with_capacity(options.len() * 2);
    for options in options {
        let baseline =
            emit_javascript_candidate(ir, module_output, options, Arc::clone(&integer_analysis))?;
        let mut alphabets = vec![options.identifier_alphabet];
        if options.mangle_identifiers && config.entropy_aware_mangling_enabled() {
            let frequency = crate::codegen_ir_js::IdentifierAlphabet::for_code(&baseline);
            if !alphabets.contains(&frequency) {
                alphabets.push(frequency);
            }
        }
        for identifier_alphabet in alphabets {
            let mut quotes = vec![options.string_quote];
            if config.quote_style_selection_enabled()
                && !quotes.contains(&crate::codegen_ir_js::StringQuote::Single)
            {
                quotes.push(crate::codegen_ir_js::StringQuote::Single);
            }
            for string_quote in quotes {
                let candidate_options = crate::codegen_ir_js::IrJsOptions {
                    identifier_alphabet,
                    string_quote,
                    ..options
                };
                let code = if candidate_options == options {
                    baseline.clone()
                } else {
                    emit_javascript_candidate(
                        ir,
                        module_output,
                        candidate_options,
                        Arc::clone(&integer_analysis),
                    )?
                };
                for code in top_level_declaration_variants(code) {
                    if candidates
                        .iter()
                        .any(|(_, _, existing, _): &(usize, usize, String, _)| existing == &code)
                    {
                        continue;
                    }
                    let cost = compressed_size(code.as_bytes(), config.javascript.cost_model)
                        .map_err(|message| {
                            crate::codegen_js::CodegenError::new(Span::empty(0), message)
                        })?;
                    candidates.push((cost, code.len(), code, candidate_options));
                    if candidates.len() == config.javascript.candidate_limit {
                        break;
                    }
                }
                if candidates.len() == config.javascript.candidate_limit {
                    break;
                }
            }
            if candidates.len() == config.javascript.candidate_limit {
                break;
            }
        }
        if candidates.len() == config.javascript.candidate_limit {
            break;
        }
    }
    candidates.sort_by(|left, right| (left.0, left.1, &left.2).cmp(&(right.0, right.1, &right.2)));
    if config.loop_spelling_selection_enabled() {
        const LOOP_SPELLING_BEAM_WIDTH: usize = 8;
        let finalists = candidates
            .iter()
            .take(LOOP_SPELLING_BEAM_WIDTH)
            .map(|candidate| candidate.3)
            .collect::<Vec<_>>();
        for options in finalists {
            for loop_spelling in [
                crate::codegen_ir_js::LoopSpelling::While,
                crate::codegen_ir_js::LoopSpelling::For,
            ] {
                let candidate_options = crate::codegen_ir_js::IrJsOptions {
                    loop_spelling,
                    ..options
                };
                let code = emit_javascript_candidate(
                    ir,
                    module_output,
                    candidate_options,
                    Arc::clone(&integer_analysis),
                )?;
                for code in top_level_declaration_variants(code) {
                    if candidates
                        .iter()
                        .any(|(_, _, existing, _)| existing == &code)
                    {
                        continue;
                    }
                    let cost = compressed_size(code.as_bytes(), config.javascript.cost_model)
                        .map_err(|message| {
                            crate::codegen_js::CodegenError::new(Span::empty(0), message)
                        })?;
                    candidates.push((cost, code.len(), code, candidate_options));
                }
            }
        }
        candidates
            .sort_by(|left, right| (left.0, left.1, &left.2).cmp(&(right.0, right.1, &right.2)));
    }
    candidates
        .into_iter()
        .next()
        .map(|(_, _, code, _)| code)
        .ok_or_else(|| {
            crate::codegen_js::CodegenError::new(
                Span::empty(0),
                "candidate search produced no JavaScript output",
            )
            .into()
        })
}

fn top_level_declaration_variants(code: String) -> Vec<String> {
    let mut variants = vec![code.clone()];
    if let Some(rest) = code.strip_prefix("var ") {
        variants.push(format!("let {rest}"));
    }
    variants
}

fn emit_javascript_candidate(
    ir: &ControlFlowModule<'_>,
    module_output: bool,
    options: crate::codegen_ir_js::IrJsOptions,
    integer_analysis: Arc<IntegerValueAnalysis>,
) -> Result<String, crate::codegen_js::CodegenError> {
    if module_output {
        emit_optimized_ir_js_module_with_options_and_analysis(ir, &options, integer_analysis)
    } else {
        emit_optimized_ir_js_with_options_and_analysis(ir, &options, integer_analysis)
    }
}

fn compressed_size(bytes: &[u8], model: CompressionCostModel) -> Result<usize, String> {
    match model {
        CompressionCostModel::Raw => Ok(bytes.len()),
        CompressionCostModel::Gzip => {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
            encoder
                .write_all(bytes)
                .map_err(|error| format!("gzip candidate measurement failed: {error}"))?;
            encoder
                .finish()
                .map(|output| output.len())
                .map_err(|error| format!("gzip candidate measurement failed: {error}"))
        }
        CompressionCostModel::Brotli => {
            let mut output = Vec::new();
            {
                let mut writer = brotli::CompressorWriter::new(&mut output, 4096, 11, 22);
                writer
                    .write_all(bytes)
                    .map_err(|error| format!("Brotli candidate measurement failed: {error}"))?;
            }
            Ok(output.len())
        }
    }
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
    use crate::config::CompressionDecision;

    #[test]
    fn compiles_source_end_to_end() {
        assert_eq!(compile_source("print(40+2);").unwrap(), "console.log(42)");
    }

    #[test]
    fn compressor_selects_a_shared_helper_over_duplicated_inlining() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int mix(int value){return value^(value<<value);}int[] values=[1,2,3,4,5,6,7,8];print(mix(values[0]));print(mix(values[1]));print(mix(values[2]));print(mix(values[3]));print(mix(values[4]));print(mix(values[5]));print(mix(values[6]));print(mix(values[7]));",
        )
        .unwrap();
        let selected =
            compile_program_to_js_configured(&program, &ProjectConfig::default()).unwrap();
        let mut inline_only = ProjectConfig::default();
        inline_only.javascript.compression = Some(vec![
            CompressionDecision::IdentifierMangling,
            CompressionDecision::EntropyAwareMangling,
            CompressionDecision::QuoteStyleSelection,
            CompressionDecision::StringPooling,
            CompressionDecision::SizeAwareInlining,
            CompressionDecision::SafeIntegerCoercionElision,
            CompressionDecision::CompactBooleanLiterals,
            CompressionDecision::StructuredClosureInlining,
            CompressionDecision::StringArrayPacking,
            CompressionDecision::ScalarPhiCopies,
            CompressionDecision::PhiAffinityCoalescing,
        ]);
        let inlined = compile_program_to_js_configured(&program, &inline_only).unwrap();
        let mut no_inlining = ProjectConfig::default();
        no_inlining.optimization.inlining = Some(false);
        let outlined = compile_program_to_js_configured(&program, &no_inlining).unwrap();

        assert_eq!(selected, outlined);
        assert!(selected.len() < inlined.len(), "{selected}\n{inlined}");
        assert!(selected.contains("function"), "{selected}");
    }

    #[test]
    fn applies_javascript_priority_without_changing_native_policy() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "pure extern int step(int value);extern int read();pure int transform(int value){value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);value=step(value);return value;}print(transform(read()));print(transform(read()));print(transform(read()));print(transform(read()));print(transform(read()));print(transform(read()));",
        )
        .unwrap();
        let mut performance = ProjectConfig::default();
        performance.javascript.priority = crate::config::JavaScriptPriority::PerformanceFirst;
        let mut realistic = ProjectConfig::default();
        realistic.javascript.priority =
            crate::config::JavaScriptPriority::RealisticPerformanceFirst;
        let mut balanced = ProjectConfig::default();
        balanced.javascript.priority = crate::config::JavaScriptPriority::Balanced;
        let mut size = ProjectConfig::default();
        size.javascript.priority = crate::config::JavaScriptPriority::SizeFirst;

        let performance = compile_program_all_configured(&program, &performance).unwrap();
        let realistic = compile_program_all_configured(&program, &realistic).unwrap();
        let balanced = compile_program_all_configured(&program, &balanced).unwrap();
        let size = compile_program_all_configured(&program, &size).unwrap();

        assert_ne!(performance.javascript, size.javascript);
        assert!(
            !performance.javascript.contains("function"),
            "{}",
            performance.javascript
        );
        assert!(
            balanced.javascript.contains("function"),
            "{}",
            balanced.javascript
        );
        assert!(size.javascript.contains("function"), "{}", size.javascript);
        for output in [&performance, &realistic, &balanced, &size] {
            assert!(
                !output.javascript.contains("Math.imul"),
                "ordinary multiplication introduced Math.imul: {}",
                output.javascript
            );
        }
        assert!(
            size.javascript.len() < performance.javascript.len(),
            "size-first={} performance-first={}",
            size.javascript,
            performance.javascript
        );
        assert_eq!(performance.c, realistic.c);
        assert_eq!(performance.c, balanced.c);
        assert_eq!(performance.c, size.c);
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
        assert!(output.contains("for(") || output.contains("while("));
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

        assert!(compile_source(
            "pure int localWork(int value){int[] work=[];work.push(value);work[0]+=1;return work[0];}print(localWork(4));"
        )
        .is_ok());

        let error = compile_source(
            "pure void mutate(int[] values){values.push(1);}int[] values=[];mutate(values);",
        )
        .expect_err("mutating a parameter from a pure function must fail");
        assert!(error.to_string().contains("declared `pure`"));
    }

    #[test]
    fn emits_typed_host_objects_as_direct_stable_javascript() {
        let source = r#"
            extern class Element {
                string textContent;
                void setAttribute(string name,string value);
            }
            extern class Document { Element createElement(string tag); }
            extern Document document;
            Element element=document.createElement("div");
            element.textContent="ready";
            element.setAttribute("data-state","active");
            print(element.textContent);
        "#;
        let output = compile_source(source).unwrap();
        assert!(output.contains("document.createElement(\"div\")"));
        assert!(output.contains(".textContent=\"ready\""));
        assert!(output.contains(".setAttribute(\"data-state\",\"active\")"));
        assert!(!output.contains("let document"));

        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.properties = Some(true);
        let mangled = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(mangled.contains("document.createElement"));
        assert!(mangled.contains(".textContent"));
        assert!(mangled.contains(".setAttribute"));

        let error = compile_source("extern int devicePixelRatio;devicePixelRatio=2;").unwrap_err();
        assert!(error
            .to_string()
            .contains("extern global bindings are read-only"));
    }

    #[test]
    fn preserves_effectful_host_reads_and_eliminates_trusted_pure_calls() {
        let output = compile_source(
            "extern class Host{string title;pure int cached();int current();}\
             extern Host window;window.cached();window.current();window.title;",
        )
        .unwrap();
        assert!(!output.contains(".cached("));
        assert!(output.contains("window.current()"));
        assert!(output.contains("window.title"));
    }

    #[test]
    fn reports_javascript_only_host_objects_for_native_targets() {
        let error = compile_source_to_c(
            "extern class Document{string title;}extern Document document;print(document.title);",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("only available for JavaScript targets"));
    }

    #[test]
    fn preserves_host_method_receivers_and_supports_callable_fields() {
        let output = compile_source(
            "extern class Host{func(string)->void callback;void method();}\
             extern Host window;window.method();window.callback(\"ready\");",
        )
        .unwrap();
        assert!(output.contains("window.method()"), "{output}");
        assert!(output.contains("window.callback(\"ready\")"), "{output}");

        let error = compile_source(
            "extern class Host{void method();}extern Host window;\
             func()->void detached=window.method;",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("must be called through their receiver"));
    }

    #[test]
    fn treats_host_operations_as_aliasing_barriers() {
        let output = compile_source(
            "class Box{int value;}extern class Host{void observe(Box box);}\
             extern Host window;Box box=new Box();box.value=1;\
             window.observe(box);box.value=2;",
        )
        .unwrap();
        let first = output.find(".value=1").expect("first write must survive");
        let observe = output
            .find("window.observe(")
            .expect("host observation must survive");
        let second = output.find(".value=2").expect("second write must survive");
        assert!(first < observe && observe < second, "{output}");
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
    fn remaps_nominal_types_nested_in_imported_unions() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-module-union-type-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("dynamic.lil"),
            r#"
                export class Node {
                    string text;
                    init(string text) { this.text = text; }
                }
                export Node render<P>(string|func(P)->Node selected, P props) {
                    if (selected is string) { return new Node(selected); }
                    else { return selected(props); }
                }
            "#,
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                import {Node, render} from "./dynamic";
                class Props {
                    string text;
                    init(string text) { this.text = text; }
                }
                Node component(Props props) { return new Node(props.text); }
                Props props = new Props("component");
                string|func(Props)->Node selected = component;
                Node result = render(selected, props);
                print(result.text);
            "#,
        )
        .unwrap();

        let artifacts = compile_path_all(&main).unwrap();
        assert!(artifacts.javascript.contains("component"));
        assert!(artifacts.c.contains("component"));
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
        config.mangle.identifiers = Some(false);
        let unoptimized = compile_program_to_js_configured(&program, &config).unwrap();
        assert_ne!(unoptimized, "console.log(7)");
        assert!(unoptimized.contains("2*3"));

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int horizontal;int vertical;}extern void send(Point point);Point point=Point{1,2};send(point);",
        )
        .unwrap();
        config.mangle.properties = Some(false);
        let preserved = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            preserved.contains("horizontal:") && preserved.contains("vertical:"),
            "{preserved}"
        );
        config.mangle.properties = Some(true);
        let mangled = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            mangled.contains("{a:") && mangled.contains(",b:"),
            "{mangled}"
        );
    }

    #[test]
    fn preserves_surviving_dependency_functions_as_esm_chunks() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-preserve-bundle-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("library.lil"),
            "int state=40;export void set(int value){state=value;}export int read(){return state;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import {set,read} from \"./library\";set(41);print(read()+1);",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.bundle.mode = BundleMode::PreserveModules;
        config.optimization.inlining = Some(false);
        config.mangle.identifiers = Some(false);

        let bundle = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap();
        for _ in 0..8 {
            assert_eq!(
                compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap(),
                bundle
            );
        }
        assert_eq!(bundle.files.len(), 2);
        assert_eq!(bundle.manifest.chunks.len(), 1);
        assert_eq!(bundle.manifest.chunks[0].modules, ["library.lil"]);
        let entry = &bundle.files[0].code;
        let chunk = &bundle.files[1].code;
        assert!(entry.contains("from\"./chunk-1-library.js\""), "{entry}");
        assert!(entry.contains("function $m1$set"), "{entry}");
        assert!(chunk.contains("function $m1$read"), "{chunk}");
        assert!(chunk.contains("from\"./entry.js\""), "{chunk}");

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn splits_only_shared_modules_that_meet_size_policy() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-split-bundle-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("shared.lil"),
            "export int shared(int value){if(value<=0){return 1;}return shared(value-1)+1;}",
        )
        .unwrap();
        std::fs::write(
            directory.join("left.lil"),
            "import {shared} from \"./shared\";export int left(){return shared(2);}",
        )
        .unwrap();
        std::fs::write(
            directory.join("right.lil"),
            "import {shared} from \"./shared\";export int right(){return shared(3);}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import {left} from \"./left\";import {right} from \"./right\";print(left()+right());",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.bundle.mode = BundleMode::Split;
        config.bundle.min_chunk_bytes = 1;
        config.bundle.max_chunks = 1;
        config.bundle.shared_min_imports = 2;
        config.optimization.inlining = Some(false);

        let bundle = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap();
        assert_eq!(bundle.files.len(), 2);
        assert_eq!(bundle.manifest.chunks.len(), 1);
        assert_eq!(bundle.manifest.chunks[0].modules, ["shared.lil"]);
        assert!(bundle.manifest.chunks[0].bytes > 0);

        config.bundle.min_chunk_bytes = usize::MAX;
        let unsplit = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap();
        assert_eq!(unsplit.files.len(), 1);
        assert!(unsplit.manifest.chunks.is_empty());

        std::fs::remove_dir_all(directory).unwrap();
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
        config.mangle.exports = Some(true);
        let output = compile_program_to_js_module_configured(&program, &config).unwrap();
        assert!(output.contains("export{"));
        assert!(!output.contains("descriptiveFunction"));

        config.mangle.identifiers = Some(false);
        config.mangle.exports = Some(false);
        let readable = compile_program_to_js_module_configured(&program, &config).unwrap();
        assert!(readable.contains("function descriptiveFunction(descriptiveValue)"));
        assert!(readable.contains("export{descriptiveFunction}"));
    }

    #[test]
    fn aliases_exported_host_globals_at_an_esm_boundary() {
        let output = compile_source_to_js_module(
            "export extern class Document{string title;}export extern Document document;",
        )
        .unwrap();
        assert!(output.contains("=document;export{"), "{output}");
        assert!(output.contains(" as document}"), "{output}");
        assert!(!output.contains("let document"), "{output}");
    }

    #[test]
    fn static_chunks_reference_host_globals_directly() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-host-chunk-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("web.lil"),
            "export extern class Document{string title;}export extern Document document;\
             export string readTitle(){return document.title;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import {readTitle} from \"./web\";print(readTitle());",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.bundle.mode = BundleMode::PreserveModules;
        config.optimization.inlining = Some(false);
        config.mangle.identifiers = Some(false);

        let bundle = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap();
        assert_eq!(bundle.files.len(), 2);
        let chunk = &bundle.files[1].code;
        assert!(chunk.contains("document.title"), "{chunk}");
        assert!(!chunk.contains("from\"./entry.js\""), "{chunk}");
        std::fs::remove_dir_all(directory).unwrap();
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
        config.mangle.identifiers = Some(false);
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
        config.mangle.identifiers = Some(false);
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
        config.mangle.identifiers = Some(false);
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
    fn compiles_union_type_guards_and_narrowed_calls() {
        let source = r#"
            bool flip=false;
            string|int next(){flip=!flip;if(flip){return "hello";}return 42;}
            string describe(string|int value){
                if(value is string){return value.toUpperCase();}
                else{return "number-"+value;}
            }
            int increment(int value){return value+1;}
            (func(int)->int)|string nextHandler(){
                flip=!flip;
                if(flip){return increment;}
                return "ready";
            }
            string invoke((func(int)->int)|string value){
                if(value is func(int)->int){return "result-"+value(4);}
                else{return value;}
            }
            print(describe(next()));
            print(describe(next()));
            print(invoke(nextHandler()));
        "#;
        let output = compile_source_all(source).unwrap();

        assert!(
            output.javascript.contains("typeof"),
            "{}",
            output.javascript
        );
        assert!(output.c.contains(".tag==4"), "{}", output.c);
        assert!(output.c.contains(".tag==5"), "{}", output.c);
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
    fn links_typed_host_interfaces_from_a_module_without_wrappers() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-module-host-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("web.lil"),
            "export extern class Document{string title;}export extern Document document;",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import {Document,document} from \"./web\";print(document.title);",
        )
        .unwrap();

        let output = compile_path(&main).unwrap();
        assert_eq!(output, "console.log(document.title)");
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
