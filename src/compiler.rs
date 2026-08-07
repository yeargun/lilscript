use ahash::AHashMap;
use bumpalo::Bump;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::codegen_ir_js::{
    emit_optimized_ir_js, emit_optimized_ir_js_chunks_with_options, emit_optimized_ir_js_module,
    emit_optimized_ir_js_module_with_options_and_analysis,
    emit_optimized_ir_js_with_options_and_analysis, ir_function_can_move_to_chunk, IrJsChunk,
    IrJsChunkPlan, IrJsChunkSpec,
};
use crate::codegen_js::{compile_to_js, CompileError};
use crate::codegen_native::{compile_to_c, emit_native_c, emit_native_c_with_options};
use crate::config::{
    BundleMode, CompressionCostModel, JavaScriptOptimization, PreloadPolicy, ProjectConfig,
};
use crate::ir::{ControlFlowModule, FunctionId};
use crate::js_peephole::{
    analyze_generated_javascript, optimize_generated_javascript, JavaScriptSyntaxMetrics,
};
use crate::lower::lower_to_control_flow;
use crate::module::{
    discover_modules, discover_modules_configured, discover_modules_configured_with_source,
    discover_modules_with_source, link_modules, locate_linked_span, parse_modules, ModuleError,
    ModuleSet,
};
use crate::optimizer::{
    optimize_control_flow, optimize_control_flow_for_module, optimize_control_flow_with_guidance,
    OptimizationGuidance, OptimizationReport,
};
use crate::parser::{parse_source, ParseError};
use crate::profile::{
    analyze_javascript_performance, function_profile_key, loop_profile_key,
    JavaScriptPerformanceMetrics, OptimizationProfile,
};
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
    pub selection_metrics: JavaScriptSelectionMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptSelectionMetrics {
    pub codec: String,
    pub transfer_bytes: usize,
    pub startup_score: u64,
    pub syntax: JavaScriptSyntaxMetrics,
    pub baseline_syntax: JavaScriptSyntaxMetrics,
    pub performance: JavaScriptPerformanceMetrics,
    pub baseline_performance: JavaScriptPerformanceMetrics,
    pub candidates_evaluated: usize,
    pub peephole_rewrites: usize,
    pub compiler_time_micros: u128,
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
    pub build_id: String,
    pub mode: String,
    pub entry: String,
    pub preload: Vec<String>,
    pub deploy_cost: u64,
    pub chunks: Vec<JavaScriptBundleManifestChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptBundleManifestChunk {
    pub file: String,
    pub modules: Vec<String>,
    pub bytes: usize,
    pub gzip_bytes: usize,
    pub brotli_bytes: usize,
    pub kind: String,
    pub dependencies: Vec<String>,
    pub dynamic_dependencies: Vec<String>,
    pub cache_key: String,
    pub deploy_cost: u64,
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
    let modules = discover_modules_configured(path, config)?;
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
        selection_metrics: selected.selection_metrics,
    })
}

pub fn profile_template_path_configured(
    path: &Path,
    config: &ProjectConfig,
) -> Result<OptimizationProfile, ModuleError> {
    let arena = Bump::new();
    let modules = discover_modules_configured(path, config)?;
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    let semantics = analyze(&linked)
        .map_err(|error| module_compile_error(&modules, CompileError::Semantic(error)))?;
    let ir = lower_to_control_flow(&linked, &semantics)
        .map_err(|error| module_compile_error(&modules, CompileError::Lower(error)))?;
    let mut profile = OptimizationProfile::default();
    for function in &ir.functions {
        profile.functions.insert(function_profile_key(function), 1);
        for (shape_index, shape) in function.shapes.iter().enumerate() {
            if matches!(shape, crate::ir::ControlShape::Loop { .. }) {
                profile
                    .loops
                    .insert(loop_profile_key(function, shape_index), 1);
            }
        }
    }
    Ok(profile)
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
    let modules = discover_modules_configured(path, config)?;
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
    let modules = discover_modules_configured(path, config)?;
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
    let guidance = load_optimization_guidance(
        config,
        config
            .javascript_optimization_configured(JavaScriptOptimization::ProfileGuidedOptimization),
    )
    .map_err(|error| module_compile_error(&modules, error))?;
    optimize_control_flow_with_guidance(&mut ir, &config.js_optimizer_options(), true, &guidance)
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
                lazy_module: spec.lazy_module,
            })
            .collect(),
    };
    let mut emitted = emit_optimized_ir_js_chunks_with_options(&ir, &config.js_options(), &plan)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    let preload = match config.bundle.preload {
        PreloadPolicy::None => Vec::new(),
        PreloadPolicy::Entry => emitted
            .iter()
            .find(|chunk| chunk.file_name == entry_file)
            .map_or_else(Vec::new, |chunk| chunk.dynamic_dependencies.clone()),
        PreloadPolicy::All => specs
            .iter()
            .filter(|spec| spec.lazy_module.is_some())
            .map(|spec| spec.file_name.clone())
            .collect(),
    };
    apply_module_preloads(&mut emitted, entry_file, &preload);
    let depths = chunk_dependency_depths(&emitted, entry_file);
    let reachability = chunk_reachability(&emitted);
    let mut deploy_cost = 0u64;
    for chunk in &emitted {
        let (gzip_bytes, brotli_bytes) =
            compressed_artifact_sizes(&chunk.code).map_err(|message| {
                ModuleError::new(
                    &modules.modules[modules.root].path,
                    &modules.modules[modules.root].source,
                    Span::empty(0),
                    message,
                )
            })?;
        deploy_cost = deploy_cost.saturating_add(artifact_deploy_cost(
            chunk.code.len(),
            gzip_bytes,
            brotli_bytes,
            depths.get(&chunk.file_name).copied().unwrap_or(0),
            preload.contains(&chunk.file_name),
            reachability
                .get(&chunk.file_name)
                .copied()
                .unwrap_or(0)
                .max(
                    specs
                        .iter()
                        .find(|spec| spec.file_name == chunk.file_name)
                        .map_or(0, |spec| spec.reachability),
                ),
            config,
        ));
    }
    let chunks = specs
        .iter()
        .map(|spec| {
            let emitted = emitted
                .iter()
                .find(|chunk| chunk.file_name == spec.file_name)
                .expect("every planned chunk is emitted");
            let (gzip_bytes, brotli_bytes) =
                compressed_artifact_sizes(&emitted.code).map_err(|message| {
                    ModuleError::new(
                        &modules.modules[modules.root].path,
                        &modules.modules[modules.root].source,
                        Span::empty(0),
                        message,
                    )
                })?;
            Ok(JavaScriptBundleManifestChunk {
                file: spec.file_name.clone(),
                modules: vec![relative_module_name(&modules, spec.module)],
                bytes: emitted.code.len(),
                gzip_bytes,
                brotli_bytes,
                kind: if spec.lazy_module.is_some() {
                    "lazy".to_string()
                } else {
                    "static".to_string()
                },
                dependencies: emitted.dependencies.clone(),
                dynamic_dependencies: emitted.dynamic_dependencies.clone(),
                cache_key: content_hash(emitted.code.as_bytes()),
                deploy_cost: artifact_deploy_cost(
                    emitted.code.len(),
                    gzip_bytes,
                    brotli_bytes,
                    depths.get(&emitted.file_name).copied().unwrap_or(0),
                    preload.contains(&emitted.file_name),
                    reachability
                        .get(&emitted.file_name)
                        .copied()
                        .unwrap_or(0)
                        .max(spec.reachability),
                    config,
                ),
            })
        })
        .collect::<Result<Vec<_>, ModuleError>>()?;
    let build_id = bundle_build_id(&emitted);
    let files = emitted
        .into_iter()
        .map(|chunk| JavaScriptBundleFile {
            file_name: chunk.file_name,
            code: chunk.code,
        })
        .collect::<Vec<_>>();
    Ok(JavaScriptBundle {
        files,
        manifest: JavaScriptBundleManifest {
            version: 2,
            build_id,
            mode: bundle_mode_name(config.bundle.mode).to_string(),
            entry: entry_file.to_string(),
            preload,
            deploy_cost,
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

pub fn compile_path_with_source_configured(
    path: &Path,
    source: &str,
    config: &ProjectConfig,
) -> Result<String, ModuleError> {
    compile_path_js_configured_inner(path, Some(source), config)
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
        Some(source) => discover_modules_configured_with_source(path, source, config)?,
        None => discover_modules_configured(path, config)?,
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
        Some(source) => discover_modules_configured_with_source(path, source, config)?,
        None => discover_modules_configured(path, config)?,
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
        Some(source) => discover_modules_configured_with_source(path, source, config)?,
        None => discover_modules_configured(path, config)?,
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
    lazy_module: Option<u32>,
    reachability: usize,
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
                lazy_module: None,
                reachability: 0,
            }
        })
        .collect::<Vec<_>>();
    let lazy_modules = ir
        .lazy_modules
        .iter()
        .filter_map(|module| {
            let module_id = usize::try_from(module.id).ok()?;
            (!modules.eager.get(module_id).copied().unwrap_or(true))
                .then_some((module_id, module.id))
        })
        .collect::<AHashMap<_, _>>();
    for (&module, &lazy_module) in &lazy_modules {
        if let Some(chunk) = candidates.iter_mut().find(|chunk| chunk.module == module) {
            chunk.lazy_module = Some(lazy_module);
        } else {
            candidates.push(PlannedChunk {
                module,
                file_name: module_chunk_file(modules, module, entry_file),
                functions: Vec::new(),
                lazy_module: Some(lazy_module),
                reachability: 1,
            });
        }
    }
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
    for chunk in &mut candidates {
        chunk.reachability = chunk.reachability.max(importer_counts[chunk.module]);
    }
    candidates.retain(|chunk| {
        !modules.eager[chunk.module]
            || importer_counts[chunk.module] >= config.bundle.shared_min_imports
    });
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
                lazy_module: chunk.lazy_module,
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
        !modules.eager[chunk.module]
            || sizes.get(&chunk.file_name).copied().unwrap_or(0) >= config.bundle.min_chunk_bytes
    });
    candidates.sort_unstable_by(|left, right| {
        sizes[&right.file_name]
            .cmp(&sizes[&left.file_name])
            .then_with(|| left.module.cmp(&right.module))
    });
    let mut selected = candidates
        .iter()
        .filter(|chunk| !modules.eager[chunk.module])
        .cloned()
        .collect::<Vec<_>>();
    let mut optional = candidates
        .into_iter()
        .filter(|chunk| modules.eager[chunk.module])
        .collect::<Vec<_>>();
    optional.truncate(config.bundle.max_chunks.saturating_mul(8).max(32));
    selected.sort_unstable_by_key(|chunk| chunk.module);
    let mut selected_cost = score_javascript_chunk_plan(ir, config, entry_file, &selected)
        .map_err(|error| module_compile_error(modules, error))?;
    while selected.len() < config.bundle.max_chunks && !optional.is_empty() {
        let mut best = None::<(usize, u64)>;
        for (index, candidate) in optional.iter().enumerate() {
            let mut trial = selected.clone();
            trial.push(candidate.clone());
            trial.sort_unstable_by_key(|chunk| chunk.module);
            let cost = score_javascript_chunk_plan(ir, config, entry_file, &trial)
                .map_err(|error| module_compile_error(modules, error))?;
            if best.is_none_or(|(best_index, best_cost)| {
                (cost, candidate.module) < (best_cost, optional[best_index].module)
            }) {
                best = Some((index, cost));
            }
        }
        let has_shared_selection = selected.iter().any(|chunk| modules.eager[chunk.module]);
        let Some((index, cost)) =
            best.filter(|(_, cost)| !has_shared_selection || *cost < selected_cost)
        else {
            break;
        };
        selected.push(optional.remove(index));
        selected.sort_unstable_by_key(|chunk| chunk.module);
        selected_cost = cost;
    }
    Ok(selected)
}

fn score_javascript_chunk_plan(
    ir: &ControlFlowModule<'_>,
    config: &ProjectConfig,
    entry_file: &str,
    chunks: &[PlannedChunk],
) -> Result<u64, CompileError> {
    let plan = IrJsChunkPlan {
        entry_file: entry_file.to_string(),
        chunks: chunks
            .iter()
            .map(|chunk| IrJsChunkSpec {
                file_name: chunk.file_name.clone(),
                functions: chunk.functions.clone(),
                lazy_module: chunk.lazy_module,
            })
            .collect(),
    };
    let mut emitted = emit_optimized_ir_js_chunks_with_options(ir, &config.js_options(), &plan)?;
    let depths = chunk_dependency_depths(&emitted, entry_file);
    let reachability = chunk_reachability(&emitted);
    let preload = match config.bundle.preload {
        PreloadPolicy::None => Vec::new(),
        PreloadPolicy::Entry => emitted
            .iter()
            .find(|chunk| chunk.file_name == entry_file)
            .map_or_else(Vec::new, |chunk| chunk.dynamic_dependencies.clone()),
        PreloadPolicy::All => chunks
            .iter()
            .filter(|chunk| chunk.lazy_module.is_some())
            .map(|chunk| chunk.file_name.clone())
            .collect(),
    };
    apply_module_preloads(&mut emitted, entry_file, &preload);
    let mut score = 0u64;
    for chunk in &emitted {
        let (gzip, brotli) = compressed_artifact_sizes(&chunk.code)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
        score = score.saturating_add(artifact_deploy_cost(
            chunk.code.len(),
            gzip,
            brotli,
            depths.get(&chunk.file_name).copied().unwrap_or(0),
            preload.contains(&chunk.file_name),
            reachability
                .get(&chunk.file_name)
                .copied()
                .unwrap_or(0)
                .max(
                    chunks
                        .iter()
                        .find(|candidate| candidate.file_name == chunk.file_name)
                        .map_or(0, |candidate| candidate.reachability),
                ),
            config,
        ));
    }
    Ok(score)
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
    let identity = relative_module_name(modules, module);
    let digest = Sha256::digest(identity.as_bytes());
    let stable_id = digest[..5]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let candidate = format!("chunk-{stable_id}-{sanitized}.{extension}");
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

fn compressed_artifact_sizes(code: &str) -> Result<(usize, usize), String> {
    Ok((
        compressed_size(code.as_bytes(), CompressionCostModel::Gzip)?,
        compressed_size(code.as_bytes(), CompressionCostModel::Brotli)?,
    ))
}

fn apply_module_preloads(chunks: &mut [IrJsChunk], entry: &str, preload: &[String]) {
    if preload.is_empty() {
        return;
    }
    let Some(entry) = chunks.iter_mut().find(|chunk| chunk.file_name == entry) else {
        return;
    };
    let files = preload
        .iter()
        .map(|file| format!("./{file}"))
        .collect::<Vec<_>>();
    let files = serde_json::to_string(&files).expect("preload file names are serializable");
    entry.code.insert_str(
        0,
        &format!(
            "typeof document!=\"undefined\"&&{files}.forEach(a=>{{let b=document.createElement(\"link\");b.rel=\"modulepreload\",b.href=a,document.head.append(b)}});"
        ),
    );
}

fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    encoded
}

fn bundle_build_id(chunks: &[IrJsChunk]) -> String {
    let mut hasher = Sha256::new();
    for chunk in chunks {
        hasher.update((chunk.file_name.len() as u64).to_le_bytes());
        hasher.update(chunk.file_name.as_bytes());
        hasher.update((chunk.code.len() as u64).to_le_bytes());
        hasher.update(chunk.code.as_bytes());
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    encoded
}

fn chunk_dependency_depths(chunks: &[IrJsChunk], entry: &str) -> AHashMap<String, usize> {
    let mut depths = AHashMap::new();
    depths.insert(entry.to_string(), 0usize);
    let mut changed = true;
    while changed {
        changed = false;
        for chunk in chunks {
            let Some(depth) = depths.get(&chunk.file_name).copied() else {
                continue;
            };
            for dependency in chunk.dependencies.iter().chain(&chunk.dynamic_dependencies) {
                let candidate = depth.saturating_add(1);
                let entry = depths.entry(dependency.clone()).or_insert(candidate);
                if candidate < *entry {
                    *entry = candidate;
                    changed = true;
                }
            }
        }
    }
    depths
}

fn chunk_reachability(chunks: &[IrJsChunk]) -> AHashMap<String, usize> {
    let mut reachability = AHashMap::new();
    for chunk in chunks {
        let mut dependencies = chunk
            .dependencies
            .iter()
            .chain(&chunk.dynamic_dependencies)
            .collect::<Vec<_>>();
        dependencies.sort_unstable();
        dependencies.dedup();
        for dependency in dependencies {
            *reachability.entry(dependency.clone()).or_insert(0) += 1;
        }
    }
    reachability
}

fn artifact_deploy_cost(
    raw: usize,
    gzip: usize,
    brotli: usize,
    depth: usize,
    preloaded: bool,
    reachability: usize,
    config: &ProjectConfig,
) -> u64 {
    let cost = &config.bundle.cost;
    let byte_cost = (raw as u64)
        .saturating_mul(u64::from(cost.raw_weight))
        .saturating_add((gzip as u64).saturating_mul(u64::from(cost.gzip_weight)))
        .saturating_add((brotli as u64).saturating_mul(u64::from(cost.brotli_weight)));
    let request = if depth == 0 {
        0
    } else {
        let request = cost.request_overhead_bytes as u64;
        if preloaded {
            request.saturating_mul(u64::from(
                100u32.saturating_sub(cost.preload_request_discount_percent),
            )) / 100
        } else {
            request
        }
    };
    let depth_cost =
        (cost.dependency_depth_penalty_bytes as u64).saturating_mul(depth.saturating_sub(1) as u64);
    let cache_reuse = reachability.saturating_sub(1).min(4) as u64;
    let cache_discount = byte_cost
        .saturating_mul(u64::from(cost.cache_reuse_discount_percent))
        .saturating_mul(cache_reuse)
        / 100;
    byte_cost
        .saturating_add(request)
        .saturating_add(depth_cost)
        .saturating_sub(cache_discount)
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
    let native_guidance =
        load_optimization_guidance(config, config.native_profile_guided_optimization())?;
    optimize_control_flow_with_guidance(
        &mut native_ir,
        &config.optimizer_options(),
        false,
        &native_guidance,
    )?;
    let c = emit_native_c_with_options(&native_ir, &config.native_options())?;
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
    selection_metrics: JavaScriptSelectionMetrics,
}

fn optimize_and_select_javascript<'src>(
    ir: ControlFlowModule<'src>,
    config: &ProjectConfig,
    preserve_exports: bool,
) -> Result<OptimizedJavascriptCandidate, CompileError> {
    let started = Instant::now();
    let profile = if config.js_profile_guided_optimization() {
        config
            .load_optimization_profile()
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?
    } else {
        OptimizationProfile::default()
    };
    let configured = config.js_optimizer_options();
    let guidance = OptimizationGuidance {
        profile: profile.clone(),
        specialization_min_count: config.profile.specialization_min_count,
        max_specializations_per_function: config.profile.max_specializations_per_function,
        max_clone_instructions: config.profile.max_clone_instructions,
    };
    let mut optimizer_options = vec![configured];
    if configured.inlining
        && configured.inline_closure_factories
        && config.ir_closure_factory_variants_enabled()
    {
        let mut outlined_factories = configured;
        outlined_factories.inline_closure_factories = false;
        optimizer_options.push(outlined_factories);
    }
    if configured.inlining && config.ir_inlining_variants_enabled() {
        let mut no_inlining = configured;
        no_inlining.inlining = false;
        no_inlining.inline_instruction_limit = 0;
        no_inlining.inline_control_flow_limit = 0;
        no_inlining.inline_growth_limit = Some(0);
        if !optimizer_options.contains(&no_inlining) {
            optimizer_options.push(no_inlining);
        }
    }
    if configured.specialize_tagged_constants
        && config.javascript_optimization_enabled(JavaScriptOptimization::IrSpecializationVariants)
    {
        let mut unspecialized = configured;
        unspecialized.specialize_tagged_constants = false;
        if !optimizer_options.contains(&unspecialized) {
            optimizer_options.push(unspecialized);
        }
    }
    if configured.call_site_specialization
        && config.javascript_optimization_enabled(JavaScriptOptimization::CallSiteSpecialization)
    {
        let mut without_call_specialization = configured;
        without_call_specialization.call_site_specialization = false;
        optimizer_options.push(without_call_specialization);
    }
    if configured.capture_signature_cloning
        && config.javascript_optimization_enabled(JavaScriptOptimization::CaptureSignatureCloning)
    {
        let mut without_capture_cloning = configured;
        without_capture_cloning.capture_signature_cloning = false;
        optimizer_options.push(without_capture_cloning);
    }
    optimizer_options.sort_by_key(|options| {
        (
            !options.inlining,
            !options.inline_closure_factories,
            !options.specialize_tagged_constants,
            !options.call_site_specialization,
            !options.capture_signature_cloning,
        )
    });
    optimizer_options.dedup();

    let mut candidates = Vec::with_capacity(optimizer_options.len());
    let mut candidates_evaluated = 0;
    for options in optimizer_options {
        let mut candidate_ir = ir.clone();
        let optimization_reports = optimize_control_flow_with_guidance(
            &mut candidate_ir,
            &options,
            preserve_exports,
            &guidance,
        )?;
        let selected =
            select_javascript_candidate(&candidate_ir, config, preserve_exports, &profile)?;
        candidates_evaluated += selected.candidates_evaluated;
        if candidates
            .iter()
            .any(|candidate: &(usize, usize, OptimizedJavascriptCandidate)| {
                candidate.2.javascript == selected.code
            })
        {
            continue;
        }
        candidates.push((
            selected.transfer_cost,
            selected.code.len(),
            OptimizedJavascriptCandidate {
                javascript: selected.code,
                optimization_reports,
                selection_metrics: JavaScriptSelectionMetrics {
                    codec: compression_cost_model_name(config.javascript.cost_model).to_string(),
                    transfer_bytes: selected.transfer_cost,
                    startup_score: selected.startup_score,
                    syntax: selected.metrics,
                    baseline_syntax: selected.baseline_metrics,
                    performance: selected.performance,
                    baseline_performance: selected.baseline_performance,
                    candidates_evaluated: selected.candidates_evaluated,
                    peephole_rewrites: selected.peephole_rewrites,
                    compiler_time_micros: 0,
                },
            },
        ));
    }
    if config.javascript_optimization_configured(JavaScriptOptimization::StartupCostGuard) {
        if let Some(baseline) = candidates
            .first()
            .map(|candidate| candidate.2.selection_metrics.syntax)
        {
            candidates.retain(|candidate| {
                startup_cost_allowed(
                    candidate.2.selection_metrics.syntax,
                    baseline,
                    &config.javascript.startup,
                )
            });
        }
    }
    let baseline_transfer = candidates.first().map_or(1, |candidate| candidate.0);
    let baseline_performance = candidates.first().map_or(0, |candidate| {
        candidate.2.selection_metrics.performance.score
    });
    candidates.sort_by(|left, right| {
        let left_rank = javascript_candidate_rank(
            config,
            left.0,
            baseline_transfer,
            left.2.selection_metrics.performance.score,
            baseline_performance,
        );
        let right_rank = javascript_candidate_rank(
            config,
            right.0,
            baseline_transfer,
            right.2.selection_metrics.performance.score,
            baseline_performance,
        );
        (
            left_rank,
            left.2.selection_metrics.startup_score,
            left.1,
            &left.2.javascript,
        )
            .cmp(&(
                right_rank,
                right.2.selection_metrics.startup_score,
                right.1,
                &right.2.javascript,
            ))
    });
    let mut selected = candidates
        .into_iter()
        .next()
        .map(|(_, _, candidate)| candidate)
        .ok_or_else(|| {
            crate::codegen_js::CodegenError::new(
                Span::empty(0),
                "optimizer-level candidate search produced no JavaScript output",
            )
        })?;
    selected.selection_metrics.compiler_time_micros = started.elapsed().as_micros();
    selected.selection_metrics.candidates_evaluated = candidates_evaluated;
    Ok(selected)
}

fn load_optimization_guidance(
    config: &ProjectConfig,
    profile_guided: bool,
) -> Result<OptimizationGuidance, CompileError> {
    let profile = if profile_guided {
        config
            .load_optimization_profile()
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?
    } else {
        OptimizationProfile::default()
    };
    Ok(OptimizationGuidance {
        profile,
        specialization_min_count: config.profile.specialization_min_count,
        max_specializations_per_function: config.profile.max_specializations_per_function,
        max_clone_instructions: config.profile.max_clone_instructions,
    })
}

#[derive(Debug, Clone)]
struct SelectedJavaScriptCandidate {
    code: String,
    transfer_cost: usize,
    startup_score: u64,
    metrics: JavaScriptSyntaxMetrics,
    baseline_metrics: JavaScriptSyntaxMetrics,
    performance: JavaScriptPerformanceMetrics,
    baseline_performance: JavaScriptPerformanceMetrics,
    candidates_evaluated: usize,
    peephole_rewrites: usize,
}

fn select_javascript_candidate(
    ir: &ControlFlowModule<'_>,
    config: &ProjectConfig,
    module_output: bool,
    profile: &OptimizationProfile,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    let configured = config.js_options();
    let integer_analysis = Arc::new(analyze_integer_values(ir));
    let configured_baseline =
        emit_javascript_candidate(ir, module_output, configured, Arc::clone(&integer_analysis))?;
    if !config.javascript.candidate_search_enabled() {
        return finalize_javascript_candidates(
            vec![(
                configured_baseline.len(),
                configured_baseline.len(),
                configured_baseline.clone(),
                configured,
            )],
            &configured_baseline,
            config,
            ir,
            profile,
        );
    }
    let mut options = Vec::new();
    let ssa_variants =
        config.javascript_optimization_enabled(JavaScriptOptimization::SsaDestructionVariants);
    let phi_affinity_modes = if !ssa_variants {
        [configured.phi_affinity_mode; 3]
    } else {
        match configured.phi_affinity_mode {
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
        }
    };
    for pool_strings in [configured.pool_strings, false] {
        for elide_safe_integer_coercions in [configured.elide_safe_integer_coercions, false] {
            for compact_boolean_literals in [configured.compact_boolean_literals, false] {
                for inline_structured_closures in [configured.inline_structured_closures, false] {
                    for pack_string_arrays in [configured.pack_string_arrays, false] {
                        let scalar_phi_candidates = if ssa_variants {
                            [configured.scalar_phi_copies, !configured.scalar_phi_copies]
                        } else {
                            [configured.scalar_phi_copies; 2]
                        };
                        for scalar_phi_copies in scalar_phi_candidates {
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
    let candidate_limit = config.javascript.effective_candidate_limit();
    let beam_policy = JavaScriptCandidateBeamPolicy {
        cost_model: config.javascript.cost_model,
        candidate_limit,
    };
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
                    if candidates.len() == candidate_limit {
                        break;
                    }
                }
                if candidates.len() == candidate_limit {
                    break;
                }
            }
            if candidates.len() == candidate_limit {
                break;
            }
        }
        if candidates.len() == candidate_limit {
            break;
        }
    }
    candidates.sort_by(|left, right| (left.0, left.1, &left.2).cmp(&(right.0, right.1, &right.2)));
    if config.javascript_optimization_enabled(JavaScriptOptimization::ConditionalExpressionVariants)
    {
        let finalists = top_candidate_options(&candidates, 12);
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [crate::codegen_ir_js::IrJsOptions {
                    conditional_expressions: !options.conditional_expressions,
                    ..options
                }]
            },
        )?;
    }
    if config.javascript_optimization_enabled(JavaScriptOptimization::CommaExpressionVariants) {
        let finalists = top_candidate_options(&candidates, 12);
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [crate::codegen_ir_js::IrJsOptions {
                    comma_expressions: !options.comma_expressions,
                    ..options
                }]
            },
        )?;
    }
    if config.javascript_optimization_enabled(JavaScriptOptimization::StructuralControlFlowVariants)
    {
        let finalists = top_candidate_options(&candidates, 12);
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [
                    crate::codegen_ir_js::IrJsOptions {
                        control_flow_spelling:
                            crate::codegen_ir_js::ControlFlowSpelling::Structured,
                        ..options
                    },
                    crate::codegen_ir_js::IrJsOptions {
                        control_flow_spelling:
                            crate::codegen_ir_js::ControlFlowSpelling::StateMachine,
                        ..options
                    },
                ]
            },
        )?;
    }
    if config.loop_spelling_selection_enabled() {
        let finalists = candidates
            .iter()
            .take(8)
            .map(|candidate| candidate.3)
            .collect::<Vec<_>>();
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [
                    crate::codegen_ir_js::IrJsOptions {
                        loop_spelling: crate::codegen_ir_js::LoopSpelling::While,
                        ..options
                    },
                    crate::codegen_ir_js::IrJsOptions {
                        loop_spelling: crate::codegen_ir_js::LoopSpelling::For,
                        ..options
                    },
                ]
            },
        )?;
    }
    if config.javascript_optimization_enabled(JavaScriptOptimization::DoLoopVariants) {
        let finalists = top_candidate_options(&candidates, 12);
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [crate::codegen_ir_js::IrJsOptions {
                    loop_spelling: crate::codegen_ir_js::LoopSpelling::Do,
                    update_loop_layout: false,
                    ..options
                }]
            },
        )?;
    }
    if config.javascript_optimization_enabled(JavaScriptOptimization::UpdateLoopVariants) {
        let finalists = top_candidate_options(&candidates, 12);
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [crate::codegen_ir_js::IrJsOptions {
                    update_loop_layout: !options.update_loop_layout,
                    ..options
                }]
            },
        )?;
    }
    if config.mutation_spelling_selection_enabled() {
        let mut finalists = Vec::new();
        for loop_spelling in [
            crate::codegen_ir_js::LoopSpelling::Auto,
            crate::codegen_ir_js::LoopSpelling::While,
            crate::codegen_ir_js::LoopSpelling::For,
            crate::codegen_ir_js::LoopSpelling::Do,
        ] {
            let mut retained = 0;
            for options in candidates
                .iter()
                .filter(|candidate| candidate.3.loop_spelling == loop_spelling)
                .map(|candidate| candidate.3)
            {
                if !finalists.contains(&options) {
                    finalists.push(options);
                    retained += 1;
                    if retained == 8 {
                        break;
                    }
                }
            }
        }
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [
                    crate::codegen_ir_js::IrJsOptions {
                        mutation_spelling: crate::codegen_ir_js::MutationSpelling::Prefix,
                        ..options
                    },
                    crate::codegen_ir_js::IrJsOptions {
                        mutation_spelling: crate::codegen_ir_js::MutationSpelling::Postfix,
                        ..options
                    },
                    crate::codegen_ir_js::IrJsOptions {
                        mutation_spelling: crate::codegen_ir_js::MutationSpelling::Compound,
                        ..options
                    },
                ]
            },
        )?;
    }
    if config.javascript_optimization_enabled(JavaScriptOptimization::SwitchLoweringVariants) {
        let finalists = top_candidate_options(&candidates, 12);
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [crate::codegen_ir_js::IrJsOptions {
                    control_flow_spelling: crate::codegen_ir_js::ControlFlowSpelling::StateMachine,
                    state_machine_spelling: crate::codegen_ir_js::StateMachineSpelling::Conditional,
                    ..options
                }]
            },
        )?;
    }
    if config.javascript_optimization_enabled(JavaScriptOptimization::FunctionLayoutVariants) {
        let finalists = top_candidate_options(&candidates, 12);
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| {
                [crate::codegen_ir_js::IrJsOptions {
                    function_layout: crate::codegen_ir_js::FunctionLayout::CompressionSimilarity,
                    ..options
                }]
            },
        )?;
    }
    if !candidates
        .iter()
        .any(|(_, _, code, _)| code == &configured_baseline)
    {
        let transfer_cost =
            compressed_size(configured_baseline.as_bytes(), config.javascript.cost_model)
                .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
        candidates.push((
            transfer_cost,
            configured_baseline.len(),
            configured_baseline.clone(),
            configured,
        ));
    }
    finalize_javascript_candidates(candidates, &configured_baseline, config, ir, profile)
}

type JavaScriptEmissionCandidate = (usize, usize, String, crate::codegen_ir_js::IrJsOptions);

#[derive(Debug)]
struct ScoredJavaScriptCandidate {
    transfer_cost: usize,
    startup_score: u64,
    code: String,
    metrics: JavaScriptSyntaxMetrics,
    peephole_rewrites: usize,
    performance: JavaScriptPerformanceMetrics,
    rank: (u64, u64),
}

fn finalize_javascript_candidates(
    candidates: Vec<JavaScriptEmissionCandidate>,
    configured_baseline: &str,
    config: &ProjectConfig,
    ir: &ControlFlowModule<'_>,
    profile: &OptimizationProfile,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    let baseline_metrics = analyze_generated_javascript(configured_baseline)
        .map_err(generated_javascript_parse_error)?;
    let peephole =
        config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole);
    let startup_guard =
        config.javascript_optimization_configured(JavaScriptOptimization::StartupCostGuard);
    let candidates_evaluated = candidates.len();
    let baseline_transfer =
        compressed_size(configured_baseline.as_bytes(), config.javascript.cost_model)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
    let configured_options = config.js_options();
    let performance_model =
        config.javascript_optimization_configured(JavaScriptOptimization::PerformanceShapeModel);
    let baseline_performance = if performance_model {
        analyze_javascript_performance(
            ir,
            &configured_options,
            profile,
            config.javascript_performance_weights(),
        )
    } else {
        JavaScriptPerformanceMetrics::default()
    };
    let mut scored = Vec::<ScoredJavaScriptCandidate>::with_capacity(candidates.len());
    for (_, _, code, options) in candidates {
        let (code, metrics, peephole_rewrites) = if peephole {
            let optimized =
                optimize_generated_javascript(&code).map_err(generated_javascript_parse_error)?;
            (optimized.code, optimized.metrics, optimized.rewrites)
        } else {
            let metrics =
                analyze_generated_javascript(&code).map_err(generated_javascript_parse_error)?;
            (code, metrics, 0)
        };
        if startup_guard
            && !startup_cost_allowed(metrics, baseline_metrics, &config.javascript.startup)
        {
            continue;
        }
        if scored.iter().any(|candidate| candidate.code == code) {
            continue;
        }
        let transfer_cost = compressed_size(code.as_bytes(), config.javascript.cost_model)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
        let startup_score = metrics.startup_score(
            config.javascript.startup.parse_weight,
            config.javascript.startup.compile_weight,
            config.javascript.startup.memory_weight,
        );
        let performance = if performance_model {
            analyze_javascript_performance(
                ir,
                &options,
                profile,
                config.javascript_performance_weights(),
            )
        } else {
            JavaScriptPerformanceMetrics::default()
        };
        let rank = javascript_candidate_rank(
            config,
            transfer_cost,
            baseline_transfer,
            performance.score,
            baseline_performance.score,
        );
        scored.push(ScoredJavaScriptCandidate {
            transfer_cost,
            startup_score,
            code,
            metrics,
            peephole_rewrites,
            performance,
            rank,
        });
    }
    scored.sort_by(|left, right| {
        (left.rank, left.startup_score, left.code.len(), &left.code).cmp(&(
            right.rank,
            right.startup_score,
            right.code.len(),
            &right.code,
        ))
    });
    let selected = scored.into_iter().next().ok_or_else(|| {
        crate::codegen_js::CodegenError::new(
            Span::empty(0),
            "startup limits rejected every JavaScript candidate",
        )
    })?;
    Ok(SelectedJavaScriptCandidate {
        code: selected.code,
        transfer_cost: selected.transfer_cost,
        startup_score: selected.startup_score,
        metrics: selected.metrics,
        baseline_metrics,
        performance: selected.performance,
        baseline_performance,
        candidates_evaluated,
        peephole_rewrites: selected.peephole_rewrites,
    })
}

fn javascript_candidate_rank(
    config: &ProjectConfig,
    transfer: usize,
    baseline_transfer: usize,
    performance: u64,
    baseline_performance: u64,
) -> (u64, u64) {
    let transfer_ratio = normalized_ratio(transfer as u64, baseline_transfer as u64);
    let performance_ratio = normalized_ratio(performance, baseline_performance);
    match config.javascript.priority {
        crate::config::JavaScriptPriority::PerformanceFirst => (performance_ratio, transfer_ratio),
        crate::config::JavaScriptPriority::RealisticPerformanceFirst => {
            let limit = 10_000u64.saturating_add(
                u64::from(config.javascript.performance.max_regression_percent).saturating_mul(100),
            );
            let rejected = u64::from(performance_ratio > limit);
            (
                rejected
                    .saturating_mul(1_000_000)
                    .saturating_add(transfer_ratio),
                performance_ratio,
            )
        }
        crate::config::JavaScriptPriority::Balanced => (
            transfer_ratio
                .saturating_mul(3)
                .saturating_add(performance_ratio.saturating_mul(2)),
            transfer_ratio,
        ),
        crate::config::JavaScriptPriority::SizeFirst => (transfer_ratio, performance_ratio),
    }
}

fn normalized_ratio(value: u64, baseline: u64) -> u64 {
    if baseline == 0 {
        return u64::from(value != 0).saturating_mul(10_000);
    }
    value.saturating_mul(10_000).saturating_div(baseline)
}

fn generated_javascript_parse_error(
    error: crate::js_peephole::JavaScriptParseError,
) -> CompileError {
    crate::codegen_js::CodegenError::new(
        Span::empty(error.offset()),
        format!("generated JavaScript parser failed: {error}"),
    )
    .into()
}

fn startup_cost_allowed(
    candidate: JavaScriptSyntaxMetrics,
    baseline: JavaScriptSyntaxMetrics,
    policy: &crate::config::StartupCostConfig,
) -> bool {
    within_startup_limit(
        candidate.parse_cost,
        baseline.parse_cost,
        policy.parse_overhead_limit_percent,
    ) && within_startup_limit(
        candidate.compile_cost,
        baseline.compile_cost,
        policy.compile_overhead_limit_percent,
    ) && within_startup_limit(
        candidate.estimated_memory_bytes,
        baseline.estimated_memory_bytes,
        policy.memory_overhead_limit_percent,
    )
}

fn within_startup_limit(candidate: u64, baseline: u64, overhead_percent: u32) -> bool {
    let maximum = baseline.saturating_add(
        baseline
            .saturating_mul(u64::from(overhead_percent))
            .saturating_div(100),
    );
    candidate <= maximum
}

const fn compression_cost_model_name(model: CompressionCostModel) -> &'static str {
    match model {
        CompressionCostModel::Raw => "raw",
        CompressionCostModel::Gzip => "gzip",
        CompressionCostModel::Brotli => "brotli",
    }
}

fn top_candidate_options(
    candidates: &[JavaScriptEmissionCandidate],
    limit: usize,
) -> Vec<crate::codegen_ir_js::IrJsOptions> {
    let mut options = Vec::new();
    for candidate in candidates {
        if !options.contains(&candidate.3) {
            options.push(candidate.3);
            if options.len() == limit {
                break;
            }
        }
    }
    options
}

#[derive(Debug, Clone, Copy)]
struct JavaScriptCandidateBeamPolicy {
    cost_model: CompressionCostModel,
    candidate_limit: usize,
}

fn extend_javascript_candidate_beam<const N: usize>(
    ir: &ControlFlowModule<'_>,
    module_output: bool,
    policy: JavaScriptCandidateBeamPolicy,
    integer_analysis: &Arc<IntegerValueAnalysis>,
    candidates: &mut Vec<JavaScriptEmissionCandidate>,
    finalists: Vec<crate::codegen_ir_js::IrJsOptions>,
    variants: impl Fn(crate::codegen_ir_js::IrJsOptions) -> [crate::codegen_ir_js::IrJsOptions; N],
) -> Result<(), CompileError> {
    let reserve = finalists.len().saturating_mul(N).saturating_mul(2);
    if candidates.len().saturating_add(reserve) > policy.candidate_limit {
        candidates.truncate(policy.candidate_limit.saturating_sub(reserve).max(1));
    }
    for options in finalists {
        for candidate_options in variants(options) {
            let code = emit_javascript_candidate(
                ir,
                module_output,
                candidate_options,
                Arc::clone(integer_analysis),
            )?;
            for code in top_level_declaration_variants(code) {
                if candidates
                    .iter()
                    .any(|(_, _, existing, _)| existing == &code)
                {
                    continue;
                }
                let cost =
                    compressed_size(code.as_bytes(), policy.cost_model).map_err(|message| {
                        crate::codegen_js::CodegenError::new(Span::empty(0), message)
                    })?;
                candidates.push((cost, code.len(), code, candidate_options));
                if candidates.len() >= policy.candidate_limit {
                    break;
                }
            }
            if candidates.len() >= policy.candidate_limit {
                break;
            }
        }
        if candidates.len() >= policy.candidate_limit {
            break;
        }
    }
    candidates.sort_by(|left, right| (left.0, left.1, &left.2).cmp(&(right.0, right.1, &right.2)));
    Ok(())
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
    let guidance = load_optimization_guidance(config, config.native_profile_guided_optimization())?;
    optimize_control_flow_with_guidance(&mut ir, &config.optimizer_options(), false, &guidance)?;
    emit_native_c_with_options(&ir, &config.native_options()).map_err(Into::into)
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
    use crate::config::{
        CandidateSearch, CompressionDecision, JavaScriptOptimization, OptimizationPreset,
        StartupCostConfig,
    };

    #[test]
    fn compiles_source_end_to_end() {
        assert_eq!(compile_source("print(40+2);").unwrap(), "console.log(42)");
    }

    #[test]
    fn parsed_peephole_is_independently_configurable() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern float read();void count(){float limit=read();for(float index=0;index<limit;index=index+1){print(index);}}count();",
        )
        .unwrap();
        let mut disabled = ProjectConfig::default();
        disabled.optimization.preset = OptimizationPreset::None;
        disabled.javascript.candidate_search = CandidateSearch::Off;
        disabled.javascript.optimizations = Some(Vec::new());
        disabled.javascript.compression = Some(Vec::new());
        disabled.mangle.identifiers = Some(false);
        let plain = compile_program_to_js_configured(&program, &disabled).unwrap();
        let update = plain
            .split_once("for(;")
            .and_then(|(_, loop_tail)| loop_tail.split_once(';'))
            .and_then(|(_, update)| update.split_once(')'))
            .map(|(update, _)| update)
            .expect("condition-only loop must contain an update");
        let variable = update
            .split_once('=')
            .map(|(variable, _)| variable)
            .expect("plain loop update must be an assignment");
        assert_eq!(update, format!("{variable}={variable}+1"), "{plain}");

        let mut enabled = disabled;
        enabled.javascript.optimizations = Some(vec![JavaScriptOptimization::ParsedPeephole]);
        let optimized = compile_program_to_js_configured(&program, &enabled).unwrap();
        assert!(optimized.contains(&format!("{variable}+=1")), "{optimized}");
        assert!(
            !optimized.contains(&format!("{variable}={variable}+1")),
            "{optimized}"
        );
    }

    #[test]
    fn startup_guard_uses_independent_saturating_limits() {
        let baseline = JavaScriptSyntaxMetrics {
            parse_cost: 100,
            compile_cost: 200,
            estimated_memory_bytes: 400,
            ..JavaScriptSyntaxMetrics::default()
        };
        let policy = StartupCostConfig {
            parse_overhead_limit_percent: 10,
            compile_overhead_limit_percent: 20,
            memory_overhead_limit_percent: 25,
            ..StartupCostConfig::default()
        };
        assert!(startup_cost_allowed(
            JavaScriptSyntaxMetrics {
                parse_cost: 110,
                compile_cost: 240,
                estimated_memory_bytes: 500,
                ..JavaScriptSyntaxMetrics::default()
            },
            baseline,
            &policy,
        ));
        assert!(!startup_cost_allowed(
            JavaScriptSyntaxMetrics {
                parse_cost: 111,
                compile_cost: 240,
                estimated_memory_bytes: 500,
                ..JavaScriptSyntaxMetrics::default()
            },
            baseline,
            &policy,
        ));
    }

    #[test]
    fn explained_compilation_reports_selection_costs() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int square(int value){return value*value;}print(square(9));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let selected =
            optimize_and_select_javascript(ir, &ProjectConfig::default(), false).unwrap();

        assert_eq!(
            selected.selection_metrics.syntax.bytes,
            selected.javascript.len()
        );
        assert!(selected.selection_metrics.syntax.tokens > 0);
        assert!(selected.selection_metrics.transfer_bytes > 0);
        assert!(selected.selection_metrics.candidates_evaluated > 0);
        assert!(selected.selection_metrics.performance.score > 0);
        assert_eq!(selected.selection_metrics.codec, "brotli");
    }

    #[test]
    fn javascript_priorities_rank_transfer_and_runtime_shape_independently() {
        let mut config = ProjectConfig::default();
        config.javascript.priority = crate::config::JavaScriptPriority::SizeFirst;
        assert!(
            javascript_candidate_rank(&config, 90, 100, 120, 100)
                < javascript_candidate_rank(&config, 100, 100, 80, 100)
        );

        config.javascript.priority = crate::config::JavaScriptPriority::PerformanceFirst;
        assert!(
            javascript_candidate_rank(&config, 100, 100, 80, 100)
                < javascript_candidate_rank(&config, 90, 100, 120, 100)
        );

        config.javascript.priority = crate::config::JavaScriptPriority::RealisticPerformanceFirst;
        config.javascript.performance.max_regression_percent = 10;
        assert!(
            javascript_candidate_rank(&config, 100, 100, 100, 100)
                < javascript_candidate_rank(&config, 80, 100, 120, 100)
        );
    }

    #[test]
    fn profile_template_lists_stable_function_and_loop_keys() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-profile-template-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("main.lil");
        std::fs::write(
            &path,
            "int update(int value){while(value<3){value++;}return value;}print(update(0));",
        )
        .unwrap();

        let profile = profile_template_path_configured(&path, &ProjectConfig::default()).unwrap();
        assert_eq!(profile.functions.get("$entry"), Some(&1));
        assert_eq!(profile.functions.get("update"), Some(&1));
        assert_eq!(profile.loops.get("update#0"), Some(&1));
        std::fs::remove_dir_all(directory).unwrap();
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
    fn codec_scoring_selects_a_better_function_layout_without_raw_growth() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            include_str!("../benchmarks/function-layout/fixture.lil"),
        )
        .unwrap();
        let mut enabled = ProjectConfig::default();
        enabled.optimization.preset = OptimizationPreset::None;
        enabled.javascript.optimizations =
            Some(vec![JavaScriptOptimization::FunctionLayoutVariants]);
        enabled.javascript.compression = Some(Vec::new());
        enabled.javascript.candidate_search = CandidateSearch::Always;
        enabled.javascript.candidate_limit = 16;
        enabled.mangle.identifiers = Some(false);
        enabled.mangle.properties = Some(false);
        enabled.mangle.exports = Some(false);
        enabled.mangle.pool_strings = Some(false);
        let mut source_order = enabled.clone();
        source_order.javascript.optimizations = Some(Vec::new());
        source_order.javascript.candidate_search = CandidateSearch::Off;

        let selected = compile_program_to_js_configured(&program, &enabled).unwrap();
        let baseline = compile_program_to_js_configured(&program, &source_order).unwrap();

        assert_eq!(selected.len(), baseline.len());
        assert!(
            compressed_size(selected.as_bytes(), CompressionCostModel::Brotli).unwrap()
                < compressed_size(baseline.as_bytes(), CompressionCostModel::Brotli).unwrap(),
            "selected:\n{selected}\nsource order:\n{baseline}"
        );
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
        assert!(
            entry.contains(&format!("from\"./{}\"", bundle.manifest.chunks[0].file)),
            "{entry}"
        );
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
            .find("length=values.length")
            .unwrap_or_else(|| panic!("array length read must be stored: {output}"));
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
