use crate::stable_hash::StableHashMap as AHashMap;
use bumpalo::Bump;
use rayon::prelude::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use crate::codegen_ir_js::{
    emit_optimized_ir_js, emit_optimized_ir_js_chunks_with_options, emit_optimized_ir_js_module,
    emit_optimized_ir_js_module_with_options_and_analysis,
    emit_optimized_ir_js_with_options_and_analysis, has_inlineable_fresh_empty_array_factory,
    ir_function_can_move_to_chunk, IrJsChunk, IrJsChunkPlan, IrJsChunkSpec,
};
use crate::codegen_js::{compile_to_js, CompileError};
use crate::codegen_native::{compile_to_c, emit_native_c, emit_native_c_with_options};
use crate::config::{
    BundleMode, CompilerResourceConfig, CompressionCostModel, JavaScriptOptimization,
    PreloadPolicy, ProjectConfig,
};
use crate::ir::{ControlFlowModule, FunctionId};
#[cfg(test)]
use crate::ir::{ControlFlowOp, Intrinsic};
use crate::js_peephole::{
    analyze_generated_javascript, converge_local_names, declared_identifier_character_use_counts,
    fold_constant_json_parse, fold_dead_identifier_copy_declarators, fold_dead_increment_snapshots,
    fold_expression_bodies, fold_fresh_empty_object_assign, fold_if_prefixed_returns,
    fold_nested_unguarded_ifs, fold_pristine_static_method_calls, fold_redundant_null_undefined_or,
    function_leading_declaration_variant, function_local_binding_swap_variants,
    generated_javascript_bit_or_zero_count, generated_javascript_export_names,
    generated_javascript_export_witnesses, generated_javascript_static_imports,
    generated_javascript_static_property_names, identifier_name_is_clear_binding,
    inline_single_use_functions, late_generated_javascript_cleanup,
    late_generated_javascript_cleanup_local_variants, late_generated_javascript_cleanup_pass,
    optimize_generated_javascript_assuming,
    optimize_generated_javascript_preserving_functions_assuming, remap_identifier,
    remap_single_character_identifiers, repair_fused_keyword_identifiers,
    single_character_identifier_use_counts, single_character_identifiers,
    single_character_name_is_clear_binding, single_character_resolved_binding_identifiers,
    two_character_identifier_use_counts, validate_generated_javascript_syntax_floor,
    JavaScriptSyntaxMetrics, LateJavaScriptCleanupPass,
};
use crate::lower::lower_to_control_flow;
use crate::module::{
    discover_modules, discover_modules_configured, discover_modules_configured_with_source,
    discover_modules_with_source, link_modules, locate_linked_span, parse_modules, ModuleError,
    ModuleSet,
};
use crate::optimizer::{
    lower_known_js_host_calls, optimize_control_flow, optimize_control_flow_for_module,
    optimize_control_flow_with_guidance, project_closed_record_observations_for_javascript,
    strip_console_output, OptimizationGuidance, OptimizationReport,
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
pub struct BundledCompilationArtifacts {
    pub javascript: JavaScriptBundle,
    pub c: String,
    pub optimization_reports: Vec<OptimizationReport>,
}

#[cfg(test)]
std::thread_local! {
    static CONFIGURED_MODULE_LOWERINGS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn record_configured_module_lowering() {
    CONFIGURED_MODULE_LOWERINGS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn reset_configured_module_lowering_count() {
    CONFIGURED_MODULE_LOWERINGS.with(|count| count.set(0));
}

#[cfg(test)]
fn configured_module_lowering_count() -> usize {
    CONFIGURED_MODULE_LOWERINGS.with(std::cell::Cell::get)
}

#[derive(Debug, Clone, PartialEq)]
pub struct JavaScriptCompilation {
    pub javascript: String,
    pub optimization_reports: Vec<OptimizationReport>,
    pub selection_metrics: JavaScriptSelectionMetrics,
    pub abi_manifest: crate::compilation_contract::JavaScriptAbiManifest,
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
    /// Distinct `(IR context, emission options)` plans registered during the
    /// bounded search, including already-scored context seeds.
    pub plans_registered: usize,
    /// Whole-artifact IR-to-JavaScript emissions attempted after the scored
    /// context seeds were installed.
    pub emissions_attempted: usize,
    /// Configured-root plus optional optimizer-context emissions attempted
    /// before the artifact-size-derived structural ledger can be installed.
    pub optimizer_emissions_attempted: usize,
    /// Optional structural plan identities admitted after the scored context
    /// seeds. Terminal naming/declaration plans use their separately reserved
    /// tail and do not debit this limit.
    pub candidate_proposal_limit: usize,
    pub candidate_proposal_work_units: usize,
    pub candidate_proposal_limit_reached: bool,
    /// Exact-codec calls made by the bounded terminal syntax/name search.
    pub terminal_codec_probes: usize,
    /// Deterministically admitted terminal proposal/validation/codec work
    /// units. Invalid proposals consume a unit before expensive validation.
    pub terminal_work_units: usize,
    pub terminal_codec_probe_limit: usize,
    pub terminal_codec_probe_limit_reached: bool,
    pub peephole_rewrites: usize,
    pub decisions: JavaScriptSelectionDecisions,
    pub layout_searched: bool,
    pub removed_compression_families: Vec<String>,
    pub scored_emission_families: Vec<String>,
    pub starved_emission_families: Vec<String>,
    pub cartesian_emission_axes: Vec<String>,
    pub ir_variants_searched: Vec<String>,
    pub source_operations: usize,
    pub generated_operations: usize,
    pub decision_registry_version: u32,
    pub search_guarantee: String,
    pub search_stop_reason: String,
    pub compiler_time_micros: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptSelectionDecisions {
    pub explicit_lowering_obligations: bool,
    pub string_pooling: bool,
    pub identifier_string_pooling: bool,
    pub string_array_packing: bool,
    pub scalar_replacement: bool,
    pub string_pool_minimum_savings: usize,
    pub transitive_nested_shadowing: bool,
    pub precise_cross_scope_shadowing: bool,
    pub reserved_local_name_prefix: bool,
    pub local_name_reserve: usize,
    pub stable_local_names: bool,
    pub frequency_order_local_names: bool,
    pub local_name_coalescing: bool,
    pub length_to_number_elision: bool,
    pub terminal_scope_naming_challengers: usize,
    pub terminal_scope_naming_selected: bool,
    pub terminal_scope_naming_incumbent_bytes: Option<usize>,
    pub terminal_scope_naming_best_bytes: Option<usize>,
    pub terminal_string_pooling_challengers: usize,
    pub terminal_string_pooling_selected: bool,
    pub terminal_string_pooling_incumbent_bytes: Option<usize>,
    pub terminal_string_pooling_best_bytes: Option<usize>,
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
    pub objective: JavaScriptBundleObjectiveManifest,
    pub objective_fingerprint: String,
    pub selected_transfer_bytes: usize,
    pub deploy_cost: u64,
    pub chunks: Vec<JavaScriptBundleManifestChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptBundleObjectiveManifest {
    pub javascript_codec: String,
    pub raw_weight: u32,
    pub gzip_weight: u32,
    pub brotli_weight: u32,
    pub request_overhead_bytes: usize,
    pub dependency_depth_penalty_bytes: usize,
    pub preload_request_discount_percent: u32,
    pub cache_reuse_discount_percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JavaScriptBundleManifestChunk {
    pub file: String,
    pub modules: Vec<String>,
    pub bytes: usize,
    pub gzip_bytes: usize,
    pub brotli_bytes: usize,
    pub selected_transfer_bytes: usize,
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
    compile_path_explained_inner(path, config, false)
}

pub fn compile_path_to_js_module_explained_configured(
    path: &Path,
    config: &ProjectConfig,
) -> Result<JavaScriptCompilation, ModuleError> {
    compile_path_explained_inner(path, config, true)
}

fn compile_path_explained_inner(
    path: &Path,
    config: &ProjectConfig,
    module_output: bool,
) -> Result<JavaScriptCompilation, ModuleError> {
    let arena = Bump::new();
    let modules = discover_modules_configured(path, config)?;
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    let semantics = analyze(&linked)
        .map_err(|error| module_compile_error(&modules, CompileError::Semantic(error)))?;
    let ir = lower_to_control_flow(&linked, &semantics)
        .map_err(|error| module_compile_error(&modules, CompileError::Lower(error)))?;
    let contract = config.javascript_compilation_contract(module_output);
    let abi_manifest = contract.abi_manifest(&ir);
    let selected = optimize_and_select_javascript(ir, config, module_output)
        .map_err(|error| module_compile_error(&modules, error))?;
    if selected.abi_manifest != abi_manifest {
        return Err(module_compile_error(
            &modules,
            CompileError::Codegen(crate::codegen_js::CodegenError::new(
                Span::empty(0),
                "selected JavaScript candidate changed the normalized ABI manifest",
            )),
        ));
    }
    let javascript = if module_output {
        finish_javascript_module(selected.javascript, config)
            .map_err(|error| module_compile_error(&modules, error))?
    } else {
        selected.javascript
    };
    Ok(JavaScriptCompilation {
        javascript,
        optimization_reports: selected.optimization_reports,
        selection_metrics: selected.selection_metrics,
        abi_manifest: selected.abi_manifest,
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
    install_configured_compiler_pool_by(
        &config.compiler.resources,
        |message| ModuleError::new(path, "", Span::empty(0), message),
        move || compile_path_to_js_bundle_configured_inner(path, config, entry_file),
    )
}

#[cfg(test)]
fn compile_path_to_js_bundle_configured_observing_pool(
    path: &Path,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<(JavaScriptBundle, usize), ModuleError> {
    install_configured_compiler_pool_by(
        &config.compiler.resources,
        |message| ModuleError::new(path, "", Span::empty(0), message),
        move || {
            let active_threads = rayon::current_num_threads();
            compile_path_to_js_bundle_configured_inner(path, config, entry_file)
                .map(|bundle| (bundle, active_threads))
        },
    )
}

fn compile_path_to_js_bundle_configured_inner(
    path: &Path,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<JavaScriptBundle, ModuleError> {
    let modules = discover_modules_configured(path, config)?;
    validate_bundle_entry_file(path, &modules, entry_file)?;
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    let semantics = analyze(&linked)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    let ir = lower_to_control_flow(&linked, &semantics)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    #[cfg(test)]
    record_configured_module_lowering();
    compile_javascript_bundle_from_ir(ir, &modules, config, entry_file).map(|(bundle, _)| bundle)
}

fn validate_bundle_entry_file(
    path: &Path,
    modules: &ModuleSet,
    entry_file: &str,
) -> Result<(), ModuleError> {
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
    Ok(())
}

fn compile_javascript_bundle_from_ir<'src>(
    mut ir: ControlFlowModule<'src>,
    modules: &ModuleSet,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<(JavaScriptBundle, Vec<OptimizationReport>), ModuleError> {
    let guidance = load_optimization_guidance(
        config,
        config
            .javascript_optimization_configured(JavaScriptOptimization::ProfileGuidedOptimization),
    )
    .map_err(|error| module_compile_error(&modules, error))?;
    prepare_javascript_ir(&mut ir, config);
    let mut optimizer_options = config.js_optimizer_options();
    // This bundle path performs one configured IR optimization rather than
    // the script/module candidate beam. Keep outlining disabled until it can
    // compete against a mandatory unoutlined baseline here too. For split and
    // preserve modes this also prevents ownerless synthesized helpers from
    // creating compiler-introduced reverse imports between chunks.
    optimizer_options.region_outlining = false;
    let optimization_reports =
        optimize_control_flow_with_guidance(&mut ir, &optimizer_options, true, &guidance)
            .map_err(CompileError::from)
            .map_err(|error| module_compile_error(&modules, error))?;

    let selected_plan = plan_javascript_chunks(&ir, &modules, config, entry_file)?;
    let specs = &selected_plan.chunks;
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
    let mut emitted = emit_optimized_ir_js_chunks_with_options(&ir, &selected_plan.options, &plan)
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
    let mut selected_transfer_bytes = 0usize;
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
        selected_transfer_bytes =
            selected_transfer_bytes.saturating_add(match config.javascript.cost_model {
                CompressionCostModel::Raw => chunk.code.len(),
                CompressionCostModel::Gzip => gzip_bytes,
                CompressionCostModel::Brotli => brotli_bytes,
            });
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
                selected_transfer_bytes: match config.javascript.cost_model {
                    CompressionCostModel::Raw => emitted.code.len(),
                    CompressionCostModel::Gzip => gzip_bytes,
                    CompressionCostModel::Brotli => brotli_bytes,
                },
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
    let objective = JavaScriptBundleObjectiveManifest {
        javascript_codec: compression_cost_model_name(config.javascript.cost_model).to_string(),
        raw_weight: config.bundle.cost.raw_weight,
        gzip_weight: config.bundle.cost.gzip_weight,
        brotli_weight: config.bundle.cost.brotli_weight,
        request_overhead_bytes: config.bundle.cost.request_overhead_bytes,
        dependency_depth_penalty_bytes: config.bundle.cost.dependency_depth_penalty_bytes,
        preload_request_discount_percent: config.bundle.cost.preload_request_discount_percent,
        cache_reuse_discount_percent: config.bundle.cost.cache_reuse_discount_percent,
    };
    let objective_fingerprint = content_hash(
        format!(
            "v1:{}:{}:{}:{}:{}:{}:{}:{}",
            objective.javascript_codec,
            objective.raw_weight,
            objective.gzip_weight,
            objective.brotli_weight,
            objective.request_overhead_bytes,
            objective.dependency_depth_penalty_bytes,
            objective.preload_request_discount_percent,
            objective.cache_reuse_discount_percent,
        )
        .as_bytes(),
    );
    let files = emitted
        .into_iter()
        .map(|chunk| JavaScriptBundleFile {
            file_name: chunk.file_name,
            code: chunk.code,
        })
        .collect::<Vec<_>>();
    let bundle = JavaScriptBundle {
        files,
        manifest: JavaScriptBundleManifest {
            version: 2,
            build_id,
            mode: bundle_mode_name(config.bundle.mode).to_string(),
            entry: entry_file.to_string(),
            preload,
            objective,
            objective_fingerprint,
            selected_transfer_bytes,
            deploy_cost,
            chunks,
        },
    };
    Ok((bundle, optimization_reports))
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

pub fn compile_path_all_to_js_bundle_configured(
    path: &Path,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<BundledCompilationArtifacts, ModuleError> {
    install_configured_compiler_pool_by(
        &config.compiler.resources,
        |message| ModuleError::new(path, "", Span::empty(0), message),
        move || compile_path_all_to_js_bundle_configured_inner(path, config, entry_file),
    )
}

fn compile_path_all_to_js_bundle_configured_inner(
    path: &Path,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<BundledCompilationArtifacts, ModuleError> {
    let modules = discover_modules_configured(path, config)?;
    validate_bundle_entry_file(path, &modules, entry_file)?;
    let arena = Bump::new();
    let programs = parse_modules(&arena, &modules)?;
    let linked = link_modules(&arena, &modules, &programs)?;
    let semantics = analyze(&linked)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    let javascript_ir = lower_to_control_flow(&linked, &semantics)
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    #[cfg(test)]
    record_configured_module_lowering();
    let mut native_ir = javascript_ir.clone();
    let (javascript, optimization_reports) =
        compile_javascript_bundle_from_ir(javascript_ir, &modules, config, entry_file)?;
    let native_guidance =
        load_optimization_guidance(config, config.native_profile_guided_optimization())
            .map_err(|error| module_compile_error(&modules, error))?;
    optimize_control_flow_with_guidance(
        &mut native_ir,
        &config.optimizer_options(),
        false,
        &native_guidance,
    )
    .map_err(CompileError::from)
    .map_err(|error| module_compile_error(&modules, error))?;
    let c = emit_native_c_with_options(&native_ir, &config.native_options())
        .map_err(CompileError::from)
        .map_err(|error| module_compile_error(&modules, error))?;
    Ok(BundledCompilationArtifacts {
        javascript,
        c,
        optimization_reports,
    })
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

#[derive(Debug, Clone)]
struct SelectedJavaScriptChunkPlan {
    chunks: Vec<PlannedChunk>,
    options: crate::codegen_ir_js::IrJsOptions,
}

fn plan_javascript_chunks(
    ir: &ControlFlowModule<'_>,
    modules: &ModuleSet,
    config: &ProjectConfig,
    entry_file: &str,
) -> Result<SelectedJavaScriptChunkPlan, ModuleError> {
    if config.bundle.mode == BundleMode::Single {
        return Ok(SelectedJavaScriptChunkPlan {
            chunks: Vec::new(),
            options: config.js_options(),
        });
    }
    let mut by_module = AHashMap::<usize, Vec<FunctionId>>::default();
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
        return Ok(SelectedJavaScriptChunkPlan {
            chunks: candidates,
            options: config.js_options(),
        });
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
        return Ok(SelectedJavaScriptChunkPlan {
            chunks: candidates,
            options: config.js_options(),
        });
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
    if selected.len() > config.bundle.max_chunks {
        return Err(ModuleError::new(
            &modules.modules[modules.root].path,
            &modules.modules[modules.root].source,
            Span::empty(0),
            format!(
                "`bundle.max_chunks` is {}, but the split plan requires {} mandatory lazy chunks; increase `bundle.max_chunks` or reduce the number of lazy modules",
                config.bundle.max_chunks,
                selected.len(),
            ),
        ));
    }
    let (mut selected_cost, mut selected_options) =
        score_javascript_chunk_plan(ir, config, entry_file, &selected)
            .map_err(|error| module_compile_error(modules, error))?;
    while selected.len() < config.bundle.max_chunks && !optional.is_empty() {
        let mut best = None::<(usize, u64, crate::codegen_ir_js::IrJsOptions)>;
        for (index, candidate) in optional.iter().enumerate() {
            let mut trial = selected.clone();
            trial.push(candidate.clone());
            trial.sort_unstable_by_key(|chunk| chunk.module);
            let (cost, options) = score_javascript_chunk_plan(ir, config, entry_file, &trial)
                .map_err(|error| module_compile_error(modules, error))?;
            if best.is_none_or(|(best_index, best_cost, _)| {
                (cost, candidate.module) < (best_cost, optional[best_index].module)
            }) {
                best = Some((index, cost, options));
            }
        }
        let Some((index, cost, options)) = best.filter(|(_, cost, _)| *cost < selected_cost) else {
            break;
        };
        selected.push(optional.remove(index));
        selected.sort_unstable_by_key(|chunk| chunk.module);
        selected_cost = cost;
        selected_options = options;
    }
    Ok(SelectedJavaScriptChunkPlan {
        chunks: selected,
        options: selected_options,
    })
}

fn score_javascript_chunk_plan(
    ir: &ControlFlowModule<'_>,
    config: &ProjectConfig,
    entry_file: &str,
    chunks: &[PlannedChunk],
) -> Result<(u64, crate::codegen_ir_js::IrJsOptions), CompileError> {
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
    let mut option_variants = vec![config.js_options()];
    if config.js_joint_chunk_symbol_search_enabled() {
        let configured = config.js_options();
        option_variants.push(crate::codegen_ir_js::IrJsOptions {
            function_layout: crate::codegen_ir_js::FunctionLayout::CompressionSimilarity,
            ..configured
        });
        option_variants.push(crate::codegen_ir_js::IrJsOptions {
            function_layout: crate::codegen_ir_js::FunctionLayout::CompressionWindow(
                codec_history_window(config.javascript.cost_model),
            ),
            ..configured
        });
        if configured.local_name_reserve > 0 {
            option_variants.push(crate::codegen_ir_js::IrJsOptions {
                local_name_reserve: 0,
                ..configured
            });
        }
        option_variants.dedup();
    }
    let mut best = None::<(u64, crate::codegen_ir_js::IrJsOptions)>;
    for options in option_variants {
        let mut emitted = emit_optimized_ir_js_chunks_with_options(ir, &options, &plan)?;
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
        if best.is_none_or(|(best_score, best_options)| {
            score < best_score
                || (score == best_score
                    && options.local_name_reserve < best_options.local_name_reserve)
        }) {
            best = Some((score, options));
        }
    }
    Ok(best.expect("every chunk plan has at least one JavaScript emission option"))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct JavaScriptTransferSizes {
    pub raw: usize,
    pub gzip9: usize,
    pub brotli11: usize,
}

pub fn measure_javascript_transfer_sizes(bytes: &[u8]) -> Result<JavaScriptTransferSizes, String> {
    Ok(JavaScriptTransferSizes {
        raw: bytes.len(),
        gzip9: compressed_size(bytes, CompressionCostModel::Gzip)?,
        brotli11: compressed_size(bytes, CompressionCostModel::Brotli)?,
    })
}

fn compressed_artifact_sizes(code: &str) -> Result<(usize, usize), String> {
    let sizes = measure_javascript_transfer_sizes(code.as_bytes())?;
    Ok((sizes.gzip9, sizes.brotli11))
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
    let mut depths = AHashMap::default();
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
    let mut reachability = AHashMap::default();
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

fn javascript_oracle_config() -> ProjectConfig {
    let mut config = ProjectConfig::default();
    config.javascript.strip_console = false;
    // These cases exercise search features across the whole effort ladder,
    // including the four gated at level 14, so the oracle pins the ceiling
    // rather than inheriting the shipped default. The default is a
    // compile-time policy for real projects (13, the measured plateau — see
    // `docs/configuration.md`); it is not what the search tests are about, and
    // letting it move them would silently narrow their coverage.
    config.javascript.optimization_level = 15;
    config
}

fn compile_program_to_js<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let mut ir = lower_to_control_flow(program, &semantics)?;
    prepare_javascript_ir(&mut ir, &javascript_oracle_config());
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
    prepare_javascript_ir(&mut ir, &javascript_oracle_config());
    optimize_control_flow_for_module(&mut ir)?;
    emit_optimized_ir_js_module(&ir).map_err(Into::into)
}

fn compile_program_to_js_module_configured<'ast, 'src>(
    program: &crate::ast::Program<'ast, 'src>,
    config: &ProjectConfig,
) -> Result<String, CompileError> {
    let semantics = analyze(program)?;
    let ir = lower_to_control_flow(program, &semantics)?;
    let selected = optimize_and_select_javascript(ir, config, true)?;
    finish_javascript_module(selected.javascript, config)
}

/// The last step of a single-bundle module: `javascript.function_scope`
/// moves the module's internal bindings into one function scope (V8 reads
/// them through context slots instead of module cells). The wrapper is a
/// textual transform of the selected artifact, after every fold and the
/// rename, and it is refused rather than guessed when the artifact's export
/// shape is not the plain trailing list.
fn finish_javascript_module(
    javascript: String,
    config: &ProjectConfig,
) -> Result<String, CompileError> {
    if config.javascript.function_scope != Some(true) {
        return Ok(javascript);
    }
    match crate::js_peephole::wrap_module_internals_in_function_scope(&javascript) {
        Ok(Ok(wrapped)) => Ok(wrapped),
        Ok(Err(_reason)) => Ok(javascript),
        Err(error) => Err(CompileError::Codegen(
            crate::codegen_js::CodegenError::new(
                Span::empty(0),
                format!("function_scope wrapper produced an unparseable module: {error}"),
            ),
        )),
    }
}

struct OptimizedJavascriptCandidate {
    javascript: String,
    #[cfg_attr(not(test), allow(dead_code))]
    plan_identity: JavaScriptPlanIdentity,
    optimization_reports: Vec<OptimizationReport>,
    selection_metrics: JavaScriptSelectionMetrics,
    abi_manifest: crate::compilation_contract::JavaScriptAbiManifest,
}

fn prepare_javascript_ir(ir: &mut ControlFlowModule<'_>, config: &ProjectConfig) {
    if config.javascript.strip_console {
        strip_console_output(ir);
    }
    lower_known_js_host_calls(ir);
}

fn optimize_and_select_javascript<'src>(
    ir: ControlFlowModule<'src>,
    config: &ProjectConfig,
    module_output: bool,
) -> Result<OptimizedJavascriptCandidate, CompileError> {
    let contract = config.javascript_compilation_contract(module_output);
    let objective = config.javascript_optimization_objective();
    debug_assert_eq!(objective.transfer, config.javascript.cost_model);
    install_configured_compiler_pool(&config.compiler.resources, move || {
        optimize_and_select_javascript_inner(ir, config, contract.abi.preserve_root_exports)
    })
}

fn install_configured_compiler_pool<Output: Send>(
    resources: &CompilerResourceConfig,
    work: impl FnOnce() -> Result<Output, CompileError> + Send,
) -> Result<Output, CompileError> {
    install_configured_compiler_pool_by(
        resources,
        |message| crate::codegen_js::CodegenError::new(Span::empty(0), message).into(),
        work,
    )
}

fn install_configured_compiler_pool_by<Output: Send, Error: Send>(
    resources: &CompilerResourceConfig,
    pool_error: impl FnOnce(String) -> Error,
    work: impl FnOnce() -> Result<Output, Error> + Send,
) -> Result<Output, Error> {
    let Some(threads) = resources.threads else {
        return work();
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.get())
        .build()
        .map_err(|error| pool_error(format!("failed to create compiler worker pool: {error}")))?;
    pool.install(work)
}

fn optimize_and_select_javascript_inner<'src>(
    ir: ControlFlowModule<'src>,
    config: &ProjectConfig,
    preserve_exports: bool,
) -> Result<OptimizedJavascriptCandidate, CompileError> {
    let started = Instant::now();
    let mut ir = ir;
    prepare_javascript_ir(&mut ir, config);
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
    let mut optimizer_options =
        crate::decision_registry::scored_ir_optimizer_clones(config, configured);
    let mut compression_contrast = None;
    let mut outline_phase_interaction = None;
    if configured.inlining && config.ir_phase_ordering_variants_enabled() {
        // Phase-order probes answer a narrow question: did an early CSE or the
        // default inlining budget hide a smaller program?  Crossing them with
        // every independent optimizer toggle is both redundant and extremely
        // expensive on large modules.  Keep the configured pipeline and the
        // one phase-adjacent specialization variant that can materially change
        // what the inliner sees.
        let broad_module = ir.functions.len() > 24
            || ir
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .map(|block| block.instructions.len() + block.phis.len() + 1)
                .sum::<usize>()
                > 2_048;
        let mut phase_bases = vec![configured];
        if configured.constant_parameter_specialization {
            let mut without_constant_specialization = configured;
            without_constant_specialization.constant_parameter_specialization = false;
            if broad_module {
                // One combined proposal retains the phase-order opportunity on
                // broad modules without making every final-emission search run
                // six additional times.
                phase_bases.clear();
            }
            phase_bases.push(without_constant_specialization);
        }
        for base in phase_bases {
            if broad_module {
                let mut combined = base;
                combined.common_subexpression_elimination = false;
                combined.inline_instruction_limit = combined.inline_instruction_limit.max(48);
                combined.inline_control_flow_limit = combined.inline_control_flow_limit.max(128);
                combined.inline_growth_limit =
                    Some(combined.inline_growth_limit.unwrap_or(0).max(40));
                if !optimizer_options.contains(&combined) {
                    optimizer_options.push(combined);
                }
                continue;
            }
            let mut without_early_cse = base;
            without_early_cse.common_subexpression_elimination = false;
            if !optimizer_options.contains(&without_early_cse) {
                optimizer_options.push(without_early_cse);
            }

            let mut aggressive_inlining = base;
            aggressive_inlining.inline_instruction_limit =
                aggressive_inlining.inline_instruction_limit.max(48);
            aggressive_inlining.inline_control_flow_limit =
                aggressive_inlining.inline_control_flow_limit.max(128);
            aggressive_inlining.inline_growth_limit =
                Some(aggressive_inlining.inline_growth_limit.unwrap_or(0).max(40));
            if !optimizer_options.contains(&aggressive_inlining) {
                optimizer_options.push(aggressive_inlining);
            }

            aggressive_inlining.common_subexpression_elimination = false;
            if !optimizer_options.contains(&aggressive_inlining) {
                optimizer_options.push(aggressive_inlining);
            }
        }
    }
    if config.javascript_optimization_enabled(JavaScriptOptimization::IrCompressPassVariants) {
        let mut without_compress = configured;
        without_compress.pipeline_fusion = false;
        without_compress.partial_escape_sinking = false;
        without_compress.region_outlining = false;
        without_compress.expression_superopt = false;
        without_compress.path_sensitive_propagation = false;
        without_compress.parameterized_function_merging = false;
        if !optimizer_options.contains(&without_compress) {
            optimizer_options.push(without_compress);
        }
        if configured.region_outlining {
            let mut without_outlining = configured;
            without_outlining.region_outlining = false;
            compression_contrast = Some(without_outlining);
            if !optimizer_options.contains(&without_outlining) {
                optimizer_options.push(without_outlining);
            }
        } else if config.js_region_outlining_candidate_enabled() {
            let mut with_outlining = configured;
            with_outlining.region_outlining = true;
            compression_contrast = Some(with_outlining);
            if !optimizer_options.contains(&with_outlining) {
                optimizer_options.push(with_outlining);
            }

            // Aggressive inlining can expose a repeated composite region that
            // the configured pipeline never presents to the outliner. Cross
            // outlining with only the strongest already-bounded phase-order
            // proposal; duplicating every optimizer tuple would multiply the
            // expensive IR search without adding a distinct proof boundary.
            if let Some(mut interaction) = optimizer_options
                .iter()
                .copied()
                .filter(|options| {
                    options.inlining
                        && !options.region_outlining
                        && !options.common_subexpression_elimination
                        && (options.inline_instruction_limit > configured.inline_instruction_limit
                            || options.inline_control_flow_limit
                                > configured.inline_control_flow_limit
                            || options.inline_growth_limit.unwrap_or(0)
                                > configured.inline_growth_limit.unwrap_or(0))
                })
                .max_by_key(|options| {
                    (
                        !options.constant_parameter_specialization,
                        options.inline_instruction_limit,
                        options.inline_control_flow_limit,
                        options.inline_growth_limit.unwrap_or(0),
                    )
                })
            {
                interaction.region_outlining = true;
                outline_phase_interaction = Some(interaction);
                if !optimizer_options.contains(&interaction) {
                    optimizer_options.push(interaction);
                }
            }
        }
        if configured.pipeline_fusion {
            let mut without_fusion = configured;
            without_fusion.pipeline_fusion = false;
            if !optimizer_options.contains(&without_fusion) {
                optimizer_options.push(without_fusion);
            }
        }
        if configured.parameterized_function_merging {
            let mut without_merging = configured;
            without_merging.parameterized_function_merging = false;
            if !optimizer_options.contains(&without_merging) {
                optimizer_options.push(without_merging);
            }
        }
    }
    optimizer_options.sort_by_key(|options| {
        (
            options.function_subsumption,
            !options.inlining,
            !options.inline_closure_factories,
            options.common_subexpression_elimination,
            !options.constant_parameter_specialization,
            !options.specialize_tagged_constants,
            !options.call_site_specialization,
            !options.capture_signature_cloning,
            options.inline_instruction_limit,
            options.inline_control_flow_limit,
            options.inline_growth_limit,
        )
    });
    optimizer_options.dedup();

    // Probe each optimizer IR with its configured emission before concentrating
    // the terminal beam on the transfer-best finalists (configured always
    // kept). Reusable boundaries receive one additional helper-interaction
    // probe: AllEligible for a successful repeated-region outline, or
    // SingleStaticUse for a deferred-inlining IR. Without that bounded joint
    // score, the useful IR can be discarded before emission search gets a
    // chance to inline its leaves while retaining its shared composite.
    if let Some(configured_index) = optimizer_options
        .iter()
        .position(|options| options == &configured)
    {
        optimizer_options.swap(0, configured_index);
    }
    let total_candidate_limit = config.javascript.effective_candidate_limit().max(1);
    if total_candidate_limit >= 2 {
        if let Some(contrast_index) = compression_contrast.and_then(|contrast| {
            optimizer_options
                .iter()
                .position(|options| *options == contrast)
        }) {
            optimizer_options.swap(1, contrast_index);
        }
    }
    if total_candidate_limit >= 3 {
        if let Some(interaction_index) = outline_phase_interaction.and_then(|interaction| {
            optimizer_options
                .iter()
                .position(|options| *options == interaction)
        }) {
            optimizer_options.swap(2, interaction_index);
        }
    }
    optimizer_options.truncate(total_candidate_limit);

    struct IrProbe<'probe> {
        context_id: usize,
        ir: ControlFlowModule<'probe>,
        optimizer_options: crate::optimizer::OptimizationOptions,
        integer_analysis: Arc<IntegerValueAnalysis>,
        reports: Vec<OptimizationReport>,
        configured_code: String,
        configured: Option<ScoredJavaScriptEmissionSeed>,
        interaction_code: Option<(String, crate::codegen_ir_js::IrJsOptions)>,
        interaction: Option<ScoredJavaScriptEmissionSeed>,
        configured_transfer: usize,
        configured_raw_size: usize,
        transfer: usize,
        raw_size: usize,
    }

    let probes = optimizer_options
        .into_par_iter()
        .enumerate()
        .map(
            |(variant_index, options)| -> Result<(usize, IrProbe<'src>), CompileError> {
                let mut candidate_ir = ir.clone();
                let reports = optimize_control_flow_with_guidance(
                    &mut candidate_ir,
                    &options,
                    preserve_exports,
                    &guidance,
                )?;
                let integer_analysis = Arc::new(analyze_javascript_integer_values(&candidate_ir));
                let configured_js = config.js_options();
                let emitted = emit_javascript_candidate(
                    &candidate_ir,
                    preserve_exports,
                    configured_js,
                    Arc::clone(&integer_analysis),
                )?;
                let outlined = reports.iter().any(|report| {
                    report.pass_name == "repeated-region-outlining" && report.changed
                });
                let interaction_policy = if outlined {
                    Some(crate::codegen_ir_js::PureHelperInliningPolicy::AllEligible)
                } else if !options.inlining {
                    Some(crate::codegen_ir_js::PureHelperInliningPolicy::SingleStaticUse)
                } else {
                    None
                };
                let interaction = if let (true, Some(interaction_policy)) = (
                    total_candidate_limit > 1
                        && !preserve_exports
                        && config.pure_helper_inlining_candidates_enabled(),
                    interaction_policy,
                ) {
                    let options = crate::codegen_ir_js::IrJsOptions {
                        pure_helper_inlining: interaction_policy,
                        inline_single_use_functions: config
                            .single_use_function_expression_candidates_enabled(),
                        ..configured_js
                    };
                    emit_javascript_candidate(
                        &candidate_ir,
                        preserve_exports,
                        options,
                        Arc::clone(&integer_analysis),
                    )
                    .ok()
                    .map(|code| (code, options))
                } else {
                    None
                };
                Ok((
                    variant_index,
                    IrProbe {
                        context_id: variant_index.saturating_mul(2),
                        ir: candidate_ir,
                        optimizer_options: options,
                        integer_analysis,
                        reports,
                        configured_code: emitted,
                        configured: None,
                        interaction_code: interaction,
                        interaction: None,
                        configured_transfer: 0,
                        configured_raw_size: 0,
                        transfer: 0,
                        raw_size: 0,
                    },
                ))
            },
        )
        .collect::<Vec<_>>();

    let mut probes = probes.into_iter();
    let configured_probe = probes
        .next()
        .expect("candidate search always retains the configured optimizer")?
        .1;
    // Context identity, not emitted bytes, owns optimizer provenance. Equal
    // seed bytes can still carry different performance shapes and later emit
    // differently under another structural option, so keep them distinct
    // through the aggregate arena and normalize bytes only at final artifact
    // selection.
    let mut ranked_probes = std::iter::once(configured_probe)
        .chain(probes.flatten().map(|(_, probe)| probe))
        .collect::<Vec<_>>();
    #[derive(Clone, Copy)]
    enum ProbeScoreOwner {
        Configured(usize),
        Interaction(usize, crate::codegen_ir_js::IrJsOptions),
    }
    let mut score_requests = Vec::with_capacity(ranked_probes.len().saturating_mul(2));
    let mut configured_score_failed = vec![false; ranked_probes.len()];
    // The configured root remains the first authoritative request, but its
    // declaration leaves share the flattened codec batch with every optional
    // probe. Results are reconstructed in request order, so scheduling cannot
    // give an optional failure authority over the root.
    validate_direct_javascript_artifact(
        &ranked_probes[0].configured_code,
        &ranked_probes[0].ir,
        config,
        preserve_exports,
    )?;
    score_requests.push(SelectedModelEmissionScoreRequest {
        owner: ProbeScoreOwner::Configured(0),
        code: std::mem::take(&mut ranked_probes[0].configured_code),
        model: config.javascript.cost_model,
        semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
    });
    for (index, probe) in ranked_probes.iter_mut().enumerate() {
        if index != 0 {
            if validate_direct_javascript_artifact(
                &probe.configured_code,
                &probe.ir,
                config,
                preserve_exports,
            )
            .is_ok()
            {
                score_requests.push(SelectedModelEmissionScoreRequest {
                    owner: ProbeScoreOwner::Configured(index),
                    code: std::mem::take(&mut probe.configured_code),
                    model: config.javascript.cost_model,
                    semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
                });
            } else {
                configured_score_failed[index] = true;
                crate::timing::PROBE_DROPPED.record_pass(1, 0);
            }
        }
        if let Some((code, options)) = probe.interaction_code.take() {
            if validate_direct_javascript_artifact(&code, &probe.ir, config, preserve_exports)
                .is_ok()
            {
                score_requests.push(SelectedModelEmissionScoreRequest {
                    owner: ProbeScoreOwner::Interaction(index, options),
                    code,
                    model: config.javascript.cost_model,
                    semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
                });
            }
        }
    }
    let optimizer_emissions_attempted = score_requests.len();
    let score_results = measure_selected_model_emission_batch(score_requests, &[]);
    let (root_emission, score_results) =
        take_required_first_selected_model_emission(score_results, |owner| {
            matches!(owner, ProbeScoreOwner::Configured(0))
        })?;
    let (root_transfer, root_raw_size) = root_emission
        .declaration_scores
        .best_transfer_and_raw(&root_emission.code);
    ranked_probes[0].configured_transfer = root_transfer;
    ranked_probes[0].configured_raw_size = root_raw_size;
    (ranked_probes[0].transfer, ranked_probes[0].raw_size) = (root_transfer, root_raw_size);
    for result in score_results {
        match (result.owner, result.emission) {
            (ProbeScoreOwner::Configured(0), _) => {
                unreachable!("the configured root was consumed before optional probe results")
            }
            (ProbeScoreOwner::Configured(index), Ok(emission)) => {
                let probe = &mut ranked_probes[index];
                let (transfer, raw_size) = emission
                    .declaration_scores
                    .best_transfer_and_raw(&emission.code);
                probe.configured = Some(ScoredJavaScriptEmissionSeed {
                    emission,
                    options: config.js_options(),
                });
                probe.configured_transfer = transfer;
                probe.configured_raw_size = raw_size;
                (probe.transfer, probe.raw_size) = (transfer, raw_size);
            }
            (ProbeScoreOwner::Configured(index), Err(_)) => {
                configured_score_failed[index] = true;
                crate::timing::PROBE_DROPPED.record_pass(1, 0);
            }
            (ProbeScoreOwner::Interaction(index, options), Ok(emission)) => {
                let probe = &mut ranked_probes[index];
                let score = emission
                    .declaration_scores
                    .best_transfer_and_raw(&emission.code);
                probe.interaction = Some(ScoredJavaScriptEmissionSeed { emission, options });
                (probe.transfer, probe.raw_size) = (probe.transfer, probe.raw_size).min(score);
            }
            // Interaction scoring is optional. Its codec failure has no
            // authority over either the selected root or another IR probe.
            (ProbeScoreOwner::Interaction(_, _), Err(_)) => {}
        }
    }
    ranked_probes[0].configured = Some(ScoredJavaScriptEmissionSeed {
        emission: root_emission,
        options: config.js_options(),
    });
    let mut index = 0usize;
    ranked_probes.retain(|_| {
        let keep = !configured_score_failed[index];
        index += 1;
        keep
    });
    if let Some((baseline_transfer, baseline_raw_size)) = ranked_probes
        .first()
        .map(|probe| (probe.configured_transfer, probe.configured_raw_size))
    {
        ranked_probes.retain(|probe| {
            optimizer_variant_candidate_allowed(
                config.javascript.cost_model,
                probe.transfer,
                baseline_transfer,
                probe.raw_size,
                baseline_raw_size,
                config.javascript.max_candidate_raw_growth_percent,
            )
        });
    }
    ranked_probes[1..].sort_by(|left, right| {
        let left_configured = left
            .configured
            .as_ref()
            .expect("ranked probes have a configured scored emission");
        let left_best = left
            .interaction
            .as_ref()
            .filter(|interaction| {
                scored_javascript_seed_rank(interaction)
                    < scored_javascript_seed_rank(left_configured)
            })
            .unwrap_or(left_configured);
        let right_configured = right
            .configured
            .as_ref()
            .expect("ranked probes have a configured scored emission");
        let right_best = right
            .interaction
            .as_ref()
            .filter(|interaction| {
                scored_javascript_seed_rank(interaction)
                    < scored_javascript_seed_rank(right_configured)
            })
            .unwrap_or(right_configured);
        (
            left.transfer,
            left.raw_size,
            left_best.emission.code.as_str(),
            left.context_id,
        )
            .cmp(&(
                right.transfer,
                right.raw_size,
                right_best.emission.code.as_str(),
                right.context_id,
            ))
    });
    let finalist_limit = config
        .javascript
        .effective_candidate_beam_width()
        .min(ranked_probes.len())
        .max(1);
    ranked_probes.truncate(finalist_limit);

    let mut ranked_probes = ranked_probes.into_iter();
    let root_probe = ranked_probes
        .next()
        .expect("candidate search always retains the configured optimizer");
    let root_context_id = root_probe.context_id;
    let root_configured_seed = root_probe
        .configured
        .as_ref()
        .expect("ranked probes have a configured scored emission")
        .clone();
    let candidate_proposal_limit = config
        .javascript
        .effective_candidate_proposal_limit_for_artifact(root_configured_seed.emission.code.len());
    let mut pre_context_proposal_work_units = 0usize;
    let mut pre_context_proposal_limit_reached = false;
    let root_configured_plan = JavaScriptEmissionPlan {
        identity: JavaScriptPlanIdentity {
            context_id: root_context_id,
            ordinal: 0,
        },
        options: root_configured_seed.options,
    };
    let root_candidate = JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
        root_configured_seed.emission.clone(),
        root_configured_plan,
    );
    let mut non_root_probes = ranked_probes.collect::<Vec<_>>();
    let ranked_non_root_pins = non_root_probes
        .iter()
        .map(|probe| {
            let configured = probe
                .configured
                .as_ref()
                .expect("ranked probes have a configured scored emission");
            let seed = probe
                .interaction
                .as_ref()
                .filter(|interaction| {
                    scored_javascript_seed_rank(interaction)
                        < scored_javascript_seed_rank(configured)
                })
                .unwrap_or(configured);
            JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
                seed.emission.clone(),
                JavaScriptEmissionPlan {
                    identity: JavaScriptPlanIdentity {
                        context_id: probe.context_id,
                        ordinal: 0,
                    },
                    options: seed.options,
                },
            )
        })
        .collect::<Vec<_>>();
    let terminal_plan_reserve = if config.javascript.effective_terminal_codec_probe_limit() == 0 {
        0
    } else {
        total_candidate_limit.div_euclid(8).min(4)
    };
    let mut candidate_arena = AggregateJavaScriptPlanArena::new_with_terminal_reserve(
        root_candidate,
        ranked_non_root_pins,
        total_candidate_limit,
        config.javascript.effective_candidate_byte_budget(),
        config.javascript.cost_model,
        terminal_plan_reserve,
    )?;
    let admitted_context_ids = candidate_arena.pinned_context_ids();
    non_root_probes.retain(|probe| admitted_context_ids.contains(&probe.context_id));

    let mut search_contexts = std::iter::once(root_probe)
        .chain(non_root_probes)
        .map(|probe| JavaScriptIrSearchContext {
            id: probe.context_id,
            ir: probe.ir,
            optimizer_options: probe.optimizer_options,
            reports: probe.reports,
            configured_seed: probe
                .configured
                .expect("admitted probes have a configured scored emission"),
            interaction_seed: probe.interaction,
            integer_analysis: Some(probe.integer_analysis),
        })
        .collect::<Vec<_>>();

    if config.js_joint_representation_search_enabled()
        && candidate_arena.optional_proposal_width() != 0
    {
        let optional_raw_size_cap = candidate_arena.optional_raw_size_cap();
        let mut projection_candidates = Vec::new();
        for context in &search_contexts {
            if pre_context_proposal_work_units >= candidate_proposal_limit {
                pre_context_proposal_limit_reached = true;
                break;
            }
            pre_context_proposal_work_units += 1;
            let mut projected_ir = context.ir.clone();
            let projection = project_closed_record_observations_for_javascript(&mut projected_ir);
            if !projection.changed {
                continue;
            }
            let integer_analysis = Arc::new(analyze_javascript_integer_values(&projected_ir));
            let options = config.js_options();
            let Ok(code) = emit_javascript_candidate(
                &projected_ir,
                preserve_exports,
                options,
                Arc::clone(&integer_analysis),
            ) else {
                continue;
            };
            if validate_direct_javascript_artifact(&code, &projected_ir, config, preserve_exports)
                .is_err()
            {
                continue;
            }
            let context_id = context.id.saturating_add(1);
            let plan = JavaScriptEmissionPlan {
                identity: JavaScriptPlanIdentity {
                    context_id,
                    ordinal: 0,
                },
                options,
            };
            let Ok(Some(candidate)) = measure_optional_javascript_candidate(
                code,
                plan,
                config.javascript.cost_model,
                optional_raw_size_cap,
            ) else {
                continue;
            };
            let seed = ScoredJavaScriptEmissionSeed {
                emission: candidate.emission.clone(),
                options,
            };
            let mut reports = context.reports.clone();
            reports.push(projection);
            projection_candidates.push((
                candidate,
                JavaScriptIrSearchContext {
                    id: context_id,
                    ir: projected_ir,
                    optimizer_options: context.optimizer_options,
                    reports,
                    configured_seed: seed,
                    interaction_seed: None,
                    integer_analysis: Some(integer_analysis),
                },
            ));
        }
        projection_candidates
            .sort_by(|(left, _), (right, _)| compare_javascript_seed_admission(left, right));
        let mut projected_contexts = Vec::new();
        for (candidate, context) in projection_candidates {
            if candidate_arena.admit_ranked_pin(candidate)? {
                projected_contexts.push(context);
            }
        }
        search_contexts.extend(projected_contexts);
    }

    let mut remaining_interactions = search_contexts
        .iter_mut()
        .filter_map(|context| {
            let seed = context.interaction_seed.take()?;
            (!candidate_arena.candidates().iter().any(|candidate| {
                candidate.identity().context_id == context.id && candidate.options() == seed.options
            }))
            .then_some((context.id, seed))
        })
        .collect::<Vec<_>>();
    remaining_interactions.sort_by(|(left_context, left), (right_context, right)| {
        (
            scored_javascript_seed_rank(left),
            left.emission.code.as_str(),
            *left_context,
        )
            .cmp(&(
                scored_javascript_seed_rank(right),
                right.emission.code.as_str(),
                *right_context,
            ))
    });
    let contexts = JavaScriptEmissionContexts::new(
        root_context_id,
        search_contexts
            .iter()
            .map(|context| {
                JavaScriptEmissionContext::new(
                    context.id,
                    &context.ir,
                    Some(&context.configured_seed),
                    context.integer_analysis.as_ref().map(Arc::clone),
                    config.javascript_optimization_enabled(
                        JavaScriptOptimization::ConstructorInitializerFusionVariants,
                    ),
                )
            })
            .collect(),
    );
    for candidate in candidate_arena.candidates() {
        let registered = contexts
            .register_plan(candidate.identity().context_id, candidate.options())
            .expect("each admitted context seed is its first registered plan");
        assert_eq!(registered.identity, candidate.identity());
    }
    let priority_family_count =
        priority_candidate_family_count(config, preserve_exports, total_candidate_limit);
    contexts.set_optional_plan_registration_limit(
        candidate_proposal_limit,
        pre_context_proposal_work_units,
        pre_context_proposal_limit_reached,
        if priority_family_count == 0 {
            0
        } else {
            priority_candidate_proposal_reserve(
                candidate_proposal_limit,
                config.javascript.effective_candidate_beam_width(),
                priority_family_count,
            )
        },
        priority_family_count,
    );
    let interaction_candidates = remaining_interactions
        .into_iter()
        .filter_map(|(context_id, seed)| {
            contexts
                .register_plan(context_id, seed.options)
                .map(|plan| {
                    JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
                        seed.emission,
                        plan,
                    )
                })
        })
        .collect::<Vec<_>>();
    if !interaction_candidates.is_empty() {
        candidate_arena.merge_precomputed_optional(interaction_candidates)?;
    }

    let selected = select_javascript_candidate_global(
        &contexts,
        config,
        preserve_exports,
        &profile,
        candidate_arena,
        root_configured_seed.emission.code,
        root_configured_plan.identity,
    )?;
    let selected_context = search_contexts
        .iter()
        .find(|context| context.id == selected.plan_identity.context_id)
        .expect("selected JavaScript plan belongs to an admitted IR context");
    let optimization_reports = selected_context.reports.clone();
    let selected_options = contexts
        .registered_plan_by_identity(selected.plan_identity)
        .expect("selected JavaScript plan remains registered")
        .options;
    let (source_operations, generated_operations) =
        selected_context.ir.operation_provenance_counts();
    let abi_manifest = config
        .javascript_compilation_contract(preserve_exports)
        .abi_manifest(&selected_context.ir);
    // The selected artifact may legitimately contain the class rewrite's own
    // `constructor`; see `validate_observed_javascript_artifact_allowing`. This is
    // the same "re-check an artifact that already won" position as the canonical
    // peephole, so it takes the same opt-in. Admission stays strict everywhere the
    // search is still choosing between candidates.
    selected.admission.validate_selected(&selected.code)?;
    let search_ctx =
        javascript_emission_search_context(config, preserve_exports, total_candidate_limit);
    let scored_emission_families =
        crate::decision_registry::admitted_scored_emission_family_names(&search_ctx)
            .into_iter()
            .map(str::to_string)
            .collect();
    let candidate_proposal_limit_reached = contexts.candidate_proposal_limit_reached();
    let mut starved_emission_families = contexts.starved_emission_families();
    if candidate_proposal_limit_reached
        && contexts.emissions_attempted() == 0
        && starved_emission_families.is_empty()
    {
        starved_emission_families.clone_from(&scored_emission_families);
    }
    let cartesian_emission_axes =
        crate::decision_registry::branching_cartesian_axis_names(&search_ctx)
            .into_iter()
            .map(str::to_string)
            .collect();
    let ir_variants_searched =
        crate::decision_registry::admitted_scored_ir_variant_names(config, configured)
            .into_iter()
            .map(str::to_string)
            .collect();
    let search_stop_reason = if !config.javascript.candidate_search_enabled() {
        "search-disabled"
    } else if candidate_proposal_limit_reached || selected.terminal_codec_probe_limit_reached {
        "work-budget-exhausted"
    } else {
        "portfolio-exhausted"
    };
    Ok(OptimizedJavascriptCandidate {
        javascript: selected.code,
        plan_identity: selected.plan_identity,
        optimization_reports,
        abi_manifest,
        selection_metrics: JavaScriptSelectionMetrics {
            codec: compression_cost_model_name(config.javascript.cost_model).to_string(),
            transfer_bytes: selected.transfer_cost,
            startup_score: selected.startup_score,
            syntax: selected.metrics,
            baseline_syntax: selected.baseline_metrics,
            performance: selected.performance,
            baseline_performance: selected.baseline_performance,
            candidates_evaluated: selected.candidates_evaluated,
            plans_registered: contexts.plans_registered(),
            emissions_attempted: contexts.emissions_attempted(),
            optimizer_emissions_attempted,
            candidate_proposal_limit,
            candidate_proposal_work_units: contexts.candidate_proposal_work_units(),
            candidate_proposal_limit_reached,
            terminal_codec_probes: selected.terminal_codec_probes,
            terminal_work_units: selected.terminal_work_units,
            terminal_codec_probe_limit: selected.terminal_codec_probe_limit,
            terminal_codec_probe_limit_reached: selected.terminal_codec_probe_limit_reached,
            peephole_rewrites: selected.peephole_rewrites,
            decisions: JavaScriptSelectionDecisions {
                explicit_lowering_obligations: selected.has_explicit_lowering_obligations,
                string_pooling: selected_options.pool_strings,
                identifier_string_pooling: selected_options.pool_identifier_strings,
                string_array_packing: selected_options.pack_string_arrays,
                scalar_replacement: selected_context.optimizer_options.scalar_replacement,
                string_pool_minimum_savings: selected_options.string_pool_minimum_savings,
                transitive_nested_shadowing: selected_options.transitive_nested_shadowing,
                precise_cross_scope_shadowing: selected_options.precise_cross_scope_shadowing,
                reserved_local_name_prefix: selected_options.reserved_local_name_prefix,
                local_name_reserve: selected_options.local_name_reserve,
                stable_local_names: selected_options.stable_local_names,
                frequency_order_local_names: selected_options.frequency_order_local_names,
                local_name_coalescing: selected_options.local_name_coalescing,
                length_to_number_elision: selected_options.elide_length_tonumber,
                terminal_scope_naming_challengers: selected.terminal_scope_naming_challengers,
                terminal_scope_naming_selected: selected.terminal_scope_naming_selected,
                terminal_scope_naming_incumbent_bytes: selected
                    .terminal_scope_naming_incumbent_bytes,
                terminal_scope_naming_best_bytes: selected.terminal_scope_naming_best_bytes,
                terminal_string_pooling_challengers: selected.terminal_string_pooling_challengers,
                terminal_string_pooling_selected: selected.terminal_string_pooling_selected,
                terminal_string_pooling_incumbent_bytes: selected
                    .terminal_string_pooling_incumbent_bytes,
                terminal_string_pooling_best_bytes: selected.terminal_string_pooling_best_bytes,
            },
            layout_searched: config.js_joint_representation_search_enabled(),
            removed_compression_families: config
                .javascript
                .removed_size_first_compression_families()
                .into_iter()
                .map(str::to_string)
                .collect(),
            scored_emission_families,
            starved_emission_families,
            cartesian_emission_axes,
            ir_variants_searched,
            source_operations,
            generated_operations,
            decision_registry_version: crate::decision_registry::DECISION_REGISTRY_VERSION,
            search_guarantee: "best-observed".to_string(),
            search_stop_reason: search_stop_reason.to_string(),
            compiler_time_micros: started.elapsed().as_micros(),
        },
    })
}

/// Report the SSA-destruction store census collected during emission.
pub fn store_census() -> [usize; 6] {
    std::array::from_fn(|index| {
        crate::codegen_ir_js::STORE_REASONS[index].load(std::sync::atomic::Ordering::Relaxed)
    })
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
    plan_identity: JavaScriptPlanIdentity,
    code: String,
    transfer_cost: usize,
    baseline_transfer: usize,
    has_explicit_lowering_obligations: bool,
    startup_score: u64,
    metrics: JavaScriptSyntaxMetrics,
    baseline_metrics: JavaScriptSyntaxMetrics,
    performance: JavaScriptPerformanceMetrics,
    baseline_performance: JavaScriptPerformanceMetrics,
    candidates_evaluated: usize,
    terminal_codec_probes: usize,
    terminal_work_units: usize,
    terminal_codec_probe_limit: usize,
    terminal_codec_probe_limit_reached: bool,
    peephole_rewrites: usize,
    terminal_scope_naming_challengers: usize,
    terminal_scope_naming_selected: bool,
    terminal_scope_naming_incumbent_bytes: Option<usize>,
    terminal_scope_naming_best_bytes: Option<usize>,
    terminal_string_pooling_challengers: usize,
    terminal_string_pooling_selected: bool,
    terminal_string_pooling_incumbent_bytes: Option<usize>,
    terminal_string_pooling_best_bytes: Option<usize>,
    admission: Arc<JavaScriptArtifactAdmission>,
}

#[derive(Debug, Clone)]
struct ScoredJavaScriptEmissionSeed {
    emission: ScoredJavaScriptEmission,
    options: crate::codegen_ir_js::IrJsOptions,
}

struct JavaScriptIrSearchContext<'src> {
    id: usize,
    ir: ControlFlowModule<'src>,
    optimizer_options: crate::optimizer::OptimizationOptions,
    reports: Vec<OptimizationReport>,
    configured_seed: ScoredJavaScriptEmissionSeed,
    interaction_seed: Option<ScoredJavaScriptEmissionSeed>,
    integer_analysis: Option<Arc<IntegerValueAnalysis>>,
}

#[cfg(test)]
fn scalar_phi_copy_candidates(config: &ProjectConfig, configured: bool) -> [bool; 2] {
    crate::decision_registry::scalar_phi_copy_candidates(config, configured)
}

#[cfg(test)]
fn phi_affinity_candidates(
    config: &ProjectConfig,
    configured: crate::codegen_ir_js::PhiAffinityMode,
) -> [crate::codegen_ir_js::PhiAffinityMode; 4] {
    crate::decision_registry::phi_affinity_candidates(config, configured)
}

/// Ordinary objects are a smaller representation for records, but only when
/// the program cannot observe the prototype distinction. The proof is
/// artifact-wide and deliberately conservative: surviving record reads and
/// writes, representation-erasing flows, public/host boundaries, and any API
/// whose result can depend on a prototype keep the null-prototype contract.
#[cfg(test)]
fn ir_javascript_ordinary_records_safe(ir: &ControlFlowModule<'_>) -> bool {
    const INHERITED_NAMES: &[&str] = &[
        "__proto__",
        "constructor",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "toLocaleString",
        "toString",
        "valueOf",
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
    ];

    // A reusable module boundary may hand the value to arbitrary JavaScript.
    // Include lazy-module exports: those bindings are just as public as entry
    // exports once their chunk is loaded.
    for export in ir
        .exports
        .iter()
        .chain(ir.lazy_modules.iter().flat_map(|module| &module.exports))
    {
        match export.binding {
            crate::ir::ExportBinding::Function(function) => {
                let Some(function) = ir.functions.get(function.0 as usize) else {
                    return false;
                };
                if function
                    .params
                    .iter()
                    .any(|parameter| ir_type_contains_record(ir, &parameter.ty))
                    || ir_type_contains_record(ir, &function.return_type)
                {
                    return false;
                }
            }
            crate::ir::ExportBinding::Global(symbol) => {
                if ir.globals.iter().any(|global| {
                    global.symbol == symbol && ir_type_contains_record(ir, &global.ty)
                }) {
                    return false;
                }
            }
            crate::ir::ExportBinding::TypeOnly => {}
        }
    }
    // An extern global is already owned by the host realm. Do not mix that
    // unknown representation with an artifact-wide record representation
    // switch.
    if ir
        .globals
        .iter()
        .any(|global| global.external && ir_type_contains_record(ir, &global.ty))
    {
        return false;
    }

    for function in &ir.functions {
        for block in &function.blocks {
            if block.phis.iter().any(|phi| {
                !ir_type_contains_record(ir, &phi.ty)
                    && phi.incoming.iter().any(|(_, incoming)| {
                        function_value_contains_record(ir, function, *incoming)
                    })
            }) {
                return false;
            }
            for instruction in &block.instructions {
                let unsafe_operation = match &instruction.op {
                    ControlFlowOp::Record(entries) => {
                        entries.iter().any(|(key, _)| {
                            let decoded = decode_ir_source_string(key);
                            INHERITED_NAMES.contains(&decoded.as_str())
                        }) || (matches!(instruction.ty.as_ref(), Some(crate::semantic::Type::Record(element)) if !ir_type_contains_record(ir, element))
                            && entries.iter().any(|(_, value)| {
                                function_value_contains_record(ir, function, *value)
                            }))
                    }
                    ControlFlowOp::RecordSpread(operands) => {
                        operands.iter().any(|operand| match operand {
                            crate::ir::RecordOperand::Entry(key, _) => {
                                let decoded = decode_ir_source_string(key);
                                INHERITED_NAMES.contains(&decoded.as_str())
                            }
                            crate::ir::RecordOperand::Spread(_) => false,
                        }) || (matches!(instruction.ty.as_ref(), Some(crate::semantic::Type::Record(element)) if !ir_type_contains_record(ir, element))
                            && operands.iter().any(|operand| match operand {
                                crate::ir::RecordOperand::Entry(_, value) => {
                                    function_value_contains_record(ir, function, *value)
                                }
                                crate::ir::RecordOperand::Spread(_) => false,
                            }))
                    }
                    // Even a non-inherited spelling is not safe without a
                    // per-allocation own-key proof: a missing read can see an
                    // inherited value and a write can invoke an inherited
                    // setter installed by the embedding realm.
                    ControlFlowOp::RecordFieldGet { .. } | ControlFlowOp::RecordFieldSet { .. } => {
                        true
                    }
                    // Reject representation erasure into `JsValue`-typed
                    // storage. Otherwise a later consumer no longer carries
                    // enough type information to prove the prototype hidden.
                    ControlFlowOp::StoreLocal { local, value }
                        if function_value_contains_record(ir, function, *value)
                            && function.locals.iter().any(|candidate| {
                                candidate.id == *local
                                    && !ir_type_contains_record(ir, &candidate.ty)
                            }) =>
                    {
                        true
                    }
                    ControlFlowOp::StoreGlobal { global, value }
                        if function_value_contains_record(ir, function, *value)
                            && ir.globals.iter().any(|candidate| {
                                candidate.symbol == *global
                                    && !ir_type_contains_record(ir, &candidate.ty)
                            }) =>
                    {
                        true
                    }
                    ControlFlowOp::Array(values)
                        if instruction
                            .ty
                            .as_ref()
                            .is_some_and(|ty| !ir_type_contains_record(ir, ty))
                            && values.iter().any(|value| {
                                function_value_contains_record(ir, function, *value)
                            }) =>
                    {
                        true
                    }
                    ControlFlowOp::ArraySpread(operands)
                        if instruction
                            .ty
                            .as_ref()
                            .is_some_and(|ty| !ir_type_contains_record(ir, ty))
                            && operands.iter().any(|operand| {
                                let value = match operand {
                                    crate::ir::ArrayOperand::Value(value)
                                    | crate::ir::ArrayOperand::Spread(value) => *value,
                                };
                                function_value_contains_record(ir, function, value)
                            }) =>
                    {
                        true
                    }
                    ControlFlowOp::Struct { fields, .. }
                        if instruction
                            .ty
                            .as_ref()
                            .is_some_and(|ty| !ir_type_contains_record(ir, ty))
                            && fields.iter().any(|value| {
                                function_value_contains_record(ir, function, *value)
                            }) =>
                    {
                        true
                    }
                    ControlFlowOp::Closure { captures, .. }
                        if captures
                            .iter()
                            .any(|value| function_value_contains_record(ir, function, *value)) =>
                    {
                        true
                    }
                    ControlFlowOp::NewClass { args, .. }
                        if args
                            .iter()
                            .any(|value| function_value_contains_record(ir, function, *value)) =>
                    {
                        true
                    }
                    // The index is a runtime string. Without a closed key set,
                    // it can be `toString` or `__proto__`.
                    ControlFlowOp::IndexGet { object, .. }
                        if function_value_contains_record(ir, function, *object) =>
                    {
                        true
                    }
                    ControlFlowOp::IndexSet { object, value, .. }
                        if function_value_contains_record(ir, function, *object)
                            || function_value_contains_record(ir, function, *value) =>
                    {
                        true
                    }
                    // Object.assign uses [[Set]] on its target, so an own
                    // `__proto__` source key is observably different for an
                    // ordinary target. Proving source key sets is deliberately
                    // left for a future data-flow analysis.
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::RecordAssign,
                        ..
                    } => true,
                    ControlFlowOp::CallDirect {
                        function: target,
                        args,
                        ..
                    } if ir.functions.get(target.0 as usize).is_none_or(|callee| {
                        args.iter().enumerate().any(|(index, argument)| {
                            function_value_contains_record(ir, function, *argument)
                                && (callee.kind == crate::ir::FunctionKind::Extern
                                    || callee.params.get(index).is_none_or(|parameter| {
                                        !ir_type_contains_record(ir, &parameter.ty)
                                    }))
                        })
                    }) =>
                    {
                        true
                    }
                    ControlFlowOp::CallValue { callee, args } => {
                        function_value_contains_record(ir, function, *callee)
                            || args.iter().any(|argument| {
                                function_value_contains_record(ir, function, *argument)
                            })
                    }
                    ControlFlowOp::HostCall { receiver, args, .. } => {
                        function_value_contains_record(ir, function, *receiver)
                            || args.iter().any(|argument| {
                                function_value_contains_record(ir, function, *argument)
                            })
                    }
                    ControlFlowOp::CallMethod { receiver, args, .. } => {
                        function_value_contains_record(ir, function, *receiver)
                            || args.iter().any(|argument| {
                                function_value_contains_record(ir, function, *argument)
                            })
                    }
                    ControlFlowOp::HostFieldGet { object, .. } => {
                        function_value_contains_record(ir, function, *object)
                    }
                    ControlFlowOp::HostFieldSet { object, value, .. } => {
                        function_value_contains_record(ir, function, *object)
                            || function_value_contains_record(ir, function, *value)
                    }
                    // These operations inspect only own keys or identity and
                    // therefore cannot distinguish the prototype. Every other
                    // intrinsic is rejected when a record (including one
                    // nested in a collection) reaches it. In particular this
                    // covers JSON.stringify's inherited `toJSON` lookup,
                    // print/host inspection, JS.get/in/has/prototype, and
                    // Object.assign's inherited-setter behavior.
                    ControlFlowOp::Intrinsic {
                        intrinsic:
                            Intrinsic::RecordKeys
                            | Intrinsic::RecordValues
                            | Intrinsic::RecordHasOwn
                            | Intrinsic::JsStrictEqual
                            | Intrinsic::JsStrictNotEqual
                            | Intrinsic::JsTruthy
                            | Intrinsic::JsIsNullish
                            | Intrinsic::JsIsUndefined
                            | Intrinsic::JsIsObject
                            | Intrinsic::UnwrapNullable
                            | Intrinsic::UnwrapUnion,
                        ..
                    } => false,
                    ControlFlowOp::Intrinsic { receiver, args, .. } => receiver
                        .iter()
                        .chain(args)
                        .any(|value| function_value_contains_record(ir, function, *value)),
                    ControlFlowOp::TypeCheck { value, .. } => {
                        function_value_contains_record(ir, function, *value)
                    }
                    _ => false,
                };
                if unsafe_operation {
                    return false;
                }
            }
            match block.terminator {
                Some(crate::ir::Terminator::Return(Some(value)))
                    if function.kind != crate::ir::FunctionKind::Entry
                        && function_value_contains_record(ir, function, value) =>
                {
                    return false;
                }
                Some(crate::ir::Terminator::Throw(value))
                    if function_value_contains_record(ir, function, value) =>
                {
                    return false;
                }
                _ => {}
            }
        }
    }
    true
}

#[cfg(test)]
fn decode_ir_source_string(value: &str) -> String {
    serde_json::from_str(&format!("\"{value}\"")).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
fn function_value_contains_record(
    ir: &ControlFlowModule<'_>,
    function: &crate::ir::ControlFlowFunction<'_>,
    value: crate::ir::ValueId,
) -> bool {
    function
        .params
        .iter()
        .any(|parameter| parameter.value == value && ir_type_contains_record(ir, &parameter.ty))
        || function.blocks.iter().any(|block| {
            block
                .phis
                .iter()
                .any(|phi| phi.out == value && ir_type_contains_record(ir, &phi.ty))
                || block.instructions.iter().any(|instruction| {
                    instruction.out == Some(value)
                        && instruction
                            .ty
                            .as_ref()
                            .is_some_and(|ty| ir_type_contains_record(ir, ty))
                })
        })
}

#[cfg(test)]
fn ir_type_contains_record(ir: &ControlFlowModule<'_>, ty: &crate::semantic::Type<'_>) -> bool {
    fn visit(
        ir: &ControlFlowModule<'_>,
        ty: &crate::semantic::Type<'_>,
        visiting: &mut Vec<String>,
    ) -> bool {
        use crate::semantic::Type;
        match ty {
            Type::Record(_) => true,
            Type::Array(value)
            | Type::Set(value)
            | Type::Task(value)
            | Type::Generator(value)
            | Type::Nullable(value) => visit(ir, value, visiting),
            Type::Map(key, value) => visit(ir, key, visiting) || visit(ir, value, visiting),
            Type::Union(members) => members.iter().any(|member| visit(ir, member, visiting)),
            Type::Function(signature) => {
                signature
                    .params
                    .iter()
                    .any(|parameter| visit(ir, parameter, visiting))
                    || visit(ir, &signature.return_type, visiting)
            }
            Type::GenericFunction(function) => {
                function
                    .signature
                    .params
                    .iter()
                    .any(|parameter| visit(ir, parameter, visiting))
                    || visit(ir, &function.signature.return_type, visiting)
            }
            Type::Struct(name)
            | Type::Class(name)
            | Type::StructInstance { name, .. }
            | Type::ClassInstance { name, .. } => {
                if visiting.iter().any(|current| current == name) {
                    return false;
                }
                let Some(layout) = ir
                    .structs
                    .iter()
                    .chain(&ir.classes)
                    .find(|layout| layout.name == *name)
                else {
                    return matches!(
                        ty,
                        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. }
                            if args.iter().any(|argument| visit(ir, argument, visiting))
                    );
                };
                visiting.push((*name).to_string());
                let contains = layout
                    .fields
                    .iter()
                    .any(|field| visit(ir, &field.ty, visiting));
                visiting.pop();
                contains
                    || matches!(
                        ty,
                        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. }
                            if args.iter().any(|argument| visit(ir, argument, visiting))
                    )
            }
            _ => false,
        }
    }

    visit(ir, ty, &mut Vec::new())
}

struct JavaScriptEmissionContext<'ir, 'src> {
    id: usize,
    baseline: &'ir ControlFlowModule<'src>,
    configured_seed: Option<&'ir ScoredJavaScriptEmissionSeed>,
    baseline_integer_analysis: OnceLock<Arc<IntegerValueAnalysis>>,
    constructor_fused: OnceLock<Option<(ControlFlowModule<'src>, Arc<IntegerValueAnalysis>)>>,
    enable_constructor_fusion: bool,
}

impl<'ir, 'src> JavaScriptEmissionContext<'ir, 'src> {
    fn new(
        id: usize,
        baseline: &'ir ControlFlowModule<'src>,
        configured_seed: Option<&'ir ScoredJavaScriptEmissionSeed>,
        baseline_integer_analysis: Option<Arc<IntegerValueAnalysis>>,
        enable_constructor_fusion: bool,
    ) -> Self {
        let analysis = OnceLock::new();
        if let Some(baseline_integer_analysis) = baseline_integer_analysis {
            analysis
                .set(baseline_integer_analysis)
                .expect("a new JavaScript emission context has an empty analysis cell");
        }
        Self {
            id,
            baseline,
            configured_seed,
            baseline_integer_analysis: analysis,
            constructor_fused: OnceLock::new(),
            enable_constructor_fusion,
        }
    }

    fn baseline_integer_analysis(&self) -> Arc<IntegerValueAnalysis> {
        Arc::clone(
            self.baseline_integer_analysis
                .get_or_init(|| Arc::new(analyze_javascript_integer_values(self.baseline))),
        )
    }

    fn constructor_fused(&self) -> Option<&(ControlFlowModule<'src>, Arc<IntegerValueAnalysis>)> {
        self.constructor_fused
            .get_or_init(|| {
                self.enable_constructor_fusion.then(|| {
                    let mut projected = (*self.baseline).clone();
                    let report = crate::optimizer::project_direct_constructor_initializers_for_javascript(
                        &mut projected,
                    );
                    report
                        .changed
                        .then(|| Arc::new(analyze_javascript_integer_values(&projected)))
                        .map(|analysis| (projected, analysis))
                })
                .flatten()
            })
            .as_ref()
    }

    fn emit(
        &self,
        module_output: bool,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Result<String, crate::codegen_js::CodegenError> {
        let (ir, integer_analysis) = if options.constructor_initializer_fusion {
            self.constructor_fused()
                .map(|(ir, analysis)| (ir, Arc::clone(analysis)))
                .unwrap_or_else(|| (self.baseline, self.baseline_integer_analysis()))
        } else {
            (self.baseline, self.baseline_integer_analysis())
        };
        emit_javascript_candidate(ir, module_output, options, integer_analysis)
    }
}

struct JavaScriptEmissionContexts<'ir, 'src> {
    root_configured_context_id: usize,
    contexts: Vec<JavaScriptEmissionContext<'ir, 'src>>,
    plan_registry: Mutex<JavaScriptPlanRegistry>,
    emissions_attempted: AtomicUsize,
}

impl<'ir, 'src> JavaScriptEmissionContexts<'ir, 'src> {
    #[cfg(test)]
    fn single(context: JavaScriptEmissionContext<'ir, 'src>) -> Self {
        Self::new(context.id, vec![context])
    }

    fn new(
        root_configured_context_id: usize,
        contexts: Vec<JavaScriptEmissionContext<'ir, 'src>>,
    ) -> Self {
        Self {
            root_configured_context_id,
            contexts,
            plan_registry: Mutex::new(JavaScriptPlanRegistry::default()),
            emissions_attempted: AtomicUsize::new(0),
        }
    }

    fn get(&self, context_id: usize) -> &JavaScriptEmissionContext<'ir, 'src> {
        self.contexts
            .iter()
            .find(|context| context.id == context_id)
            .unwrap_or_else(|| panic!("JavaScript plan references unknown context {context_id}"))
    }

    fn root(&self) -> &JavaScriptEmissionContext<'ir, 'src> {
        self.get(self.root_configured_context_id)
    }

    fn context_ids(&self) -> Vec<usize> {
        self.contexts.iter().map(|context| context.id).collect()
    }

    fn emit(
        &self,
        context_id: usize,
        module_output: bool,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Result<String, crate::codegen_js::CodegenError> {
        self.emissions_attempted.fetch_add(1, Ordering::Relaxed);
        self.get(context_id).emit(module_output, options)
    }

    fn plans_registered(&self) -> usize {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .plans
            .len()
    }

    /// Freeze a total registry ceiling relative to the already-installed,
    /// scored context seeds. Every newly registered structural identity
    /// consumes one slot before emission, validation, or codec work, including
    /// identities whose eventual emission fails or is rejected.
    fn set_optional_plan_registration_limit(
        &self,
        optional_limit: usize,
        already_used: usize,
        limit_reached: bool,
        priority_reserve: usize,
        priority_family_count: usize,
    ) {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .set_optional_limit(
                optional_limit,
                already_used,
                limit_reached,
                priority_reserve,
                priority_family_count,
            );
    }

    fn candidate_proposal_limit_reached(&self) -> bool {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .limit_reached
    }

    fn candidate_proposal_work_units(&self) -> usize {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .structural_work_used
    }

    fn begin_scored_family(&self, name: &'static str) {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .begin_scored_family(name);
    }

    fn mark_active_scored_family_starved(&self) {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .mark_active_scored_family_starved();
    }

    fn end_scored_family(&self) {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .end_scored_family();
    }

    fn starved_emission_families(&self) -> Vec<String> {
        let mut families = self
            .plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .starved_scored_families
            .iter()
            .map(|family| (*family).to_string())
            .collect::<Vec<_>>();
        families.sort();
        families.dedup();
        families
    }

    fn reserve_candidate_proposal_work(&self, requested: usize) -> usize {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .reserve_structural_work(requested)
    }

    fn remaining_candidate_proposal_work(&self) -> usize {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .remaining_structural_work()
    }

    fn emissions_attempted(&self) -> usize {
        self.emissions_attempted.load(Ordering::Relaxed)
    }

    fn register_plan(
        &self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .register(context_id, options)
    }

    /// Admit one of the small, authoritative late structural coordinates from
    /// the protected slice of the same compilation-wide proposal ledger.
    /// These plans still count against the hard total cap; the reserve only
    /// prevents broad early Cartesian families from consuming every permit.
    fn begin_priority_plan_family(&self) -> usize {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .begin_priority_family()
    }

    fn end_priority_plan_family(&self) {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .end_priority_family();
    }

    fn finish_priority_plan_families(&self) {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .finish_priority_families();
    }

    fn register_priority_plan(
        &self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .register_priority(context_id, options)
    }

    /// Terminal factored challengers have their own plan/byte and work
    /// reserves. Let them register after the structural proposal ledger is
    /// full so a broad early cross product cannot starve the valuable naming
    /// or declaration tail.
    fn register_terminal_plan(
        &self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .register_terminal(context_id, options)
    }

    fn registered_plan(
        &self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .find(context_id, options)
    }

    fn registered_plan_by_identity(
        &self,
        identity: JavaScriptPlanIdentity,
    ) -> Option<JavaScriptEmissionPlan> {
        self.plan_registry
            .lock()
            .expect("JavaScript plan registry lock is not poisoned")
            .plans
            .iter()
            .copied()
            .find(|plan| plan.identity == identity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct JavaScriptPlanIdentity {
    context_id: usize,
    ordinal: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JavaScriptEmissionPlan {
    identity: JavaScriptPlanIdentity,
    options: crate::codegen_ir_js::IrJsOptions,
}

struct JavaScriptPlanRegistry {
    plans: Vec<JavaScriptEmissionPlan>,
    structural_work_limit: usize,
    structural_work_used: usize,
    priority_work_reserve: usize,
    priority_families_remaining: usize,
    active_priority_family_remaining: Option<usize>,
    active_scored_family: Option<&'static str>,
    starved_scored_families: Vec<&'static str>,
    limit_reached: bool,
}

impl Default for JavaScriptPlanRegistry {
    fn default() -> Self {
        Self {
            plans: Vec::new(),
            structural_work_limit: usize::MAX,
            structural_work_used: 0,
            priority_work_reserve: 0,
            priority_families_remaining: 0,
            active_priority_family_remaining: None,
            active_scored_family: None,
            starved_scored_families: Vec::new(),
            limit_reached: false,
        }
    }
}

impl JavaScriptPlanRegistry {
    fn set_optional_limit(
        &mut self,
        optional_limit: usize,
        already_used: usize,
        limit_reached: bool,
        priority_reserve: usize,
        priority_family_count: usize,
    ) {
        self.structural_work_limit = optional_limit;
        self.structural_work_used = already_used.min(optional_limit);
        self.priority_work_reserve = priority_reserve.min(
            self.structural_work_limit
                .saturating_sub(self.structural_work_used),
        );
        self.priority_families_remaining = priority_family_count;
        self.active_priority_family_remaining = None;
        self.active_scored_family = None;
        self.starved_scored_families.clear();
        self.limit_reached = limit_reached || already_used > optional_limit;
    }

    fn reserve_structural_work(&mut self, requested: usize) -> usize {
        let remaining = self.remaining_structural_work();
        let admitted = requested.min(remaining);
        self.structural_work_used = self.structural_work_used.saturating_add(admitted);
        self.limit_reached |= admitted < requested;
        if admitted < requested {
            self.mark_active_scored_family_starved();
        }
        admitted
    }

    fn remaining_structural_work(&self) -> usize {
        self.structural_work_limit
            .saturating_sub(self.structural_work_used)
            .saturating_sub(self.priority_work_reserve)
    }

    fn reserve_priority_work(&mut self, requested: usize) -> usize {
        let remaining = self
            .structural_work_limit
            .saturating_sub(self.structural_work_used);
        let admitted = requested.min(remaining);
        self.structural_work_used = self.structural_work_used.saturating_add(admitted);
        self.priority_work_reserve = self.priority_work_reserve.saturating_sub(admitted);
        self.limit_reached |= admitted < requested;
        if admitted < requested {
            self.mark_active_scored_family_starved();
        }
        admitted
    }

    fn begin_scored_family(&mut self, name: &'static str) {
        debug_assert!(self.active_scored_family.is_none());
        self.active_scored_family = Some(name);
    }

    fn mark_active_scored_family_starved(&mut self) {
        if let Some(family) = self.active_scored_family {
            if !self.starved_scored_families.contains(&family) {
                self.starved_scored_families.push(family);
            }
        }
    }

    fn end_scored_family(&mut self) {
        self.active_scored_family = None;
    }

    fn begin_priority_family(&mut self) -> usize {
        debug_assert!(self.active_priority_family_remaining.is_none());
        if self.priority_families_remaining == 0 {
            self.active_priority_family_remaining = Some(0);
            return 0;
        }
        let allowance = self
            .priority_work_reserve
            .div_ceil(self.priority_families_remaining);
        self.priority_families_remaining -= 1;
        self.active_priority_family_remaining = Some(allowance);
        if allowance == 0 {
            self.mark_active_scored_family_starved();
        }
        allowance
    }

    fn end_priority_family(&mut self) {
        self.active_priority_family_remaining = None;
        if self.priority_families_remaining == 0 {
            self.priority_work_reserve = 0;
        }
    }

    fn finish_priority_families(&mut self) {
        self.priority_families_remaining = 0;
        self.active_priority_family_remaining = None;
        self.priority_work_reserve = 0;
    }

    fn find(
        &self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        self.plans
            .iter()
            .copied()
            .find(|plan| plan.identity.context_id == context_id && plan.options == options)
    }

    fn register(
        &mut self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        if self.find(context_id, options).is_some() {
            return None;
        }
        if self.reserve_structural_work(1) == 0 {
            return None;
        }
        self.register_terminal(context_id, options)
    }

    fn register_terminal(
        &mut self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        if self.find(context_id, options).is_some() {
            return None;
        }
        let ordinal = self
            .plans
            .iter()
            .filter(|plan| plan.identity.context_id == context_id)
            .count();
        let plan = JavaScriptEmissionPlan {
            identity: JavaScriptPlanIdentity {
                context_id,
                ordinal,
            },
            options,
        };
        self.plans.push(plan);
        Some(plan)
    }

    fn register_priority(
        &mut self,
        context_id: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> Option<JavaScriptEmissionPlan> {
        if self.find(context_id, options).is_some() {
            return None;
        }
        if self
            .active_priority_family_remaining
            .is_some_and(|remaining| remaining == 0)
        {
            return None;
        }
        if self.reserve_priority_work(1) == 0 {
            return None;
        }
        if let Some(remaining) = &mut self.active_priority_family_remaining {
            *remaining = remaining.saturating_sub(1);
        }
        self.register_terminal(context_id, options)
    }
}

fn javascript_emission_search_context<'a>(
    config: &'a ProjectConfig,
    module_output: bool,
    candidate_limit: usize,
) -> crate::decision_registry::EmissionSearchContext<'a> {
    let candidate_beam_width = config.javascript.effective_candidate_beam_width();
    crate::decision_registry::EmissionSearchContext {
        config,
        configured: config.js_options(),
        module_output,
        candidate_limit,
        candidate_beam_width,
        narrow_candidate_beam_width: candidate_beam_width.saturating_mul(2).div_ceil(3),
        family_candidate_beam_width: candidate_beam_width.div_ceil(3),
        codec_history_window: codec_history_window(config.javascript.cost_model),
        declaration_variant_cap: MAX_DECLARATION_VARIANTS,
    }
}

fn priority_candidate_proposal_reserve(
    limit: usize,
    beam_width: usize,
    priority_family_count: usize,
) -> usize {
    limit
        .div_ceil(3)
        .min(beam_width.saturating_mul(priority_family_count))
        .min(limit)
}

fn priority_candidate_family_count(
    config: &ProjectConfig,
    module_output: bool,
    candidate_limit: usize,
) -> usize {
    crate::decision_registry::priority_scored_family_count(&javascript_emission_search_context(
        config,
        module_output,
        candidate_limit,
    ))
}

#[cfg(test)]
fn resolve_configured_javascript_emission<Error>(
    seed: Option<String>,
    emit: impl FnOnce() -> Result<String, Error>,
) -> Result<String, Error> {
    match seed {
        Some(code) => Ok(code),
        None => emit(),
    }
}

#[cfg(test)]
fn javascript_projection_can_compete(
    config: &ProjectConfig,
    candidate_budget: usize,
    byte_budget: usize,
) -> bool {
    config.js_joint_representation_search_enabled() && candidate_budget >= 2 && byte_budget >= 2
}

fn scored_javascript_seed_rank(seed: &ScoredJavaScriptEmissionSeed) -> (usize, usize) {
    seed.emission
        .declaration_scores
        .best_transfer_and_raw(&seed.emission.code)
}

fn compare_javascript_seed_admission(
    left: &JavaScriptEmissionCandidate,
    right: &JavaScriptEmissionCandidate,
) -> std::cmp::Ordering {
    (
        left.transfer_cost,
        left.raw_size,
        left.code(),
        left.identity().context_id,
    )
        .cmp(&(
            right.transfer_cost,
            right.raw_size,
            right.code(),
            right.identity().context_id,
        ))
}

fn select_javascript_candidate_global(
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    config: &ProjectConfig,
    module_output: bool,
    profile: &OptimizationProfile,
    mut candidates: AggregateJavaScriptPlanArena,
    configured_baseline: String,
    configured_plan_identity: JavaScriptPlanIdentity,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    let configured = config.js_options();
    if !config.javascript.candidate_search_enabled() {
        return finalize_javascript_candidates(
            candidates.into_candidates(),
            &configured_baseline,
            configured_plan_identity,
            config,
            contexts,
            profile,
            usize::MAX,
        );
    }
    let candidate_limit = candidates.effective_plan_count_cap;
    // Pinned context seeds already occupy their aggregate bytes and slots. If
    // the frozen optional capacity is empty, no later structural proposal can
    // survive, so finalize the exact retained plans without analysis/emission.
    if candidates.optional_proposal_width() == 0 {
        return finalize_javascript_candidates(
            candidates.into_candidates(),
            &configured_baseline,
            configured_plan_identity,
            config,
            contexts,
            profile,
            candidate_limit,
        );
    }
    let optional_raw_size_cap = candidates.optional_raw_size_cap();
    let ir = contexts.root().baseline;
    let integer_analysis = contexts;
    let search = javascript_emission_search_context(config, module_output, candidate_limit);
    let mut options = crate::decision_registry::cartesian_emission_seeds(&search);
    let candidate_beam_width = search.candidate_beam_width;
    let beam_policy = JavaScriptCandidateBeamPolicy {
        cost_model: config.javascript.cost_model,
    };
    // The initial option cross product used to be emitted in full before its
    // results were truncated to `candidate_limit`. On a broad numeric module,
    // five binary decisions, three phi modes, two alphabets, two quote styles,
    // and top-level declaration alternatives made a nominal 384-candidate
    // production search perform thousands of complete emissions and Brotli-11
    // probes. Bound proposals before emission as well as after scoring.
    let alphabet_variants =
        usize::from(configured.mangle_identifiers && config.entropy_aware_mangling_enabled()) + 1;
    let quote_variants = usize::from(config.quote_style_selection_enabled()).saturating_mul(2) + 1;
    let maximum_spelling_variants = alphabet_variants
        .saturating_mul(quote_variants)
        .saturating_mul(MAX_DECLARATION_VARIANTS);
    let proposal_limit = candidates
        .optional_proposal_width()
        .div_ceil(maximum_spelling_variants)
        .max(1);
    if options.len() > proposal_limit {
        // Enumeration order is not an optimization policy. Always retain the
        // configured spelling and sample the remaining cross product across
        // its full range; prefix truncation otherwise over-represents early
        // booleans and can omit every safe SSA-destruction mode.
        let configured_position = options
            .iter()
            .position(|options| *options == configured)
            .unwrap_or(0);
        options.swap(0, configured_position);
        let remaining = options.len() - 1;
        let extras = proposal_limit.saturating_sub(1);
        let mut sampled = Vec::with_capacity(options.len().min(proposal_limit));
        sampled.push(options[0]);
        if extras == 1 {
            sampled.push(options[1 + remaining.saturating_sub(1)]);
        } else if extras > 1 {
            for sample in 0..extras {
                let index = 1 + sample
                    .saturating_mul(remaining.saturating_sub(1))
                    .checked_div(extras - 1)
                    .unwrap_or(0);
                let candidate = options[index.min(options.len() - 1)];
                if !sampled.contains(&candidate) {
                    sampled.push(candidate);
                }
            }
        }
        options = sampled;
    }
    let base_requests = bounded_javascript_variant_options(
        contexts
            .context_ids()
            .into_iter()
            .map(|context_id| {
                options
                    .iter()
                    .copied()
                    .filter(|options| contexts.registered_plan(context_id, *options).is_none())
                    .map(|options| (context_id, options))
                    .collect::<Vec<_>>()
            })
            .collect(),
        proposal_limit,
    );
    // Identities are assigned on this coordinator before any parallel work.
    // A configured seed carries its exact score ledger with its bytes, so no
    // re-emission or second codec pass is needed when that option is sampled.
    let mut base_plans = Vec::new();
    for (context_id, options) in base_requests {
        let context = contexts.get(context_id);
        let seed = context
            .configured_seed
            .filter(|seed| seed.options == options)
            .map(|seed| seed.emission.clone());
        if seed.as_ref().is_some_and(|emission| {
            emission.declaration_scores.model != config.javascript.cost_model
        }) {
            return Err(crate::codegen_js::CodegenError::new(
                Span::empty(0),
                "configured JavaScript seed uses a different codec score ledger",
            )
            .into());
        }
        if let Some(plan) = contexts.registered_plan(context_id, options) {
            let retained = candidates
                .candidates()
                .iter()
                .find(|candidate| candidate.identity() == plan.identity)
                .map(|candidate| candidate.emission.clone());
            if let Some(emission) = retained.or(seed) {
                base_plans.push((plan, Some(emission)));
            }
            continue;
        }
        if let Some(plan) = contexts.register_plan(context_id, options) {
            base_plans.push((plan, seed));
        }
    }
    let mut baseline_results = candidates
        .candidates()
        .iter()
        .map(|candidate| (candidate.plan, candidate.emission.clone()))
        .collect::<Vec<_>>();
    baseline_results.extend(
        base_plans
            .into_par_iter()
            .filter_map(|(plan, seed)| {
                let emission = match seed {
                    Some(emission) => emission,
                    None => {
                        let code = contexts
                            .emit(plan.identity.context_id, module_output, plan.options)
                            .ok()?;
                        validate_direct_javascript_artifact(
                            &code,
                            contexts.get(plan.identity.context_id).baseline,
                            config,
                            module_output,
                        )
                        .ok()?;
                        measure_initial_javascript_emission(code, config.javascript.cost_model)
                            .ok()?
                    }
                };
                Some((plan, emission))
            })
            .collect::<Vec<_>>(),
    );
    let spelling_groups = baseline_results
        .iter()
        .map(|(plan, emission)| {
            let options = plan.options;
            let mut alphabets = vec![options.identifier_alphabet];
            if options.mangle_identifiers && config.entropy_aware_mangling_enabled() {
                let frequency = crate::codegen_ir_js::IdentifierAlphabet::for_code(&emission.code);
                if !alphabets.contains(&frequency) {
                    alphabets.push(frequency);
                }
                if let Ok(binding_characters) =
                    declared_identifier_character_use_counts(&emission.code)
                {
                    let contextual = crate::codegen_ir_js::IdentifierAlphabet::
                        for_code_excluding_binding_characters(
                            &emission.code,
                            &binding_characters,
                        );
                    if !alphabets.contains(&contextual) {
                        alphabets.push(contextual);
                    }
                }
                let keyword = crate::codegen_ir_js::IdentifierAlphabet::javascript_keyword();
                if !alphabets.contains(&keyword) {
                    alphabets.push(keyword);
                }
            }
            let mut family = Vec::new();
            for identifier_alphabet in alphabets {
                let mut quotes = vec![options.string_quote];
                if config.quote_style_selection_enabled()
                    && !quotes.contains(&crate::codegen_ir_js::StringQuote::Single)
                {
                    quotes.push(crate::codegen_ir_js::StringQuote::Single);
                }
                if config.quote_style_selection_enabled()
                    && !quotes.contains(&crate::codegen_ir_js::StringQuote::Template)
                {
                    quotes.push(crate::codegen_ir_js::StringQuote::Template);
                }
                family.extend(quotes.into_iter().filter_map(|string_quote| {
                    let candidate_options = crate::codegen_ir_js::IrJsOptions {
                        identifier_alphabet,
                        string_quote,
                        ..options
                    };
                    match contexts.registered_plan(plan.identity.context_id, candidate_options) {
                        Some(registered)
                            if candidates
                                .candidates()
                                .iter()
                                .any(|candidate| candidate.identity() == registered.identity) =>
                        {
                            None
                        }
                        Some(registered)
                            if !baseline_results.iter().any(|(baseline_plan, _)| {
                                baseline_plan.identity == registered.identity
                            }) =>
                        {
                            None
                        }
                        _ => Some((plan.identity.context_id, candidate_options)),
                    }
                }));
            }
            family
        })
        .collect::<Vec<_>>();
    let spelling_requests =
        bounded_javascript_variant_options(spelling_groups, candidates.optional_proposal_width());
    let mut spelling_plans = Vec::new();
    for (context_id, options) in spelling_requests {
        let registered = contexts.registered_plan(context_id, options);
        if registered.is_some_and(|plan| {
            candidates
                .candidates()
                .iter()
                .any(|candidate| candidate.identity() == plan.identity)
        }) {
            continue;
        }
        let plan = match registered {
            Some(plan) => plan,
            None => {
                let Some(plan) = contexts.register_plan(context_id, options) else {
                    continue;
                };
                plan
            }
        };
        let emission = baseline_results
            .iter()
            .find(|(baseline_plan, _)| {
                baseline_plan.identity.context_id == context_id && baseline_plan.options == options
            })
            .map(|(_, emission)| emission.clone());
        // A previously attempted identity that is no longer retained is not
        // re-run. This keeps search identity `(context, options)` exact.
        if registered.is_some() && emission.is_none() {
            continue;
        }
        spelling_plans.push((plan, emission));
    }
    let spelling_candidates = spelling_plans
        .into_par_iter()
        .filter_map(|(plan, emission)| {
            let candidate = match emission {
                Some(emission) => {
                    if emission.code.len() > optional_raw_size_cap {
                        return None;
                    }
                    JavaScriptEmissionCandidate::new_declaration_plan_with_scores(emission, plan)
                }
                None => {
                    let code = contexts
                        .emit(plan.identity.context_id, module_output, plan.options)
                        .ok()?;
                    validate_direct_javascript_artifact(
                        &code,
                        contexts.get(plan.identity.context_id).baseline,
                        config,
                        module_output,
                    )
                    .ok()?;
                    measure_optional_javascript_candidate(
                        code,
                        plan,
                        config.javascript.cost_model,
                        optional_raw_size_cap,
                    )
                    .ok()
                    .flatten()?
                }
            };
            Some(candidate)
        })
        .collect::<Vec<_>>();
    candidates.merge_optional(spelling_candidates)?;
    extend_scored_emission_phase(
        ir,
        module_output,
        beam_policy,
        &integer_analysis,
        &mut candidates,
        &search,
        crate::decision_registry::EmissionPhase::BeforeEntropy,
    )?;
    if configured.mangle_identifiers && config.entropy_aware_mangling_enabled() {
        let entropy_width = candidates.optional_proposal_width();
        let finalists = entropy_alphabet_candidate_options(
            &mut candidates,
            candidate_beam_width.min(entropy_width),
            config.javascript.cost_model,
        )?;
        extend_javascript_candidate_beam(
            ir,
            module_output,
            beam_policy,
            &integer_analysis,
            &mut candidates,
            finalists,
            |options| [options],
        )?;
        // Frequency order is a useful deterministic first proposal, but gzip
        // and Brotli care about complete byte history rather than individual
        // character counts. Search permutations of the actual live one-byte
        // identifiers in a few best final structural candidates, then re-emit
        // through the normal name allocator so binding and reservation proofs
        // remain authoritative. The original candidates are never removed.
        let probe_with_peephole =
            config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole);
        let entropy_source_indices = objective_stratified_candidate_indices(
            &mut candidates,
            entropy_source_limit(candidate_beam_width, entropy_width),
            config.javascript.cost_model,
        )?;
        let mut entropy_sources = entropy_source_indices
            .into_iter()
            .map(|index| candidates[index].clone())
            .collect::<Vec<_>>();
        // Parsed preparation is whole-artifact work too. Admit a deterministic
        // source prefix before running it; invalid/no-op preparations still
        // consume their structural proposal unit.
        let admitted_entropy_sources =
            contexts.reserve_candidate_proposal_work(entropy_sources.len());
        entropy_sources.truncate(admitted_entropy_sources);
        let mut entropy_requests = prepare_entropy_source_requests_by(
            entropy_sources,
            entropy_width,
            move |code, _options| {
                if !probe_with_peephole {
                    return Some(code);
                }
                optimize_generated_javascript_assuming(
                    &code,
                    config.javascript.assume_pristine_builtins,
                )
                .ok()
                .map(|optimized| optimized.code)
            },
        );
        // Each mapping trial remaps and exact-scores a whole artifact. Keep
        // this exploratory coordinate to a small fraction of the shared work
        // ledger so later authoritative IR coordinates (including local
        // coalescing and helper placement) retain deterministic admission.
        let mut mapping_allowance = contexts
            .remaining_candidate_proposal_work()
            .div_euclid(4)
            .min(64);
        for (_, _, trials) in &mut entropy_requests {
            let requested = (*trials).min(mapping_allowance);
            *trials = contexts.reserve_candidate_proposal_work(requested);
            mapping_allowance = mapping_allowance.saturating_sub(*trials);
        }
        entropy_requests.retain(|(_, _, trials)| *trials != 0);
        // Parsed-peephole preparation is an indexed parallel transform, but
        // trial allocation and identifier-alphabet scoring stay strictly
        // coordinator-ordered. A Brotli-11 workspace is large enough that
        // running these short source searches concurrently increases memory
        // pressure and can make the dependent beam tail slower.
        let entropy_groups =
            search_identifier_alphabet_groups(entropy_requests, config.javascript.cost_model);
        let entropy_plans = bounded_javascript_variant_options(
            entropy_groups,
            candidates.optional_proposal_width(),
        )
        .into_iter()
        .filter_map(|(context_id, options)| contexts.register_plan(context_id, options))
        .collect::<Vec<_>>();
        let entropy_candidates = entropy_plans
            .into_par_iter()
            .filter_map(|plan| {
                let code = contexts
                    .emit(plan.identity.context_id, module_output, plan.options)
                    .ok()?;
                validate_direct_javascript_artifact(
                    &code,
                    contexts.get(plan.identity.context_id).baseline,
                    config,
                    module_output,
                )
                .ok()?;
                measure_optional_javascript_candidate(
                    code,
                    plan,
                    config.javascript.cost_model,
                    optional_raw_size_cap,
                )
                .ok()
                .flatten()
            })
            .collect();
        candidates.merge_optional(entropy_candidates)?;
    }
    extend_scored_emission_phase(
        ir,
        module_output,
        beam_policy,
        &integer_analysis,
        &mut candidates,
        &search,
        crate::decision_registry::EmissionPhase::AfterEntropy,
    )?;
    contexts.finish_priority_plan_families();
    finalize_javascript_candidates_with_terminal_objective_challengers(
        candidates.into_candidates(),
        &configured_baseline,
        configured_plan_identity,
        config,
        contexts,
        profile,
        candidate_limit,
        module_output,
    )
}

#[cfg(test)]
fn select_javascript_candidate(
    context_id: usize,
    ir: &ControlFlowModule<'_>,
    config: &ProjectConfig,
    module_output: bool,
    profile: &OptimizationProfile,
    candidate_limit: usize,
    candidate_byte_budget: usize,
    configured_seed: Option<ScoredJavaScriptEmissionSeed>,
    seeded_candidates: Vec<ScoredJavaScriptEmissionSeed>,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    let configured_options = config.js_options();
    if configured_seed.as_ref().is_some_and(|seed| {
        seed.options != configured_options
            || seed.emission.declaration_scores.model != config.javascript.cost_model
    }) {
        return Err(crate::codegen_js::CodegenError::new(
            Span::empty(0),
            "configured JavaScript seed does not match its emission context",
        )
        .into());
    }
    let configured_seed = match configured_seed {
        Some(seed) => seed,
        None => {
            let analysis = Arc::new(analyze_javascript_integer_values(ir));
            let code = emit_javascript_candidate(ir, module_output, configured_options, analysis)?;
            ScoredJavaScriptEmissionSeed {
                emission: ScoredJavaScriptEmission::measure(code, config.javascript.cost_model)?,
                options: configured_options,
            }
        }
    };
    let configured_baseline = configured_seed.emission.code.clone();
    let contexts = JavaScriptEmissionContexts::single(JavaScriptEmissionContext::new(
        context_id,
        ir,
        Some(&configured_seed),
        None,
        config.javascript_optimization_enabled(
            JavaScriptOptimization::ConstructorInitializerFusionVariants,
        ),
    ));
    let configured_plan = contexts
        .register_plan(context_id, configured_options)
        .expect("the configured JavaScript plan is registered first");
    let configured_candidate = JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
        configured_seed.emission.clone(),
        configured_plan,
    );
    let terminal_plan_reserve = if config.javascript.effective_terminal_codec_probe_limit() == 0 {
        0
    } else {
        candidate_limit.div_euclid(8).min(4)
    };
    let mut arena = AggregateJavaScriptPlanArena::new_with_terminal_reserve(
        configured_candidate,
        Vec::new(),
        candidate_limit,
        candidate_byte_budget,
        config.javascript.cost_model,
        terminal_plan_reserve,
    )?;
    for seed in seeded_candidates {
        if seed.emission.declaration_scores.model != config.javascript.cost_model {
            return Err(crate::codegen_js::CodegenError::new(
                Span::empty(0),
                "seeded JavaScript candidate uses a different codec score ledger",
            )
            .into());
        }
        let Some(plan) = contexts.register_plan(context_id, seed.options) else {
            continue;
        };
        arena.merge_optional(vec![
            JavaScriptEmissionCandidate::new_declaration_plan_with_scores(seed.emission, plan),
        ])?;
    }
    select_javascript_candidate_global(
        &contexts,
        config,
        module_output,
        profile,
        arena,
        configured_baseline,
        configured_plan.identity,
    )
}

const MAX_DECLARATION_VARIANTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JavaScriptDeclarationScoreSemantics {
    #[cfg(test)]
    ExactSpelling,
    DeclarationPlan,
}

impl JavaScriptDeclarationScoreSemantics {
    const fn declaration_plan(self) -> bool {
        matches!(self, Self::DeclarationPlan)
    }

    const fn sort_key(self) -> u8 {
        match self {
            #[cfg(test)]
            Self::ExactSpelling => 0,
            Self::DeclarationPlan => 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SelectedModelDeclarationScores {
    model: CompressionCostModel,
    semantics: JavaScriptDeclarationScoreSemantics,
    variant_count: u8,
    costs: [usize; MAX_DECLARATION_VARIANTS],
    known_mask: u8,
}

#[derive(Debug, Clone)]
struct ScoredJavaScriptEmission {
    code: String,
    declaration_scores: SelectedModelDeclarationScores,
}

impl ScoredJavaScriptEmission {
    fn measure(code: String, model: CompressionCostModel) -> Result<Self, CompileError> {
        let declaration_scores = SelectedModelDeclarationScores::measure(
            &code,
            model,
            JavaScriptDeclarationScoreSemantics::DeclarationPlan,
        )?;
        Ok(Self {
            code,
            declaration_scores,
        })
    }

    #[cfg(test)]
    fn with_exact_test_score(
        code: String,
        model: CompressionCostModel,
        transfer_cost: usize,
    ) -> Self {
        Self {
            code,
            declaration_scores: SelectedModelDeclarationScores::exact_spelling(
                model,
                transfer_cost,
            ),
        }
    }
}

fn measure_initial_javascript_emission(
    code: String,
    model: CompressionCostModel,
) -> Result<ScoredJavaScriptEmission, CompileError> {
    // Initial option plans are also parents for quote/alphabet descendants.
    // Measure them even when their own bytes cannot fit the optional arena;
    // a child can be shorter, and registration order owns stable identities.
    ScoredJavaScriptEmission::measure(code, model)
}

fn measure_optional_javascript_emission(
    code: String,
    model: CompressionCostModel,
    raw_size_cap: usize,
) -> Result<Option<ScoredJavaScriptEmission>, CompileError> {
    if code.len() > raw_size_cap {
        return Ok(None);
    }
    ScoredJavaScriptEmission::measure(code, model).map(Some)
}

impl SelectedModelDeclarationScores {
    fn measure(
        code: &str,
        model: CompressionCostModel,
        semantics: JavaScriptDeclarationScoreSemantics,
    ) -> Result<Self, CompileError> {
        Self::measure_message(code, model, semantics).map_err(selected_model_score_error)
    }

    fn measure_message(
        code: &str,
        model: CompressionCostModel,
        semantics: JavaScriptDeclarationScoreSemantics,
    ) -> Result<Self, String> {
        let variants = declaration_score_variants(code, semantics);
        let variant_count = variants.len();
        let results = variants
            .iter()
            .map(|source| admitted_generated_javascript_size(source, model));
        Self::from_ordered_results(model, semantics, variant_count, results)
    }

    fn from_ordered_results(
        model: CompressionCostModel,
        semantics: JavaScriptDeclarationScoreSemantics,
        variant_count: usize,
        results: impl IntoIterator<Item = Result<usize, String>>,
    ) -> Result<Self, String> {
        assert!(
            variant_count <= MAX_DECLARATION_VARIANTS,
            "declaration variant count exceeds its fixed score ledger"
        );
        let mut costs = [0; MAX_DECLARATION_VARIANTS];
        let mut known_mask = 0u8;
        let mut measured = 0usize;
        for (index, result) in results.into_iter().enumerate() {
            assert!(
                index < variant_count,
                "declaration score result count exceeds its variant count"
            );
            costs[index] = result?;
            known_mask |= 1 << index;
            measured += 1;
        }
        assert_eq!(
            measured, variant_count,
            "declaration score result count does not match its variant count"
        );
        Ok(Self {
            model,
            semantics,
            variant_count: variant_count as u8,
            costs,
            known_mask,
        })
    }

    fn is_complete_for(self, semantics: JavaScriptDeclarationScoreSemantics) -> bool {
        let expected_mask = (1u8 << self.variant_count).saturating_sub(1);
        self.semantics == semantics && self.known_mask & expected_mask == expected_mask
    }

    #[cfg(test)]
    fn exact_spelling(model: CompressionCostModel, cost: usize) -> Self {
        let mut costs = [0; MAX_DECLARATION_VARIANTS];
        costs[0] = cost;
        Self {
            model,
            semantics: JavaScriptDeclarationScoreSemantics::ExactSpelling,
            variant_count: 1,
            costs,
            known_mask: 1,
        }
    }

    fn exact_cost(self, model: CompressionCostModel, variant_index: usize) -> Option<usize> {
        (self.model == model
            && variant_index < MAX_DECLARATION_VARIANTS
            && self.known_mask & (1 << variant_index) != 0)
            .then(|| self.costs[variant_index])
    }

    fn minimum(self) -> usize {
        (0..MAX_DECLARATION_VARIANTS)
            .filter_map(|index| self.exact_cost(self.model, index))
            .min()
            .expect("declaration score provenance contains one exact spelling")
    }

    fn best_transfer_and_raw(self, code: &str) -> (usize, usize) {
        top_level_declaration_variants(code.to_string())
            .into_iter()
            .enumerate()
            .filter_map(|(index, source)| {
                self.exact_cost(self.model, index)
                    .map(|transfer| (transfer, source.len()))
            })
            .min()
            .expect("measured declaration plan contains every spelling")
    }
}

fn declaration_score_variants(
    code: &str,
    semantics: JavaScriptDeclarationScoreSemantics,
) -> Vec<String> {
    match semantics {
        #[cfg(test)]
        JavaScriptDeclarationScoreSemantics::ExactSpelling => vec![code.to_string()],
        JavaScriptDeclarationScoreSemantics::DeclarationPlan => {
            top_level_declaration_variants(code.to_string())
        }
    }
}

fn selected_model_score_error(message: String) -> CompileError {
    crate::codegen_js::CodegenError::new(Span::empty(0), message).into()
}

struct SelectedModelEmissionScoreRequest<Owner> {
    owner: Owner,
    code: String,
    model: CompressionCostModel,
    semantics: JavaScriptDeclarationScoreSemantics,
}

fn selected_model_score_request_with_raw_cap<Owner>(
    owner: Owner,
    code: String,
    model: CompressionCostModel,
    semantics: JavaScriptDeclarationScoreSemantics,
    raw_size_cap: usize,
) -> Option<SelectedModelEmissionScoreRequest<Owner>> {
    (code.len() <= raw_size_cap).then_some(SelectedModelEmissionScoreRequest {
        owner,
        code,
        model,
        semantics,
    })
}

struct SelectedModelEmissionScoreResult<Owner> {
    owner: Owner,
    emission: Result<ScoredJavaScriptEmission, CompileError>,
}

/// Removes the required configured root before any optional probe result can
/// be observed. Batch workers may finish in any order, but the score batch
/// reconstructs request order and this boundary preserves root error
/// authority without serializing its declaration leaves.
fn take_required_first_selected_model_emission<Owner>(
    results: Vec<SelectedModelEmissionScoreResult<Owner>>,
    is_required: impl FnOnce(&Owner) -> bool,
) -> Result<
    (
        ScoredJavaScriptEmission,
        std::vec::IntoIter<SelectedModelEmissionScoreResult<Owner>>,
    ),
    CompileError,
> {
    let mut results = results.into_iter();
    let required = results
        .next()
        .expect("selected-model probe batch contains its configured root");
    assert!(
        is_required(&required.owner),
        "configured root must be the first selected-model probe result"
    );
    Ok((required.emission?, results))
}

/// Scores exact selected-model emission groups without collapsing their plan
/// identities. The temporary leaf list holds at most the fixed declaration
/// family for each distinct request group and is released before results enter
/// the retained arena, so reuse cannot grow into an unbounded artifact cache.
fn measure_selected_model_emission_batch<Owner: Send + Sync>(
    requests: Vec<SelectedModelEmissionScoreRequest<Owner>>,
    retained: &[JavaScriptEmissionCandidate],
) -> Vec<SelectedModelEmissionScoreResult<Owner>> {
    measure_selected_model_emission_batch_by(
        requests,
        retained,
        rayon::current_num_threads(),
        |source, model| admitted_generated_javascript_size(source, model),
    )
}

fn measure_selected_model_emission_batch_by<Owner: Send + Sync>(
    requests: Vec<SelectedModelEmissionScoreRequest<Owner>>,
    retained: &[JavaScriptEmissionCandidate],
    maximum_workers: usize,
    score_leaf: impl Fn(&str, CompressionCostModel) -> Result<usize, String> + Sync,
) -> Vec<SelectedModelEmissionScoreResult<Owner>> {
    if requests.is_empty() {
        return Vec::new();
    }

    let same_key = |left: usize, right: usize| {
        requests[left].model == requests[right].model
            && requests[left].semantics == requests[right].semantics
            && requests[left].code == requests[right].code
    };
    let mut scoring_order = (0..requests.len()).collect::<Vec<_>>();
    scoring_order.sort_unstable_by(|left, right| {
        (
            objective_index(requests[*left].model),
            requests[*left].semantics.sort_key(),
            requests[*left].code.as_str(),
        )
            .cmp(&(
                objective_index(requests[*right].model),
                requests[*right].semantics.sort_key(),
                requests[*right].code.as_str(),
            ))
    });

    let mut representative_indices = Vec::new();
    let mut group_for_request = vec![0usize; requests.len()];
    for index in scoring_order {
        let group = if representative_indices
            .last()
            .is_some_and(|representative| same_key(*representative, index))
        {
            representative_indices.len() - 1
        } else {
            representative_indices.push(index);
            representative_indices.len() - 1
        };
        group_for_request[index] = group;
    }

    let mut group_scores = vec![None; representative_indices.len()];
    let mut group_leaf_ranges = vec![None; representative_indices.len()];
    let mut leaf_jobs = Vec::<(CompressionCostModel, String)>::new();
    for (group, representative) in representative_indices.iter().copied().enumerate() {
        let request = &requests[representative];
        if let Some(scores) = retained.iter().find_map(|candidate| {
            let scores = candidate.emission.declaration_scores;
            (candidate.declaration_plan == request.semantics.declaration_plan()
                && scores.model == request.model
                && candidate.code() == request.code
                && scores.is_complete_for(request.semantics))
            .then_some(scores)
        }) {
            group_scores[group] = Some(Ok(scores));
            continue;
        }
        let variants = declaration_score_variants(&request.code, request.semantics);
        let start = leaf_jobs.len();
        leaf_jobs.extend(variants.into_iter().map(|source| (request.model, source)));
        group_leaf_ranges[group] = Some((start, leaf_jobs.len()));
    }

    // A retained plan can leave no work at all, while one declaration plan can
    // expose four independent exact codec leaves. Flatten leaves before Rayon
    // scheduling so a narrow structural beam can still use the active pool.
    // Indexed contiguous batches cap live codec work and preserve leaf order.
    let maximum_workers = maximum_workers.min(rayon::current_num_threads()).max(1);
    let leaf_results = if leaf_jobs.len() <= 1 || maximum_workers == 1 {
        leaf_jobs
            .into_iter()
            .map(|(model, source)| score_leaf(&source, model))
            .collect::<Vec<_>>()
    } else {
        into_bounded_contiguous_batches(leaf_jobs, maximum_workers)
            .into_par_iter()
            .map(|batch| {
                batch
                    .into_iter()
                    .map(|(model, source)| score_leaf(&source, model))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };
    for (group, range) in group_leaf_ranges.into_iter().enumerate() {
        let Some((start, end)) = range else {
            continue;
        };
        let request = &requests[representative_indices[group]];
        group_scores[group] = Some(SelectedModelDeclarationScores::from_ordered_results(
            request.model,
            request.semantics,
            end - start,
            leaf_results[start..end].iter().cloned(),
        ));
    }
    let group_scores = group_scores
        .into_iter()
        .map(|score| score.expect("every selected-model score group is resolved"))
        .collect::<Vec<_>>();

    requests
        .into_iter()
        .enumerate()
        .map(|(index, request)| {
            let emission = group_scores[group_for_request[index]]
                .clone()
                .map(|declaration_scores| ScoredJavaScriptEmission {
                    code: request.code,
                    declaration_scores,
                })
                .map_err(selected_model_score_error);
            SelectedModelEmissionScoreResult {
                owner: request.owner,
                emission,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct JavaScriptEmissionCandidate {
    transfer_cost: usize,
    raw_size: usize,
    emission: ScoredJavaScriptEmission,
    plan: JavaScriptEmissionPlan,
    objective_costs: [Option<usize>; 3],
    declaration_plan: bool,
}

impl JavaScriptEmissionCandidate {
    #[cfg(test)]
    fn new(
        transfer_cost: usize,
        code: String,
        options: crate::codegen_ir_js::IrJsOptions,
        cost_model: CompressionCostModel,
    ) -> Self {
        let ordinal = code.bytes().fold(0usize, |state, byte| {
            state.wrapping_mul(16777619).wrapping_add(usize::from(byte))
        });
        Self::from_scored_emission(
            ScoredJavaScriptEmission::with_exact_test_score(code, cost_model, transfer_cost),
            JavaScriptEmissionPlan {
                identity: JavaScriptPlanIdentity {
                    context_id: 0,
                    ordinal,
                },
                options,
            },
            false,
        )
    }

    fn from_scored_emission(
        emission: ScoredJavaScriptEmission,
        plan: JavaScriptEmissionPlan,
        declaration_plan: bool,
    ) -> Self {
        let cost_model = emission.declaration_scores.model;
        let transfer_cost = if declaration_plan {
            emission.declaration_scores.minimum()
        } else {
            emission
                .declaration_scores
                .exact_cost(cost_model, 0)
                .expect("an exact JavaScript spelling has a score")
        };
        let raw_size = emission.code.len();
        let mut objective_costs = [None; 3];
        objective_costs[objective_index(CompressionCostModel::Raw)] = Some(raw_size);
        objective_costs[objective_index(cost_model)] = Some(transfer_cost);
        Self {
            transfer_cost,
            raw_size,
            emission,
            plan,
            objective_costs,
            declaration_plan,
        }
    }

    #[cfg(test)]
    fn new_declaration_plan(
        code: String,
        plan: JavaScriptEmissionPlan,
        cost_model: CompressionCostModel,
    ) -> Result<Self, CompileError> {
        Ok(Self::from_scored_emission(
            ScoredJavaScriptEmission::measure(code, cost_model)?,
            plan,
            true,
        ))
    }

    fn new_declaration_plan_with_scores(
        emission: ScoredJavaScriptEmission,
        plan: JavaScriptEmissionPlan,
    ) -> Self {
        Self::from_scored_emission(emission, plan, true)
    }

    fn code(&self) -> &str {
        &self.emission.code
    }

    fn options(&self) -> crate::codegen_ir_js::IrJsOptions {
        self.plan.options
    }

    fn identity(&self) -> JavaScriptPlanIdentity {
        self.plan.identity
    }

    fn objective_cost(&mut self, model: CompressionCostModel) -> Result<usize, CompileError> {
        let index = objective_index(model);
        if let Some(cost) = self.objective_costs[index] {
            return Ok(cost);
        }
        let cost = if self.declaration_plan {
            best_declaration_variant_by(self.code(), |source| {
                admitted_generated_javascript_size(source, model).map_err(|message| {
                    crate::codegen_js::CodegenError::new(Span::empty(0), message)
                })
            })?
            .1
        } else {
            admitted_generated_javascript_size(self.code(), model)
                .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?
        };
        self.objective_costs[index] = Some(cost);
        Ok(cost)
    }
}

fn measure_optional_javascript_candidate(
    code: String,
    plan: JavaScriptEmissionPlan,
    selected_model: CompressionCostModel,
    raw_size_cap: usize,
) -> Result<Option<JavaScriptEmissionCandidate>, CompileError> {
    let Some(emission) = measure_optional_javascript_emission(code, selected_model, raw_size_cap)?
    else {
        return Ok(None);
    };
    Ok(Some(
        JavaScriptEmissionCandidate::new_declaration_plan_with_scores(emission, plan),
    ))
}

fn best_declaration_variant_by<Error>(
    code: &str,
    mut score: impl FnMut(&str) -> Result<usize, Error>,
) -> Result<(String, usize), Error> {
    let mut best = None::<(String, usize)>;
    for source in top_level_declaration_variants(code.to_string()) {
        let transfer = score(&source)?;
        let replace = best.as_ref().is_none_or(|(current, current_transfer)| {
            (transfer, source.len(), source.as_str())
                < (*current_transfer, current.len(), current.as_str())
        });
        if replace {
            best = Some((source, transfer));
        }
    }
    Ok(best.expect("every emission has a declaration variant"))
}

const fn objective_index(model: CompressionCostModel) -> usize {
    match model {
        CompressionCostModel::Raw => 0,
        CompressionCostModel::Gzip => 1,
        CompressionCostModel::Brotli => 2,
    }
}

fn sort_javascript_emission_candidates(candidates: &mut [JavaScriptEmissionCandidate]) {
    candidates.sort_by(|left, right| {
        (
            left.transfer_cost,
            left.raw_size,
            left.code(),
            left.identity().context_id,
            left.identity().ordinal,
        )
            .cmp(&(
                right.transfer_cost,
                right.raw_size,
                right.code(),
                right.identity().context_id,
                right.identity().ordinal,
            ))
    });
}

#[cfg(test)]
fn deduplicate_live_javascript_candidate_frontier(
    candidates: &mut Vec<JavaScriptEmissionCandidate>,
) {
    sort_javascript_emission_candidates(candidates);
    // Preserve the pre-context search policy within each IR: once two option
    // plans emit byte-identical code, only the stable first spelling needs to
    // seed later families. Equal bytes from distinct IR contexts remain live
    // until final artifact ranking because their performance provenance can
    // differ.
    candidates.dedup_by(|left, right| {
        left.identity().context_id == right.identity().context_id && left.code() == right.code()
    });
}

#[derive(Debug, Clone)]
struct ScoredJavaScriptCandidate {
    plan_identity: JavaScriptPlanIdentity,
    transfer_cost: usize,
    startup_score: u64,
    code: String,
    metrics: JavaScriptSyntaxMetrics,
    peephole_rewrites: usize,
    performance: JavaScriptPerformanceMetrics,
    rank: (u64, u64),
    has_explicit_lowering_obligations: bool,
    admission: Arc<JavaScriptArtifactAdmission>,
}

#[derive(Debug, Clone)]
struct JavaScriptArtifactAdmission {
    direct_source: Arc<str>,
    abi_manifest: Arc<crate::compilation_contract::JavaScriptAbiManifest>,
    lowering_obligations: usize,
    ecmascript: crate::js_syntax_target::EcmaScriptEdition,
}

impl JavaScriptArtifactAdmission {
    fn validate(&self, source: &str) -> Result<(), CompileError> {
        let outcome = self.validate_inner(source);
        if crate::timing::enabled() {
            // `bytes` accumulates rejections so the report shows discards
            // against total validations.
            crate::timing::ADMISSION.record_pass(u64::from(outcome.is_err()), 0);
        }
        outcome
    }

    /// Re-check an artifact that has already been selected, allowing the class
    /// rewrite's own `constructor` keyword. See
    /// [`validate_observed_javascript_artifact_allowing`] for why this is opt-in.
    fn validate_selected(&self, source: &str) -> Result<(), CompileError> {
        let outcome = validate_generated_javascript_syntax_floor(source, self.ecmascript)
            .map_err(generated_javascript_parse_error)
            .and_then(|()| {
                validate_observed_javascript_artifact_allowing(
                    source,
                    &self.direct_source,
                    &self.abi_manifest,
                    self.lowering_obligations,
                    true,
                )
            });
        if crate::timing::enabled() {
            crate::timing::ADMISSION.record_pass(u64::from(outcome.is_err()), 0);
        }
        outcome
    }

    fn validate_inner(&self, source: &str) -> Result<(), CompileError> {
        validate_generated_javascript_syntax_floor(source, self.ecmascript)
            .map_err(generated_javascript_parse_error)?;
        validate_observed_javascript_artifact(
            source,
            &self.direct_source,
            &self.abi_manifest,
            self.lowering_obligations,
        )
    }
}

#[cfg(test)]
fn test_artifact_admission(source: &str) -> Arc<JavaScriptArtifactAdmission> {
    Arc::new(JavaScriptArtifactAdmission {
        direct_source: Arc::from(source),
        abi_manifest: Arc::new(crate::compilation_contract::JavaScriptAbiManifest {
            world: "closed-application",
            exports: Vec::new(),
            export_names_may_mangle: false,
            foreign_imports: Vec::new(),
            public_aggregate_abi: "named",
            stable_aggregate_fields: Vec::new(),
            stable_extern_fields: Vec::new(),
        }),
        lowering_obligations: 0,
        ecmascript: crate::js_syntax_target::EcmaScriptEdition::Es2022,
    })
}

fn sort_scored_javascript_candidates(candidates: &mut [ScoredJavaScriptCandidate]) {
    candidates.sort_by(|left, right| {
        (left.rank, scored_javascript_candidate_tiebreak(left))
            .cmp(&(right.rank, scored_javascript_candidate_tiebreak(right)))
    });
}

fn scored_javascript_candidate_tiebreak(
    candidate: &ScoredJavaScriptCandidate,
) -> (usize, u8, u64, &str, usize, usize) {
    (
        candidate.code.len(),
        top_level_declaration_preference(&candidate.code),
        candidate.startup_score,
        &candidate.code,
        candidate.plan_identity.context_id,
        candidate.plan_identity.ordinal,
    )
}

fn sort_terminal_javascript_candidates(
    candidates: &mut [ScoredJavaScriptCandidate],
    preserve_binding_topology: bool,
) {
    if !preserve_binding_topology {
        sort_scored_javascript_candidates(candidates);
        return;
    }
    candidates.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then_with(|| {
                resolved_one_byte_binding_count(&right.code)
                    .cmp(&resolved_one_byte_binding_count(&left.code))
            })
            .then_with(|| {
                scored_javascript_candidate_tiebreak(left)
                    .cmp(&scored_javascript_candidate_tiebreak(right))
            })
    });
}

fn resolved_one_byte_binding_count(code: &str) -> usize {
    single_character_resolved_binding_identifiers(code).map_or(0, |bindings| bindings.len())
}

fn into_bounded_contiguous_batches<T>(items: Vec<T>, maximum_batches: usize) -> Vec<Vec<T>> {
    if items.is_empty() {
        return Vec::new();
    }
    let batch_count = items.len().min(maximum_batches.max(1));
    let batch_size = items.len() / batch_count;
    let larger_batches = items.len() % batch_count;
    let mut items = items.into_iter();
    (0..batch_count)
        .map(|index| {
            let len = batch_size + usize::from(index < larger_batches);
            items.by_ref().take(len).collect()
        })
        .collect()
}

fn terminal_scope_naming_options(
    parent: crate::codegen_ir_js::IrJsOptions,
    configured: crate::codegen_ir_js::IrJsOptions,
) -> Vec<crate::codegen_ir_js::IrJsOptions> {
    if !configured.mangle_identifiers || !configured.cross_scope_name_reuse {
        return Vec::new();
    }

    let mut variants = Vec::new();
    let mut push = |options| {
        if options != parent && !variants.contains(&options) {
            variants.push(options);
        }
    };

    // This is the closest safe analogue of Terser's per-scope allocator: a
    // nested scope restarts the alphabet and excludes only parent bindings
    // that its complete transitive reference graph can observe.
    push(crate::codegen_ir_js::IrJsOptions {
        precise_cross_scope_shadowing: true,
        reserved_local_name_prefix: false,
        ..parent
    });

    // A small globally unused prefix is intentionally raw-positive: module
    // bindings move later in the alphabet so independent functions can start
    // with exactly the same local names. Dictionary codecs may recover more
    // from that repetition than the module namespace costs. Search a bounded
    // geometric family, including both the selected and configured reserve.
    let mut reserve_counts = vec![parent.local_name_reserve, configured.local_name_reserve];
    reserve_counts.extend([8, 16, 32]);
    reserve_counts.retain(|reserve| *reserve != 0);
    reserve_counts.dedup();
    for local_name_reserve in reserve_counts {
        push(crate::codegen_ir_js::IrJsOptions {
            precise_cross_scope_shadowing: true,
            reserved_local_name_prefix: true,
            local_name_reserve,
            ..parent
        });
    }

    // The narrower proof can occasionally spell a nested-function-heavy
    // artifact better without perturbing globals and entry bindings.
    if !parent.precise_cross_scope_shadowing {
        push(crate::codegen_ir_js::IrJsOptions {
            transitive_nested_shadowing: true,
            ..parent
        });
    }
    variants
}

fn terminal_string_pooling_options(
    parent: crate::codegen_ir_js::IrJsOptions,
    configured: crate::codegen_ir_js::IrJsOptions,
) -> Vec<crate::codegen_ir_js::IrJsOptions> {
    if !configured.pool_strings {
        return Vec::new();
    }
    let mut variants = Vec::new();
    let mut push = |options| {
        if options != parent && !variants.contains(&options) {
            variants.push(options);
        }
    };

    // Repeated literals are already dictionary material. Keeping every
    // raw-profitable alias can therefore make gzip/Brotli larger, while a
    // sparse set of very expensive literals can still win. Revisit both the
    // unpooled spelling and a denser threshold ladder on the actual final
    // structural/naming winner.
    push(crate::codegen_ir_js::IrJsOptions {
        pool_strings: false,
        ..parent
    });
    for string_pool_minimum_savings in [
        parent.string_pool_minimum_savings,
        configured.string_pool_minimum_savings,
        16,
        32,
        64,
        96,
        128,
        192,
        256,
        384,
        512,
        768,
        1024,
    ] {
        push(crate::codegen_ir_js::IrJsOptions {
            pool_strings: true,
            string_pool_minimum_savings,
            ..parent
        });
    }
    variants
}

fn finalized_javascript_candidate_precedes(
    left: &SelectedJavaScriptCandidate,
    right: &SelectedJavaScriptCandidate,
    config: &ProjectConfig,
    baseline_transfer: usize,
) -> bool {
    let left_rank = javascript_candidate_rank(
        config,
        left.transfer_cost,
        baseline_transfer,
        left.performance.score,
        left.baseline_performance.score,
    );
    let right_rank = javascript_candidate_rank(
        config,
        right.transfer_cost,
        baseline_transfer,
        right.performance.score,
        right.baseline_performance.score,
    );
    (
        left_rank,
        left.code.len(),
        top_level_declaration_preference(&left.code),
        left.startup_score,
        left.code.as_str(),
        left.plan_identity.context_id,
        left.plan_identity.ordinal,
    ) < (
        right_rank,
        right.code.len(),
        top_level_declaration_preference(&right.code),
        right.startup_score,
        right.code.as_str(),
        right.plan_identity.context_id,
        right.plan_identity.ordinal,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalJavaScriptCandidateBudget {
    remaining_plans: usize,
    remaining_code_bytes: usize,
}

impl TerminalJavaScriptCandidateBudget {
    fn after_retained(
        plan_limit: usize,
        code_byte_limit: usize,
        retained_plans: usize,
        retained_code_bytes: usize,
    ) -> Self {
        Self {
            remaining_plans: plan_limit.saturating_sub(retained_plans),
            remaining_code_bytes: code_byte_limit.saturating_sub(retained_code_bytes),
        }
    }

    const fn has_plan_slot(self) -> bool {
        self.remaining_plans != 0
    }

    const fn can_admit(self, code_bytes: usize) -> bool {
        self.has_plan_slot() && code_bytes <= self.remaining_code_bytes
    }

    const fn cannot_admit_any_challenger(self) -> bool {
        !self.has_plan_slot() || self.remaining_code_bytes == 0
    }

    fn charge(&mut self, code_bytes: usize) {
        debug_assert!(self.can_admit(code_bytes));
        self.remaining_plans -= 1;
        self.remaining_code_bytes -= code_bytes;
    }
}

/// Compilation-wide ledger for optional terminal work after structural
/// emission plans have been ranked. Work is admitted before whole-artifact
/// repair/validation and exact-codec scoring, so invalid proposals cannot
/// bypass the ceiling. This is intentionally not a survivor budget: one
/// surviving large artifact can expose thousands of binding permutations.
/// Exhaustion is normal and leaves the already-scored incumbent eligible.
#[derive(Debug)]
struct TerminalCodecProbeBudget {
    limit: usize,
    used: usize,
    codec_calls: AtomicUsize,
    limit_reached: bool,
    reserved_for_final: usize,
    final_phase: bool,
    slice_end: Option<usize>,
    challenger_reserve_released: bool,
    finalist_reserve_released: bool,
}

impl TerminalCodecProbeBudget {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            used: 0,
            codec_calls: AtomicUsize::new(0),
            limit_reached: false,
            reserved_for_final: 0,
            final_phase: false,
            slice_end: None,
            challenger_reserve_released: false,
            finalist_reserve_released: false,
        }
    }

    fn with_final_reserve(limit: usize, reserved_for_final: usize) -> Self {
        Self {
            reserved_for_final: reserved_for_final.min(limit),
            ..Self::new(limit)
        }
    }

    fn remaining(&self) -> usize {
        let mut remaining = self.limit.saturating_sub(self.used);
        if !self.final_phase {
            remaining = remaining.saturating_sub(self.reserved_for_final);
        }
        if let Some(slice_end) = self.slice_end {
            remaining = remaining.min(slice_end.saturating_sub(self.used));
        }
        remaining
    }

    fn begin_fair_slice(&mut self, allowance: usize) {
        debug_assert!(self.slice_end.is_none());
        self.slice_end = Some(self.used.saturating_add(allowance));
    }

    fn end_fair_slice(&mut self) {
        self.slice_end = None;
    }

    /// Reserve a deterministic prefix before parallel scoring. Workers never
    /// race for the final permit, so budget exhaustion cannot change output
    /// with Rayon scheduling.
    fn reserve(&mut self, requested: usize) -> usize {
        let admitted = requested.min(self.remaining());
        self.used = self.used.saturating_add(admitted);
        self.limit_reached |= admitted < requested;
        admitted
    }

    fn reserve_complete(&mut self, requested: usize) -> bool {
        if requested > self.remaining() {
            self.limit_reached = true;
            return false;
        }
        self.used = self.used.saturating_add(requested);
        true
    }

    fn reserve_work_unit(&mut self) -> bool {
        self.reserve(1) == 1
    }

    /// Make one reserved terminal family available while preserving the tail
    /// promised to a still-later family. This lets scope naming run after the
    /// base finalist without consuming the exact two-binding neighborhood
    /// that must remain last for topology-sensitive joint wins.
    fn release_reserved(&mut self, released: usize) {
        self.reserved_for_final = self.reserved_for_final.saturating_sub(released);
    }

    fn release_challenger_reserve_once(&mut self, released: usize) {
        if !self.challenger_reserve_released {
            self.release_reserved(released);
            self.challenger_reserve_released = true;
        }
    }

    fn release_finalist_reserve_once(&mut self, released: usize) {
        if !self.finalist_reserve_released {
            self.release_reserved(released);
            self.finalist_reserve_released = true;
        }
    }

    fn begin_final_phase(&mut self) {
        self.final_phase = true;
    }

    fn compressed_size(
        &mut self,
        bytes: &[u8],
        model: CompressionCostModel,
    ) -> Result<Option<usize>, CompileError> {
        if self.reserve(1) == 0 {
            return Ok(None);
        }
        self.measure_reserved(bytes, model)
            .map(Some)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message).into())
    }

    /// Measure a permit already reserved by the deterministic coordinator.
    /// This method is shared by parallel workers; the atomic records actual
    /// codec invocations, while `used` remains the conservative admission
    /// ledger that enforces the hard ceiling.
    fn measure_reserved(&self, bytes: &[u8], model: CompressionCostModel) -> Result<usize, String> {
        let source = std::str::from_utf8(bytes)
            .map_err(|error| format!("generated JavaScript is not UTF-8: {error}"))?;
        analyze_generated_javascript(source)
            .map_err(|error| format!("generated JavaScript admission failed: {error}"))?;
        self.codec_calls.fetch_add(1, Ordering::Relaxed);
        compressed_size(bytes, model)
    }

    fn measure_reserved_compile(
        &self,
        bytes: &[u8],
        model: CompressionCostModel,
    ) -> Result<usize, CompileError> {
        self.measure_reserved(bytes, model)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message).into())
    }

    fn codec_calls(&self) -> usize {
        self.codec_calls.load(Ordering::Relaxed)
    }
}

fn emit_terminal_javascript_challengers(
    options: Vec<crate::codegen_ir_js::IrJsOptions>,
    context_id: usize,
    module_output: bool,
    model: CompressionCostModel,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    budget: &mut TerminalJavaScriptCandidateBudget,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Vec<JavaScriptEmissionCandidate> {
    let mut candidates = Vec::new();
    if codec_budget.remaining() == 0 {
        return candidates;
    }
    for options in options {
        if !budget.has_plan_slot() {
            break;
        }
        // Registration and IR emission are whole-artifact work too. Charge
        // the deterministic option before either step; invalid or duplicate
        // proposals cannot bypass the terminal ledger.
        if !codec_budget.reserve_work_unit() {
            break;
        }
        let plan = contexts
            .registered_plan(context_id, options)
            .or_else(|| contexts.register_terminal_plan(context_id, options));
        let Some(plan) = plan else {
            continue;
        };
        let Ok(code) = contexts.emit(plan.identity.context_id, module_output, plan.options) else {
            continue;
        };
        if candidates
            .iter()
            .any(|candidate: &JavaScriptEmissionCandidate| candidate.code() == code)
        {
            continue;
        }
        // The aggregate arena already charged every retained structural plan.
        // A terminal re-emission is another whole-artifact plan, so reject it
        // before codec scoring when the shared source-byte tail cannot hold it.
        // Continue rather than stop: a later option can emit substantially
        // fewer bytes while consuming the same single plan slot.
        if !budget.can_admit(code.len()) {
            continue;
        }
        let variants =
            declaration_score_variants(&code, JavaScriptDeclarationScoreSemantics::DeclarationPlan);
        if !codec_budget.reserve_complete(variants.len()) {
            break;
        }
        let results = variants
            .iter()
            .map(|source| codec_budget.measure_reserved(source.as_bytes(), model));
        let Ok(declaration_scores) = SelectedModelDeclarationScores::from_ordered_results(
            model,
            JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            variants.len(),
            results,
        ) else {
            continue;
        };
        let emission = ScoredJavaScriptEmission {
            code,
            declaration_scores,
        };
        let candidate =
            JavaScriptEmissionCandidate::new_declaration_plan_with_scores(emission, plan);
        budget.charge(candidate.raw_size);
        candidates.push(candidate);
    }
    candidates
}

fn install_terminal_javascript_codec_pool<Output>(
    config: &ProjectConfig,
    work: impl FnOnce() -> Result<Output, CompileError> + Send,
) -> Result<Output, CompileError>
where
    Output: Send,
{
    let active_threads = rayon::current_num_threads();
    let codec_workers = config
        .compiler
        .resources
        .effective_codec_workers(active_threads);
    if codec_workers >= active_threads {
        return work();
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(codec_workers)
        .build()
        .map_err(|error| {
            crate::codegen_js::CodegenError::new(
                Span::empty(0),
                format!("failed to create terminal JavaScript codec worker pool: {error}"),
            )
        })?;
    pool.install(work)
}

#[allow(clippy::too_many_arguments)]
fn finalize_javascript_candidates_with_terminal_objective_challengers(
    candidates: Vec<JavaScriptEmissionCandidate>,
    configured_baseline: &str,
    configured_plan_identity: JavaScriptPlanIdentity,
    config: &ProjectConfig,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    profile: &OptimizationProfile,
    candidate_limit: usize,
    module_output: bool,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    const TERMINAL_CHALLENGER_CODEC_RESERVE: usize = 4 * (MAX_DECLARATION_VARIANTS + 1);
    let terminal_codec_probe_limit = config
        .javascript
        .effective_terminal_codec_probe_limit_for_artifact(configured_baseline.len());
    let exact_pair_reserve = if exact_two_binding_terminal_search_enabled_for_artifact(
        config,
        configured_baseline.len(),
    ) && candidates
        .iter()
        .any(|candidate| resolved_one_byte_binding_count(candidate.code()) == 2)
    {
        let requested = EXACT_TWO_BINDING_MAX_PAIR_TRIALS
            .saturating_mul(MAX_DECLARATION_VARIANTS)
            // One initial exact score and one pre-analysis work permit.
            .saturating_add(2);
        if terminal_codec_probe_limit >= 384 {
            requested
        } else {
            requested.min(terminal_codec_probe_limit.div_euclid(2))
        }
    } else {
        0
    };
    let terminal_finalist_reserve = terminal_codec_probe_limit.div_euclid(4).min(96);
    // Preserve enough selected-model calls for the factored terminal naming
    // family even when ordinary cleanup exhausts its general allowance. Four
    // reserved plan slots expose at most four declaration spellings each.
    let mut codec_budget = TerminalCodecProbeBudget::with_final_reserve(
        terminal_codec_probe_limit,
        exact_pair_reserve
            .saturating_add(TERMINAL_CHALLENGER_CODEC_RESERVE)
            .saturating_add(terminal_finalist_reserve),
    );
    let mut selected = install_terminal_javascript_codec_pool(config, || {
        finalize_javascript_candidates_with_terminal_objective_challengers_in_current_pool(
            candidates,
            configured_baseline,
            configured_plan_identity,
            config,
            contexts,
            profile,
            candidate_limit,
            module_output,
            &mut codec_budget,
        )
    })?;
    selected.terminal_codec_probes = codec_budget.codec_calls();
    selected.terminal_work_units = codec_budget.used;
    selected.terminal_codec_probe_limit = terminal_codec_probe_limit;
    selected.terminal_codec_probe_limit_reached = codec_budget.limit_reached;
    apply_selected_canonical_peephole(selected, config)
}

#[allow(clippy::too_many_arguments)]
fn finalize_javascript_candidates_with_terminal_objective_challengers_in_current_pool(
    candidates: Vec<JavaScriptEmissionCandidate>,
    configured_baseline: &str,
    configured_plan_identity: JavaScriptPlanIdentity,
    config: &ProjectConfig,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    profile: &OptimizationProfile,
    candidate_limit: usize,
    module_output: bool,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    // The parsed peephole can make a structurally weaker emission plan become
    // the final exact-codec winner. Applying naming leaves only to the
    // pre-peephole beam therefore misses the parent that actually matters.
    // Finalize normally first, then re-emit a small set of safe naming-only
    // challengers from that exact winning plan. The incumbent remains an
    // unconditional fallback.
    let plan_options = candidates
        .iter()
        .map(|candidate| (candidate.identity(), candidate.options()))
        .collect::<Vec<_>>();
    let configured_code_bytes = candidates
        .iter()
        .find(|candidate| candidate.identity() == configured_plan_identity)
        .map_or(configured_baseline.len(), |candidate| candidate.raw_size);
    let retained_code_bytes = candidates.iter().fold(0usize, |total, candidate| {
        total.saturating_add(candidate.raw_size)
    });
    let mut terminal_budget = TerminalJavaScriptCandidateBudget::after_retained(
        candidate_limit,
        config
            .javascript
            .effective_candidate_byte_budget()
            .max(configured_code_bytes),
        candidates.len(),
        retained_code_bytes,
    );
    if terminal_budget.cannot_admit_any_challenger() {
        codec_budget
            .release_challenger_reserve_once(4usize.saturating_mul(MAX_DECLARATION_VARIANTS + 1));
        codec_budget.release_finalist_reserve_once(codec_budget.limit.div_euclid(4).min(96));
    }
    let baseline_transfer = if let Some(transfer) = candidates
        .iter()
        .find(|candidate| candidate.identity() == configured_plan_identity)
        .and_then(|candidate| {
            candidate
                .emission
                .declaration_scores
                .exact_cost(config.javascript.cost_model, 0)
        }) {
        transfer
    } else {
        admitted_generated_javascript_size(configured_baseline, config.javascript.cost_model)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?
    };
    let mut selected = finalize_javascript_candidates_with_parallelism(
        candidates,
        configured_baseline,
        configured_plan_identity,
        config,
        contexts,
        profile,
        candidate_limit,
        true,
        codec_budget,
    )?;
    // Naming and declaration spelling jointly determine the binding topology.
    // Release only their small allowance here; keep the exact pair reserve
    // protected until every naming/string-pooling challenger has settled.
    codec_budget
        .release_challenger_reserve_once(4usize.saturating_mul(MAX_DECLARATION_VARIANTS + 1));
    let Some(parent_options) = plan_options
        .iter()
        .find_map(|(identity, options)| (*identity == selected.plan_identity).then_some(*options))
    else {
        return Ok(selected);
    };
    let mut candidates_evaluated = selected.candidates_evaluated;
    let mut terminal_scope_naming_challengers = 0usize;
    let mut terminal_scope_naming_selected = false;
    let mut terminal_scope_naming_incumbent_bytes = None;
    let mut terminal_scope_naming_best_bytes = None;
    let mut terminal_string_pooling_challengers = 0usize;
    let mut terminal_string_pooling_selected = false;
    let mut terminal_string_pooling_incumbent_bytes = None;
    let mut terminal_string_pooling_best_bytes = None;

    let challenger_options = terminal_scope_naming_options(parent_options, config.js_options());
    let challenger_candidates = emit_terminal_javascript_challengers(
        challenger_options,
        selected.plan_identity.context_id,
        module_output,
        config.javascript.cost_model,
        contexts,
        &mut terminal_budget,
        codec_budget,
    );
    if !challenger_candidates.is_empty() {
        terminal_scope_naming_challengers = challenger_candidates.len();
        // Rank the whole naming family together so the expensive
        // post-selection remap and cleanup searches run once, on its exact
        // best member.
        if let Ok(candidate) = finalize_javascript_candidates_with_parallelism(
            challenger_candidates,
            configured_baseline,
            configured_plan_identity,
            config,
            contexts,
            profile,
            usize::MAX,
            true,
            codec_budget,
        ) {
            candidates_evaluated =
                candidates_evaluated.saturating_add(candidate.candidates_evaluated);
            terminal_scope_naming_incumbent_bytes = Some(selected.transfer_cost);
            terminal_scope_naming_best_bytes = Some(candidate.transfer_cost);
            terminal_scope_naming_selected = finalized_javascript_candidate_precedes(
                &candidate,
                &selected,
                config,
                baseline_transfer,
            );
            if terminal_scope_naming_selected {
                selected = candidate;
            }
        }
    }

    // String pooling has the same late-parent problem as naming. A raw-saving
    // alias can merely duplicate bytes already represented in a codec's
    // dictionary, so re-score an unpooled spelling and a denser threshold
    // ladder from the actual naming winner instead of assuming that every
    // repeated literal should be shared.
    if let Some(pooling_parent_options) = contexts
        .registered_plan_by_identity(selected.plan_identity)
        .map(|plan| plan.options)
    {
        let pooling_options =
            terminal_string_pooling_options(pooling_parent_options, config.js_options());
        let pooling_candidates = emit_terminal_javascript_challengers(
            pooling_options,
            selected.plan_identity.context_id,
            module_output,
            config.javascript.cost_model,
            contexts,
            &mut terminal_budget,
            codec_budget,
        );
        if !pooling_candidates.is_empty() {
            terminal_string_pooling_challengers = pooling_candidates.len();
            if let Ok(candidate) = finalize_javascript_candidates_with_parallelism(
                pooling_candidates,
                configured_baseline,
                configured_plan_identity,
                config,
                contexts,
                profile,
                usize::MAX,
                true,
                codec_budget,
            ) {
                candidates_evaluated =
                    candidates_evaluated.saturating_add(candidate.candidates_evaluated);
                terminal_string_pooling_incumbent_bytes = Some(selected.transfer_cost);
                terminal_string_pooling_best_bytes = Some(candidate.transfer_cost);
                terminal_string_pooling_selected = finalized_javascript_candidate_precedes(
                    &candidate,
                    &selected,
                    config,
                    baseline_transfer,
                );
                if terminal_string_pooling_selected {
                    selected = candidate;
                }
            }
        }
    }

    selected.candidates_evaluated = candidates_evaluated;
    selected.terminal_scope_naming_challengers = terminal_scope_naming_challengers;
    selected.terminal_scope_naming_selected = terminal_scope_naming_selected;
    selected.terminal_scope_naming_incumbent_bytes = terminal_scope_naming_incumbent_bytes;
    selected.terminal_scope_naming_best_bytes = terminal_scope_naming_best_bytes;
    selected.terminal_string_pooling_challengers = terminal_string_pooling_challengers;
    selected.terminal_string_pooling_selected = terminal_string_pooling_selected;
    selected.terminal_string_pooling_incumbent_bytes = terminal_string_pooling_incumbent_bytes;
    selected.terminal_string_pooling_best_bytes = terminal_string_pooling_best_bytes;
    codec_budget.begin_final_phase();
    selected = apply_exact_two_binding_unused_letter_remap(selected, config, codec_budget)?;
    Ok(selected)
}

fn finalize_javascript_candidates(
    candidates: Vec<JavaScriptEmissionCandidate>,
    configured_baseline: &str,
    configured_plan_identity: JavaScriptPlanIdentity,
    config: &ProjectConfig,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    profile: &OptimizationProfile,
    candidate_limit: usize,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    let terminal_codec_probe_limit = config
        .javascript
        .effective_terminal_codec_probe_limit_for_artifact(configured_baseline.len());
    let mut codec_budget = TerminalCodecProbeBudget::new(terminal_codec_probe_limit);
    let mut selected = install_terminal_javascript_codec_pool(config, || {
        finalize_javascript_candidates_with_parallelism(
            candidates,
            configured_baseline,
            configured_plan_identity,
            config,
            contexts,
            profile,
            candidate_limit,
            true,
            &mut codec_budget,
        )
    })?;
    selected.terminal_codec_probes = codec_budget.codec_calls();
    selected.terminal_work_units = codec_budget.used;
    selected.terminal_codec_probe_limit = terminal_codec_probe_limit;
    selected.terminal_codec_probe_limit_reached = codec_budget.limit_reached;
    let selected = apply_selected_canonical_peephole(selected, config)?;
    apply_search_off_declaration_peephole(selected, config)
}

#[allow(clippy::too_many_arguments)]
fn finalize_javascript_candidates_with_parallelism(
    candidates: Vec<JavaScriptEmissionCandidate>,
    configured_baseline: &str,
    configured_plan_identity: JavaScriptPlanIdentity,
    config: &ProjectConfig,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    profile: &OptimizationProfile,
    candidate_limit: usize,
    allow_parallel_brotli: bool,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    // Candidate limits count structural emission plans, not their equivalent
    // declaration spellings. Preserve the configured plan at this boundary;
    // its exact source spelling remains one fallback leaf and must not evict a
    // codec-better `var`/`let` leaf after expansion.
    let pristine_builtins = config.javascript.assume_pristine_builtins;
    let mut candidates = candidates;
    if candidates.len() > candidate_limit {
        let configured_plan = candidates
            .iter()
            .find(|candidate| candidate.identity() == configured_plan_identity)
            .cloned();
        retain_objective_stratified_candidates(
            &mut candidates,
            candidate_limit,
            config.javascript.cost_model,
        )?;
        if let Some(configured_plan) = configured_plan {
            if !candidates
                .iter()
                .any(|candidate| candidate.identity() == configured_plan_identity)
            {
                candidates.pop();
                candidates.push(configured_plan);
                sort_javascript_emission_candidates(&mut candidates);
            }
        }
    }
    let baseline_metrics = analyze_generated_javascript(configured_baseline)
        .map_err(generated_javascript_parse_error)?;
    let peephole =
        config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole);
    let parsed_function_elision = config.single_use_function_expression_candidates_enabled();
    let startup_guard =
        config.javascript_optimization_configured(JavaScriptOptimization::StartupCostGuard);
    let baseline_transfer = if let Some(transfer) = candidates
        .iter()
        .find(|candidate| candidate.identity() == configured_plan_identity)
        .and_then(|candidate| {
            candidate
                .emission
                .declaration_scores
                .exact_cost(config.javascript.cost_model, 0)
        }) {
        transfer
    } else {
        admitted_generated_javascript_size(configured_baseline, config.javascript.cost_model)
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?
    };
    let configured_options = config.js_options();
    let performance_model =
        config.javascript_optimization_configured(JavaScriptOptimization::PerformanceShapeModel);
    let baseline_performance = if performance_model {
        analyze_javascript_performance(
            contexts.root().baseline,
            &configured_options,
            profile,
            config.javascript_performance_weights(),
        )
    } else {
        JavaScriptPerformanceMetrics::default()
    };
    #[derive(Debug)]
    struct PreparedJavaScriptLeaf {
        code: String,
        metrics: JavaScriptSyntaxMetrics,
        peephole_rewrites: usize,
        transfer_cost: Option<usize>,
    }

    #[derive(Debug)]
    struct PreparedJavaScriptPlan {
        plan_identity: JavaScriptPlanIdentity,
        performance: JavaScriptPerformanceMetrics,
        has_explicit_lowering_obligations: bool,
        admission: Arc<JavaScriptArtifactAdmission>,
        leaves: Vec<PreparedJavaScriptLeaf>,
    }

    // Parsing and rewriting are separated from exact-codec scoring so the
    // compilation-wide permit ledger can admit one deterministic prefix, then
    // let that bounded prefix run in parallel. A shared atomic counter would
    // make the final admitted leaf depend on Rayon scheduling.
    // Parsed preparation itself performs whole-artifact syntax work, so admit
    // those plans before entering the parallel parser too. Keep at least half
    // of the current general allowance available for the exact scores that
    // give prepared leaves selection authority.
    let mut peephole_plan_candidates = Vec::new();
    if peephole {
        if let Some(configured) = candidates
            .iter()
            .find(|candidate| candidate.identity() == configured_plan_identity)
        {
            peephole_plan_candidates.push(configured);
        }
        let mut ranked = candidates.iter().collect::<Vec<_>>();
        ranked.sort_unstable_by(|left, right| compare_javascript_seed_admission(left, right));
        let mut seen_code = crate::stable_hash::StableHashSet::default();
        if let Some(configured) = peephole_plan_candidates.first() {
            seen_code.insert(configured.code());
        }
        // Parsing duplicate emissions cannot expose a distinct rewrite. Rank
        // one representative per exact byte string before reserving work so a
        // broad family of equivalent early plans cannot starve a later
        // topology (for example an inlined function expression).
        for candidate in ranked {
            if seen_code.insert(candidate.code()) {
                peephole_plan_candidates.push(candidate);
            }
        }
    }
    let peephole_plan_limit = peephole_plan_candidates
        .len()
        .min(codec_budget.remaining().div_euclid(2));
    let admitted_peephole_plans = codec_budget.reserve(peephole_plan_limit);
    let peephole_plan_identities = peephole_plan_candidates
        .into_iter()
        .take(admitted_peephole_plans)
        .map(|candidate| candidate.identity())
        .collect::<Vec<_>>();
    let prepare_plan = |candidate: JavaScriptEmissionCandidate| {
        let plan_identity = candidate.identity();
        let context = contexts.get(plan_identity.context_id);
        let has_explicit_lowering_obligations =
            context.baseline.has_explicit_lowering_obligations();
        let lowering_obligations = context
            .baseline
            .lowering_obligation_count(crate::ir::LoweringObligation::PreserveJavaScriptBitOrZero);
        let direct_source = candidate.emission.code.clone();
        let module_output = generated_javascript_export_names(&direct_source)
            .is_ok_and(|exports| !exports.is_empty());
        let abi_manifest = config
            .javascript_compilation_contract(module_output)
            .abi_manifest(context.baseline);
        let admission = Arc::new(JavaScriptArtifactAdmission {
            direct_source: Arc::from(direct_source.as_str()),
            abi_manifest: Arc::new(abi_manifest),
            lowering_obligations,
            ecmascript: config.javascript.resolved_ecmascript(),
        });
        let prepare_peephole =
            peephole_plan_identities.contains(&plan_identity) && !has_explicit_lowering_obligations;
        let options = candidate.options();
        let declaration_scores = candidate.emission.declaration_scores;
        let performance = if performance_model {
            analyze_javascript_performance(
                contexts.get(candidate.identity().context_id).baseline,
                &options,
                profile,
                config.javascript_performance_weights(),
            )
        } else {
            JavaScriptPerformanceMetrics::default()
        };
        let mut leaves = Vec::<PreparedJavaScriptLeaf>::with_capacity(
            MAX_DECLARATION_VARIANTS.saturating_mul(if prepare_peephole { 2 } else { 1 }),
        );
        for (declaration_index, declaration) in
            top_level_declaration_variants(candidate.emission.code)
                .into_iter()
                .enumerate()
        {
            let configured_declaration =
                plan_identity == configured_plan_identity && declaration_index == 0;
            let variants = if prepare_peephole {
                match optimize_generated_javascript_assuming(&declaration, pristine_builtins) {
                    Err(_) if configured_declaration => {
                        vec![peephole_preserve_or_baseline(
                            declaration,
                            baseline_metrics,
                            true,
                            pristine_builtins,
                        )]
                    }
                    Err(_) => continue,
                    Ok(optimized) if optimized.code == declaration => {
                        vec![(declaration, optimized.metrics, optimized.rewrites, true)]
                    }
                    Ok(optimized) => {
                        let original_metrics = if configured_declaration {
                            baseline_metrics
                        } else {
                            let Ok(metrics) = analyze_generated_javascript(&declaration) else {
                                continue;
                            };
                            metrics
                        };
                        // Parsed cleanup may move and erase single-use function
                        // bindings. That representation belongs to the
                        // StructuredClosureInlining decision; ParsedPeephole
                        // alone must not make the dedicated pure-helper family
                        // or function-layout family indistinguishable from its
                        // control. Keep other local rewrites, but reject a leaf
                        // that crosses this explicit function-count boundary.
                        if !parsed_function_elision
                            && optimized.metrics.functions < original_metrics.functions
                        {
                            vec![peephole_preserve_or_baseline(
                                declaration,
                                original_metrics,
                                true,
                                pristine_builtins,
                            )]
                        } else if analyze_generated_javascript(&optimized.code).is_ok() {
                            vec![
                                (declaration, original_metrics, 0, true),
                                (optimized.code, optimized.metrics, optimized.rewrites, false),
                            ]
                        } else {
                            vec![peephole_preserve_or_baseline(
                                declaration,
                                original_metrics,
                                true,
                                pristine_builtins,
                            )]
                        }
                    }
                }
            } else {
                let metrics = if configured_declaration {
                    baseline_metrics
                } else {
                    let Ok(metrics) = analyze_generated_javascript(&declaration) else {
                        continue;
                    };
                    metrics
                };
                vec![(declaration, metrics, 0, true)]
            };
            for (code, metrics, peephole_rewrites, is_declaration_spelling) in variants {
                if admission.validate(&code).is_err() {
                    continue;
                }
                if config
                    .javascript
                    .startup
                    .max_nesting
                    .is_some_and(|maximum| metrics.max_nesting > maximum)
                    || (startup_guard
                        && !startup_cost_allowed(
                            metrics,
                            baseline_metrics,
                            &config.javascript.startup,
                        ))
                {
                    continue;
                }
                if leaves.iter().any(|candidate| candidate.code == code) {
                    continue;
                }
                let transfer_cost = if is_declaration_spelling {
                    declaration_scores
                        .exact_cost(config.javascript.cost_model, declaration_index)
                        // The configured incumbent is mandatory. Its rare
                        // missing-ledger fallback was measured above, outside
                        // the optional terminal-search budget, so an exhausted
                        // optional budget can never discard the only root.
                        .or_else(|| configured_declaration.then_some(baseline_transfer))
                } else {
                    None
                };
                leaves.push(PreparedJavaScriptLeaf {
                    code,
                    metrics,
                    peephole_rewrites,
                    transfer_cost,
                });
            }
        }
        PreparedJavaScriptPlan {
            plan_identity,
            performance,
            has_explicit_lowering_obligations,
            admission,
            leaves,
        }
    };
    let parallel_brotli = allow_parallel_brotli
        && config.javascript.cost_model == CompressionCostModel::Brotli
        && candidates.len() >= 2;
    let codec_workers = config
        .compiler
        .resources
        .effective_codec_workers(rayon::current_num_threads());
    let mut prepared = if parallel_brotli {
        into_bounded_contiguous_batches(candidates, codec_workers)
            .into_par_iter()
            .map(|batch| batch.into_iter().map(&prepare_plan).collect::<Vec<_>>())
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    } else {
        candidates.into_iter().map(prepare_plan).collect()
    };

    // Keep the same objective-ranked plan order used to admit parsed work,
    // then traverse leaf columns before later spellings from the first plan.
    // A bounded score prefix therefore gives every admitted topology one
    // chance instead of spending the ledger on duplicate declaration leaves.
    prepared.sort_by_key(|plan| {
        peephole_plan_identities
            .iter()
            .position(|identity| *identity == plan.plan_identity)
            .unwrap_or(usize::MAX)
    });
    let missing_leaf_indices = prepared
        .iter()
        .map(|plan| {
            plan.leaves
                .iter()
                .enumerate()
                .filter_map(|(index, leaf)| leaf.transfer_cost.is_none().then_some(index))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let maximum_missing_leaves = missing_leaf_indices.iter().map(Vec::len).max().unwrap_or(0);
    let mut terminal_score_locations = Vec::new();
    for leaf_rank in 0..maximum_missing_leaves {
        for (plan_index, leaves) in missing_leaf_indices.iter().enumerate() {
            if let Some(leaf_index) = leaves.get(leaf_rank).copied() {
                terminal_score_locations.push((plan_index, leaf_index));
            }
        }
    }
    let admitted_terminal_scores = codec_budget.reserve(terminal_score_locations.len());
    terminal_score_locations.truncate(admitted_terminal_scores);
    let terminal_score_sources = terminal_score_locations
        .iter()
        .map(|(plan_index, leaf_index)| prepared[*plan_index].leaves[*leaf_index].code.clone())
        .collect::<Vec<_>>();
    let terminal_score_results = if parallel_brotli && terminal_score_sources.len() >= 2 {
        into_bounded_contiguous_batches(terminal_score_sources, codec_workers)
            .into_par_iter()
            .map(|batch| {
                batch
                    .into_iter()
                    .map(|code| {
                        codec_budget.measure_reserved(code.as_bytes(), config.javascript.cost_model)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    } else {
        terminal_score_sources
            .into_iter()
            .map(|code| {
                codec_budget.measure_reserved(code.as_bytes(), config.javascript.cost_model)
            })
            .collect::<Vec<_>>()
    };
    for ((plan_index, leaf_index), score) in terminal_score_locations
        .into_iter()
        .zip(terminal_score_results)
    {
        prepared[plan_index].leaves[leaf_index].transfer_cost = score.ok();
    }

    let mut scored = prepared
        .into_iter()
        .filter_map(|plan| {
            let mut plan_scored = plan
                .leaves
                .into_iter()
                .filter_map(|leaf| {
                    let transfer_cost = leaf.transfer_cost?;
                    if !optimizer_variant_candidate_allowed(
                        config.javascript.cost_model,
                        transfer_cost,
                        baseline_transfer,
                        leaf.code.len(),
                        configured_baseline.len(),
                        config.javascript.max_candidate_raw_growth_percent,
                    ) {
                        return None;
                    }
                    let startup_score = leaf.metrics.startup_score(
                        config.javascript.startup.parse_weight,
                        config.javascript.startup.compile_weight,
                        config.javascript.startup.memory_weight,
                    );
                    let rank = javascript_candidate_rank(
                        config,
                        transfer_cost,
                        baseline_transfer,
                        plan.performance.score,
                        baseline_performance.score,
                    );
                    Some(ScoredJavaScriptCandidate {
                        plan_identity: plan.plan_identity,
                        transfer_cost,
                        startup_score,
                        code: leaf.code,
                        metrics: leaf.metrics,
                        peephole_rewrites: leaf.peephole_rewrites,
                        performance: plan.performance,
                        rank,
                        has_explicit_lowering_obligations: plan.has_explicit_lowering_obligations,
                        admission: Arc::clone(&plan.admission),
                    })
                })
                .collect::<Vec<_>>();
            sort_scored_javascript_candidates(&mut plan_scored);
            plan_scored.into_iter().next()
        })
        .collect::<Vec<_>>();
    sort_scored_javascript_candidates(&mut scored);
    let mut seen_code = crate::stable_hash::StableHashSet::default();
    scored.retain(|candidate| seen_code.insert(candidate.code.clone()));
    let candidates_evaluated = scored.len();
    // A representation that is slightly worse before syntax recovery can win
    // after branch/conditional cleanup (single-use function inlining is a
    // common example). Give a small set of independently emitted finalists a
    // structural late pass before making the irreversible plan choice.
    const LATE_IR_RANKED_FINALIST_WIDTH: usize = 4;
    const LATE_IR_TOTAL_FINALIST_WIDTH: usize = 12;
    let mut finalist_indices =
        (0..scored.len().min(LATE_IR_RANKED_FINALIST_WIDTH)).collect::<Vec<_>>();
    // Reusing one spelling in disjoint scopes can be locally attractive but
    // removes a coordinate from terminal entropy search. Preserve the
    // spelling with the richest one-byte binding topology before context
    // stratification so a jointly better pair of distinct names remains
    // reachable even when every individual rename loses.
    if finalist_indices.len() < LATE_IR_TOTAL_FINALIST_WIDTH {
        let richest_binding_count = scored
            .iter()
            .enumerate()
            .filter(|(index, _)| !finalist_indices.contains(index))
            .map(|(_, candidate)| resolved_one_byte_binding_count(&candidate.code))
            .max();
        if let Some((index, _)) = scored.iter().enumerate().find(|(index, candidate)| {
            !finalist_indices.contains(index)
                && Some(resolved_one_byte_binding_count(&candidate.code)) == richest_binding_count
        }) {
            finalist_indices.push(index);
        }
    }
    let mut represented_contexts = finalist_indices
        .iter()
        .map(|index| scored[*index].plan_identity.context_id)
        .collect::<crate::stable_hash::StableHashSet<_>>();
    for (index, candidate) in scored.iter().enumerate() {
        if finalist_indices.len() == LATE_IR_TOTAL_FINALIST_WIDTH {
            break;
        }
        if represented_contexts.insert(candidate.plan_identity.context_id) {
            finalist_indices.push(index);
        }
    }
    finalist_indices.sort_unstable();
    let mut late_finalists = finalist_indices
        .into_iter()
        .map(|index| scored[index].clone())
        .map(|candidate| {
            let cleaned =
                apply_late_javascript_cleanup(candidate.clone(), config, 0, codec_budget)?;
            let mut candidate = retain_resolved_javascript(candidate, cleaned);
            candidate.rank = javascript_candidate_rank(
                config,
                candidate.transfer_cost,
                baseline_transfer,
                candidate.performance.score,
                baseline_performance.score,
            );
            Ok(candidate)
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    sort_scored_javascript_candidates(&mut late_finalists);
    // Naming and structural cleanup interact strongly under dictionary
    // codecs. Do not make the irreversible plan choice from the pre-remap
    // score: a runner-up can become the exact whole-artifact winner after its
    // local namespaces are permuted. Keep the two best exact spellings, then
    // retain a representative of each distinct IR context. Otherwise two
    // locally lucky names from one context can prevent a structurally better
    // context from ever reaching the namespace search that makes it win.
    const TERMINAL_JAVASCRIPT_RANKED_FINALIST_WIDTH: usize = 2;
    const TERMINAL_JAVASCRIPT_TOTAL_FINALIST_WIDTH: usize = 4;
    let mut terminal_source_indices = (0..late_finalists
        .len()
        .min(TERMINAL_JAVASCRIPT_RANKED_FINALIST_WIDTH))
        .collect::<Vec<_>>();
    if terminal_source_indices.len() < TERMINAL_JAVASCRIPT_TOTAL_FINALIST_WIDTH {
        let richest_binding_count = late_finalists
            .iter()
            .enumerate()
            .filter(|(index, _)| !terminal_source_indices.contains(index))
            .map(|(_, candidate)| resolved_one_byte_binding_count(&candidate.code))
            .max();
        if let Some((index, _)) = late_finalists
            .iter()
            .enumerate()
            .find(|(index, candidate)| {
                !terminal_source_indices.contains(index)
                    && Some(resolved_one_byte_binding_count(&candidate.code))
                        == richest_binding_count
            })
        {
            terminal_source_indices.push(index);
        }
    }
    let mut terminal_contexts = terminal_source_indices
        .iter()
        .map(|index| late_finalists[*index].plan_identity.context_id)
        .collect::<crate::stable_hash::StableHashSet<_>>();
    for (index, candidate) in late_finalists.iter().enumerate() {
        if terminal_source_indices.len() == TERMINAL_JAVASCRIPT_TOTAL_FINALIST_WIDTH {
            break;
        }
        if terminal_contexts.insert(candidate.plan_identity.context_id) {
            terminal_source_indices.push(index);
        }
    }
    terminal_source_indices.sort_unstable();
    // Earlier syntax preparation cannot consume this protected quarter of the
    // compilation-wide ledger. Split the released allowance fairly across
    // finalists so a broad first namespace cannot starve every later
    // structural topology. Unused work from one slice remains available to
    // subsequent finalists; the total hard cap is unchanged.
    let terminal_finalist_reserve = codec_budget.limit.div_euclid(4).min(96);
    codec_budget.release_finalist_reserve_once(terminal_finalist_reserve);
    let terminal_source_count = terminal_source_indices.len();
    let mut terminal_finalists = Vec::with_capacity(terminal_source_count);
    for (position, index) in terminal_source_indices.into_iter().enumerate() {
        let remaining_finalists = terminal_source_count.saturating_sub(position).max(1);
        let allowance = codec_budget.remaining().div_ceil(remaining_finalists);
        let result = (|| {
            let selected = late_finalists[index].clone();
            // Run the cheap factored naming/syntax bridge before exhaustive
            // namespace neighborhoods. Otherwise the first remapper can
            // consume this finalist's fair slice before a locally neutral
            // rename exposes single-use function movement.
            let candidate_end = codec_budget.used.saturating_add(allowance);
            codec_budget.begin_fair_slice(allowance.min(8));
            let cleaned = late_javascript_cleanup_finalists(
                selected.clone(),
                config,
                0,
                codec_budget,
                config.javascript.terminal_cleanup_finalists(),
            );
            codec_budget.end_fair_slice();
            let cleaned = cleaned?;
            // Finish each cleanup spelling and keep the one that ends smallest.
            // The cleanup ranked them by what they cost before the remapping,
            // and the remapping is not monotone in that cost.
            let mut finished = Vec::with_capacity(cleaned.len());
            let carried = cleaned.len();
            for (offset, cleaned) in cleaned.into_iter().enumerate() {
                let selected = retain_resolved_javascript(selected.clone(), cleaned);
                let share = candidate_end
                    .saturating_sub(codec_budget.used)
                    .div_ceil(carried.saturating_sub(offset).max(1));
                codec_budget.begin_fair_slice(share);
                let remainder = (|| {
                let remapped = apply_unused_letter_binding_remaps(
                    selected.clone(),
                    config,
                    true,
                    codec_budget,
                )?;
                let selected = retain_resolved_javascript(selected, remapped);
                let cleaned =
                    apply_late_javascript_cleanup(selected.clone(), config, 6, codec_budget)?;
                let selected = retain_resolved_javascript(selected, cleaned);
                // Late control/sequence selection changes identifier adjacency and
                // use frequency. Re-run the exact codec-scored remapper on those
                // final bytes; the pre-cleanup optimum is not necessarily optimal
                // for the transformed artifact, and unchanged naming remains the
                // incumbent candidate.
                let remapped = apply_unused_letter_binding_remaps(
                    selected.clone(),
                    config,
                    true,
                    codec_budget,
                )?;
                let selected = retain_resolved_javascript(selected, remapped);
                let mut selected =
                    apply_terminal_boolean_binding_remap(selected, config, codec_budget)?;
                selected.rank = javascript_candidate_rank(
                    config,
                    selected.transfer_cost,
                    baseline_transfer,
                    selected.performance.score,
                    baseline_performance.score,
                );
                Ok::<_, CompileError>(selected)
                })();
                codec_budget.end_fair_slice();
                finished.push(remainder?);
            }
            sort_terminal_javascript_candidates(
                &mut finished,
                exact_two_binding_terminal_search_enabled_for_artifact(
                    config,
                    configured_baseline.len(),
                ),
            );
            finished
                .into_iter()
                .next()
                .ok_or_else(|| -> CompileError {
                    crate::codegen_js::CodegenError::new(
                        Span::empty(0),
                        "terminal cleanup returned no candidate",
                    )
                    .into()
                })
        })();
        terminal_finalists.push(result?);
    }
    sort_terminal_javascript_candidates(
        &mut terminal_finalists,
        exact_two_binding_terminal_search_enabled_for_artifact(config, configured_baseline.len()),
    );
    let selected = terminal_finalists.into_iter().next().ok_or_else(|| {
        crate::codegen_js::CodegenError::new(
            Span::empty(0),
            "startup limits rejected every JavaScript candidate",
        )
    })?;
    // The complete normal remap/cleanup pipeline above decides the structural
    // plan. Coordinate descent is a terminal namespace fine-tune and cannot
    // expose more structure, so run it once on that exact winner instead of
    // duplicating its exhaustive swap neighborhood for every finalist.
    let selected = apply_terminal_binding_coordinate_descent(selected, config, codec_budget)?;
    Ok(SelectedJavaScriptCandidate {
        plan_identity: selected.plan_identity,
        code: selected.code,
        transfer_cost: selected.transfer_cost,
        baseline_transfer,
        has_explicit_lowering_obligations: contexts
            .get(selected.plan_identity.context_id)
            .baseline
            .has_explicit_lowering_obligations(),
        startup_score: selected.startup_score,
        metrics: selected.metrics,
        baseline_metrics,
        performance: selected.performance,
        baseline_performance,
        candidates_evaluated,
        terminal_codec_probes: 0,
        terminal_work_units: 0,
        terminal_codec_probe_limit: codec_budget.limit,
        terminal_codec_probe_limit_reached: false,
        peephole_rewrites: selected.peephole_rewrites,
        terminal_scope_naming_challengers: 0,
        terminal_scope_naming_selected: false,
        terminal_scope_naming_incumbent_bytes: None,
        terminal_scope_naming_best_bytes: None,
        terminal_string_pooling_challengers: 0,
        terminal_string_pooling_selected: false,
        terminal_string_pooling_incumbent_bytes: None,
        terminal_string_pooling_best_bytes: None,
        admission: selected.admission,
    })
}

fn candidate_raw_size_allowed(candidate: usize, baseline: usize, growth_percent: u16) -> bool {
    let maximum = baseline
        .saturating_mul(100usize.saturating_add(usize::from(growth_percent)))
        .saturating_div(100);
    candidate <= maximum
}

fn optimizer_variant_candidate_allowed(
    cost_model: CompressionCostModel,
    transfer: usize,
    baseline_transfer: usize,
    raw_size: usize,
    baseline_raw_size: usize,
    growth_percent: u16,
) -> bool {
    match cost_model {
        CompressionCostModel::Raw => {
            candidate_raw_size_allowed(raw_size, baseline_raw_size, growth_percent)
        }
        CompressionCostModel::Gzip | CompressionCostModel::Brotli => {
            transfer <= baseline_transfer
                || candidate_raw_size_allowed(raw_size, baseline_raw_size, growth_percent)
        }
    }
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
        // A size-first build is an exact served-byte objective. Ratios are
        // useful when combining unlike dimensions, but quantizing transfer to
        // basis points can tie several bytes on a real bundle and allow a
        // larger artifact to win through the secondary performance score.
        crate::config::JavaScriptPriority::SizeFirst => (
            u64::try_from(transfer).unwrap_or(u64::MAX),
            performance_ratio,
        ),
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

const fn codec_history_window(model: CompressionCostModel) -> usize {
    match model {
        // Raw-size selection has no history. Retain a bounded proposal without
        // giving this heuristic any authority over the exact raw-byte score.
        CompressionCostModel::Raw | CompressionCostModel::Gzip => 32 * 1024,
        // `compressed_size` configures Brotli with lgwin=22.
        CompressionCostModel::Brotli => 1 << 22,
    }
}

fn top_candidate_options(
    candidates: &mut [JavaScriptEmissionCandidate],
    limit: usize,
    selected_model: CompressionCostModel,
) -> Result<Vec<JavaScriptEmissionPlan>, CompileError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut plans = Vec::new();
    let rankings = objective_ranked_candidate_indices(candidates, selected_model)?;
    // Structural IR contexts are independent search regimes. When the beam
    // can afford one member per live context, seed each regime from its best
    // selected-objective plan before alternate codec rankings consume the
    // remaining width. Otherwise a pair of spellings from one context can
    // starve a tied context of every later naming/layout proposal, even when
    // that context contains the eventual exact-codec winner.
    let context_count = candidates
        .iter()
        .map(|candidate| candidate.identity().context_id)
        .collect::<crate::stable_hash::StableHashSet<_>>()
        .len();
    let context_seed_limit = context_count.min(limit);
    let mut seeded_contexts = crate::stable_hash::StableHashSet::default();
    if let Some(selected_ranking) = rankings.first() {
        for index in selected_ranking {
            let candidate = &candidates[*index];
            if seeded_contexts.insert(candidate.identity().context_id) {
                plans.push(candidate.plan);
                if plans.len() == context_seed_limit {
                    break;
                }
            }
        }
    }
    if plans.len() == limit {
        return Ok(plans);
    }
    for rank in 0..candidates.len() {
        for ranking in &rankings {
            let Some(index) = ranking.get(rank).copied() else {
                continue;
            };
            let candidate = &candidates[index];
            if !plans
                .iter()
                .any(|plan: &JavaScriptEmissionPlan| plan.identity == candidate.identity())
            {
                plans.push(candidate.plan);
            }
            if plans.len() == limit {
                return Ok(plans);
            }
        }
    }
    Ok(plans)
}

fn objective_models(selected: CompressionCostModel) -> Vec<CompressionCostModel> {
    let mut models = vec![selected];
    for model in [
        CompressionCostModel::Raw,
        CompressionCostModel::Gzip,
        CompressionCostModel::Brotli,
    ] {
        if !models.contains(&model) {
            models.push(model);
        }
    }
    models
}

struct PendingAlternateGzipGroup {
    members: Vec<usize>,
    variants: Vec<String>,
}

/// Populates the one missing diagnostic objective in a Brotli-selected
/// frontier. Candidate identities remain distinct: only exact gzip leaf bytes
/// are shared, and the resulting family cost is copied into each identity's
/// score ledger.
fn populate_missing_gzip_objectives_for_brotli_candidates(
    candidates: &mut [JavaScriptEmissionCandidate],
) {
    populate_missing_gzip_objectives_for_brotli_candidates_by(
        candidates,
        rayon::current_num_threads(),
        |source| admitted_generated_javascript_size(source, CompressionCostModel::Gzip),
    );
}

fn populate_missing_gzip_objectives_for_brotli_candidates_by(
    candidates: &mut [JavaScriptEmissionCandidate],
    maximum_workers: usize,
    score_leaf: impl Fn(&str) -> Result<usize, String> + Sync,
) {
    let gzip_objective = objective_index(CompressionCostModel::Gzip);
    let mut scoring_order = (0..candidates.len()).collect::<Vec<_>>();
    scoring_order.sort_by(|left, right| {
        (candidates[*left].code(), candidates[*left].declaration_plan).cmp(&(
            candidates[*right].code(),
            candidates[*right].declaration_plan,
        ))
    });

    let mut pending = Vec::<PendingAlternateGzipGroup>::new();
    let mut group_start = 0usize;
    while group_start < scoring_order.len() {
        let representative = scoring_order[group_start];
        let mut group_end = group_start + 1;
        while group_end < scoring_order.len()
            && candidates[scoring_order[group_end]].declaration_plan
                == candidates[representative].declaration_plan
            && candidates[scoring_order[group_end]].code() == candidates[representative].code()
        {
            group_end += 1;
        }
        let members = scoring_order[group_start..group_end].to_vec();
        let known_cost = members
            .iter()
            .find_map(|index| candidates[*index].objective_costs[gzip_objective]);
        if let Some(cost) = known_cost {
            debug_assert!(members
                .iter()
                .filter_map(|index| candidates[*index].objective_costs[gzip_objective])
                .all(|known| known == cost));
            for index in members {
                candidates[index].objective_costs[gzip_objective].get_or_insert(cost);
            }
        } else {
            let variants = if candidates[representative].declaration_plan {
                top_level_declaration_variants(candidates[representative].code().to_string())
            } else {
                vec![candidates[representative].code().to_string()]
            };
            pending.push(PendingAlternateGzipGroup { members, variants });
        }
        group_start = group_end;
    }
    if pending.is_empty() {
        return;
    }

    // Flatten complete declaration spellings before scheduling, then intern
    // exact source bytes across otherwise-distinct spelling families. Sorting
    // only this ephemeral occurrence list makes sharing deterministic without
    // retaining a whole-artifact cache beyond the ranking call.
    let mut occurrences = pending
        .iter()
        .enumerate()
        .flat_map(|(group, pending)| {
            (0..pending.variants.len()).map(move |variant| (group, variant))
        })
        .collect::<Vec<_>>();
    occurrences.sort_by(|(left_group, left_variant), (right_group, right_variant)| {
        pending[*left_group].variants[*left_variant]
            .cmp(&pending[*right_group].variants[*right_variant])
    });
    let mut unique_sources = Vec::<String>::new();
    let mut leaf_indices = pending
        .iter()
        .map(|group| vec![0usize; group.variants.len()])
        .collect::<Vec<_>>();
    for (group, variant) in occurrences {
        let source = &pending[group].variants[variant];
        let leaf = if unique_sources.last().is_some_and(|known| known == source) {
            unique_sources.len() - 1
        } else {
            unique_sources.push(source.clone());
            unique_sources.len() - 1
        };
        leaf_indices[group][variant] = leaf;
    }
    let pending_members = pending
        .into_iter()
        .map(|group| group.members)
        .collect::<Vec<_>>();

    let maximum_workers = maximum_workers.min(rayon::current_num_threads()).max(1);
    let leaf_results = if unique_sources.len() <= 1 || maximum_workers == 1 {
        unique_sources
            .into_iter()
            .map(|source| score_leaf(&source))
            .collect::<Vec<_>>()
    } else {
        into_bounded_contiguous_batches(unique_sources, maximum_workers)
            .into_par_iter()
            .map(|batch| {
                batch
                    .into_iter()
                    .map(|source| score_leaf(&source))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };

    for (group, members) in pending_members.into_iter().enumerate() {
        // `best_declaration_variant_by` fails the complete family when any
        // spelling fails. Preserve that behavior before applying the existing
        // rule that a diagnostic objective failure ranks as `usize::MAX`.
        let cost = leaf_indices[group]
            .iter()
            .try_fold(usize::MAX, |best, leaf| {
                leaf_results[*leaf].clone().map(|cost| best.min(cost))
            })
            .unwrap_or(usize::MAX);
        for index in members {
            candidates[index].objective_costs[gzip_objective] = Some(cost);
        }
    }
}

fn objective_ranked_candidate_indices(
    candidates: &mut [JavaScriptEmissionCandidate],
    selected_model: CompressionCostModel,
) -> Result<Vec<Vec<usize>>, CompileError> {
    let models = objective_models(selected_model);
    debug_assert!(
        candidates
            .iter()
            .all(|candidate| { candidate.emission.declaration_scores.model == selected_model }),
        "objective ranking requires the selected-model score ledger"
    );

    // Distinct IR contexts can intentionally retain byte-identical plans: the
    // final performance ranking still needs each context's provenance. Their
    // missing codec objectives do not need another encoder pass, though. Sort
    // indices into exact byte/spelling-family groups without moving or
    // deduplicating candidates, score one representative, and copy only that
    // exact objective result to the other identities. `declaration_plan` is
    // part of the key because those candidates rank the complete equivalent
    // declaration-spelling family, while other candidates rank only `code`.
    if candidates.iter().any(|candidate| {
        models
            .iter()
            .any(|model| candidate.objective_costs[objective_index(*model)].is_none())
    }) {
        let mut scoring_order = (0..candidates.len()).collect::<Vec<_>>();
        scoring_order.sort_by(|left, right| {
            (candidates[*left].code(), candidates[*left].declaration_plan).cmp(&(
                candidates[*right].code(),
                candidates[*right].declaration_plan,
            ))
        });
        if selected_model == CompressionCostModel::Brotli {
            // The selected Brotli ledger and raw bytes are already populated.
            // Gzip is the sole missing objective and its small independent
            // workspaces can use the complete active Rayon pool.
            populate_missing_gzip_objectives_for_brotli_candidates(candidates);
        }
        let mut group_start = 0usize;
        while group_start < scoring_order.len() {
            let representative = scoring_order[group_start];
            let mut group_end = group_start + 1;
            while group_end < scoring_order.len()
                && candidates[scoring_order[group_end]].declaration_plan
                    == candidates[representative].declaration_plan
                && candidates[scoring_order[group_end]].code() == candidates[representative].code()
            {
                group_end += 1;
            }
            for model in &models {
                let objective = objective_index(*model);
                let known_cost = scoring_order[group_start..group_end]
                    .iter()
                    .find_map(|index| candidates[*index].objective_costs[objective]);
                let cost = match known_cost {
                    Some(cost) => cost,
                    None => {
                        let result = candidates[representative].objective_cost(*model);
                        retain_objective_cost_result(
                            &mut candidates[representative],
                            *model,
                            selected_model,
                            result,
                        )?;
                        candidates[representative].objective_costs[objective]
                            .expect("retained objective result populates its exact score")
                    }
                };
                debug_assert!(scoring_order[group_start..group_end]
                    .iter()
                    .filter_map(|index| candidates[*index].objective_costs[objective])
                    .all(|known| known == cost));
                for index in &scoring_order[group_start..group_end] {
                    candidates[*index].objective_costs[objective].get_or_insert(cost);
                }
            }
            group_start = group_end;
        }
    }
    Ok(models
        .into_iter()
        .map(|model| {
            let objective = objective_index(model);
            let mut indices = (0..candidates.len()).collect::<Vec<_>>();
            indices.sort_by(|left, right| {
                let left = &candidates[*left];
                let right = &candidates[*right];
                (
                    left.objective_costs[objective].expect("objective costs were populated"),
                    left.raw_size,
                    left.code(),
                    left.identity().context_id,
                    left.identity().ordinal,
                )
                    .cmp(&(
                        right.objective_costs[objective].expect("objective costs were populated"),
                        right.raw_size,
                        right.code(),
                        right.identity().context_id,
                        right.identity().ordinal,
                    ))
            });
            indices
        })
        .collect())
}

fn retain_objective_cost_result(
    candidate: &mut JavaScriptEmissionCandidate,
    model: CompressionCostModel,
    selected_model: CompressionCostModel,
    result: Result<usize, CompileError>,
) -> Result<(), CompileError> {
    match result {
        Ok(cost) => {
            candidate.objective_costs[objective_index(model)] = Some(cost);
            Ok(())
        }
        Err(error) if model == selected_model => Err(error),
        Err(_) => {
            // Alternate objectives broaden the bounded proposal frontier but
            // have no authority over this build. An unavailable diagnostic
            // scorer must not make the configured objective fail.
            candidate.objective_costs[objective_index(model)] = Some(usize::MAX);
            Ok(())
        }
    }
}

fn objective_stratified_candidate_indices(
    candidates: &mut [JavaScriptEmissionCandidate],
    limit: usize,
    selected_model: CompressionCostModel,
) -> Result<Vec<usize>, CompileError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let rankings = objective_ranked_candidate_indices(candidates, selected_model)?;
    let mut retained = Vec::with_capacity(limit.min(candidates.len()));
    let mut present = vec![false; candidates.len()];
    for rank in 0..candidates.len() {
        for ranking in &rankings {
            let Some(index) = ranking.get(rank).copied() else {
                continue;
            };
            if !present[index] {
                present[index] = true;
                retained.push(index);
                if retained.len() == limit {
                    return Ok(retained);
                }
            }
        }
    }
    Ok(retained)
}

fn retain_objective_stratified_candidates(
    candidates: &mut Vec<JavaScriptEmissionCandidate>,
    limit: usize,
    selected_model: CompressionCostModel,
) -> Result<(), CompileError> {
    if limit == 0 {
        candidates.clear();
        return Ok(());
    }
    if candidates.len() <= limit {
        return Ok(());
    }
    let retained = objective_stratified_candidate_indices(candidates, limit, selected_model)?;
    let mut present = vec![false; candidates.len()];
    for index in retained {
        present[index] = true;
    }
    let mut index = 0usize;
    candidates.retain(|_| {
        let keep = present[index];
        index += 1;
        keep
    });
    Ok(())
}

/// Bounded ownership container for the aggregate cross-IR JavaScript search.
struct AggregateJavaScriptPlanArena {
    candidates: Vec<JavaScriptEmissionCandidate>,
    pinned_identities: Vec<JavaScriptPlanIdentity>,
    selected_model: CompressionCostModel,
    effective_plan_count_cap: usize,
    effective_code_byte_cap: usize,
    optional_plan_count_cap: usize,
    optional_code_byte_cap: usize,
}

impl AggregateJavaScriptPlanArena {
    #[cfg(test)]
    fn new(
        configured: JavaScriptEmissionCandidate,
        ranked_non_root_pins: Vec<JavaScriptEmissionCandidate>,
        requested_plan_count_cap: usize,
        requested_code_byte_cap: usize,
        selected_model: CompressionCostModel,
    ) -> Result<Self, CompileError> {
        Self::new_with_terminal_reserve(
            configured,
            ranked_non_root_pins,
            requested_plan_count_cap,
            requested_code_byte_cap,
            selected_model,
            0,
        )
    }

    fn new_with_terminal_reserve(
        configured: JavaScriptEmissionCandidate,
        ranked_non_root_pins: Vec<JavaScriptEmissionCandidate>,
        requested_plan_count_cap: usize,
        requested_code_byte_cap: usize,
        selected_model: CompressionCostModel,
        requested_terminal_plan_reserve: usize,
    ) -> Result<Self, CompileError> {
        // Only the root configured artifact defines the non-negotiable floor.
        // Other context seeds are ranked pins: retain them when they fit the
        // caller's effective caps, but never expand those caps on their behalf.
        let effective_plan_count_cap = requested_plan_count_cap.max(1);
        let effective_code_byte_cap = requested_code_byte_cap.max(configured.raw_size);
        let terminal_plan_reserve =
            requested_terminal_plan_reserve.min(effective_plan_count_cap.saturating_sub(1));
        let terminal_code_byte_reserve = configured
            .raw_size
            .saturating_mul(terminal_plan_reserve)
            .min(effective_code_byte_cap.saturating_sub(configured.raw_size));
        let mut remaining_plan_count = effective_plan_count_cap
            .saturating_sub(1)
            .saturating_sub(terminal_plan_reserve);
        let mut remaining_code_bytes = effective_code_byte_cap
            .saturating_sub(configured.raw_size)
            .saturating_sub(terminal_code_byte_reserve);
        let mut candidates = Vec::with_capacity(
            1usize.saturating_add(
                ranked_non_root_pins
                    .len()
                    .min(effective_plan_count_cap.saturating_sub(1)),
            ),
        );
        candidates.push(configured);
        for candidate in ranked_non_root_pins {
            if let Some(existing) = candidates
                .iter()
                .find(|existing| existing.identity() == candidate.identity())
            {
                if existing.options() != candidate.options() || existing.code() != candidate.code()
                {
                    return Err(crate::codegen_js::CodegenError::new(
                        Span::empty(0),
                        "aggregate JavaScript arena received conflicting bytes for one pinned plan identity",
                    )
                    .into());
                }
                continue;
            }
            if remaining_plan_count == 0 {
                break;
            }
            if candidate.raw_size > remaining_code_bytes {
                continue;
            }
            remaining_plan_count -= 1;
            remaining_code_bytes -= candidate.raw_size;
            candidates.push(candidate);
        }
        let pinned_identities = candidates
            .iter()
            .map(JavaScriptEmissionCandidate::identity)
            .collect();
        let arena = Self {
            candidates,
            pinned_identities,
            selected_model,
            effective_plan_count_cap,
            effective_code_byte_cap,
            optional_plan_count_cap: remaining_plan_count,
            optional_code_byte_cap: remaining_code_bytes,
        };
        arena.validate_retained_caps()?;
        Ok(arena)
    }

    fn admit_ranked_pin(
        &mut self,
        candidate: JavaScriptEmissionCandidate,
    ) -> Result<bool, CompileError> {
        if let Some(existing) = self
            .candidates
            .iter()
            .find(|existing| existing.identity() == candidate.identity())
        {
            if existing.options() != candidate.options() || existing.code() != candidate.code() {
                return Err(crate::codegen_js::CodegenError::new(
                    Span::empty(0),
                    "aggregate JavaScript arena received conflicting bytes for one pinned plan identity",
                )
                .into());
            }
            return Ok(self.pinned_identities.contains(&candidate.identity()));
        }
        if self.optional_plan_count_cap == 0 || candidate.raw_size > self.optional_code_byte_cap {
            return Ok(false);
        }
        self.optional_plan_count_cap -= 1;
        self.optional_code_byte_cap -= candidate.raw_size;
        self.pinned_identities.push(candidate.identity());
        self.candidates.push(candidate);
        self.validate_retained_caps()?;
        Ok(true)
    }

    fn merge_optional(
        &mut self,
        proposals: Vec<JavaScriptEmissionCandidate>,
    ) -> Result<(), CompileError> {
        let proposal_width = self.optional_proposal_width();
        if proposals.len() > proposal_width {
            return Err(crate::codegen_js::CodegenError::new(
                Span::empty(0),
                format!(
                    "aggregate JavaScript proposal batch has {} plans but its byte-derived width is {}",
                    proposals.len(), proposal_width
                ),
            )
            .into());
        }
        self.merge_optional_candidates(proposals)
    }

    fn merge_precomputed_optional(
        &mut self,
        proposals: Vec<JavaScriptEmissionCandidate>,
    ) -> Result<(), CompileError> {
        // These seeds were emitted and scored during IR probing. Let exact
        // byte packing see all of them: the proposal-width work bound applies
        // to new emissions, not to already-paid artifacts.
        self.merge_optional_candidates(proposals)
    }

    fn merge_optional_candidates(
        &mut self,
        proposals: Vec<JavaScriptEmissionCandidate>,
    ) -> Result<(), CompileError> {
        let mut optional_proposals = Vec::with_capacity(proposals.len());
        for proposal in proposals {
            if proposal.emission.declaration_scores.model != self.selected_model {
                return Err(crate::codegen_js::CodegenError::new(
                    Span::empty(0),
                    "aggregate JavaScript arena received a different codec score ledger",
                )
                .into());
            }
            if self.pinned_identities.contains(&proposal.identity()) {
                let pinned = self
                    .candidates
                    .iter()
                    .find(|candidate| candidate.identity() == proposal.identity())
                    .expect("a pinned JavaScript identity has a retained candidate");
                if pinned.options() != proposal.options() || pinned.code() != proposal.code() {
                    return Err(crate::codegen_js::CodegenError::new(
                        Span::empty(0),
                        "aggregate JavaScript arena received conflicting bytes for one pinned plan identity",
                    )
                    .into());
                }
                continue;
            }
            optional_proposals.push(proposal);
        }
        let mut pinned = Vec::with_capacity(self.pinned_identities.len());
        let mut optional = Vec::new();
        for candidate in std::mem::take(&mut self.candidates) {
            if self.pinned_identities.contains(&candidate.identity()) {
                pinned.push(candidate);
            } else {
                optional.push(candidate);
            }
        }
        optional.extend(optional_proposals);
        optional.sort_by_key(JavaScriptEmissionCandidate::identity);
        for pair in optional.windows(2) {
            if pair[0].identity() == pair[1].identity()
                && (pair[0].options() != pair[1].options() || pair[0].code() != pair[1].code())
            {
                return Err(crate::codegen_js::CodegenError::new(
                    Span::empty(0),
                    "aggregate JavaScript arena received conflicting bytes for one plan identity",
                )
                .into());
            }
        }
        optional.dedup_by(|left, right| left.identity() == right.identity());
        // Validate identity conflicts before dropping impossible plans so a
        // precomputed seed cannot hide inconsistent provenance. Past this
        // boundary, a plan larger than the complete optional byte pool cannot
        // affect packing or descendants and must not reach alternate codecs.
        optional.retain(|candidate| candidate.raw_size <= self.optional_code_byte_cap);
        let optional_count = optional.len();
        let ranked = objective_stratified_candidate_indices(
            &mut optional,
            optional_count,
            self.selected_model,
        )?;
        let mut slots = optional.into_iter().map(Some).collect::<Vec<_>>();
        let mut retained = Vec::with_capacity(optional_count.min(self.optional_plan_count_cap));
        let mut remaining_bytes = self.optional_code_byte_cap;
        let is_redundant = |candidate: &JavaScriptEmissionCandidate,
                            retained: &[JavaScriptEmissionCandidate]| {
            pinned.iter().chain(retained).any(|existing| {
                existing.identity().context_id == candidate.identity().context_id
                    && existing.code() == candidate.code()
            })
        };
        // Diversity must never make bounded search worse than the ordinary
        // selected-objective frontier. Protect its best admitted optional
        // candidate first; only a remaining slot may carry a regime whose
        // pre-terminal score is worse but can reverse after final cleanup.
        if self.optional_plan_count_cap != 0 {
            if let Some(index) = ranked.iter().copied().find(|index| {
                slots[*index]
                    .as_ref()
                    .is_some_and(|candidate| !is_redundant(candidate, &retained))
            }) {
                let candidate = slots[index]
                    .take()
                    .expect("the best optional JavaScript plan is present");
                debug_assert!(candidate.raw_size <= remaining_bytes);
                remaining_bytes -= candidate.raw_size;
                retained.push(candidate);
            }
        }
        // Local-name coalescing is scored before the parsed-peephole terminal
        // pass, but that pass can remove the extra declaration syntax from an
        // uncoalesced plan and reverse the codec ranking. Preserve one
        // representative of each non-pinned regime when the budget permits so
        // the exact finalizer, rather than the pre-terminal heuristic, owns
        // that decision.
        let mut represented_coalescing = pinned
            .iter()
            .chain(retained.iter())
            .map(|candidate| candidate.options().local_name_coalescing)
            .collect::<crate::stable_hash::StableHashSet<_>>();
        for regime in [true, false] {
            if represented_coalescing.contains(&regime)
                || retained.len() == self.optional_plan_count_cap
            {
                continue;
            }
            let Some(index) = ranked.iter().copied().find(|index| {
                slots[*index].as_ref().is_some_and(|candidate| {
                    candidate.options().local_name_coalescing == regime
                        && candidate.raw_size <= remaining_bytes
                        && !is_redundant(candidate, &retained)
                })
            }) else {
                continue;
            };
            let candidate = slots[index]
                .take()
                .expect("a local-name regime seed is present");
            remaining_bytes -= candidate.raw_size;
            represented_coalescing.insert(regime);
            retained.push(candidate);
        }
        for index in ranked {
            if retained.len() == self.optional_plan_count_cap {
                break;
            }
            let Some(candidate) = slots[index].take() else {
                continue;
            };
            if candidate.raw_size > remaining_bytes || is_redundant(&candidate, &retained) {
                continue;
            }
            remaining_bytes -= candidate.raw_size;
            retained.push(candidate);
        }
        pinned.extend(retained);
        self.candidates = pinned;
        self.validate_retained_caps()?;
        Ok(())
    }

    fn candidates(&self) -> &[JavaScriptEmissionCandidate] {
        &self.candidates
    }

    fn optional_proposal_width(&self) -> usize {
        if self.optional_plan_count_cap == 0 || self.optional_code_byte_cap == 0 {
            return 0;
        }
        // This is a work/RSS bound, not a claim that every later variant is
        // seed-sized. Use the largest admitted pin as the conservative emitted
        // string estimate, while retaining one discovery slot for any nonzero
        // byte tail because a later variant may be substantially smaller.
        let seed_byte_estimate = self
            .candidates
            .iter()
            .filter(|candidate| self.pinned_identities.contains(&candidate.identity()))
            .map(|candidate| candidate.raw_size)
            .max()
            .unwrap_or(1)
            .max(1);
        self.optional_plan_count_cap.min(
            self.optional_code_byte_cap
                .checked_div(seed_byte_estimate)
                .unwrap_or(0)
                .max(1),
        )
    }

    fn optional_raw_size_cap(&self) -> usize {
        self.optional_code_byte_cap
    }

    fn pinned_context_ids(&self) -> Vec<usize> {
        self.candidates
            .iter()
            .filter(|candidate| self.pinned_identities.contains(&candidate.identity()))
            .map(|candidate| candidate.identity().context_id)
            .collect()
    }

    #[cfg(test)]
    fn retained_plan_count(&self) -> usize {
        self.candidates.len()
    }

    #[cfg(test)]
    fn retained_code_bytes(&self) -> usize {
        self.candidates.iter().fold(0usize, |total, candidate| {
            total.saturating_add(candidate.raw_size)
        })
    }

    fn into_candidates(self) -> Vec<JavaScriptEmissionCandidate> {
        self.candidates
    }

    fn validate_retained_caps(&self) -> Result<(), CompileError> {
        let retained_bytes = self.candidates.iter().fold(0usize, |total, candidate| {
            total.saturating_add(candidate.raw_size)
        });
        if self.candidates.len() > self.effective_plan_count_cap
            || retained_bytes > self.effective_code_byte_cap
        {
            return Err(crate::codegen_js::CodegenError::new(
                Span::empty(0),
                "aggregate JavaScript arena exceeded its retained plan budget",
            )
            .into());
        }
        Ok(())
    }
}

impl std::ops::Deref for AggregateJavaScriptPlanArena {
    type Target = [JavaScriptEmissionCandidate];

    fn deref(&self) -> &Self::Target {
        &self.candidates
    }
}

impl std::ops::DerefMut for AggregateJavaScriptPlanArena {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.candidates
    }
}

fn entropy_alphabet_candidate_options(
    candidates: &mut [JavaScriptEmissionCandidate],
    limit: usize,
    selected_model: CompressionCostModel,
) -> Result<Vec<JavaScriptEmissionPlan>, CompileError> {
    let mut variants = Vec::new();
    let indices =
        objective_stratified_candidate_indices(candidates, candidates.len(), selected_model)?;
    for index in indices {
        let candidate = &candidates[index];
        let parent_identity = candidate.identity();
        let code = candidate.code();
        let options = candidate.options();
        if !options.mangle_identifiers {
            continue;
        }
        // Structural candidates can remove declarations, inline a sole
        // function, or change punctuation long after the initial entropy
        // alphabet was derived. Feed the complete transformed artifact back
        // into the safe emitter rather than renaming JavaScript text: the
        // emitter still owns every binding/reservation proof.
        let mut alphabets = vec![crate::codegen_ir_js::IdentifierAlphabet::for_code(code)];
        if let Ok(binding_characters) = declared_identifier_character_use_counts(code) {
            let contextual =
                crate::codegen_ir_js::IdentifierAlphabet::for_code_excluding_binding_characters(
                    code,
                    &binding_characters,
                );
            if !alphabets.contains(&contextual) {
                alphabets.push(contextual);
            }
        }
        for identifier_alphabet in alphabets {
            let option_candidate = crate::codegen_ir_js::IrJsOptions {
                identifier_alphabet,
                ..options
            };
            if option_candidate.identifier_alphabet != options.identifier_alphabet
                && !variants.iter().any(|variant: &JavaScriptEmissionPlan| {
                    variant.identity.context_id == parent_identity.context_id
                        && variant.options == option_candidate
                })
            {
                variants.push(JavaScriptEmissionPlan {
                    identity: parent_identity,
                    options: option_candidate,
                });
                if variants.len() == limit {
                    return Ok(variants);
                }
            }
        }
    }
    Ok(variants)
}

fn prepare_entropy_source_requests_by<Prepare>(
    entropy_sources: Vec<JavaScriptEmissionCandidate>,
    entropy_width: usize,
    prepare_probe: Prepare,
) -> Vec<(JavaScriptEmissionPlan, String, usize)>
where
    Prepare: Fn(String, crate::codegen_ir_js::IrJsOptions) -> Option<String> + Send + Sync,
{
    let entropy_source_count = entropy_sources.len();
    // `Vec`'s indexed parallel iterator collects into the source order. Only
    // the source-local parsed peephole runs here: plan registration and the
    // adaptive trial ledger remain coordinator-owned below.
    let prepared = entropy_sources
        .into_par_iter()
        .map(|candidate| {
            let parent_plan = candidate.plan;
            prepare_probe(candidate.emission.code, parent_plan.options)
                .map(|probe| (parent_plan, probe))
        })
        .collect::<Vec<_>>();

    let mut remaining_entropy_trials = entropy_mapping_trial_budget(entropy_width);
    let mut entropy_requests = Vec::new();
    for (source_index, prepared_source) in prepared.into_iter().enumerate() {
        let Some((parent_plan, probe)) = prepared_source else {
            continue;
        };
        let trials = entropy_trials_for_next_source(
            entropy_width,
            probe.len(),
            remaining_entropy_trials,
            entropy_source_count.saturating_sub(source_index),
        );
        remaining_entropy_trials = remaining_entropy_trials.saturating_sub(trials);
        if trials != 0 {
            entropy_requests.push((parent_plan, probe, trials));
        }
    }
    entropy_requests
}

fn search_identifier_alphabet_groups(
    requests: Vec<(JavaScriptEmissionPlan, String, usize)>,
    model: CompressionCostModel,
) -> Vec<Vec<(usize, crate::codegen_ir_js::IrJsOptions)>> {
    search_identifier_alphabet_groups_by(requests, |probe, baseline, trials| {
        search_identifier_alphabets(probe, baseline, model, trials, 4)
    })
}

fn search_identifier_alphabet_groups_by<Error>(
    requests: Vec<(JavaScriptEmissionPlan, String, usize)>,
    mut search: impl FnMut(
        &str,
        crate::codegen_ir_js::IdentifierAlphabet,
        usize,
    ) -> Result<Vec<crate::codegen_ir_js::IdentifierAlphabet>, Error>,
) -> Vec<Vec<(usize, crate::codegen_ir_js::IrJsOptions)>> {
    requests
        .into_iter()
        .filter_map(|(parent_plan, probe, trials)| {
            let options = parent_plan.options;
            let alphabets = search(&probe, options.identifier_alphabet, trials).ok()?;
            Some(
                alphabets
                    .into_iter()
                    .filter(|alphabet| *alphabet != options.identifier_alphabet)
                    .map(|identifier_alphabet| {
                        (
                            parent_plan.identity.context_id,
                            crate::codegen_ir_js::IrJsOptions {
                                identifier_alphabet,
                                ..options
                            },
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

const ONE_BYTE_IDENTIFIER_STARTS: &[u8] = b"etnrisouacldhpfmgybvwkxzjqETNRISOUACLDHPFMGYBVWKXZJQ_$";
const UNUSED_LETTER_REMAP_PASSES: usize = 8;
const EXACT_TWO_BINDING_UNUSED_NAME_LIMIT: usize = 8;
const EXACT_TWO_BINDING_MAX_PAIR_TRIALS: usize = EXACT_TWO_BINDING_UNUSED_NAME_LIMIT
    .saturating_mul(EXACT_TWO_BINDING_UNUSED_NAME_LIMIT.saturating_sub(1));

fn unused_letter_remap_pair_budget(code_len: usize) -> usize {
    match code_len {
        0..4_096 => 512,
        4_096..8_192 => 384,
        8_192..16_384 => 192,
        16_384..65_536 => 96,
        _ => 48,
    }
}

fn retain_resolved_javascript(
    previous: ScoredJavaScriptCandidate,
    next: ScoredJavaScriptCandidate,
) -> ScoredJavaScriptCandidate {
    if next.code == previous.code || next.admission.validate(&next.code).is_ok() {
        next
    } else {
        previous
    }
}

fn apply_unused_letter_binding_remaps(
    mut selected: ScoredJavaScriptCandidate,
    config: &ProjectConfig,
    include_live_swaps: bool,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<ScoredJavaScriptCandidate, CompileError> {
    if codec_budget.remaining() == 0
        || !config.js_options().mangle_identifiers
        || !config.entropy_aware_mangling_enabled()
    {
        return Ok(selected);
    }
    let mut code = selected.code.clone();
    let mut transfer_cost = selected.transfer_cost;
    // A bijection between live one-byte names cannot improve the raw-byte
    // objective. Under Raw, only a proven two-byte-to-one-byte replacement is
    // useful; avoiding the other neighborhoods also lets the finalizer reuse
    // an exact declaration score ledger without redundant measurements.
    if !matches!(config.javascript.cost_model, CompressionCostModel::Raw) {
        if let Some((next, cost)) =
            best_unused_letter_binding_remaps(&code, config.javascript.cost_model, codec_budget)?
        {
            code = next;
            transfer_cost = cost;
        }
    } else {
        let has_shortenable_binding = two_character_identifier_use_counts(&code)
            .map_err(generated_javascript_parse_error)?
            .into_iter()
            .any(|(name, _)| {
                !TWO_BYTE_RESERVED_BINDINGS.contains(&name.as_str())
                    && identifier_name_is_clear_binding(&code, &name).unwrap_or(false)
            });
        if !has_shortenable_binding {
            return Ok(selected);
        }
    }
    if let Some((next, cost)) =
        best_short_binding_remaps(&code, config.javascript.cost_model, codec_budget)?
    {
        code = next;
        transfer_cost = cost;
    }
    if include_live_swaps && !matches!(config.javascript.cost_model, CompressionCostModel::Raw) {
        if let Some((next, cost)) =
            best_live_letter_binding_remaps(&code, config.javascript.cost_model, codec_budget)?
        {
            code = next;
            transfer_cost = cost;
        }
        if let Some((next, cost)) =
            best_function_local_binding_remaps(&code, config.javascript.cost_model, codec_budget)?
        {
            code = next;
            transfer_cost = cost;
        }
    }
    if code == selected.code {
        return Ok(selected);
    }
    if selected.admission.validate(&code).is_err() {
        return Ok(selected);
    }
    selected.metrics =
        analyze_generated_javascript(&code).map_err(generated_javascript_parse_error)?;
    selected.startup_score = selected.metrics.startup_score(
        config.javascript.startup.parse_weight,
        config.javascript.startup.compile_weight,
        config.javascript.startup.memory_weight,
    );
    selected.code = code;
    selected.transfer_cost = transfer_cost;
    Ok(selected)
}

fn apply_terminal_boolean_binding_remap(
    mut selected: ScoredJavaScriptCandidate,
    config: &ProjectConfig,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<ScoredJavaScriptCandidate, CompileError> {
    if codec_budget.remaining() == 0
        || !config.js_options().mangle_identifiers
        || !config.entropy_aware_mangling_enabled()
        || matches!(config.javascript.cost_model, CompressionCostModel::Raw)
    {
        return Ok(selected);
    }
    if !codec_budget.reserve_work_unit() {
        return Ok(selected);
    }
    let Ok(boolean_code) = late_generated_javascript_cleanup_pass(
        &selected.code,
        LateJavaScriptCleanupPass::BooleanConditionalValues,
    ) else {
        return Ok(selected);
    };
    let boolean_code = repair_late_javascript_candidate(boolean_code);
    if boolean_code == selected.code || selected.admission.validate(&boolean_code).is_err() {
        return Ok(selected);
    }
    let boolean_cost = codec_budget
        .measure_reserved_compile(boolean_code.as_bytes(), config.javascript.cost_model)?;
    let (candidate, candidate_cost) =
        if matches!(config.javascript.cost_model, CompressionCostModel::Raw) {
            (boolean_code, boolean_cost)
        } else {
            best_function_local_binding_remaps_with_passes(
                &boolean_code,
                config.javascript.cost_model,
                TERMINAL_BOOLEAN_BINDING_REMAP_PASSES,
                codec_budget,
            )?
            .unwrap_or((boolean_code, boolean_cost))
        };
    if candidate_cost >= selected.transfer_cost {
        return Ok(selected);
    }
    if selected.admission.validate(&candidate).is_err() {
        return Ok(selected);
    }
    let metrics =
        analyze_generated_javascript(&candidate).map_err(generated_javascript_parse_error)?;
    selected.startup_score = metrics.startup_score(
        config.javascript.startup.parse_weight,
        config.javascript.startup.compile_weight,
        config.javascript.startup.memory_weight,
    );
    selected.metrics = metrics;
    selected.code = candidate;
    selected.transfer_cost = candidate_cost;
    Ok(selected)
}

fn exact_two_binding_terminal_search_enabled(config: &ProjectConfig) -> bool {
    config.js_options().mangle_identifiers
        && config.entropy_aware_mangling_enabled()
        && !matches!(config.javascript.cost_model, CompressionCostModel::Raw)
        // This exact neighborhood is deliberately a high-effort terminal
        // search. Level 12 is the first built-in tier with enough retained
        // capacity to pay its bounded codec work; lower tiers keep the
        // existing greedy/randomized naming neighborhoods.
        && config.javascript.effective_candidate_limit() >= 768
}

fn exact_two_binding_terminal_search_enabled_for_artifact(
    config: &ProjectConfig,
    raw_size: usize,
) -> bool {
    raw_size <= 16 * 1024 && exact_two_binding_terminal_search_enabled(config)
}

fn apply_exact_two_binding_unused_letter_remap(
    mut selected: SelectedJavaScriptCandidate,
    config: &ProjectConfig,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    if codec_budget.remaining() == 0
        || !exact_two_binding_terminal_search_enabled_for_artifact(config, selected.code.len())
    {
        return Ok(selected);
    }
    let Some((code, transfer_cost)) = best_two_binding_unused_letter_remap(
        &selected.code,
        config.javascript.cost_model,
        codec_budget,
    )?
    else {
        return Ok(selected);
    };
    if selected.admission.validate(&code).is_err() {
        return Ok(selected);
    }
    let metrics = analyze_generated_javascript(&code).map_err(generated_javascript_parse_error)?;
    selected.startup_score = metrics.startup_score(
        config.javascript.startup.parse_weight,
        config.javascript.startup.compile_weight,
        config.javascript.startup.memory_weight,
    );
    selected.metrics = metrics;
    selected.code = code;
    selected.transfer_cost = transfer_cost;
    selected.candidates_evaluated = selected.candidates_evaluated.saturating_add(1);
    Ok(selected)
}

fn apply_terminal_binding_coordinate_descent(
    mut selected: ScoredJavaScriptCandidate,
    config: &ProjectConfig,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<ScoredJavaScriptCandidate, CompileError> {
    if codec_budget.remaining() == 0
        || !config.js_options().mangle_identifiers
        || !config.entropy_aware_mangling_enabled()
        || matches!(config.javascript.cost_model, CompressionCostModel::Raw)
    {
        return Ok(selected);
    }

    const LOCAL_PASSES: usize = 4;
    const LOCAL_BEAM_WIDTH: usize = 24;
    const GLOBAL_PASSES: usize = 4;
    const GLOBAL_BEAM_WIDTH: usize = 48;

    let mut code = selected.code.clone();
    let mut transfer_cost = selected.transfer_cost;
    if let Some((next, cost)) = best_function_local_binding_remaps_with_beam(
        &code,
        config.javascript.cost_model,
        LOCAL_PASSES,
        LOCAL_BEAM_WIDTH,
        codec_budget,
    )? {
        code = next;
        transfer_cost = cost;
    }
    // Local permutations change cross-function token adjacency. Re-open the
    // whole-program namespace afterward and retain temporary non-winners so a
    // short sequence of bijective swaps can escape the greedy local minimum.
    if let Some((next, cost)) = best_live_letter_binding_remaps_with_beam(
        &code,
        config.javascript.cost_model,
        GLOBAL_PASSES,
        GLOBAL_BEAM_WIDTH,
        codec_budget,
    )? {
        code = next;
        transfer_cost = cost;
    }
    if code == selected.code {
        return Ok(selected);
    }
    let metrics = analyze_generated_javascript(&code).map_err(generated_javascript_parse_error)?;
    selected.startup_score = metrics.startup_score(
        config.javascript.startup.parse_weight,
        config.javascript.startup.compile_weight,
        config.javascript.startup.memory_weight,
    );
    selected.metrics = metrics;
    selected.code = code;
    selected.transfer_cost = transfer_cost;
    Ok(selected)
}

fn peephole_preserve_or_baseline(
    declaration: String,
    baseline_metrics: JavaScriptSyntaxMetrics,
    is_declaration_spelling: bool,
    pristine_builtins: bool,
) -> (String, JavaScriptSyntaxMetrics, usize, bool) {
    match optimize_generated_javascript_preserving_functions_assuming(
        &declaration,
        pristine_builtins,
    ) {
        Ok(preserved) if preserved.code != declaration => {
            if let Ok(metrics) = analyze_generated_javascript(&preserved.code) {
                if metrics.functions >= baseline_metrics.functions {
                    return (
                        preserved.code,
                        metrics,
                        preserved.rewrites,
                        is_declaration_spelling,
                    );
                }
            }
        }
        _ => {}
    }
    (declaration, baseline_metrics, 0, is_declaration_spelling)
}

fn configured_declaration_peephole(
    declaration: String,
    baseline_metrics: JavaScriptSyntaxMetrics,
    allow_function_elision: bool,
    pristine_builtins: bool,
) -> (String, JavaScriptSyntaxMetrics, usize, bool) {
    if allow_function_elision {
        if let Ok(optimized) =
            optimize_generated_javascript_assuming(&declaration, pristine_builtins)
        {
            let code = repair_late_javascript_candidate(optimized.code);
            if let Ok(metrics) = analyze_generated_javascript(&code) {
                return (code, metrics, optimized.rewrites, true);
            }
        }
    }
    let (code, metrics, rewrites, _) =
        peephole_preserve_or_baseline(declaration, baseline_metrics, true, pristine_builtins);
    if rewrites == 0 {
        return (code, metrics, 0, true);
    }
    let repaired = repair_late_javascript_candidate(code.clone());
    if let Ok(repaired_metrics) = analyze_generated_javascript(&repaired) {
        if repaired_metrics.functions >= baseline_metrics.functions {
            return (repaired, repaired_metrics, rewrites, true);
        }
    }
    (code, metrics, rewrites, true)
}

fn apply_selected_canonical_peephole(
    mut selected: SelectedJavaScriptCandidate,
    config: &ProjectConfig,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    // Search probes bound permutation scoring. The selected artifact still
    // gets one canonical rewrite: otherwise a full ledger leaves ParsedPeephole
    // as a no-op on the winner, even when that rewrite is cheaper.
    const CANONICAL_PEEPHOLE_WORK_UNITS: usize = 2;
    if selected.terminal_codec_probe_limit == 0
        || selected.has_explicit_lowering_obligations
        || !config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole)
    {
        return Ok(selected);
    }
    // Deliberately *not* gated on the remaining probe budget. Charging these two
    // units against the same ledger as the search probes means a big artifact --
    // exactly the kind with the most to gain -- spends its budget on permutation
    // scoring and then skips the one rewrite this function exists to guarantee.
    // Measured on micromarklil, whose ledger is full long before here: the emitted
    // artifact still had 434 `;var ` runs the canonical rewrite merges, and running
    // it cost 3574 raw and 171 Brotli to skip. The rewrite is two units against a
    // limit of 384 and is still scored below like any other candidate, so it can
    // only be kept when it measures smaller.
    selected.terminal_work_units = selected
        .terminal_work_units
        .saturating_add(CANONICAL_PEEPHOLE_WORK_UNITS);
    let Ok(optimized) = optimize_generated_javascript_assuming(
        &selected.code,
        config.javascript.assume_pristine_builtins,
    ) else {
        return Ok(selected);
    };
    let code = repair_late_javascript_candidate(optimized.code);
    if code == selected.code {
        return Ok(selected);
    }
    let Ok(metrics) = analyze_generated_javascript(&code) else {
        return Ok(selected);
    };
    if !config.single_use_function_expression_candidates_enabled()
        && metrics.functions < selected.metrics.functions
    {
        return Ok(selected);
    }
    if config
        .javascript
        .startup
        .max_nesting
        .is_some_and(|maximum| metrics.max_nesting > maximum)
        || (config.javascript_optimization_configured(JavaScriptOptimization::StartupCostGuard)
            && !startup_cost_allowed(
                metrics,
                selected.baseline_metrics,
                &config.javascript.startup,
            ))
    {
        return Ok(selected);
    }
    if selected.admission.validate_selected(&code).is_err() {
        return Ok(selected);
    }
    let cost = admitted_generated_javascript_size(&code, config.javascript.cost_model)
        .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
    selected.terminal_codec_probes = selected.terminal_codec_probes.saturating_add(1);
    let startup_score = metrics.startup_score(
        config.javascript.startup.parse_weight,
        config.javascript.startup.compile_weight,
        config.javascript.startup.memory_weight,
    );
    let mut challenger = selected.clone();
    challenger.code = code;
    challenger.transfer_cost = cost;
    challenger.startup_score = startup_score;
    challenger.metrics = metrics;
    challenger.peephole_rewrites = challenger
        .peephole_rewrites
        .saturating_add(optimized.rewrites);
    if !finalized_javascript_candidate_precedes(
        &challenger,
        &selected,
        config,
        selected.baseline_transfer,
    ) {
        return Ok(selected);
    }
    Ok(challenger)
}

fn apply_search_off_declaration_peephole(
    selected: SelectedJavaScriptCandidate,
    config: &ProjectConfig,
) -> Result<SelectedJavaScriptCandidate, CompileError> {
    if config.javascript.candidate_search_enabled()
        || selected.has_explicit_lowering_obligations
        || selected.peephole_rewrites > 0
        || !config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole)
    {
        return Ok(selected);
    }
    let (code, metrics, rewrites, _) = configured_declaration_peephole(
        selected.code.clone(),
        selected.baseline_metrics,
        config.single_use_function_expression_candidates_enabled(),
        config.javascript.assume_pristine_builtins,
    );
    if rewrites == 0 || code == selected.code {
        return Ok(selected);
    }
    if config
        .javascript
        .startup
        .max_nesting
        .is_some_and(|maximum| metrics.max_nesting > maximum)
        || (config.javascript_optimization_configured(JavaScriptOptimization::StartupCostGuard)
            && !startup_cost_allowed(
                metrics,
                selected.baseline_metrics,
                &config.javascript.startup,
            ))
    {
        return Ok(selected);
    }
    if selected.admission.validate(&code).is_err() {
        return Ok(selected);
    }
    let transfer_cost = admitted_generated_javascript_size(&code, config.javascript.cost_model)
        .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
    let mut challenger = selected.clone();
    challenger.code = code;
    challenger.metrics = metrics;
    challenger.transfer_cost = transfer_cost;
    challenger.peephole_rewrites = rewrites;
    challenger.terminal_codec_probes = challenger.terminal_codec_probes.saturating_add(1);
    challenger.startup_score = metrics.startup_score(
        config.javascript.startup.parse_weight,
        config.javascript.startup.compile_weight,
        config.javascript.startup.memory_weight,
    );
    if finalized_javascript_candidate_precedes(
        &challenger,
        &selected,
        config,
        selected.baseline_transfer,
    ) {
        Ok(challenger)
    } else {
        Ok(selected)
    }
}

fn repair_late_javascript_candidate(mut code: String) -> String {
    if let Ok(repaired) =
        late_generated_javascript_cleanup_pass(&code, LateJavaScriptCleanupPass::OrAssignmentParens)
    {
        code = repaired;
    }
    if let Ok((repaired, rewritten)) = fold_constant_json_parse(&code) {
        if rewritten > 0 {
            code = repaired;
        }
    }
    if let Ok((repaired, rewritten)) = fold_redundant_null_undefined_or(&code) {
        if rewritten > 0 {
            code = repaired;
        }
    }
    if let Ok((repaired, rewritten)) = fold_dead_identifier_copy_declarators(&code) {
        if rewritten > 0 {
            code = repaired;
        }
    }
    if let Ok((repaired, rewritten)) = fold_dead_increment_snapshots(&code) {
        if rewritten > 0 {
            code = repaired;
        }
    }
    if let Ok((repaired, rewritten)) = fold_pristine_static_method_calls(&code) {
        if rewritten > 0 {
            code = repaired;
        }
    }
    if let Ok((repaired, rewritten)) = fold_if_prefixed_returns(&code) {
        if rewritten > 0 {
            code = repaired;
        }
    }
    if let Ok((repaired, rewritten)) = fold_nested_unguarded_ifs(&code) {
        if rewritten > 0 {
            code = repaired;
        }
    }
    if let Ok(repaired) = repair_fused_keyword_identifiers(&code) {
        code = repaired;
    }
    code
}

/// One spelling of the finalist that the terminal cleanup is considering, with
/// what the codec charged for it.
#[derive(Clone)]
struct CleanupCandidate {
    code: String,
    cost: usize,
}

/// Offer one scored cleanup family: rewrite each beam member, keep what the
/// codec says is cheaper.
///
/// `share` bounds how many probes the family may spend. The families divide the
/// cleanup's slice between them rather than draining it in order: on katexlil
/// the shaped-declaration candidate is worth 831 Brotli, and a family placed
/// ahead of it that offered itself to all eight beam members starved it for
/// +1356. What a family leaves unspent stays for the next one, so an equal
/// division costs nothing when families are cheap or refuse early.
fn offer_cleanup_family(
    beam: &mut Vec<CleanupCandidate>,
    codec_budget: &mut TerminalCodecProbeBudget,
    admission: &JavaScriptArtifactAdmission,
    cost_model: CompressionCostModel,
    share: usize,
    shape: impl Fn(&str) -> Option<String>,
) -> Result<(), CompileError> {
    let mut spent = 0usize;
    for candidate in beam.clone() {
        if spent >= share || !codec_budget.reserve_work_unit() {
            break;
        }
        spent += 1;
        let Some(rewritten) = shape(&candidate.code) else {
            continue;
        };
        let code = repair_late_javascript_candidate(rewritten);
        if code == candidate.code {
            continue;
        }
        if analyze_generated_javascript(&code).is_err()
            || admission.validate(&code).is_err()
            || beam.iter().any(|existing| existing.code == code)
        {
            crate::timing::CLEANUP_SHAPED_REFUSED.event(0);
            continue;
        }
        let Some(cost) = codec_budget.compressed_size(code.as_bytes(), cost_model)? else {
            continue;
        };
        if cost < candidate.cost {
            crate::timing::CLEANUP_SHAPED_PUSHED.event((candidate.cost - cost) as u64);
            beam.push(CleanupCandidate { code, cost });
        } else {
            crate::timing::CLEANUP_SHAPED_LOST.event((cost - candidate.cost) as u64);
        }
    }
    Ok(())
}

fn apply_late_javascript_cleanup(
    selected: ScoredJavaScriptCandidate,
    config: &ProjectConfig,
    terminal_local_rounds: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<ScoredJavaScriptCandidate, CompileError> {
    let fallback = selected.clone();
    Ok(
        late_javascript_cleanup_finalists(selected, config, terminal_local_rounds, codec_budget, 1)?
            .into_iter()
            .next()
            .unwrap_or(fallback),
    )
}

/// The cleanup beam's best `keep` spellings, each materialised as a candidate.
///
/// Keeping more than one exists because the beam ranks by what a spelling costs
/// *here*, while the namespace remapping that runs after it decides the bytes
/// that ship. Those stages are not monotone in the cleanup's cost: a locally
/// cheaper spelling can remap worse, which is how a single extra candidate has
/// been measured moving a finished artifact by more than a percent. Carrying
/// several and ranking them by their finished cost is the only way to tell.
fn late_javascript_cleanup_finalists(
    mut selected: ScoredJavaScriptCandidate,
    config: &ProjectConfig,
    terminal_local_rounds: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
    keep: usize,
) -> Result<Vec<ScoredJavaScriptCandidate>, CompileError> {
    // Late syntax search is the terminal half of ParsedPeephole. An explicit
    // optimization allowlist that omits that feature must preserve the exact
    // emitter spelling (and its already-measured declaration score ledger).
    if selected.has_explicit_lowering_obligations
        || codec_budget.remaining() == 0
        || !config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole)
    {
        if codec_budget.remaining() == 0 {
            crate::timing::CLEANUP_UNBUDGETED.event(0);
        } else {
            crate::timing::CLEANUP_SKIPPED.event(0);
        }
        return Ok(vec![selected]);
    }
    crate::timing::CLEANUP_ENTERED.event(codec_budget.remaining() as u64);

    // Keep enough independently valid spellings for later passes to skip a
    // locally attractive rewrite. Small codec differences can otherwise
    // discard the topology that wins after terminal namespace remapping.
    const BEAM_WIDTH: usize = 8;
    const ROUNDS: usize = 2;
    let admission = Arc::clone(&selected.admission);

    let original = CleanupCandidate {
        code: selected.code.clone(),
        cost: selected.transfer_cost,
    };
    let mut beam = vec![original.clone()];
    // A terminal finalist need not have been among the bounded plans admitted
    // to parsed preparation. Re-open the canonical peephole on the finalist
    // itself before narrower cleanup families spend its fair slice. This is
    // especially important when an earlier, highly ranked parsed plan is
    // rejected for syntax: the best unparsed fallback may expose the same
    // large general rewrite only after plan selection.
    let mut canonical_peephole = None;
    if codec_budget.reserve_work_unit() {
        let optimized = optimize_generated_javascript_assuming(
            &original.code,
            config.javascript.assume_pristine_builtins,
        );
        if optimized.is_err() {
            crate::timing::CLEANUP_CANONICAL_ERR.event(0);
        }
        if let Ok(optimized) = optimized {
            let code = repair_late_javascript_candidate(optimized.code);
            let metrics = analyze_generated_javascript(&code).ok();
            let crosses_disabled_function_boundary = !config
                .single_use_function_expression_candidates_enabled()
                && metrics.is_some_and(|metrics| metrics.functions < selected.metrics.functions);
            if crosses_disabled_function_boundary {
                crate::timing::CLEANUP_CANONICAL_BOUNDARY.event(0);
            }
            let (code, metrics) = if crosses_disabled_function_boundary {
                let (code, _, _, _) = peephole_preserve_or_baseline(
                    original.code.clone(),
                    selected.metrics,
                    true,
                    config.javascript.assume_pristine_builtins,
                );
                let code = repair_late_javascript_candidate(code);
                let metrics = analyze_generated_javascript(&code).ok();
                (code, metrics)
            } else {
                (code, metrics)
            };
            if code == original.code {
                crate::timing::CLEANUP_CANONICAL_SAME.event(0);
            } else if !metrics.is_some_and(|metrics| {
                config.single_use_function_expression_candidates_enabled()
                    || metrics.functions >= selected.metrics.functions
            }) {
                crate::timing::CLEANUP_CANONICAL_BOUNDARY.event(0);
            } else if let Err(refusal) = admission.validate_selected(&code) {
                // `validate_selected`: this is a re-check of the selected artifact
                // after a whole-artifact rewrite, the one place the class rewrite's
                // own `constructor` is exempt (see `validate_observed_javascript_artifact_allowing`);
                // the candidate is still scored by the codec before it enters the beam.
                crate::timing::CLEANUP_CANONICAL_REFUSED.event(0);
                if std::env::var_os("LILSCRIPT_TIMING").is_some() {
                    eprintln!("late-cleanup canonical peephole refused: {refusal:?}");
                }
            } else if let Some(cost) =
                codec_budget.compressed_size(code.as_bytes(), config.javascript.cost_model)?
            {
                crate::timing::CLEANUP_CANONICAL_PUSHED.event(cost as u64);
                let candidate = CleanupCandidate { code, cost };
                beam.push(candidate.clone());
                canonical_peephole = Some(candidate);
            } else {
                crate::timing::CLEANUP_CANONICAL_UNPROBED.event(0);
            }
        }
    }
    // Converged local naming: reassign each function scope's own bindings from
    // one canonical sequence so same-arity headers spell identically. An LZ
    // match costs about the same however long its text is, so a repeated
    // spelling can beat several shorter ones -- but it costs raw bytes and the
    // trade only pays on some artifacts (measured: Monaco -343 Brotli, otlp
    // -28, jQuery +43). Score it and keep it only where it wins.
    if config.js_options().mangle_identifiers
        && !matches!(config.javascript.cost_model, CompressionCostModel::Raw)
    {
        let sources = beam.clone();
        let source_count = sources.len();
        for (position, candidate) in sources.into_iter().enumerate() {
            if !codec_budget.reserve_work_unit() {
                crate::timing::RENAME_STARVED.event((source_count - position) as u64);
                break;
            }
            crate::timing::RENAME_CANDIDATES.event(candidate.code.len() as u64);
            let Ok((converged, rewrites)) = converge_local_names(&candidate.code) else {
                crate::timing::RENAME_UNPARSED.event(0);
                continue;
            };
            if rewrites == 0 || converged == candidate.code {
                crate::timing::RENAME_IDLE.event(0);
                continue;
            }
            if analyze_generated_javascript(&converged).is_err() {
                crate::timing::RENAME_UNPARSED.event(rewrites as u64);
                continue;
            }
            if admission.validate(&converged).is_err() {
                crate::timing::RENAME_REFUSED.event(rewrites as u64);
                continue;
            }
            let Some(cost) =
                codec_budget.compressed_size(converged.as_bytes(), config.javascript.cost_model)?
            else {
                crate::timing::RENAME_UNPROBED.event(rewrites as u64);
                continue;
            };
            if cost < candidate.cost {
                crate::timing::RENAME_WON.event((candidate.cost - cost) as u64);
                beam.push(CleanupCandidate {
                    code: converged,
                    cost,
                });
            } else {
                crate::timing::RENAME_LOST.event((cost - candidate.cost) as u64);
            }
        }
    }
    // The uniform scored families, offered in order, each rewriting every beam
    // member and keeping what the codec says is cheaper.
    //
    // They divide the cleanup's slice rather than draining it in order. That
    // slice is eight work units for all of them, so a family that offers itself
    // to all eight members leaves nothing for the ones behind it -- measured on
    // katexlil, a family placed ahead of declaration shaping starved a
    // candidate worth 831 Brotli and cost 1356. An equal share bounds that, and
    // what a family leaves unspent stays for the next, so cheap families and
    // early refusals still give the later ones everything they would have had.
    let pristine = config.javascript.assume_pristine_builtins;
    let mangles = config.js_options().mangle_identifiers;
    let families: [(bool, &dyn Fn(&str) -> Option<String>); 4] = [
        // `d={p:1};…;d.k=v` is one object, and the builders a port ends in are
        // written that way. Gated like the session's empty-literal case: a
        // literal property is an own property, where an assignment goes through
        // whatever setter the prototype chain offers.
        (pristine, &|code| {
            crate::js_peephole::absorb_property_writes_into_literals(code)
                .ok()
                .filter(|(_, rewrites)| *rewrites > 0)
                .map(|(absorbed, _)| absorbed)
        }),
        // One `var` per module binding, each initialised `void 0`, is the
        // emitter's faithful spelling of `JsValue x = undef()` globals; the
        // joins only exist across declarations, so the per-declaration pass
        // cannot reach them. Applied unconditionally it lost on two portfolio
        // ports to naming cascades, so it is scored here instead.
        (true, &|code| {
            crate::js_peephole::shape_declarations(code)
                .ok()
                .filter(|(_, rewrites)| *rewrites > 0)
                .map(|(shaped, _)| shaped)
        }),
        // `new RegExp("…")` → `/…/`; the compact lexer's `/` certainty is why
        // this is a scored candidate rather than an ordinary fold.
        (pristine, &|code| {
            crate::js_peephole::spell_regexp_literals(code)
                .ok()
                .filter(|(_, rewrites)| *rewrites > 0)
                .map(|(spelled, _)| spelled)
        }),
        // A function bound once and read once costs a declarator for nothing.
        // Moving the literal drops the name JavaScript infers from the binding,
        // so it runs only where identifier mangling is already in force: such a
        // build has replaced every inferred name with a generated one already.
        (mangles, &|code| {
            // One move can expose the next: a list that loses its last function
            // declarator leaves a statement the ordinary passes can fold. The
            // result is offered as it stands -- running those passes here was
            // measured as a loss, buying raw bytes by specializing shapes (on
            // jQuery a 33-byte Brotli win became 16); the cleanup rounds below
            // still reach this candidate.
            let mut inlined = code.to_string();
            let mut moved = 0usize;
            for _ in 0..4 {
                let Ok((next, count)) = inline_single_use_functions(&inlined) else {
                    break;
                };
                if count == 0 || next == inlined {
                    break;
                }
                inlined = next;
                moved += count;
            }
            (moved > 0).then_some(inlined)
        }),
    ];
    let mut enabled = families.iter().filter(|(on, _)| *on).count();
    for (on, shape) in families {
        if !on {
            continue;
        }
        let share = codec_budget.remaining().div_ceil(enabled);
        enabled -= 1;
        offer_cleanup_family(
            &mut beam,
            codec_budget,
            &admission,
            config.javascript.cost_model,
            share,
            shape,
        )?;
    }
    // Braces and `return` around a body that is only expressions are syntax
    // spent on nothing: `()=>{q();return v}` says what `()=>(q(),v)` says in
    // six fewer bytes, and the sequence form is one shape where the block form
    // was several. Scored, because a shape that repeats can beat a shape that
    // is short.
    {
        let sources = beam.clone();
        for candidate in sources {
            if !codec_budget.reserve_work_unit() {
                break;
            }
            let mut folded = candidate.code.clone();
            let mut moved = 0usize;
            for _ in 0..4 {
                let Ok((next, count)) = fold_expression_bodies(&folded) else {
                    break;
                };
                if count == 0 || next == folded {
                    break;
                }
                folded = next;
                moved += count;
            }
            if moved == 0 || folded == candidate.code {
                continue;
            }
            let code = repair_late_javascript_candidate(folded);
            if analyze_generated_javascript(&code).is_err()
                || admission.validate(&code).is_err()
                || beam.iter().any(|existing| existing.code == code)
            {
                continue;
            }
            let Some(cost) =
                codec_budget.compressed_size(code.as_bytes(), config.javascript.cost_model)?
            else {
                continue;
            };
            if cost < candidate.cost {
                beam.push(CleanupCandidate { code, cost });
            }
        }
    }
    // A namespace change can be locally neutral or worse yet unlock whole
    // single-use function movement by separating a binding from a shadowing
    // callback parameter. Carry a tiny, deterministic punctuation-name
    // neighborhood through parsed cleanup before the general cleanup beam can
    // spend this finalist's fair slice. Every attempted remap is charged
    // before repair/analysis and every valid leaf pays its exact-codec unit.
    if config.js_options().mangle_identifiers
        && config.entropy_aware_mangling_enabled()
        && !matches!(config.javascript.cost_model, CompressionCostModel::Raw)
    {
        let identifiers = single_character_identifiers(&original.code)
            .map_err(generated_javascript_parse_error)?;
        let mut sources = single_character_resolved_binding_identifiers(&original.code)
            .map_err(generated_javascript_parse_error)?;
        let counts = single_character_identifier_use_counts(&original.code)
            .map_err(generated_javascript_parse_error)?;
        sources.sort_unstable_by(|left, right| {
            counts[*right as usize]
                .cmp(&counts[*left as usize])
                .then_with(|| left.cmp(right))
        });
        'factored_naming: for replacement in [b'_', b'$'] {
            if identifiers.contains(&replacement) {
                continue;
            }
            for source in sources.iter().copied().take(8) {
                if !codec_budget.reserve_work_unit() {
                    break 'factored_naming;
                }
                let mut mapping = std::array::from_fn(|index| index as u8);
                mapping[source as usize] = replacement;
                mapping[replacement as usize] = source;
                let Ok(remapped) = remap_single_character_identifiers(&original.code, &mapping)
                else {
                    continue;
                };
                let Ok(optimized) = optimize_generated_javascript_assuming(
                    &remapped,
                    config.javascript.assume_pristine_builtins,
                ) else {
                    continue;
                };
                let code = repair_late_javascript_candidate(optimized.code);
                if code == original.code
                    || analyze_generated_javascript(&code).is_err()
                    || admission.validate(&code).is_err()
                    || beam.iter().any(|candidate| candidate.code == code)
                {
                    continue;
                }
                let Some(cost) =
                    codec_budget.compressed_size(code.as_bytes(), config.javascript.cost_model)?
                else {
                    break 'factored_naming;
                };
                beam.push(CleanupCandidate { code, cost });
            }
        }
    }
    'cleanup_rounds: for _ in 0..ROUNDS {
        for pass in LateJavaScriptCleanupPass::ALL {
            // Skipping a rewrite is a first-class branch. In particular, a
            // raw-byte reduction is not assumed to help either dictionary
            // codec, and a codec win in one artifact is not generalized to
            // another artifact.
            let mut proposals = beam.clone();
            let mut exhausted = false;
            for candidate in &beam {
                // Charge the proposal before transformation, repair, or
                // whole-artifact validation. Invalid/no-op variants still
                // consume one deterministic work unit, preventing syntax
                // analysis from multiplying beyond the codec-call cap.
                if !codec_budget.reserve_work_unit() {
                    exhausted = true;
                    break;
                }
                let Ok(code) = late_generated_javascript_cleanup_pass(&candidate.code, pass) else {
                    continue;
                };
                if code == candidate.code
                    || analyze_generated_javascript(&code).is_err()
                    || admission.validate(&code).is_err()
                    || proposals.iter().any(|proposal| proposal.code == code)
                {
                    continue;
                }
                let cost = codec_budget
                    .measure_reserved_compile(code.as_bytes(), config.javascript.cost_model)?;
                proposals.push(CleanupCandidate { code, cost });
            }
            proposals.sort_by(|left, right| {
                (left.cost, left.code.len()).cmp(&(right.cost, right.code.len()))
            });
            proposals.dedup_by(|left, right| left.code == right.code);
            proposals.truncate(BEAM_WIDTH);
            beam = proposals;
            if exhausted {
                break 'cleanup_rounds;
            }
        }
    }

    // Pin the historical all-pass pipeline as a synergy proposal. The beam
    // normally rediscovers it, but an individually losing precursor can be
    // necessary for a later fold and may have been pruned at an earlier step.
    if codec_budget.reserve_work_unit() {
        if let Ok(code) = late_generated_javascript_cleanup(&original.code) {
            let code = repair_late_javascript_candidate(code);
            if code != original.code
                && analyze_generated_javascript(&code).is_ok()
                && admission.validate(&code).is_ok()
                && !beam.iter().any(|candidate| candidate.code == code)
            {
                let cost = codec_budget
                    .measure_reserved_compile(code.as_bytes(), config.javascript.cost_model)?;
                if let Some((remapped, remapped_cost)) = best_one_function_local_binding_remap(
                    &code,
                    config.javascript.cost_model,
                    cost,
                    codec_budget,
                )? {
                    beam.push(CleanupCandidate {
                        code: remapped,
                        cost: remapped_cost,
                    });
                }
                beam.push(CleanupCandidate { code, cost });
            }
        }
    }
    // A locally worse canonical structural leaf can still expose a different
    // binding graph whose unused one-byte name wins under gzip or Brotli.
    // Score that interaction before collapsing the cleanup beam, reusing the
    // canonical leaf's already-paid exact cost.
    if config.js_options().mangle_identifiers
        && config.entropy_aware_mangling_enabled()
        && !matches!(config.javascript.cost_model, CompressionCostModel::Raw)
    {
        if let Some(candidate) = canonical_peephole {
            let remapped = best_unused_letter_binding_remaps_from_cost(
                &candidate.code,
                config.javascript.cost_model,
                candidate.cost,
                codec_budget,
            )?;
            if let Some((remapped, remapped_cost)) = remapped {
                beam.push(CleanupCandidate {
                    code: remapped,
                    cost: remapped_cost,
                });
            }
        }
    }
    // Expression-prefixed branch returns expose adjacent lone-return ladders,
    // which in turn expose common conditional arms. Score that interaction as
    // one structural challenger while preserving every incumbent beam entry.
    let structural_sources = beam.clone();
    'structural: for candidate in structural_sources {
        for include_statement_assignments in [false, true] {
            let single_use_function_variants: &[bool] =
                if config.single_use_function_expression_candidates_enabled() {
                    &[false, true]
                } else {
                    &[false]
                };
            for include_single_use_functions in single_use_function_variants.iter().copied() {
                if !codec_budget.reserve_work_unit() {
                    break 'structural;
                }
                let mut code = candidate.code.clone();
                let passes = [
                    include_single_use_functions
                        .then_some(LateJavaScriptCleanupPass::SingleUseFunctionExpressions),
                    include_statement_assignments
                        .then_some(LateJavaScriptCleanupPass::StatementAssignmentFirstUse),
                    include_statement_assignments
                        .then_some(LateJavaScriptCleanupPass::StatementAssignmentFirstUse),
                    Some(LateJavaScriptCleanupPass::ExpressionReturnBranches),
                    Some(LateJavaScriptCleanupPass::ConditionalReturnTails),
                    Some(LateJavaScriptCleanupPass::CommonConditionalArms),
                    Some(LateJavaScriptCleanupPass::NegatedConditionalArms),
                    Some(LateJavaScriptCleanupPass::BooleanConditionalValues),
                    Some(LateJavaScriptCleanupPass::UnitCounterUpdates),
                    Some(LateJavaScriptCleanupPass::SequenceAssignmentFirstUse),
                    Some(LateJavaScriptCleanupPass::SequenceAssignmentFirstUse),
                    Some(LateJavaScriptCleanupPass::CommonConditionalArms),
                ];
                for pass in passes.into_iter().flatten() {
                    let Ok(next) = late_generated_javascript_cleanup_pass(&code, pass) else {
                        continue;
                    };
                    code = next;
                }
                code = repair_late_javascript_candidate(code);
                if code == candidate.code
                    || analyze_generated_javascript(&code).is_err()
                    || admission.validate(&code).is_err()
                    || beam.iter().any(|proposal| proposal.code == code)
                {
                    continue;
                }
                let cost = codec_budget
                    .measure_reserved_compile(code.as_bytes(), config.javascript.cost_model)?;
                if let Some((remapped, remapped_cost)) = best_one_function_local_binding_remap(
                    &code,
                    config.javascript.cost_model,
                    cost,
                    codec_budget,
                )? {
                    beam.push(CleanupCandidate {
                        code: remapped,
                        cost: remapped_cost,
                    });
                }
                beam.push(CleanupCandidate { code, cost });
            }
        }
    }
    beam.sort_by(|left, right| (left.cost, left.code.len()).cmp(&(right.cost, right.code.len())));
    beam.dedup_by(|left, right| left.code == right.code);
    beam.truncate(BEAM_WIDTH);

    // Sequence-return topology is terminal. Feeding `return E,V` back into
    // loop-header reconstruction can make that older syntax fold mistake the
    // statement for an update expression. More importantly, applying every
    // raw-neutral sequence rewrite as one switch is the wrong abstraction for
    // gzip/Brotli: a sparse subset can improve the dictionary while the global
    // spelling loses. Walk independently proven local variants with the exact
    // configured scorer, retaining the unchanged spelling at every round.
    const MAX_LOCAL_VARIANTS_PER_PASS_AND_SOURCE: usize = 24;
    const TERMINAL_LOCAL_PASSES: [LateJavaScriptCleanupPass; 1] =
        [LateJavaScriptCleanupPass::ExpressionSuffixReturns];
    for round in 0..terminal_local_rounds {
        let previous_codes = beam
            .iter()
            .map(|candidate| candidate.code.clone())
            .collect::<Vec<_>>();
        let terminal_sources = beam.clone();
        let mut proposals = beam.clone();
        let mut proposal_codes = Vec::new();
        'local_proposals: for candidate in terminal_sources {
            for pass in TERMINAL_LOCAL_PASSES {
                // Local-variant discovery parses and walks the complete
                // artifact. Charge it before constructing the vector; each
                // admitted leaf below pays its own scoring unit separately.
                if !codec_budget.reserve_work_unit() {
                    break 'local_proposals;
                }
                let mut variants =
                    late_generated_javascript_cleanup_local_variants(&candidate.code, pass)
                        .unwrap_or_default();
                // Keep each all-sites spelling as a synergy challenger. It is
                // scored once per retained starting topology; later rounds
                // build sparse combinations one local edit at a time.
                if round == 0 {
                    if let Ok(code) = late_generated_javascript_cleanup_pass(&candidate.code, pass)
                    {
                        variants.push(code);
                    }
                }
                let stride = variants
                    .len()
                    .div_ceil(MAX_LOCAL_VARIANTS_PER_PASS_AND_SOURCE)
                    .max(1);
                for code in variants
                    .into_iter()
                    .step_by(stride)
                    .take(MAX_LOCAL_VARIANTS_PER_PASS_AND_SOURCE)
                {
                    if !codec_budget.reserve_work_unit() {
                        break 'local_proposals;
                    }
                    let code = repair_late_javascript_candidate(code);
                    if code == candidate.code
                        || analyze_generated_javascript(&code).is_err()
                        || admission.validate(&code).is_err()
                        || proposals.iter().any(|proposal| proposal.code == code)
                        || proposal_codes.contains(&code)
                    {
                        continue;
                    }
                    proposal_codes.push(code);
                }
            }
        }
        let measured = proposal_codes
            .into_par_iter()
            .map(|code| {
                codec_budget
                    .measure_reserved(code.as_bytes(), config.javascript.cost_model)
                    .map(|cost| CleanupCandidate { code, cost })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?;
        proposals.extend(measured);
        proposals.sort_by(|left, right| {
            (left.cost, left.code.len()).cmp(&(right.cost, right.code.len()))
        });
        proposals.dedup_by(|left, right| left.code == right.code);
        proposals.truncate(BEAM_WIDTH);
        let next_codes = proposals
            .iter()
            .map(|candidate| candidate.code.clone())
            .collect::<Vec<_>>();
        beam = proposals;
        if next_codes == previous_codes {
            break;
        }
    }
    // Common-arm factoring is a terminal challenger rather than another
    // member of the fixed-width interaction beam. Adding a new optional pass
    // must not evict a previously winning topology merely because several
    // candidates tie before later sequence scoring.
    let terminal_sources = beam.clone();
    for candidate in terminal_sources {
        if !codec_budget.reserve_work_unit() {
            break;
        }
        let Ok(code) = late_generated_javascript_cleanup_pass(
            &candidate.code,
            LateJavaScriptCleanupPass::CommonConditionalArms,
        ) else {
            continue;
        };
        let code = repair_late_javascript_candidate(code);
        if code == candidate.code
            || analyze_generated_javascript(&code).is_err()
            || admission.validate(&code).is_err()
            || beam.iter().any(|proposal| proposal.code == code)
        {
            continue;
        }
        let cost =
            codec_budget.measure_reserved_compile(code.as_bytes(), config.javascript.cost_model)?;
        beam.push(CleanupCandidate { code, cost });
    }
    beam.push(original);
    beam.sort_by(|left, right| (left.cost, left.code.len()).cmp(&(right.cost, right.code.len())));
    beam.dedup_by(|left, right| left.code == right.code);

    let mut finalists = Vec::new();
    for cleaned in beam.into_iter() {
        if finalists.len() >= keep.max(1) {
            break;
        }
        // A spelling the beam ranks below the incumbent is not carried: it lost
        // on the only evidence available here, and the caller pays a full
        // remap for each one it takes.
        let takes = cleaned.code != selected.code
            && cleaned.cost <= selected.transfer_cost
            && !(cleaned.cost == selected.transfer_cost
                && cleaned.code.len() >= selected.code.len());
        let mut candidate = selected.clone();
        if takes {
            let Ok(metrics) = analyze_generated_javascript(&cleaned.code) else {
                continue;
            };
            candidate.metrics = metrics;
            candidate.startup_score = candidate.metrics.startup_score(
                config.javascript.startup.parse_weight,
                config.javascript.startup.compile_weight,
                config.javascript.startup.memory_weight,
            );
            candidate.code = cleaned.code;
            candidate.transfer_cost = cleaned.cost;
        } else if !finalists.is_empty() {
            continue; // the incumbent is already among them
        }
        let candidate = parenthesize_logical_assignments(candidate, config, codec_budget)?;
        if finalists
            .iter()
            .any(|existing: &ScoredJavaScriptCandidate| existing.code == candidate.code)
        {
            continue;
        }
        finalists.push(candidate);
    }
    if finalists.is_empty() {
        finalists.push(parenthesize_logical_assignments(
            selected,
            config,
            codec_budget,
        )?);
    }
    Ok(finalists)
}

fn parenthesize_logical_assignments(
    mut selected: ScoredJavaScriptCandidate,
    config: &ProjectConfig,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<ScoredJavaScriptCandidate, CompileError> {
    if !codec_budget.reserve_work_unit() {
        return Ok(selected);
    }
    let mut repaired = match late_generated_javascript_cleanup_pass(
        &selected.code,
        LateJavaScriptCleanupPass::OrAssignmentParens,
    ) {
        Ok(code) => code,
        Err(_) => selected.code.clone(),
    };
    if let Ok(split) = repair_fused_keyword_identifiers(&repaired) {
        repaired = split;
    }
    if repaired == selected.code {
        return Ok(selected);
    }
    let Ok(metrics) = analyze_generated_javascript(&repaired) else {
        return Ok(selected);
    };
    if selected.admission.validate(&repaired).is_err() {
        return Ok(selected);
    }
    let transfer_cost =
        codec_budget.measure_reserved_compile(repaired.as_bytes(), config.javascript.cost_model)?;
    selected.metrics = metrics;
    selected.startup_score = selected.metrics.startup_score(
        config.javascript.startup.parse_weight,
        config.javascript.startup.compile_weight,
        config.javascript.startup.memory_weight,
    );
    selected.code = repaired;
    selected.transfer_cost = transfer_cost;
    Ok(selected)
}

const TWO_BYTE_RESERVED_BINDINGS: &[&str] = &["do", "if", "in"];

const LIVE_LETTER_REMAP_PASSES: usize = 8;

fn live_letter_remap_pair_budget(code_len: usize) -> usize {
    match code_len {
        0..32_768 => 1_536,
        32_768..65_536 => 768,
        65_536..262_144 => 384,
        _ => 192,
    }
}

fn best_live_letter_binding_remaps(
    code: &str,
    model: CompressionCostModel,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    let mut current = code.to_string();
    let Some(mut current_cost) = codec_budget.compressed_size(current.as_bytes(), model)? else {
        return Ok(None);
    };
    let mut improved = false;
    for _ in 0..LIVE_LETTER_REMAP_PASSES {
        let Some((next, cost)) =
            best_one_live_letter_binding_remap(&current, model, current_cost, codec_budget)?
        else {
            break;
        };
        current = next;
        current_cost = cost;
        improved = true;
    }
    Ok(improved.then_some((current, current_cost)))
}

fn best_one_live_letter_binding_remap(
    code: &str,
    model: CompressionCostModel,
    current_cost: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    if !codec_budget.reserve_work_unit() {
        return Ok(None);
    }
    let counts =
        single_character_identifier_use_counts(code).map_err(generated_javascript_parse_error)?;
    let mut identifiers = single_character_resolved_binding_identifiers(code)
        .map_err(generated_javascript_parse_error)?;
    identifiers.sort_unstable_by(|left, right| {
        counts[*right as usize]
            .cmp(&counts[*left as usize])
            .then_with(|| left.cmp(right))
    });
    let budget = live_letter_remap_pair_budget(code.len());
    let mut pairs = Vec::new();
    'pairs: for (index, left) in identifiers.iter().copied().enumerate() {
        for right in identifiers.iter().copied().skip(index + 1) {
            pairs.push((left, right));
            if pairs.len() == budget {
                break 'pairs;
            }
        }
    }
    let admitted = codec_budget.reserve(pairs.len());
    pairs.truncate(admitted);
    let best = pairs
        .into_par_iter()
        .filter_map(|(left, right)| {
            let mut mapping = std::array::from_fn(|index| index as u8);
            mapping[left as usize] = right;
            mapping[right as usize] = left;
            let remapped = remap_single_character_identifiers(code, &mapping).ok()?;
            let cost = codec_budget
                .measure_reserved(remapped.as_bytes(), model)
                .ok()?;
            (cost < current_cost).then_some((remapped, cost))
        })
        .min_by(|left, right| (left.1, left.0.as_str()).cmp(&(right.1, right.0.as_str())));
    Ok(best)
}

fn best_live_letter_binding_remaps_with_beam(
    code: &str,
    model: CompressionCostModel,
    passes: usize,
    beam_width: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    let Some(initial_cost) = codec_budget.compressed_size(code.as_bytes(), model)? else {
        return Ok(None);
    };
    let mut beam = vec![(code.to_string(), initial_cost)];
    for _ in 0..passes {
        if !codec_budget.reserve_work_unit() {
            break;
        }
        let proposal_groups = beam
            .par_iter()
            .map(|(candidate, _)| live_letter_binding_swap_variants(candidate))
            .collect::<Result<Vec<_>, CompileError>>()?;
        let mut proposals = proposal_groups.into_iter().flatten().collect::<Vec<_>>();
        proposals.sort();
        proposals.dedup();
        let admitted = codec_budget.reserve(proposals.len());
        proposals.truncate(admitted);
        let mut next = proposals
            .into_par_iter()
            .filter_map(|candidate| {
                let cost = codec_budget
                    .measure_reserved(candidate.as_bytes(), model)
                    .ok()?;
                Some((candidate, cost))
            })
            .collect::<Vec<_>>();
        next.extend(beam.iter().cloned());
        next.sort_by(|left, right| (left.1, left.0.as_str()).cmp(&(right.1, right.0.as_str())));
        next.dedup_by(|left, right| left.0 == right.0);
        next.truncate(beam_width);
        let unchanged = next.len() == beam.len()
            && next
                .iter()
                .zip(&beam)
                .all(|(left, right)| left.0 == right.0);
        beam = next;
        if unchanged {
            break;
        }
    }
    let (best, best_cost) = beam
        .into_iter()
        .next()
        .expect("binding remap beam is non-empty");
    Ok((best_cost < initial_cost).then_some((best, best_cost)))
}

fn live_letter_binding_swap_variants(code: &str) -> Result<Vec<String>, CompileError> {
    let counts =
        single_character_identifier_use_counts(code).map_err(generated_javascript_parse_error)?;
    let mut identifiers = single_character_resolved_binding_identifiers(code)
        .map_err(generated_javascript_parse_error)?;
    identifiers.sort_unstable_by(|left, right| {
        counts[*right as usize]
            .cmp(&counts[*left as usize])
            .then_with(|| left.cmp(right))
    });
    let budget = live_letter_remap_pair_budget(code.len());
    let mut pairs = Vec::new();
    'pairs: for (index, left) in identifiers.iter().copied().enumerate() {
        for right in identifiers.iter().copied().skip(index + 1) {
            pairs.push((left, right));
            if pairs.len() == budget {
                break 'pairs;
            }
        }
    }
    Ok(pairs
        .into_iter()
        .filter_map(|(left, right)| {
            let mut mapping = std::array::from_fn(|index| index as u8);
            mapping[left as usize] = right;
            mapping[right as usize] = left;
            remap_single_character_identifiers(code, &mapping).ok()
        })
        .collect())
}

const FUNCTION_LOCAL_BINDING_REMAP_PASSES: usize = 6;
const FUNCTION_LOCAL_BINDING_REMAP_BEAM_WIDTH: usize = 12;
const TERMINAL_BOOLEAN_BINDING_REMAP_PASSES: usize = 12;

fn best_function_local_binding_remaps(
    code: &str,
    model: CompressionCostModel,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    best_function_local_binding_remaps_with_passes(
        code,
        model,
        FUNCTION_LOCAL_BINDING_REMAP_PASSES,
        codec_budget,
    )
}

fn best_function_local_binding_remaps_with_passes(
    code: &str,
    model: CompressionCostModel,
    passes: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    best_function_local_binding_remaps_with_beam(
        code,
        model,
        passes,
        FUNCTION_LOCAL_BINDING_REMAP_BEAM_WIDTH,
        codec_budget,
    )
}

fn best_function_local_binding_remaps_with_beam(
    code: &str,
    model: CompressionCostModel,
    passes: usize,
    beam_width: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    let Some(initial_cost) = codec_budget.compressed_size(code.as_bytes(), model)? else {
        return Ok(None);
    };
    let mut beam = vec![(code.to_string(), initial_cost)];
    for _ in 0..passes {
        if !codec_budget.reserve_work_unit() {
            break;
        }
        let proposal_groups = beam
            .par_iter()
            .map(|(candidate, _)| {
                function_local_binding_swap_variants(candidate)
                    .map_err(generated_javascript_parse_error)
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let mut proposals = proposal_groups.into_iter().flatten().collect::<Vec<_>>();
        proposals.sort();
        proposals.dedup();
        let admitted = codec_budget.reserve(proposals.len());
        proposals.truncate(admitted);
        let mut next = proposals
            .into_par_iter()
            .filter_map(|candidate| {
                let cost = codec_budget
                    .measure_reserved(candidate.as_bytes(), model)
                    .ok()?;
                Some((candidate, cost))
            })
            .collect::<Vec<_>>();
        next.extend(beam.iter().cloned());
        next.sort_by(|left, right| (left.1, left.0.as_str()).cmp(&(right.1, right.0.as_str())));
        next.dedup_by(|left, right| left.0 == right.0);
        next.truncate(beam_width);
        let unchanged = next.len() == beam.len()
            && next
                .iter()
                .zip(&beam)
                .all(|(left, right)| left.0 == right.0);
        beam = next;
        if unchanged {
            break;
        }
    }
    let (best, best_cost) = beam
        .into_iter()
        .next()
        .expect("binding remap beam is non-empty");
    Ok((best_cost < initial_cost).then_some((best, best_cost)))
}

fn best_one_function_local_binding_remap(
    code: &str,
    model: CompressionCostModel,
    current_cost: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    if !codec_budget.reserve_work_unit() {
        return Ok(None);
    }
    let mut variants =
        function_local_binding_swap_variants(code).map_err(generated_javascript_parse_error)?;
    let admitted = codec_budget.reserve(variants.len());
    variants.truncate(admitted);
    Ok(variants
        .into_par_iter()
        .filter_map(|remapped| {
            let cost = codec_budget
                .measure_reserved(remapped.as_bytes(), model)
                .ok()?;
            (cost < current_cost).then_some((remapped, cost))
        })
        .min_by(|left, right| (left.1, left.0.as_str()).cmp(&(right.1, right.0.as_str()))))
}

fn best_short_binding_remaps(
    code: &str,
    model: CompressionCostModel,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    let mut current = code.to_string();
    let Some(mut current_cost) = codec_budget.compressed_size(current.as_bytes(), model)? else {
        return Ok(None);
    };
    let mut improved = false;
    for _ in 0..UNUSED_LETTER_REMAP_PASSES {
        let Some((next, cost)) =
            best_one_short_binding_remap(&current, model, current_cost, codec_budget)?
        else {
            break;
        };
        current = next;
        current_cost = cost;
        improved = true;
    }
    Ok(improved.then_some((current, current_cost)))
}

fn best_one_short_binding_remap(
    code: &str,
    model: CompressionCostModel,
    current_cost: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    if !codec_budget.reserve_work_unit() {
        return Ok(None);
    }
    let identifiers =
        single_character_identifiers(code).map_err(generated_javascript_parse_error)?;
    let unused = ONE_BYTE_IDENTIFIER_STARTS
        .iter()
        .copied()
        .filter(|byte| !identifiers.contains(byte) && byte.is_ascii_alphabetic())
        .collect::<Vec<_>>();
    if unused.is_empty() {
        return Ok(None);
    }
    let mut sources =
        two_character_identifier_use_counts(code).map_err(generated_javascript_parse_error)?;
    sources.retain(|(name, _)| {
        !TWO_BYTE_RESERVED_BINDINGS.contains(&name.as_str())
            && identifier_name_is_clear_binding(code, name).unwrap_or(false)
    });
    sources.sort_unstable_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let budget = unused_letter_remap_pair_budget(code.len());
    let mut pairs = Vec::new();
    'pairs: for (source, _) in sources {
        for replacement in unused.iter().copied() {
            pairs.push((source.clone(), replacement));
            if pairs.len() == budget {
                break 'pairs;
            }
        }
    }
    let admitted = codec_budget.reserve(pairs.len());
    pairs.truncate(admitted);
    let best = pairs
        .into_par_iter()
        .filter_map(|(source, replacement)| {
            let remapped =
                remap_identifier(code, &source, std::str::from_utf8(&[replacement]).ok()?).ok()?;
            let cost = codec_budget
                .measure_reserved(remapped.as_bytes(), model)
                .ok()?;
            (cost < current_cost).then_some((remapped, cost))
        })
        .min_by(|left, right| (left.1, left.0.as_str()).cmp(&(right.1, right.0.as_str())));
    Ok(best)
}

fn best_unused_letter_binding_remaps(
    code: &str,
    model: CompressionCostModel,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    let Some(current_cost) = codec_budget.compressed_size(code.as_bytes(), model)? else {
        return Ok(None);
    };
    best_unused_letter_binding_remaps_from_cost(code, model, current_cost, codec_budget)
}

fn best_unused_letter_binding_remaps_from_cost(
    code: &str,
    model: CompressionCostModel,
    mut current_cost: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    let mut current = code.to_string();
    let mut improved = false;
    for _ in 0..UNUSED_LETTER_REMAP_PASSES {
        let Some((next, cost)) =
            best_one_unused_letter_binding_remap(&current, model, current_cost, codec_budget)?
        else {
            break;
        };
        current = next;
        current_cost = cost;
        improved = true;
    }
    Ok(improved.then_some((current, current_cost)))
}

fn exact_two_binding_replacement_pairs(unused: &[u8]) -> Vec<(u8, u8)> {
    let unused = &unused[..unused.len().min(EXACT_TWO_BINDING_UNUSED_NAME_LIMIT)];
    let mut replacements = Vec::with_capacity(unused.len().saturating_mul(unused.len() - 1));
    for left in unused.iter().copied() {
        for right in unused.iter().copied() {
            if left != right {
                replacements.push((left, right));
            }
        }
    }
    debug_assert!(replacements.len() <= EXACT_TWO_BINDING_MAX_PAIR_TRIALS);
    replacements
}

fn best_two_binding_unused_letter_remap(
    code: &str,
    model: CompressionCostModel,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    if !codec_budget.reserve_work_unit() {
        return Ok(None);
    }
    let mut sources = single_character_resolved_binding_identifiers(code)
        .map_err(generated_javascript_parse_error)?;
    // Check the topology before any codec work. Most larger artifacts have a
    // different number of bindings and leave this terminal neighborhood after
    // one syntax analysis with zero declaration or pair probes.
    if sources.len() != 2 {
        return Ok(None);
    }
    sources.retain(|byte| single_character_name_is_clear_binding(code, *byte).unwrap_or(false));
    if sources.len() != 2 {
        return Ok(None);
    }
    let Some(initial_cost) = codec_budget.compressed_size(code.as_bytes(), model)? else {
        return Ok(None);
    };
    let identifiers =
        single_character_identifiers(code).map_err(generated_javascript_parse_error)?;
    let counts =
        single_character_identifier_use_counts(code).map_err(generated_javascript_parse_error)?;
    let binding_character_counts =
        declared_identifier_character_use_counts(code).map_err(generated_javascript_parse_error)?;
    let mut surrounding_character_counts = [0usize; 128];
    for byte in code.bytes().filter(u8::is_ascii) {
        surrounding_character_counts[byte as usize] += 1;
    }
    for (count, binding_count) in surrounding_character_counts
        .iter_mut()
        .zip(binding_character_counts)
    {
        *count = count.saturating_sub(binding_count);
    }
    let mut unused = ONE_BYTE_IDENTIFIER_STARTS
        .iter()
        .copied()
        .filter(|byte| !identifiers.contains(byte))
        .collect::<Vec<_>>();
    if unused.len() < 2 {
        return Ok(None);
    }
    unused.sort_unstable_by(|left, right| {
        surrounding_character_counts[*right as usize]
            .cmp(&surrounding_character_counts[*left as usize])
            .then_with(|| {
                ONE_BYTE_IDENTIFIER_STARTS
                    .iter()
                    .position(|candidate| candidate == left)
                    .cmp(
                        &ONE_BYTE_IDENTIFIER_STARTS
                            .iter()
                            .position(|candidate| candidate == right),
                    )
            })
    });
    sources.sort_unstable_by(|left, right| {
        counts[*right as usize]
            .cmp(&counts[*left as usize])
            .then_with(|| left.cmp(right))
    });
    // Exact two-coordinate search closes a hole left by the greedy remapper:
    // changing either binding alone can lose while changing both wins for a
    // dictionary codec. It runs only when the final artifact has exactly two
    // globally resolved one-byte bindings. Surrounding-code character
    // frequency selects eight replacement anchors, bounding the joint step at
    // 56 mappings and at most 224 declaration codec probes.
    let [left, right] = sources.as_slice() else {
        unreachable!("the exact pair search requires two resolved bindings")
    };
    let (left, right) = (*left, *right);
    let mut replacements = exact_two_binding_replacement_pairs(&unused);
    // Each mapping scores the complete declaration family. Reserve whole
    // families so partial exhaustion cannot bias `var`/`let` selection.
    let declaration_variants = top_level_declaration_variants(code.to_string()).len();
    let admitted_mappings = codec_budget.remaining() / declaration_variants.max(1);
    if admitted_mappings < replacements.len() {
        codec_budget.limit_reached = true;
        replacements.truncate(admitted_mappings);
    }
    codec_budget.used = codec_budget
        .used
        .saturating_add(replacements.len().saturating_mul(declaration_variants));
    Ok(replacements
        .into_par_iter()
        .filter_map(|(left_replacement, right_replacement)| {
            let mut mapping = std::array::from_fn(|index| index as u8);
            mapping[left as usize] = left_replacement;
            mapping[left_replacement as usize] = left;
            mapping[right as usize] = right_replacement;
            mapping[right_replacement as usize] = right;
            let remapped = remap_single_character_identifiers(code, &mapping).ok()?;
            // Declaration spelling and binding entropy are one terminal
            // objective: `let a=b` can prefer `let`, while the jointly
            // remapped `g/l` namespace can prefer `var`. Score that complete
            // interaction instead of freezing the pre-remap declaration.
            let (remapped, cost) = best_declaration_variant_by(&remapped, |candidate| {
                codec_budget.measure_reserved(candidate.as_bytes(), model)
            })
            .ok()?;
            (cost < initial_cost).then_some((remapped, cost))
        })
        .min_by(|left, right| (left.1, left.0.as_str()).cmp(&(right.1, right.0.as_str()))))
}

fn best_one_unused_letter_binding_remap(
    code: &str,
    model: CompressionCostModel,
    current_cost: usize,
    codec_budget: &mut TerminalCodecProbeBudget,
) -> Result<Option<(String, usize)>, CompileError> {
    if !codec_budget.reserve_work_unit() {
        return Ok(None);
    }
    let identifiers =
        single_character_identifiers(code).map_err(generated_javascript_parse_error)?;
    if identifiers.is_empty() {
        return Ok(None);
    }
    let counts =
        single_character_identifier_use_counts(code).map_err(generated_javascript_parse_error)?;
    let mut unused = ONE_BYTE_IDENTIFIER_STARTS
        .iter()
        .copied()
        .filter(|byte| !identifiers.contains(byte))
        .collect::<Vec<_>>();
    if unused.is_empty() {
        return Ok(None);
    }
    // Punctuation identifiers can interact unusually well with arrows and
    // repeated property syntax, but the canonical frequency alphabet places
    // them last. Score them first, then retain canonical order for letters so
    // every smaller budget remains a useful prefix of every larger one.
    unused.sort_by_key(|byte| match byte {
        b'_' => 0,
        b'$' => 1,
        _ => 2,
    });
    let mut sources = single_character_resolved_binding_identifiers(code)
        .map_err(generated_javascript_parse_error)?;
    sources.retain(|byte| single_character_name_is_clear_binding(code, *byte).unwrap_or(false));
    sources.sort_unstable_by(|left, right| {
        counts[*right as usize]
            .cmp(&counts[*left as usize])
            .then_with(|| left.cmp(right))
    });
    let budget = unused_letter_remap_pair_budget(code.len());
    let mut pairs = Vec::new();
    'pairs: for replacement in unused.iter().copied() {
        for source in sources.iter().copied() {
            pairs.push((source, replacement));
            if pairs.len() == budget {
                break 'pairs;
            }
        }
    }
    let admitted = codec_budget.reserve(pairs.len());
    pairs.truncate(admitted);
    let best = pairs
        .into_par_iter()
        .filter_map(|(source, replacement)| {
            let mut mapping = std::array::from_fn(|index| index as u8);
            mapping[source as usize] = replacement;
            mapping[replacement as usize] = source;
            let remapped = remap_single_character_identifiers(code, &mapping).ok()?;
            let cost = codec_budget
                .measure_reserved(remapped.as_bytes(), model)
                .ok()?;
            (cost < current_cost).then_some((remapped, cost))
        })
        .min_by(|left, right| (left.1, left.0.as_str()).cmp(&(right.1, right.0.as_str())));
    Ok(best)
}

fn search_identifier_alphabets(
    code: &str,
    baseline: crate::codegen_ir_js::IdentifierAlphabet,
    model: CompressionCostModel,
    trials: usize,
    retain: usize,
) -> Result<Vec<crate::codegen_ir_js::IdentifierAlphabet>, CompileError> {
    let identifiers =
        single_character_identifiers(code).map_err(generated_javascript_parse_error)?;
    if identifiers.is_empty() || trials == 0 || retain == 0 {
        return Ok(Vec::new());
    }
    let mut ranked = Vec::<IdentifierMappingCandidate>::new();
    let mut considered = 0usize;

    // Permuting only the names already present cannot improve a one-binding
    // artifact, and it unnecessarily traps every other small artifact inside
    // its initial character set. Probe bijective swaps with the complete
    // one-byte identifier alphabet first. These textual remaps are used only
    // to rank alphabets: every retained proposal is re-emitted from IR, so the
    // mangler still proves collisions, scopes, exports, and reserved names.
    const IDENTIFIER_STARTS: &[u8] = ONE_BYTE_IDENTIFIER_STARTS;
    let swap_budget = if identifiers.len() == 1 {
        trials
    } else {
        trials.div_ceil(2)
    };
    'swaps: for replacement in IDENTIFIER_STARTS.iter().copied() {
        if identifiers.contains(&replacement) {
            continue;
        }
        for source in identifiers.iter().copied() {
            if considered == swap_budget {
                break 'swaps;
            }
            let mut mapping = std::array::from_fn(|index| index as u8);
            mapping[source as usize] = replacement;
            mapping[replacement as usize] = source;
            considered += 1;
            rank_identifier_mapping(code, model, retain, mapping, &mut ranked)?;
        }
    }

    let mut state = 0x3141_5926_u32;
    while considered < trials && identifiers.len() >= 2 {
        let mut permutation = identifiers.clone();
        for index in (1..permutation.len()).rev() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let swap = (state as usize) % (index + 1);
            permutation.swap(index, swap);
        }
        let mut mapping = std::array::from_fn(|index| index as u8);
        for (source, replacement) in identifiers.iter().zip(&permutation) {
            mapping[*source as usize] = *replacement;
        }
        considered += 1;
        rank_identifier_mapping(code, model, retain, mapping, &mut ranked)?;
    }
    Ok(ranked
        .into_iter()
        .map(|candidate| baseline.remapped(&candidate.mapping))
        .collect())
}

#[derive(Clone)]
struct IdentifierMappingCandidate {
    objective_costs: [usize; 3],
    remapped: String,
    mapping: [u8; 128],
}

fn rank_identifier_mapping(
    code: &str,
    model: CompressionCostModel,
    retain: usize,
    mapping: [u8; 128],
    ranked: &mut Vec<IdentifierMappingCandidate>,
) -> Result<(), CompileError> {
    let remapped = remap_single_character_identifiers(code, &mapping)
        .map_err(generated_javascript_parse_error)?;
    analyze_generated_javascript(&remapped).map_err(generated_javascript_parse_error)?;
    if ranked
        .iter()
        .any(|candidate| candidate.remapped == remapped)
    {
        return Ok(());
    }
    let objective_costs = [
        remapped.len(),
        optional_entropy_objective_cost_result(
            compressed_size(remapped.as_bytes(), CompressionCostModel::Gzip),
            CompressionCostModel::Gzip,
            model,
        )
        .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?,
        optional_entropy_objective_cost_result(
            compressed_size(remapped.as_bytes(), CompressionCostModel::Brotli),
            CompressionCostModel::Brotli,
            model,
        )
        .map_err(|message| crate::codegen_js::CodegenError::new(Span::empty(0), message))?,
    ];
    ranked.push(IdentifierMappingCandidate {
        objective_costs,
        remapped,
        mapping,
    });
    retain_objective_stratified_identifier_mappings(ranked, retain, model);
    Ok(())
}

fn optional_entropy_objective_cost_result<Error>(
    result: Result<usize, Error>,
    objective: CompressionCostModel,
    selected: CompressionCostModel,
) -> Result<usize, Error> {
    match result {
        Ok(cost) => Ok(cost),
        Err(error) if objective == selected => Err(error),
        Err(_) => Ok(usize::MAX),
    }
}

fn retain_objective_stratified_identifier_mappings(
    candidates: &mut Vec<IdentifierMappingCandidate>,
    limit: usize,
    selected_model: CompressionCostModel,
) {
    if limit == 0 {
        candidates.clear();
        return;
    }
    let rankings = objective_models(selected_model)
        .into_iter()
        .map(|model| {
            let objective = objective_index(model);
            let mut indices = (0..candidates.len()).collect::<Vec<_>>();
            indices.sort_by(|left, right| {
                let left = &candidates[*left];
                let right = &candidates[*right];
                (left.objective_costs[objective], &left.remapped)
                    .cmp(&(right.objective_costs[objective], &right.remapped))
            });
            indices
        })
        .collect::<Vec<_>>();
    let mut retained = Vec::with_capacity(limit.min(candidates.len()));
    let mut present = vec![false; candidates.len()];
    'ranks: for rank in 0..candidates.len() {
        for ranking in &rankings {
            let Some(index) = ranking.get(rank).copied() else {
                continue;
            };
            if !present[index] {
                present[index] = true;
                retained.push(candidates[index].clone());
                if retained.len() == limit {
                    break 'ranks;
                }
            }
        }
    }
    *candidates = retained;
}

fn entropy_search_trials(candidate_beam_width: usize, code_len: usize) -> usize {
    let requested = candidate_beam_width.saturating_mul(64).min(1_024);
    let adaptive_cap = match code_len {
        0..4_096 => 1_024,
        4_096..8_192 => 64,
        8_192..16_384 => 16,
        16_384..65_536 => 8,
        _ => 4,
    };
    requested.min(adaptive_cap)
}

fn entropy_source_limit(candidate_beam_width: usize, proposal_width: usize) -> usize {
    candidate_beam_width.min(proposal_width).min(12)
}

fn entropy_mapping_trial_budget(proposal_width: usize) -> usize {
    proposal_width.saturating_mul(64).min(1_024)
}

fn entropy_trials_for_next_source(
    proposal_width: usize,
    code_len: usize,
    remaining_trials: usize,
    remaining_sources: usize,
) -> usize {
    if remaining_trials == 0 || remaining_sources == 0 {
        return 0;
    }
    entropy_search_trials(proposal_width, code_len)
        .min(remaining_trials.div_ceil(remaining_sources))
}

#[cfg(test)]
fn local_name_reserve_variants(
    options: crate::codegen_ir_js::IrJsOptions,
) -> [crate::codegen_ir_js::IrJsOptions; 4] {
    crate::decision_registry::local_name_reserve_variants(options)
}

#[derive(Debug, Clone, Copy)]
struct JavaScriptCandidateBeamPolicy {
    cost_model: CompressionCostModel,
}

fn mutation_spelling_stratified_finalists(
    candidates: &mut AggregateJavaScriptPlanArena,
    search: &crate::decision_registry::EmissionSearchContext<'_>,
) -> Result<Vec<JavaScriptEmissionPlan>, CompileError> {
    let mut finalists = Vec::new();
    let stratified_limit = candidates.len();
    let stratified_indices = objective_stratified_candidate_indices(
        candidates,
        stratified_limit,
        search.config.javascript.cost_model,
    )?;
    for loop_spelling in [
        crate::codegen_ir_js::LoopSpelling::Auto,
        crate::codegen_ir_js::LoopSpelling::While,
        crate::codegen_ir_js::LoopSpelling::For,
        crate::codegen_ir_js::LoopSpelling::Do,
    ] {
        for phi_affinity_mode in [
            crate::codegen_ir_js::PhiAffinityMode::Grouped,
            crate::codegen_ir_js::PhiAffinityMode::Direct,
            crate::codegen_ir_js::PhiAffinityMode::Conservative,
        ] {
            let mut retained = 0;
            for options in stratified_indices
                .iter()
                .map(|index| candidates[*index].plan)
                .filter(|candidate| {
                    candidate.options.loop_spelling == loop_spelling
                        && candidate.options.phi_affinity_mode == phi_affinity_mode
                })
            {
                if !finalists.contains(&options) {
                    finalists.push(options);
                    retained += 1;
                    if retained == search.family_candidate_beam_width {
                        break;
                    }
                }
            }
        }
    }
    Ok(finalists)
}

fn extend_scored_emission_phase(
    ir: &ControlFlowModule<'_>,
    module_output: bool,
    beam_policy: JavaScriptCandidateBeamPolicy,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    candidates: &mut AggregateJavaScriptPlanArena,
    search: &crate::decision_registry::EmissionSearchContext<'_>,
    phase: crate::decision_registry::EmissionPhase,
) -> Result<(), CompileError> {
    use crate::decision_registry::{
        BeamAdmission, BeamWidthPolicy, FinalistPolicy, SCORED_EMISSION_FAMILIES,
    };
    for family in SCORED_EMISSION_FAMILIES
        .iter()
        .filter(|family| family.phase == phase)
    {
        if !(family.admitted)(search) {
            continue;
        }
        let width = match family.width {
            BeamWidthPolicy::Full => search.candidate_beam_width,
            BeamWidthPolicy::Narrow => search.narrow_candidate_beam_width,
            BeamWidthPolicy::Half => search.candidate_beam_width.div_ceil(2),
            BeamWidthPolicy::Min2 => search.candidate_beam_width.min(2),
            BeamWidthPolicy::AtomicHelperTable => {
                crate::decision_registry::helper_table_atomic_width(search)
            }
        };
        if width == 0 {
            continue;
        }
        let finalists = match family.finalists {
            FinalistPolicy::Top => {
                top_candidate_options(candidates, width, search.config.javascript.cost_model)?
            }
            FinalistPolicy::MutationStratified => {
                mutation_spelling_stratified_finalists(candidates, search)?
            }
            FinalistPolicy::FreshFactoryEligible => {
                let mut finalists =
                    top_candidate_options(candidates, width, search.config.javascript.cost_model)?;
                finalists.retain(|plan| {
                    has_inlineable_fresh_empty_array_factory(
                        contexts.get(plan.identity.context_id).baseline,
                    )
                });
                finalists
            }
        };
        contexts.begin_scored_family(family.name);
        let result = match family.admission {
            BeamAdmission::Sequential => extend_javascript_candidate_beam(
                ir,
                module_output,
                beam_policy,
                contexts,
                candidates,
                finalists,
                |options| (family.variants)(search, options),
            ),
            BeamAdmission::Priority => extend_priority_javascript_candidate_beam(
                ir,
                module_output,
                beam_policy,
                contexts,
                candidates,
                finalists,
                |options| (family.variants)(search, options),
            ),
        };
        contexts.end_scored_family();
        result?;
    }
    Ok(())
}

fn extend_javascript_candidate_beam<Variants>(
    _ir: &ControlFlowModule<'_>,
    module_output: bool,
    policy: JavaScriptCandidateBeamPolicy,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    candidates: &mut AggregateJavaScriptPlanArena,
    finalists: Vec<JavaScriptEmissionPlan>,
    variants: impl Fn(crate::codegen_ir_js::IrJsOptions) -> Variants,
) -> Result<(), CompileError>
where
    Variants: IntoIterator<Item = crate::codegen_ir_js::IrJsOptions>,
{
    extend_javascript_candidate_beam_with_admission(
        module_output,
        policy,
        contexts,
        candidates,
        finalists,
        variants,
        false,
    )
}

fn extend_priority_javascript_candidate_beam<Variants>(
    _ir: &ControlFlowModule<'_>,
    module_output: bool,
    policy: JavaScriptCandidateBeamPolicy,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    candidates: &mut AggregateJavaScriptPlanArena,
    finalists: Vec<JavaScriptEmissionPlan>,
    variants: impl Fn(crate::codegen_ir_js::IrJsOptions) -> Variants,
) -> Result<(), CompileError>
where
    Variants: IntoIterator<Item = crate::codegen_ir_js::IrJsOptions>,
{
    extend_javascript_candidate_beam_with_admission(
        module_output,
        policy,
        contexts,
        candidates,
        finalists,
        variants,
        true,
    )
}

fn extend_javascript_candidate_beam_with_admission<Variants>(
    module_output: bool,
    policy: JavaScriptCandidateBeamPolicy,
    contexts: &JavaScriptEmissionContexts<'_, '_>,
    candidates: &mut AggregateJavaScriptPlanArena,
    finalists: Vec<JavaScriptEmissionPlan>,
    variants: impl Fn(crate::codegen_ir_js::IrJsOptions) -> Variants,
    priority: bool,
) -> Result<(), CompileError>
where
    Variants: IntoIterator<Item = crate::codegen_ir_js::IrJsOptions>,
{
    let priority_allowance = priority.then(|| contexts.begin_priority_plan_family());
    let mut variant_groups = finalists
        .into_iter()
        .map(|plan| {
            variants(plan.options)
                .into_iter()
                .map(|options| (plan.identity.context_id, options))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if priority {
        for group in &mut variant_groups {
            group.retain(|(context_id, options)| {
                contexts.registered_plan(*context_id, *options).is_none()
            });
        }
    }
    let family_limit = priority_allowance
        .unwrap_or(usize::MAX)
        .min(candidates.optional_proposal_width());
    if variant_groups.iter().map(Vec::len).sum::<usize>() > family_limit {
        contexts.mark_active_scored_family_starved();
    }
    let candidate_options = bounded_javascript_variant_options(variant_groups, family_limit);
    // Score a bounded proposal frontier independently. Reserving proposal
    // slots by deleting incumbents first gives enumeration order authority and
    // can discard the parent needed by a later candidate family.
    let plans = candidate_options
        .into_iter()
        .filter_map(|(context_id, candidate_options)| {
            if priority {
                contexts.register_priority_plan(context_id, candidate_options)
            } else {
                contexts.register_plan(context_id, candidate_options)
            }
        })
        .collect::<Vec<_>>();
    if priority {
        contexts.end_priority_plan_family();
    }
    // Registration is deliberately coordinator-only. Parallel workers emit
    // immutable, already-identified plans and cannot make ordinal assignment
    // depend on scheduling.
    let optional_raw_size_cap = candidates.optional_raw_size_cap();
    let score_requests = plans
        .into_par_iter()
        .filter_map(|plan| {
            let code = contexts
                .emit(plan.identity.context_id, module_output, plan.options)
                .ok()?;
            // A proposal that cannot fit the frozen arena byte pool cannot
            // affect this frontier. Apply that cap before even consulting an
            // incumbent score ledger, so cache hits cannot authorize work the
            // uncached path would have skipped.
            selected_model_score_request_with_raw_cap(
                plan,
                code,
                policy.cost_model,
                JavaScriptDeclarationScoreSemantics::DeclarationPlan,
                optional_raw_size_cap,
            )
        })
        .collect::<Vec<_>>();
    let proposals = measure_selected_model_emission_batch(score_requests, candidates.candidates())
        .into_iter()
        .filter_map(|result| {
            result.emission.ok().map(|emission| {
                JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
                    emission,
                    result.owner,
                )
            })
        })
        .collect();
    candidates.merge_optional(proposals)
}

fn bounded_javascript_variant_options<T: Copy + PartialEq>(
    variant_groups: Vec<Vec<T>>,
    limit: usize,
) -> Vec<T> {
    if limit == 0 {
        return Vec::new();
    }
    // Traverse variant columns before sampling. When one family has several
    // spellings, this keeps a bounded sample spread across both its parent
    // candidates and its spelling choices instead of filling the budget from
    // the first parent's variants.
    let maximum_variants = variant_groups.iter().map(Vec::len).max().unwrap_or(0);
    let mut options = Vec::new();
    for variant in 0..maximum_variants {
        for group in &variant_groups {
            let Some(candidate) = group.get(variant).copied() else {
                continue;
            };
            if !options.contains(&candidate) {
                options.push(candidate);
            }
        }
    }
    if options.len() <= limit {
        return options;
    }
    if limit == 1 {
        return vec![options[0]];
    }
    (0..limit)
        .map(|sample| {
            let index = sample
                .saturating_mul(options.len().saturating_sub(1))
                .checked_div(limit - 1)
                .unwrap_or(0);
            options[index]
        })
        .collect()
}

#[cfg(test)]
fn merge_javascript_candidate_frontiers(
    candidates: &mut Vec<JavaScriptEmissionCandidate>,
    mut proposals: Vec<JavaScriptEmissionCandidate>,
    limit: usize,
    selected_model: CompressionCostModel,
) -> Result<(), CompileError> {
    candidates.append(&mut proposals);
    deduplicate_live_javascript_candidate_frontier(candidates);
    retain_objective_stratified_candidates(candidates, limit, selected_model)?;
    sort_javascript_emission_candidates(candidates);
    Ok(())
}

fn top_level_declaration_variants(code: String) -> Vec<String> {
    let mut variants = vec![code.clone()];
    if let Some(rest) = code.strip_prefix("var ") {
        variants.push(format!("let {rest}"));
    }
    // `let` and `var` have identical global-script semantics for generated
    // entry bindings: neither is exported through the global object by this
    // module path, and direct eval is not part of the typed language surface.
    // They can nevertheless differ by a byte under an exact codec, so score
    // both directions rather than letting the emitter's declaration spelling
    // decide gzip/Brotli outcomes accidentally.
    if let Some(rest) = code.strip_prefix("let ") {
        variants.push(format!("var {rest}"));
    }
    // Function-local declaration spelling has an independent codec effect.
    // Expand the complete cross product so gzip/Brotli can select interactions
    // (for example, outer `let` plus inner `let`) that neither isolated edit
    // improves. The token-aware helper only touches the emitter's proven-safe
    // function-leading declaration shape, and the originals remain present.
    for source in variants.clone() {
        if let Some(function_variant) = function_leading_declaration_variant(&source) {
            if !variants.contains(&function_variant) {
                variants.push(function_variant);
            }
        }
    }
    variants.dedup();
    variants
}

fn top_level_declaration_preference(code: &str) -> u8 {
    if code.starts_with("let ") {
        0
    } else if code.starts_with("var ") {
        1
    } else {
        2
    }
}

#[cfg(test)]
std::thread_local! {
    static JAVASCRIPT_CANDIDATE_EMISSIONS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
    static JAVASCRIPT_INTEGER_ANALYSES: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
    static JAVASCRIPT_CODEC_MEASUREMENTS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn javascript_candidate_emission_count() -> usize {
    JAVASCRIPT_CANDIDATE_EMISSIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
fn javascript_integer_analysis_count() -> usize {
    JAVASCRIPT_INTEGER_ANALYSES.with(std::cell::Cell::get)
}

#[cfg(test)]
fn javascript_codec_measurement_count() -> usize {
    JAVASCRIPT_CODEC_MEASUREMENTS.with(std::cell::Cell::get)
}

fn analyze_javascript_integer_values(ir: &ControlFlowModule<'_>) -> IntegerValueAnalysis {
    #[cfg(test)]
    JAVASCRIPT_INTEGER_ANALYSES.with(|count| count.set(count.get() + 1));
    analyze_integer_values(ir)
}

fn emit_javascript_candidate(
    ir: &ControlFlowModule<'_>,
    module_output: bool,
    options: crate::codegen_ir_js::IrJsOptions,
    integer_analysis: Arc<IntegerValueAnalysis>,
) -> Result<String, crate::codegen_js::CodegenError> {
    #[cfg(test)]
    JAVASCRIPT_CANDIDATE_EMISSIONS.with(|count| count.set(count.get() + 1));
    let started = std::time::Instant::now();
    let code = if module_output {
        emit_optimized_ir_js_module_with_options_and_analysis(ir, &options, integer_analysis)?
    } else {
        emit_optimized_ir_js_with_options_and_analysis(ir, &options, integer_analysis)?
    };
    // Each of these six re-scans the whole artifact, on every emission, which
    // is the hottest path in the compiler on a large program. Routing them
    // through the decline memo means an emission whose bytes a fold has already
    // refused does not pay for that refusal again.
    let code = if options.assume_pristine_builtins {
        match crate::js_peephole::fold_once_memoized(&code, fold_fresh_empty_object_assign) {
            Ok((folded, rewritten)) if rewritten > 0 => folded,
            _ => code,
        }
    } else {
        code
    };
    let code = match crate::js_peephole::fold_once_memoized(&code, fold_constant_json_parse) {
        Ok((folded, rewritten)) if rewritten > 0 => folded,
        _ => code,
    };
    let code = match crate::js_peephole::fold_once_memoized(&code, fold_redundant_null_undefined_or) {
        Ok((folded, rewritten)) if rewritten > 0 => folded,
        _ => code,
    };
    let code = match crate::js_peephole::fold_once_memoized(&code, fold_dead_identifier_copy_declarators) {
        Ok((folded, rewritten)) if rewritten > 0 => folded,
        _ => code,
    };
    let code = match crate::js_peephole::fold_once_memoized(&code, fold_if_prefixed_returns) {
        Ok((folded, rewritten)) if rewritten > 0 => folded,
        _ => code,
    };
    let code = match crate::js_peephole::fold_once_memoized(&code, fold_nested_unguarded_ifs) {
        Ok((folded, rewritten)) if rewritten > 0 => folded,
        _ => code,
    };
    if crate::timing::enabled() {
        crate::timing::EMIT.record(code.len() as u64, started.elapsed().as_nanos() as u64);
    }
    Ok(code)
}

fn admitted_generated_javascript_size(
    source: &str,
    model: CompressionCostModel,
) -> Result<usize, String> {
    analyze_generated_javascript(source)
        .map_err(|error| format!("generated JavaScript admission failed: {error}"))?;
    compressed_size(source.as_bytes(), model)
}

fn validate_observed_javascript_artifact(
    source: &str,
    direct_source: &str,
    expected: &crate::compilation_contract::JavaScriptAbiManifest,
    expected_bit_or_zero: usize,
) -> Result<(), CompileError> {
    validate_observed_javascript_artifact_allowing(
        source,
        direct_source,
        expected,
        expected_bit_or_zero,
        false,
    )
}

/// `allow_class_constructor` exempts the single name `constructor` from the
/// introduced-property check.
///
/// The peephole's class rewrite spells `function X(){...}` plus its prototype
/// table as `class X{constructor(){...}}`, and the property-name census counts
/// that class element like any other property, so the rewrite is refused for
/// containing its own keyword. Every object already carries `constructor`
/// through its prototype chain, so nothing there is newly observable and there
/// is nothing for property mangling to get wrong.
///
/// It is opt-in rather than unconditional because admission runs over every
/// candidate in the search, and relaxing it globally admits a different
/// portfolio: measured on micromarklil that settles 826 Brotli *worse*. Only the
/// one call that re-checks an already-selected artifact passes `true`.
fn validate_observed_javascript_artifact_allowing(
    source: &str,
    direct_source: &str,
    expected: &crate::compilation_contract::JavaScriptAbiManifest,
    expected_bit_or_zero: usize,
    allow_class_constructor: bool,
) -> Result<(), CompileError> {
    let observed =
        generated_javascript_export_names(source).map_err(generated_javascript_parse_error)?;
    let export_witnesses =
        generated_javascript_export_witnesses(source).map_err(generated_javascript_parse_error)?;
    let observed_imports =
        generated_javascript_static_imports(source).map_err(generated_javascript_parse_error)?;
    let mut expected_names = expected
        .exports
        .iter()
        .filter(|export| export.kind != crate::compilation_contract::JavaScriptExportKind::TypeOnly)
        .map(|export| export.name.clone())
        .collect::<Vec<_>>();
    expected_names.sort();
    expected_names.dedup();
    let matches = if expected.export_names_may_mangle {
        observed.len() == expected_names.len()
    } else {
        observed == expected_names
    };
    if !matches {
        return Err(crate::codegen_js::CodegenError::new(
            Span::empty(0),
            format!(
                "generated JavaScript export ABI mismatch: expected {expected_names:?}, observed {observed:?}"
            ),
        )
        .into());
    }
    // Candidate rewrites must preserve the declared callable ABI. Deriving the
    // expectation from the *direct emission's* witnesses instead was measured
    // to cost real bytes: it constrains every candidate to the incidental
    // shape of one unoptimized lowering rather than to the contract, and on
    // markedlil that was 1568 raw bytes, 4.7%. See
    // finer/hypotheses/016-marked-size-regression.
    //
    // Where a LilScript default is materialized in a function body, the
    // emitted JavaScript `length` legitimately differs from the typed arity,
    // so arity is compared against the direct emission below rather than
    // against the manifest.
    let direct_witnesses = generated_javascript_export_witnesses(direct_source)
        .map_err(generated_javascript_parse_error)?;
    let direct_arity = direct_witnesses
        .iter()
        .map(|export| (export.name.clone(), export.arity))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut expected_callables = expected
        .exports
        .iter()
        .filter_map(|export| {
            let kind = match export.kind {
                crate::compilation_contract::JavaScriptExportKind::Function => {
                    crate::js_peephole::GeneratedJavaScriptExportKind::Function
                }
                crate::compilation_contract::JavaScriptExportKind::Constructor => {
                    crate::js_peephole::GeneratedJavaScriptExportKind::Constructor
                }
                crate::compilation_contract::JavaScriptExportKind::Global
                | crate::compilation_contract::JavaScriptExportKind::TypeOnly => return None,
            };
            let methods = export
                .methods
                .iter()
                .map(|method| {
                    (
                        method.name.clone(),
                        method.arity,
                        method.is_async,
                        method.is_generator,
                    )
                })
                .collect::<Vec<_>>();
            let arity = direct_arity
                .get(&export.name)
                .copied()
                .unwrap_or(export.arity);
            Some((
                export.name.clone(),
                kind,
                arity,
                export.constructible,
                methods,
            ))
        })
        .collect::<Vec<_>>();
    let mut observed_callables = export_witnesses
        .into_iter()
        .map(|export| {
            let methods = export
                .methods
                .into_iter()
                .map(|method| {
                    (
                        method.name,
                        method.arity,
                        method.is_async,
                        method.is_generator,
                    )
                })
                .collect::<Vec<_>>();
            (
                export.name,
                export.kind,
                export.arity,
                export.constructible,
                methods,
            )
        })
        .collect::<Vec<_>>();
    expected_callables.sort();
    observed_callables.sort();
    for expected_callable in &expected_callables {
        let match_at = observed_callables.iter().position(|observed| {
            (expected.export_names_may_mangle || observed.0 == expected_callable.0)
                && observed.1 == expected_callable.1
                && observed.2 == expected_callable.2
                && observed.3 == expected_callable.3
                && observed.4 == expected_callable.4
        });
        let Some(match_at) = match_at else {
            return Err(crate::codegen_js::CodegenError::new(
                Span::empty(0),
                format!(
                    "generated JavaScript callable ABI mismatch: expected {expected_callables:?}, observed {observed_callables:?}"
                ),
            )
            .into());
        };
        observed_callables.remove(match_at);
    }
    let expected_imports = expected
        .foreign_imports
        .iter()
        .map(|import| (import.source.clone(), import.imported.clone()))
        .collect::<Vec<_>>();
    if observed_imports != expected_imports {
        return Err(crate::codegen_js::CodegenError::new(
            Span::empty(0),
            format!(
                "generated JavaScript module ABI mismatch: expected {expected_imports:?}, observed {observed_imports:?}"
            ),
        )
        .into());
    }
    let observed_bit_or_zero =
        generated_javascript_bit_or_zero_count(source).map_err(generated_javascript_parse_error)?;
    if observed_bit_or_zero < expected_bit_or_zero {
        return Err(crate::codegen_js::CodegenError::new(
            Span::empty(0),
            format!(
                "generated JavaScript lowering-obligation mismatch: expected at least {expected_bit_or_zero} source `|0` operations, observed {observed_bit_or_zero}"
            ),
        )
        .into());
    }
    let direct_properties = generated_javascript_static_property_names(direct_source)
        .map_err(generated_javascript_parse_error)?;
    let observed_properties = generated_javascript_static_property_names(source)
        .map_err(generated_javascript_parse_error)?;
    let introduced = observed_properties
        .iter()
        .filter(|property| !direct_properties.contains(property))
        .filter(|property| !(allow_class_constructor && property.as_str() == "constructor"))
        .cloned()
        .collect::<Vec<_>>();
    if !introduced.is_empty() {
        return Err(crate::codegen_js::CodegenError::new(
            Span::empty(0),
            format!(
                "generated JavaScript introduced unclassified static properties: {introduced:?}"
            ),
        )
        .into());
    }
    Ok(())
}

fn validate_direct_javascript_artifact(
    source: &str,
    ir: &ControlFlowModule<'_>,
    config: &ProjectConfig,
    module_output: bool,
) -> Result<(), CompileError> {
    let outcome = validate_direct_javascript_artifact_inner(source, ir, config, module_output);
    if crate::timing::enabled() {
        crate::timing::DIRECT_VALIDATE.record_pass(u64::from(outcome.is_err()), 0);
    }
    outcome
}

fn validate_direct_javascript_artifact_inner(
    source: &str,
    ir: &ControlFlowModule<'_>,
    config: &ProjectConfig,
    module_output: bool,
) -> Result<(), CompileError> {
    validate_generated_javascript_syntax_floor(source, config.javascript.resolved_ecmascript())
        .map_err(generated_javascript_parse_error)?;
    let manifest = config
        .javascript_compilation_contract(module_output)
        .abi_manifest(ir);
    let lowering_obligations =
        ir.lowering_obligation_count(crate::ir::LoweringObligation::PreserveJavaScriptBitOrZero);
    validate_observed_javascript_artifact(source, source, &manifest, lowering_obligations)
}

fn compressed_size(bytes: &[u8], model: CompressionCostModel) -> Result<usize, String> {
    #[cfg(test)]
    JAVASCRIPT_CODEC_MEASUREMENTS.with(|count| count.set(count.get() + 1));
    if matches!(model, CompressionCostModel::Raw) {
        // Nothing to memoize: the answer is already the length.
        return Ok(bytes.len());
    }
    // Digesting the artifact costs microseconds; a canonical encode of the same
    // bytes costs tens of milliseconds. A hit returns exactly the value the
    // encoder would have produced, so this cannot move a selection.
    let key = (
        crate::artifact_memo::content_digest(bytes),
        compression_cost_model_key(model),
    );
    if let Some(size) = crate::artifact_memo::COMPRESSED_SIZE.get(&key) {
        return Ok(size);
    }
    let _timing = crate::timing::CODEC.scope(bytes.len());
    let size = match model {
        CompressionCostModel::Raw => bytes.len(),
        CompressionCostModel::Gzip => canonical_gzip_size(bytes)?,
        CompressionCostModel::Brotli => canonical_brotli_size(bytes)?,
    };
    crate::artifact_memo::COMPRESSED_SIZE.insert(key, size);
    dump_scored_candidate(bytes, &key.0, size);
    Ok(size)
}

/// Write every distinctly scored artifact to `LILSCRIPT_DUMP_CANDIDATES` as
/// `<size>-<digest>.js`. This exists to study whether a cheaper encoder ranks
/// the search's real candidate population the same way the canonical one does;
/// it is never enabled in a normal compile.
fn dump_scored_candidate(bytes: &[u8], digest: &[u8; 32], size: usize) {
    static DIRECTORY: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    let Some(directory) = DIRECTORY.get_or_init(|| {
        std::env::var_os("LILSCRIPT_DUMP_CANDIDATES").map(std::path::PathBuf::from)
    }) else {
        return;
    };
    let mut name = String::with_capacity(24);
    name.push_str(&format!("{size:09}-"));
    for byte in &digest[..8] {
        name.push_str(&format!("{byte:02x}"));
    }
    name.push_str(".js");
    let _ = std::fs::create_dir_all(directory);
    let _ = std::fs::write(directory.join(name), bytes);
}

/// Stable discriminant for the memo key. Written out rather than derived so a
/// future cost model cannot silently alias an existing one's cache entries.
const fn compression_cost_model_key(model: CompressionCostModel) -> u8 {
    match model {
        CompressionCostModel::Raw => 0,
        CompressionCostModel::Gzip => 1,
        CompressionCostModel::Brotli => 2,
    }
}

pub const CANONICAL_ZLIB_PACKAGE_VERSION: &str = "1.1.24";
pub const CANONICAL_ZLIB_LIBRARY_VERSION: &str = "1.3.1";
pub const CANONICAL_BROTLI_PACKAGE_VERSION: &str = "1.1.0";
pub const CANONICAL_BROTLI_LIBRARY_VERSION: u32 = 0x0100_1000;

pub fn canonical_zlib_version() -> Result<&'static str, String> {
    // SAFETY: zlib returns a process-lifetime NUL-terminated version string.
    let version = unsafe { std::ffi::CStr::from_ptr(libz_sys::zlibVersion()) };
    version
        .to_str()
        .map_err(|error| format!("zlib returned a non-UTF-8 version: {error}"))
}

fn canonical_gzip_size(bytes: &[u8]) -> Result<usize, String> {
    let version = canonical_zlib_version()?;
    if version != CANONICAL_ZLIB_LIBRARY_VERSION {
        return Err(format!(
            "canonical gzip scoring requires zlib {}, linked {version}",
            CANONICAL_ZLIB_LIBRARY_VERSION
        ));
    }
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder
        .write_all(bytes)
        .map_err(|error| format!("gzip candidate measurement failed: {error}"))?;
    encoder
        .finish()
        .map(|output| output.len())
        .map_err(|error| format!("gzip candidate measurement failed: {error}"))
}

fn canonical_brotli_size(bytes: &[u8]) -> Result<usize, String> {
    let version = canonical_brotli_version();
    if version != CANONICAL_BROTLI_LIBRARY_VERSION {
        return Err(format!(
            "canonical Brotli scoring requires encoder {:#010x}, linked {version:#010x}",
            CANONICAL_BROTLI_LIBRARY_VERSION
        ));
    }
    // SAFETY: the pinned C API only reads `bytes[0..len]` and writes at most
    // `encoded_size` bytes. `BrotliEncoderMaxCompressedSize` supplies that
    // capacity; `max(1)` keeps the output pointer valid for empty input.
    let capacity = unsafe { compu_brotli_sys::BrotliEncoderMaxCompressedSize(bytes.len()) };
    if capacity == 0 && !bytes.is_empty() {
        return Err("Brotli candidate is too large to measure".to_string());
    }
    let mut output = vec![0u8; capacity.max(1)];
    let mut encoded_size = output.len();
    let succeeded = unsafe {
        compu_brotli_sys::BrotliEncoderCompress(
            11,
            22,
            compu_brotli_sys::BrotliEncoderMode_BROTLI_MODE_GENERIC,
            bytes.len(),
            bytes.as_ptr(),
            &mut encoded_size,
            output.as_mut_ptr(),
        )
    };
    if succeeded == 0 {
        return Err("Brotli candidate measurement failed".to_string());
    }
    Ok(encoded_size)
}

pub fn canonical_brotli_version() -> u32 {
    // SAFETY: this function has no arguments, side effects, or memory access;
    // it returns the statically linked encoder's encoded version number.
    unsafe { compu_brotli_sys::BrotliEncoderVersion() }
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
        CandidateSearch, CompressionDecision, JavaScriptOptimization, JavaScriptPriority,
        OptimizationPreset, StartupCostConfig,
    };

    fn test_javascript_plan(
        ordinal: usize,
        options: crate::codegen_ir_js::IrJsOptions,
    ) -> JavaScriptEmissionPlan {
        JavaScriptEmissionPlan {
            identity: JavaScriptPlanIdentity {
                context_id: 0,
                ordinal,
            },
            options,
        }
    }

    fn test_brotli_declaration_candidate(
        context_id: usize,
        code: &str,
    ) -> JavaScriptEmissionCandidate {
        JavaScriptEmissionCandidate::new_declaration_plan(
            code.to_string(),
            JavaScriptEmissionPlan {
                identity: JavaScriptPlanIdentity {
                    context_id,
                    ordinal: 0,
                },
                options: crate::codegen_ir_js::IrJsOptions::default(),
            },
            CompressionCostModel::Brotli,
        )
        .unwrap()
    }

    fn test_entropy_source_candidate(context_id: usize, code: &str) -> JavaScriptEmissionCandidate {
        JavaScriptEmissionCandidate::from_scored_emission(
            ScoredJavaScriptEmission::with_exact_test_score(
                code.to_string(),
                CompressionCostModel::Raw,
                code.len(),
            ),
            JavaScriptEmissionPlan {
                identity: JavaScriptPlanIdentity {
                    context_id,
                    ordinal: 0,
                },
                options: crate::codegen_ir_js::IrJsOptions::default(),
            },
            false,
        )
    }

    fn test_javascript_contexts<'ir, 'src>(
        ir: &'ir ControlFlowModule<'src>,
    ) -> JavaScriptEmissionContexts<'ir, 'src> {
        JavaScriptEmissionContexts::single(JavaScriptEmissionContext::new(0, ir, None, None, false))
    }

    fn assert_same_selected_javascript_candidate(
        left: &SelectedJavaScriptCandidate,
        right: &SelectedJavaScriptCandidate,
    ) {
        assert_eq!(left.plan_identity, right.plan_identity);
        assert_eq!(left.code, right.code);
        assert_eq!(left.transfer_cost, right.transfer_cost);
        assert_eq!(left.startup_score, right.startup_score);
        assert_eq!(left.metrics, right.metrics);
        assert_eq!(left.baseline_metrics, right.baseline_metrics);
        assert_eq!(left.performance, right.performance);
        assert_eq!(left.baseline_performance, right.baseline_performance);
        assert_eq!(left.candidates_evaluated, right.candidates_evaluated);
        assert_eq!(left.peephole_rewrites, right.peephole_rewrites);
        assert_eq!(
            left.terminal_scope_naming_challengers,
            right.terminal_scope_naming_challengers
        );
        assert_eq!(
            left.terminal_scope_naming_selected,
            right.terminal_scope_naming_selected
        );
        assert_eq!(
            left.terminal_scope_naming_incumbent_bytes,
            right.terminal_scope_naming_incumbent_bytes
        );
        assert_eq!(
            left.terminal_scope_naming_best_bytes,
            right.terminal_scope_naming_best_bytes
        );
        assert_eq!(
            left.terminal_string_pooling_challengers,
            right.terminal_string_pooling_challengers
        );
        assert_eq!(
            left.terminal_string_pooling_selected,
            right.terminal_string_pooling_selected
        );
        assert_eq!(
            left.terminal_string_pooling_incumbent_bytes,
            right.terminal_string_pooling_incumbent_bytes
        );
        assert_eq!(
            left.terminal_string_pooling_best_bytes,
            right.terminal_string_pooling_best_bytes
        );
    }

    #[test]
    fn compiles_source_end_to_end() {
        assert_eq!(compile_source("print(40+2);").unwrap(), "console.log(42)");
    }

    #[test]
    fn ssa_candidate_crossproduct_obeys_exact_allowlists() {
        fn candidates(
            config: &ProjectConfig,
        ) -> ([bool; 2], [crate::codegen_ir_js::PhiAffinityMode; 4]) {
            let configured = config.js_options();
            (
                scalar_phi_copy_candidates(config, configured.scalar_phi_copies),
                phi_affinity_candidates(config, configured.phi_affinity_mode),
            )
        }

        let conservative = crate::codegen_ir_js::PhiAffinityMode::Conservative;
        let grouped = crate::codegen_ir_js::PhiAffinityMode::Grouped;
        let direct = crate::codegen_ir_js::PhiAffinityMode::Direct;

        let mut empty = ProjectConfig::default();
        empty.javascript.optimizations = Some(Vec::new());
        empty.javascript.compression = Some(Vec::new());
        assert_eq!(candidates(&empty), ([false; 2], [conservative; 4]));

        let mut optimization_only = ProjectConfig::default();
        optimization_only.javascript.optimizations =
            Some(vec![JavaScriptOptimization::SsaDestructionVariants]);
        optimization_only.javascript.compression = Some(Vec::new());
        assert_eq!(
            candidates(&optimization_only),
            ([false; 2], [conservative; 4])
        );

        let mut compression_only = ProjectConfig::default();
        compression_only.javascript.optimizations = Some(Vec::new());
        compression_only.javascript.compression = Some(vec![
            CompressionDecision::ScalarPhiCopies,
            CompressionDecision::PhiAffinityCoalescing,
        ]);
        assert_eq!(candidates(&compression_only), ([true; 2], [grouped; 4]));

        let mut scalar_only = ProjectConfig::default();
        scalar_only.javascript.optimizations =
            Some(vec![JavaScriptOptimization::SsaDestructionVariants]);
        scalar_only.javascript.compression = Some(vec![CompressionDecision::ScalarPhiCopies]);
        assert_eq!(candidates(&scalar_only), ([true, false], [conservative; 4]));

        let mut affinity_only = ProjectConfig::default();
        affinity_only.javascript.optimizations =
            Some(vec![JavaScriptOptimization::SsaDestructionVariants]);
        affinity_only.javascript.compression =
            Some(vec![CompressionDecision::PhiAffinityCoalescing]);
        assert_eq!(
            candidates(&affinity_only),
            ([false; 2], [grouped, grouped, direct, conservative])
        );
    }

    #[test]
    fn emits_typed_foreign_modules_as_native_esm_imports() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-foreign-module-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("host.ts"),
            "export const add=(left:number,right:number):number=>left+right;",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import extern {add as hostAdd} from \"./host.ts\";\
             extern int hostAdd(int left,int right);\
             export int answer(){return hostAdd(20,22);}",
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            output.contains("import{add as hostAdd}from\"./host.ts\";"),
            "{output}"
        );
        assert!(output.contains("hostAdd(20,22)"), "{output}");

        let native_error = compile_path_to_c(&main).unwrap_err();
        assert!(native_error
            .message
            .contains("only available for JavaScript"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn preserves_side_effect_foreign_esm_imports() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-side-effect-foreign-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("setup.ts"), "globalThis.ready=true;").unwrap();
        let main = directory.join("main.lil");
        std::fs::write(&main, "import extern \"./setup.ts\";print(42);").unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains("import\"./setup.ts\";"), "{output}");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn tree_shakes_unused_foreign_imports_through_barrel_reexports() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-foreign-barrel-shake-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("host.ts"),
            "export const used=(n:number)=>n+1;export const unused=(n:number)=>n+2;",
        )
        .unwrap();
        std::fs::write(
            directory.join("barrel.lil"),
            r#"
                import extern { used, unused } from "./host.ts";
                export extern int used(int value);
                export extern int unused(int value);
            "#,
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                import { used } from "./barrel";
                export int answer() { return used(41); }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            output.contains("import{used}from\"./host.ts\";"),
            "{output}"
        );
        assert!(
            !output.contains("unused"),
            "unused foreign import survived tree shake:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn tree_shakes_unused_foreign_imports_from_a_closed_js_module_entry() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-closed-foreign-shake-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("host.js"),
            "export const used=(n)=>n;export const unused=(n)=>n+1;",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                import extern { used, unused } from "./host.js";
                extern int used(int value);
                extern int unused(int value);
                print(used(41));
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            output.contains("import{used}from\"./host.js\";") || output.contains("import{used as"),
            "{output}"
        );
        assert!(
            !output.contains("unused"),
            "unused foreign import survived closed js-module tree shake:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn keeps_ambient_dom_host_names_that_are_not_foreign_imports() {
        let output = compile_source(
            "extern JsValue domQueryRoot(string selector);print(domQueryRoot(\"#app\"));",
        )
        .unwrap();
        assert!(
            output.contains("domQueryRoot"),
            "ambient DOM host was rewritten as a native call:\n{output}"
        );
        assert!(
            !output.contains("querySelector"),
            "ambient DOM host was rewritten as a native call:\n{output}"
        );
    }

    #[test]
    fn inlines_known_dom_host_calls_without_keeping_the_import() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-dom-host-inline-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("host.js"),
            "export const domAppendChild=(a,b)=>a.appendChild(b);export const unused=(n)=>n;",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                import extern { domAppendChild, unused } from "./host.js";
                extern void domAppendChild(JsValue parent, JsValue child);
                extern JsValue unused(JsValue value);
                extern JsValue parent();
                extern JsValue child();
                domAppendChild(parent(), child());
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains(".appendChild("), "{output}");
        assert!(
            !output.contains("domAppendChild"),
            "domAppendChild import survived known-host lowering:\n{output}"
        );
        assert!(
            !output.contains("unused"),
            "unused foreign import survived known-host lowering:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn inlines_document_and_clone_host_calls() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-dom-document-inline-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("host.js"),
            "export const domQueryRoot=(s)=>document.querySelector(s);export const domCloneNode=(n)=>n.cloneNode(true);export const unused=()=>{};",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r##"
                import extern { domQueryRoot, domCloneNode, unused } from "./host.js";
                extern JsValue domQueryRoot(string selector);
                extern JsValue domCloneNode(JsValue node);
                extern void unused();
                print(domCloneNode(domQueryRoot("#app")));
            "##,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains("document.querySelector("), "{output}");
        assert!(output.contains("cloneNode("), "{output}");
        assert!(!output.contains("typeof window"), "{output}");
        assert!(
            !output.contains("domQueryRoot") && !output.contains("domCloneNode"),
            "DOM host names survived known-host lowering:\n{output}"
        );
        assert!(!output.contains("unused"), "{output}");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lowers_first_class_javascript_literals_without_a_host_import() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-empty-object-literal-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue bag() { return JS.object(); }
                export JsValue list() { return JS.array(); }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains("{}") && output.contains("[]"), "{output}");
        assert!(
            !output.contains("JS."),
            "first-class JavaScript literals should lower directly:\n{output}"
        );
        assert!(
            !output.contains("import{") && !output.contains("from\"./host.ts\""),
            "foreign import should shake after literal lowering:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn preserves_explicit_null_this_in_first_class_javascript_calls() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-null-this-call-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue run(JsValue fn, JsValue value) {
                  return JS.call(fn, null, value);
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains(".call(null,"), "{output}");
        assert!(
            !output.contains("JS.call"),
            "first-class call should not retain a runtime helper:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lowers_first_class_javascript_primitives() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-js-primitive-lowering-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export string inspect(JsValue value) {
                  return JS.typeOf(value) + ":" + JS.string(value);
                }
                export float numeric(JsValue value) {
                  return JS.number(value);
                }
                export JsValue add(JsValue left, JsValue right) {
                  return JS.add(left, right);
                }
                export JsValue remainder(JsValue left, JsValue right) {
                  return JS.mod(left, right);
                }
                export bool ordered(JsValue left, JsValue right) {
                  return JS.lessThan(left, right) ||
                    JS.lessThanOrEqual(left, right) ||
                    JS.greaterThan(left, right) ||
                    JS.greaterThanOrEqual(left, right);
                }
                export bool flags(JsValue value) {
                  return JS.isNullish(value) || JS.isFalse(value) || JS.isUndefined(value);
                }
                export bool ownsChain(JsValue key, JsValue object) {
                  return JS.in(key, object);
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains("typeof "), "{output}");
        assert!(output.contains("==null"), "{output}");
        assert!(
            output.contains("===false") || output.contains("===!1"),
            "{output}"
        );
        assert!(output.contains("===void 0"), "{output}");
        assert!(output.contains("return +"), "{output}");
        assert!(output.contains('+'), "{output}");
        assert!(output.contains('%'), "{output}");
        assert!(output.contains('<'), "{output}");
        assert!(output.contains("<="), "{output}");
        assert!(output.contains('>'), "{output}");
        assert!(output.contains(">="), "{output}");
        assert!(output.contains(" in "), "{output}");
        assert!(!output.contains("JS."), "{output}");
        assert!(!output.contains("from\"./host.ts\""), "{output}");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lowers_first_class_javascript_method_invocation() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-js-method-invoke-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue staticKey(JsValue object, JsValue value) {
                  return JS.invoke(object, "method", value);
                }
                export JsValue dynamicKey(JsValue object, string key, JsValue value) {
                  return JS.invoke(object, key, value);
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains(".method("), "{output}");
        assert!(output.contains("["), "{output}");
        assert!(!output.contains(".call("), "{output}");
        assert!(!output.contains("JS.invoke"), "{output}");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn keeps_return_separated_from_an_inlined_nullish_operand() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-return-nullish-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue check = JS.method1((JsValue _s, JsValue e) => {
                  if (JS.isNullish(e)) {
                    return false;
                  }
                  return true;
                });
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            !output.contains("returne") && !output.contains("returna"),
            "{output}"
        );
        assert!(
            output.contains("==null") || output.contains("== null"),
            "{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn inlines_js_invoke_wrappers_to_direct_members() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-js-invoke-wrapper-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        let calls = (0..24)
            .map(|index| format!("  invoke0(obj, \"m{index}\");"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(
            &main,
            format!(
                r#"
                JsValue invoke0(JsValue obj, string method) {{
                  return JS.invoke(obj, method);
                }}
                export JsValue tick(JsValue obj) {{
{calls}
                  return invoke0(obj, "onBO");
                }}
            "#
            ),
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains(".onBO("), "{output}");
        assert!(output.contains(".m0("), "{output}");
        assert!(output.contains(".m23("), "{output}");
        assert!(
            !output.contains("\"onBO\"") && !output.contains("[t]("),
            "{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn restores_direct_method_calls_only_for_the_same_receiver() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-direct-host-method-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue direct(JsValue object, JsValue value) {
                  return JS.call(object["run"], object, value);
                }
                export JsValue rebound(JsValue object, JsValue receiver, JsValue value) {
                  return JS.call(object["run"], receiver, value);
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            output.contains(".run("),
            "same-receiver member call should be direct:\n{output}"
        );
        assert!(
            output.contains(".run.call("),
            "a rebound member call must preserve its explicit receiver:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rematerializes_same_receiver_member_calls_across_other_reads() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-same-receiver-member-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue prepare(JsValue object, JsValue value) {
                  JsValue next = JS.call(object["enhancer_"], object, value, object["value_"], object["name_"]);
                  if (JS.call(object["equals_"], object, object["value_"], next).truthy()) {
                    return value;
                  }
                  return next;
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            output.contains(".enhancer_(") && output.contains(".equals_("),
            "same-receiver helpers should stay direct member calls:\n{output}"
        );
        assert!(
            !output.contains(".call("),
            "member functions must not go through Function.call:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn js_ident_ending_at(source: &str, end: usize) -> Option<(usize, &str)> {
        let bytes = source.as_bytes();
        let mut start = end;
        while start > 0
            && (bytes[start - 1].is_ascii_alphanumeric() || matches!(bytes[start - 1], b'_' | b'$'))
        {
            start -= 1;
        }
        (start < end && !bytes[start].is_ascii_digit()).then(|| (start, &source[start..end]))
    }

    fn rebound_then_sibling_member(
        source: &str,
        assigned_field: &str,
        sibling_field: &str,
    ) -> bool {
        let assigned = format!(".{assigned_field}");
        let mut from = 0usize;
        while let Some(rel) = source[from..].find(&assigned) {
            let field_at = from + rel;
            from = field_at + 1;
            let Some((rhs_start, ident)) = js_ident_ending_at(source, field_at) else {
                continue;
            };
            if rhs_start == 0 || source.as_bytes()[rhs_start - 1] != b'=' {
                continue;
            }
            let Some((_, lhs)) = js_ident_ending_at(source, rhs_start - 1) else {
                continue;
            };
            if lhs != ident {
                continue;
            }
            let rest = &source[field_at + assigned.len()..];
            let function_end = ["};function ", "};let ", ";export", "function "]
                .iter()
                .filter_map(|marker| rest.find(marker))
                .min()
                .unwrap_or(rest.len());
            if rest[..function_end].contains(&format!("{ident}.{sibling_field}")) {
                return true;
            }
        }
        false
    }

    #[test]
    fn sibling_javascript_members_keep_the_receiver() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-sibling-js-members-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                class LinkDef {
                  string href;
                  string title;
                  init(string href = "", string title = "") {
                    this.href = href;
                    this.title = title;
                  }
                }
                extern class Response {
                  string url;
                  string statusText;
                }
                string join(JsValue cap, string href, string title, string raw, bool bang, string extra) {
                  if (bang) {
                    return href + "|" + title + "|" + raw + "|1|" + extra + JS.string(cap[1]);
                  }
                  return href + "|" + title + "|" + raw + "|0|" + extra + JS.string(cap[1]);
                }
                export string read(Map<string, LinkDef> links, JsValue cap, string key) {
                  LinkDef? defn = links.get(key);
                  if (defn == null) {
                    return JS.string(cap[0]).charAt(0);
                  }
                  return join(cap, defn.href, defn.title, JS.string(cap[0]), JS.string(cap[0]).charAt(0) == "!", key);
                }
                export string read2(Map<string, LinkDef> links, JsValue cap, string key) {
                  LinkDef? defn = links.get(key);
                  if (defn == null) {
                    return "";
                  }
                  return join(cap, defn.href, defn.title, JS.string(cap[0]), false, "x");
                }
                string pair(string left, string right) {
                  if (left.length > 0) {
                    return left + "|" + right;
                  }
                  return right;
                }
                export string webRead(Response response) {
                  return pair(response.url, response.statusText);
                }
                export string webRead2(Response response) {
                  return pair(response.url, response.statusText);
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            !rebound_then_sibling_member(&output, "title", "href"),
            "reusing a receiver name must not rematerialize a sibling member:\n{output}"
        );
        assert!(
            !rebound_then_sibling_member(&output, "statusText", "url"),
            "extern class fields are the same JavaScript members:\n{output}"
        );
        assert!(
            output.contains(".href") && output.contains(".title"),
            "typed fields should emit as JavaScript members:\n{output}"
        );
        assert!(
            output.contains(".url") && output.contains(".statusText"),
            "extern class fields should emit as JavaScript members:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rematerializes_same_receiver_method_calls_on_this() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-same-receiver-this-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue prepare = JS.method1((JsValue self, JsValue value) => {
                  JsValue next = JS.call(self["enhancer_"], self, value, self["value_"], self["name_"]);
                  if (JS.call(self["equals_"], self, self["value_"], next).truthy()) {
                    return value;
                  }
                  return next;
                });
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            output.contains(".enhancer_(") && output.contains(".equals_("),
            "this-receiver helpers should stay direct member calls:\n{output}"
        );
        assert!(
            !output.contains(".call("),
            "member functions must not go through Function.call:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn keeps_this_on_same_receiver_member_calls_inside_nested_closures() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-nested-same-receiver-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export JsValue getDisposer(JsValue self, JsValue abortSignal) {
                  JsValue dispose = () => {
                    JS.call(self["dispose"], self);
                    if (abortSignal.truthy()) {
                      JS.call(abortSignal["removeEventListener"], abortSignal, "abort", dispose);
                    }
                    return JS.undefined();
                  };
                  return dispose;
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(
            output.contains(".dispose("),
            "captured same-receiver methods must stay member calls:\n{output}"
        );
        assert!(
            !output.contains(".call("),
            "member functions must not go through Function.call:\n{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn nested_same_receiver_member_calls_keep_javascript_this() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue probe(){\
               JsValue self=JS.object();\
               self[\"x\"]=1;\
               self[\"dispose\"]=JS.method0((JsValue s)=>{s[\"x\"]=2;return JS.undefined();});\
               JsValue extra=JS.object();\
               extra[\"name_\"]=self[\"x\"];\
               JsValue wrap=()=>{\
                 JsValue name=extra[\"name_\"];\
                 JS.call(self[\"dispose\"],self);\
                 print(name);\
                 return JS.undefined();\
               };\
               wrap();\
               print(self[\"x\"]);\
               return JS.undefined();\
             }\
             probe();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            javascript.contains(".dispose("),
            "nested same-receiver dispose must remain a member call:\n{javascript}"
        );
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\n2\n",
            "{javascript}"
        );
    }

    #[test]
    fn lowers_first_class_javascript_string_and_regex_operations() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-typed-host-methods-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                export bool inspect(string value, Regex regex) {
                  return JS.stringSlice(value, 1.0, 3.0) == "ok" ||
                    JS.stringIndexOf(value, "x", 0.0) > -1.0 ||
                    JS.regexTest(regex, value) || JS.regexExec(regex, value).truthy();
                }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains(".slice("), "{output}");
        assert!(output.contains(".indexOf("), "{output}");
        assert!(output.contains(".test("), "{output}");
        assert!(output.contains(".exec("), "{output}");
        assert!(!output.contains("JS."), "{output}");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lowers_javascript_short_circuit_and_strict_comparison_without_helpers() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-js-short-circuit-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            r#"
                extern JsValue left();
                extern JsValue right();
                export JsValue fallback() { return JS.or(left(), right()); }
                export JsValue guarded() { return JS.and(left(), right()); }
                export bool same(JsValue value) { return JS.strictEqual(value, 0.0); }
                export bool different(JsValue value) { return JS.strictNotEqual(value, false); }
            "#,
        )
        .unwrap();

        let output = compile_path_to_js_module(&main).unwrap();
        assert!(output.contains("left()"), "{output}");
        assert!(output.contains("right()"), "{output}");
        assert!(output.contains("&&"), "{output}");
        assert!(
            output.contains("===0") || output.contains("0==="),
            "{output}"
        );
        assert!(
            output.contains("!==false")
                || output.contains("false!==")
                || output.contains("!==!1")
                || output.contains("!1!=="),
            "{output}"
        );
        assert!(!output.contains("JS."), "{output}");

        std::fs::write(directory.join("compiled.mjs"), &output).unwrap();
        std::fs::write(
            directory.join("runner.mjs"),
            r#"
                let calls = [];
                globalThis.left = () => { calls.push("l"); return 0; };
                globalThis.right = () => { calls.push("r"); return 7; };
                const module = await import("./compiled.mjs");
                console.log(module.fallback(), calls.join(""));
                calls.length = 0;
                console.log(module.guarded(), calls.join(""));
                console.log(
                    module.same(0),
                    module.same("0"),
                    module.different(false),
                    module.different(0),
                );
            "#,
        )
        .unwrap();
        let runtime = std::process::Command::new("node")
            .arg(directory.join("runner.mjs"))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            runtime.status.success(),
            "node failed with {}:\n{}\n{output}",
            runtime.status,
            String::from_utf8_lossy(&runtime.stderr)
        );
        assert_eq!(
            String::from_utf8(runtime.stdout).expect("node stdout is UTF-8"),
            "7 lr\n0 l\ntrue false false true\n",
            "{output}"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn foreign_imports_require_matching_extern_contracts() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-foreign-contract-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("host.js"), "export const add=(a,b)=>a+b;").unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import extern {add} from \"./host.js\";print(add(1,2));",
        )
        .unwrap();

        let error = compile_path(&main).unwrap_err();
        assert!(error.message.contains("matching extern declaration"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn candidate_limit_is_shared_across_optimizer_variants() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int helper(int value){return value*3+1;}print(helper(4));print(helper(8));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.candidate_limit = 7;
        let selected = optimize_and_select_javascript(ir, &config, false).unwrap();
        assert!(selected.selection_metrics.candidates_evaluated > 0);
        assert!(
            selected.selection_metrics.candidates_evaluated <= 7,
            "candidate limit was multiplied across optimizer variants: {}",
            selected.selection_metrics.candidates_evaluated
        );

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> source=record{left:1,right:2,middle:3};Record<int> copy=record{...source,right:11};source.left=21;print(copy.left??0);print(Object.keys(copy).join(\",\"));print(JSON.stringify(copy));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let selected = optimize_and_select_javascript(ir, &config, false).unwrap();
        assert!(
            selected.selection_metrics.candidates_evaluated <= 7,
            "record projection multiplied the shared candidate limit: {}",
            selected.selection_metrics.candidates_evaluated
        );
    }

    #[test]
    fn fresh_literal_factory_candidate_uses_a_two_slot_terminal_frontier() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                JsValue fresh() { JsValue[] value = []; return value; }
                print(JS.strictEqual(fresh(), fresh()));
                print(JS.strictEqual(fresh(), fresh()));
                print(JS.strictEqual(fresh(), fresh()));
                print(JS.strictEqual(fresh(), fresh()));
                print(JS.strictEqual(fresh(), fresh()));
                print(JS.strictEqual(fresh(), fresh()));
            "#,
        )
        .unwrap();
        let mut enabled = javascript_oracle_config();
        enabled.optimization.inlining = Some(false);
        enabled.javascript.cost_model = CompressionCostModel::Raw;
        enabled.javascript.candidate_search = CandidateSearch::Always;
        enabled.javascript.candidate_limit = 2;
        // This fixture tests a two-survivor frontier, not a two-attempt search.
        enabled.javascript.candidate_proposal_limit = Some(128);
        enabled.javascript.candidate_beam_width = 2;
        enabled.javascript.optimizations = Some(vec![
            JavaScriptOptimization::FreshLiteralFactoryInliningVariants,
        ]);
        enabled.javascript.compression = Some(vec![
            CompressionDecision::IdentifierMangling,
            CompressionDecision::StandardGrammarElision,
        ]);
        let mut disabled = enabled.clone();
        disabled.javascript.optimizations = Some(Vec::new());
        disabled.javascript.candidate_search = CandidateSearch::Off;

        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let selected = optimize_and_select_javascript(ir, &enabled, false).unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let baseline = optimize_and_select_javascript(ir, &disabled, false).unwrap();

        assert!(
            selected.selection_metrics.transfer_bytes < baseline.selection_metrics.transfer_bytes,
            "selected={} baseline={}\n{}\n{}",
            selected.selection_metrics.transfer_bytes,
            baseline.selection_metrics.transfer_bytes,
            selected.javascript,
            baseline.javascript,
        );
        assert!(
            selected.selection_metrics.syntax.functions
                < baseline.selection_metrics.syntax.functions,
            "{}\n{}",
            selected.javascript,
            baseline.javascript,
        );
        assert!(selected.selection_metrics.candidates_evaluated <= 2);
    }

    #[test]
    fn each_codec_objective_beats_the_other_objectives_reverse_artifact() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                int[] values = [1, 2, 3, 4, 5];
                int[] alias = values;
                print(alias == values);
                alias.reverse(); print(values.join("-"));
            "#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let models = [
            CompressionCostModel::Raw,
            CompressionCostModel::Gzip,
            CompressionCostModel::Brotli,
        ];
        let mut selected = Vec::new();
        for model in models {
            let mut config = javascript_oracle_config();
            config.javascript.cost_model = model;
            config.javascript.candidate_search = CandidateSearch::Always;
            config.javascript.candidate_limit = 256;
            config.javascript.candidate_beam_width = 12;
            config.mangle.identifiers = Some(true);
            config.mangle.properties = Some(true);
            config.mangle.exports = Some(true);
            let candidate = optimize_and_select_javascript(ir.clone(), &config, false).unwrap();
            assert!(
                candidate.selection_metrics.candidates_evaluated <= 256,
                "{model:?} evaluated {} candidates",
                candidate.selection_metrics.candidates_evaluated
            );
            selected.push(candidate.javascript);
        }

        for (objective_index, model) in models.iter().copied().enumerate() {
            let selected_size =
                compressed_size(selected[objective_index].as_bytes(), model).unwrap();
            for (alternative_index, (alternative_model, alternative)) in
                models.iter().zip(&selected).enumerate()
            {
                let alternative_size = compressed_size(alternative.as_bytes(), model).unwrap();
                assert!(
                    selected_size <= alternative_size,
                    "{model:?} selected {selected_size} bytes but {alternative_model:?} objective {alternative_index} emitted a reachable {alternative_size}-byte artifact\nselected: {}\nalternative: {alternative}",
                    selected[objective_index]
                );
            }
        }
    }

    #[test]
    fn compiles_inlined_aggregate_accumulators_with_scalar_replacement() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                extern int algorithmInt(int index);
                extern int algorithmCount();
                struct Ledger { int total; int weighted; int minimum; int maximum; }
                pure int smaller(int left,int right){if(left<right){return left;}return right;}
                pure int larger(int left,int right){if(left>right){return left;}return right;}
                pure Ledger beginLedger(int value){return Ledger{value,value,value,value};}
                pure Ledger appendLedger(Ledger ledger,int value,int index){
                    return Ledger{
                        ledger.total+value,
                        ledger.weighted+value*(index+1),
                        smaller(ledger.minimum,value),
                        larger(ledger.maximum,value)
                    };
                }
                pure int finishLedger(Ledger ledger){
                    return ledger.total+ledger.weighted+ledger.maximum-ledger.minimum;
                }
                int analyzeLedger(){
                    int count=algorithmCount();
                    Ledger ledger=beginLedger(algorithmInt(0));
                    for(int index=1;index<count;index++){
                        ledger=appendLedger(ledger,algorithmInt(index),index);
                    }
                    return finishLedger(ledger);
                }
                print(analyzeLedger());
            "#,
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;

        let output = compile_program_to_js_configured(&program, &config).unwrap();

        assert!(output.contains("algorithmCount"), "{output}");
        assert!(output.contains("algorithmInt"), "{output}");
        assert!(output.contains("console.log"), "{output}");
    }

    fn ordinary_record_safety(source: &str) -> bool {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        if program.exports.is_empty() {
            optimize_control_flow(&mut ir).unwrap();
        } else {
            optimize_control_flow_for_module(&mut ir).unwrap();
        }
        ir_javascript_ordinary_records_safe(&ir)
    }

    #[test]
    fn ordinary_record_candidate_requires_closed_non_inherited_keys() {
        assert!(ordinary_record_safety(
            "Record<int> value=record{left:1,right:2};print(Object.keys(value).length);print(Object.values(value).length);print(Object.hasOwn(value,\"left\"));"
        ));
        assert!(ordinary_record_safety(
            "Record<int> source=record{left:1};Record<int> copy=record{...source,right:2};print(Object.keys(copy).length);"
        ));
        assert!(!ordinary_record_safety(
            "extern string key();Record<int> value=record{left:1};print(value[key()]??0);"
        ));
        assert!(!ordinary_record_safety(
            "Record<int> value=record{toString:1};print(value.toString??0);"
        ));
        assert!(!ordinary_record_safety(
            "extern void consume(Record<int> value);Record<int> value=record{left:1};consume(value);"
        ));
        assert!(!ordinary_record_safety(
            "Record<int> value=record{left:1};print(value.missing??0);"
        ));
        assert!(!ordinary_record_safety(
            "Record<int> value=record{left:1};print(JSON.stringify(value));"
        ));
        assert!(!ordinary_record_safety(
            "extern void tick();Record<int> value=record{left:1};tick();value.right=2;print(Object.keys(value).length);"
        ));
        assert!(!ordinary_record_safety(
            "Record<int> value=record{left:1};for(string key in value){print(key);}"
        ));
        assert!(!ordinary_record_safety(
            "extern void consume(JsValue value);Record<int>[] values=[record{left:1}];consume(values);"
        ));
        assert!(!ordinary_record_safety(
            "Record<int> value=record{left:1};export {value};"
        ));
        assert!(!ordinary_record_safety(
            "extern Record<int> value;print(Object.keys(value).length);"
        ));
        assert!(!ordinary_record_safety(
            "Record<int> value=record{left:1};JsValue boxed=JS.box(value);print(JS.string(boxed));"
        ));
        assert!(!ordinary_record_safety(
            "Record<int> value=record{left:1};JsValue erased=value;print(JS.string(erased));"
        ));
        assert!(!ordinary_record_safety(
            "export Record<int> make(){return record{left:1};}"
        ));
        assert!(!ordinary_record_safety(
            "export int count(Record<int> value){return Object.keys(value).length;}"
        ));

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> value=record{left:1};print(Object.keys(value).length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let symbol = ir
            .globals
            .iter()
            .find(|global| global.name == "value")
            .unwrap()
            .symbol;
        ir.lazy_modules.push(crate::ir::IrLazyModule {
            id: 7,
            source: "./feature.lil",
            exports: vec![crate::ir::IrExport {
                name: "value",
                binding: crate::ir::ExportBinding::Global(symbol),
                span: Span::empty(0),
            }],
            span: Span::empty(0),
        });
        optimize_control_flow_for_module(&mut ir).unwrap();
        assert!(!ir_javascript_ordinary_records_safe(&ir));
    }

    #[test]
    fn ordinary_record_candidate_obeys_joint_search_allowlists() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> value=record{left:1};print(Object.keys(value).length);",
        )
        .unwrap();

        let mut compression_disabled = javascript_oracle_config();
        compression_disabled.javascript.compression = Some(Vec::new());
        let output = compile_program_to_js_configured(&program, &compression_disabled).unwrap();
        assert!(output.contains("__proto__"), "{output}");

        let mut optimization_disabled = javascript_oracle_config();
        optimization_disabled.javascript.optimizations = Some(Vec::new());
        let output = compile_program_to_js_configured(&program, &optimization_disabled).unwrap();
        assert!(output.contains("__proto__"), "{output}");

        let projected_program = parse_source(
            &arena,
            "Record<int> value=record{left:1};print(JSON.stringify(value));",
        )
        .unwrap();
        let output =
            compile_program_to_js_configured(&projected_program, &compression_disabled).unwrap();
        assert!(output.contains("JSON.stringify"), "{output}");
        let output =
            compile_program_to_js_configured(&projected_program, &optimization_disabled).unwrap();
        assert!(output.contains("JSON.stringify"), "{output}");

        // The projection pass itself is part of joint representation search,
        // not an unconditional preprocessing step hidden behind selection.
        assert!(!compression_disabled.js_joint_representation_search_enabled());
        assert!(!optimization_disabled.js_joint_representation_search_enabled());

        let exposed =
            toml::from_str::<ProjectConfig>("[javascript]\nordinary_record_literals=true\n");
        assert!(
            exposed.is_err(),
            "record representation must remain proof-gated and internal"
        );
    }

    #[test]
    fn gzip_scores_top_level_declaration_variants_exactly() {
        let var = "var b={left:2,right:3,middle:4},a={...b,right:12};b.left=22;console.log(a.left??0);console.log(a.right??0);console.log(a.none??-1);console.log(Object.keys(a).join(','));console.log(JSON.stringify(a))";
        let let_code = format!("let {}", &var[4..]);
        let var_size = compressed_size(var.as_bytes(), CompressionCostModel::Gzip).unwrap();
        let let_size = compressed_size(let_code.as_bytes(), CompressionCostModel::Gzip).unwrap();
        assert!(let_size <= var_size, "let={let_size}, var={var_size}");

        let generator = "var o=0,l=0,c;for(c of function*(){yield-1,yield*function*(){var o=1;for(;o<7;o=o+2)yield o}()}())o=o+c|0,l=l+1|0;console.log(o),console.log(l);";
        let variants = top_level_declaration_variants(generator.to_string());
        assert!(
            variants.iter().any(|candidate| candidate
                == "let o=0,l=0,c;for(c of function*(){yield-1,yield*function*(){let o=1;for(;o<7;o=o+2)yield o}()}())o=o+c|0,l=l+1|0;console.log(o),console.log(l);"),
            "{variants:?}"
        );
    }

    #[test]
    fn canonical_brotli_scorer_matches_node_24_fixtures() {
        assert_eq!(canonical_brotli_version(), CANONICAL_BROTLI_LIBRARY_VERSION);

        let baseline =
            r#"let q=[1,2,3,4,5];console.log(q==q);q.reverse();console.log(q.join("-"))"#;
        let divergent =
            r#"var Z=[1,2,3,4,5];console.log(Z==Z),Z.reverse(),console.log(Z.join('-'))"#;
        assert_eq!(canonical_brotli_size(baseline.as_bytes()).unwrap(), 68);
        assert_eq!(canonical_brotli_size(divergent.as_bytes()).unwrap(), 76);
    }

    #[test]
    fn canonical_gzip_scorer_uses_bundled_official_zlib_fixtures() {
        assert_eq!(
            canonical_zlib_version().unwrap(),
            CANONICAL_ZLIB_LIBRARY_VERSION
        );

        // These are complete no-newline JavaScript artifacts. Their sizes
        // distinguish upstream zlib 1.3.1 from both the host's system zlib and
        // Node's separately patched zlib build.
        let fixtures: &[(&[u8], usize, usize)] = &[
            (
                br#"let W=c=>c<=1?1:c*W(c-1|0)|0;console.log(W(7));var Y=1071,X=462,Z;while(X!=0){Z=Y;Y=X;X=Z%X|0;}console.log(Y);X=0;Y=1;Z=0;while(Z<12){X=X+Y|0;Z+=1;var pa=X;X=Y;Y=pa;}console.log(X)"#,
                180,
                156,
            ),
            (
                br#"let W=[1,2,3,4],Y=W.push(5),Z=W.pop(),X=W.filter(a=>a%2==0),_=X.reduce((h,a)=>h+a|0,0);X.forEach(a=>{console.log(a)});console.log(`sum=${_},last=${Z},pushed=${Y},len=${W.length}`);console.log("checks=true,true,true");console.log("LILSCRIPT");console.log("lilscript")"#,
                266,
                204,
            ),
            (
                b"let a=[3,4];console.log(42);let b=((a[0]+a[1]|0)+6|0);console.log(b);console.log(b+7|0)",
                87,
                79,
            ),
            (
                b"function a(b){return b<=1?1:Math.imul(b,a(b-1|0))}console.log(a(7));var c=1071,b=462,d;while(b!=0){[c,b]=[b,c%b|0];}console.log(c);b=0;c=1;d=0;while(d<12){[b,c,d]=[c,b+c|0,d+1|0];}console.log(b)",
                194,
                167,
            ),
        ];
        for (source, raw, gzip) in fixtures {
            assert_eq!(source.len(), *raw);
            assert_eq!(
                compressed_size(source, CompressionCostModel::Gzip).unwrap(),
                *gzip
            );
        }
    }

    #[test]
    fn configured_probe_seed_skips_terminal_emission() {
        let calls = std::cell::Cell::new(0usize);
        let seeded =
            resolve_configured_javascript_emission::<()>(Some("probe-output".to_string()), || {
                calls.set(calls.get() + 1);
                Ok("new-output".to_string())
            })
            .unwrap();
        assert_eq!(seeded, "probe-output");
        assert_eq!(calls.get(), 0);

        let emitted = resolve_configured_javascript_emission::<()>(None, || {
            calls.set(calls.get() + 1);
            Ok("new-output".to_string())
        })
        .unwrap();
        assert_eq!(emitted, "new-output");
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn keeps_a_saved_previous_value_readable_across_its_own_update() {
        // `prev = cur` before `cur` advances is a parallel copy on the loop's
        // back edge: the old `cur` must still be readable when `prev` takes it.
        // Two-address coalescing used to merge the `cur` phi with its own
        // incoming value even here, so both landed in one JavaScript name, the
        // header compared the new value against itself, and the body ran once.
        // marked's GFM autolink backpedal is this loop; it stopped after a
        // single trim and left an unbalanced `)` inside the link.
        let emitted = compile_source_to_js_module(concat!(
            "export int steps(int n){",
            "int prev=0;int cur=n;int count=0;",
            "while(prev!=cur){",
            "prev=cur;",
            "if(cur>3){cur=cur-3;}else{cur=0;}",
            "count=count+1;",
            "}",
            "return count;}",
        ))
        .unwrap();

        assert!(!copies_an_already_updated_loop_name(&emitted), "{emitted}");
    }

    fn node_stdout(javascript: &str) -> String {
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("node stdout is UTF-8")
    }

    #[test]
    fn snapshot_of_a_record_field_survives_a_later_write() {
        let javascript = compile_source(concat!(
            "int snapshotHrefThenWrite(Record<int> node, int next) {",
            "  int saved = node.href ?? 0;",
            "  node.href = next;",
            "  return saved + (node.href ?? 0);",
            "}",
            "int snapshotComputedThenWrite(Record<int> node, int next) {",
            "  int saved = node[\"href\"] ?? 0;",
            "  node.href = next;",
            "  return saved + (node.href ?? 0);",
            "}",
            "print(snapshotHrefThenWrite(record{href: 42, title: 43}, 35));",
            "print(snapshotComputedThenWrite(record{href: 42, title: 43}, 33));",
        ))
        .unwrap();
        assert_eq!(node_stdout(&javascript).trim(), "77\n75", "{javascript}");
    }

    #[test]
    fn snapshot_of_a_record_field_survives_a_captured_rebind() {
        let source = concat!(
            "int snapshotHrefThenCapturedRebind(Record<int> node, int next) {",
            "  int saved = node.href ?? 0;",
            "  func()->void rebind = () => { node = record{href: next, title: 0}; };",
            "  rebind();",
            "  return saved + (node.href ?? 0);",
            "}",
            "print(snapshotHrefThenCapturedRebind(record{href: 42, title: 43}, 47));",
        );
        let javascript = compile_source(source).unwrap();
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();
        let mut dump = String::new();
        for function in &ir.functions {
            if !function.live {
                continue;
            }
            dump.push_str(&format!(
                "fn {:?} capture={} mutable={:?}\n",
                function.name, function.capture_count, function.mutable_capture_locals
            ));
            for param in &function.params {
                dump.push_str(&format!("  param {} {:?}\n", param.name, param.value));
            }
            for block in &function.blocks {
                dump.push_str(&format!("  block {:?}\n", block.id));
                for phi in &block.phis {
                    dump.push_str(&format!("    phi {:?}\n", phi));
                }
                for instruction in &block.instructions {
                    dump.push_str(&format!(
                        "    {:?} = {:?}\n",
                        instruction.out, instruction.op
                    ));
                }
                dump.push_str(&format!("    term {:?}\n", block.terminator));
            }
        }
        assert_eq!(
            node_stdout(&javascript).trim(),
            "89",
            "js:\n{javascript}\nir:\n{dump}"
        );
    }

    #[test]
    fn snapshot_of_a_top_level_record_field_survives_a_captured_rebind() {
        let javascript = compile_source(concat!(
            "Record<int> node = record{href: 42, title: 43};",
            "int saved = node.href ?? 0;",
            "func()->void rebind = () => { node = record{href: 47, title: 0}; };",
            "rebind();",
            "print(saved + (node.href ?? 0));",
        ))
        .unwrap();
        assert_eq!(node_stdout(&javascript).trim(), "89", "{javascript}");
    }

    #[test]
    fn search_does_not_rank_a_nested_local_that_shadows_an_outer_binding_still_read() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                extern void keep(func(JsValue)->void cb);
                extern void observe(JsValue value);
                void go(JsValue callback) {
                  func()->void inner = () => {
                    JsValue notFn = JS.object();
                    func()->void nested = () => {
                      JS.call(callback, JS.undefined());
                    };
                    nested();
                    observe(notFn);
                  };
                  inner();
                }
                keep(go);
            "#,
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.local_name_reserve = 12;
        config.mangle.identifiers = Some(true);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "let go,seen=[];function keep(cb){{go=cb}}function observe(v){{seen.push(typeof v)}};{javascript};go(function(){{seen.push('called')}});process.stdout.write(seen.join(','))"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "called,object",
            "{javascript}"
        );
    }

    #[test]
    fn size_first_search_keeps_js_string_of_a_jsvalue() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                extern void consume(string text, JsValue file);
                extern JsValue file;
                consume(JS.string(file), file);
            "#,
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.local_name_reserve = 12;
        config.mangle.identifiers = Some(true);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            javascript.contains("+\"\"")
                || javascript.contains("+''")
                || javascript.contains("String("),
            "size-first search dropped ToString on JS.string(JsValue):\n{javascript}"
        );
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "let seen=[];function consume(a,b){{seen.push(typeof a,String(a),typeof b)}};var file={{toString(){{return 'hello'}}}};{javascript};process.stdout.write(seen.join(':'))"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "string:hello:object",
            "{javascript}"
        );
    }

    #[test]
    fn size_first_search_spreads_a_delimiter_not_the_live_match() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                extern void consume(int n);
                extern JsValue cap;
                string pickDelim(JsValue m) {
                  int i = 1;
                  while (i <= 6) {
                    JsValue g = m[i];
                    if (!JS.isUndefined(g) && !JS.isNullish(g) && JS.string(g).length > 0) {
                      return JS.string(g);
                    }
                    i = i + 1;
                  }
                  return "";
                }
                int points(string s) {
                  return s.codePointLength();
                }
                void go() {
                  string rDelim = pickDelim(cap);
                  int rLength = 0;
                  if (rDelim.length != 0) {
                    rLength = points(rDelim);
                  }
                  bool later = !JS.isUndefined(cap[3]) && !JS.isNullish(cap[3]);
                  int flag = 0;
                  if (later) {
                    flag = 1;
                  }
                  consume(rLength);
                  consume(flag);
                }
                go();
            "#,
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.local_name_reserve = 0;
        config.mangle.identifiers = Some(true);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "let seen=[];function consume(n){{seen.push(n)}};var cap=['full',null,null,'**',null,null,null,null];{javascript};process.stdout.write(seen.join(','))"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "2,1",
            "{javascript}"
        );
    }

    /// Read the two names the loop header compares, then reject an emission
    /// where one is copied *from* the other after that other one has already
    /// been assigned in the body. Any correct spelling — an ordered pair of
    /// copies, a temporary, or destructuring — keeps the read ahead of the
    /// update; only the collapsed one reads a value the update destroyed.
    fn copies_an_already_updated_loop_name(code: &str) -> bool {
        let Some(comparison) = code.find("!=") else {
            return false;
        };
        let left = trailing_identifier(&code[..comparison]);
        let right = leading_identifier(&code[comparison + 2..]);
        let (Some(left), Some(right)) = (left, right) else {
            return false;
        };
        let body = &code[comparison + 2 + right.len()..];
        [(left, right), (right, left)]
            .into_iter()
            .any(|(saved, live)| {
                let update = simple_assignment_position(body, live);
                let copy = body.find(&format!("{saved}={live}")).filter(|index| {
                    body[index + saved.len() + 1 + live.len()..]
                        .chars()
                        .next()
                        .is_none_or(|next| {
                            !next.is_ascii_alphanumeric() && next != '_' && next != '$'
                        })
                });
                matches!((update, copy), (Some(update), Some(copy)) if copy > update)
            })
    }

    fn simple_assignment_position(code: &str, name: &str) -> Option<usize> {
        let mut from = 0;
        while let Some(offset) = code[from..].find(&format!("{name}=")) {
            let index = from + offset;
            let after = index + name.len() + 1;
            let before_is_identifier = code[..index].chars().next_back().is_some_and(|previous| {
                previous.is_ascii_alphanumeric() || previous == '_' || previous == '$'
            });
            if !before_is_identifier && !code[after..].starts_with('=') {
                return Some(index);
            }
            from = index + name.len();
        }
        None
    }

    fn trailing_identifier(code: &str) -> Option<&str> {
        let end = code.len();
        let start = code
            .char_indices()
            .rev()
            .take_while(|(_, character)| {
                character.is_ascii_alphanumeric() || *character == '_' || *character == '$'
            })
            .last()
            .map(|(index, _)| index)?;
        (start < end).then(|| &code[start..end])
    }

    fn leading_identifier(code: &str) -> Option<&str> {
        let end = code
            .char_indices()
            .take_while(|(_, character)| {
                character.is_ascii_alphanumeric() || *character == '_' || *character == '$'
            })
            .map(|(index, character)| index + character.len_utf8())
            .last()?;
        Some(&code[..end])
    }

    #[test]
    fn closed_record_projection_requires_two_competing_slots() {
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.optimizations =
            Some(vec![JavaScriptOptimization::JointRepresentationSearch]);
        config.javascript.compression = Some(vec![CompressionDecision::JointRepresentationSearch]);

        assert!(config.js_joint_representation_search_enabled());
        assert!(!javascript_projection_can_compete(&config, 1, 2));
        assert!(!javascript_projection_can_compete(&config, 2, 1));
        assert!(javascript_projection_can_compete(&config, 2, 2));
    }

    #[test]
    fn one_slot_terminal_search_finalizes_the_seed_without_emitting_variants() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(vec![JavaScriptOptimization::ParsedPeephole]);

        // The parsed rewrite is deliberately worse for Brotli, while the
        // declaration plan may independently prefer the equivalent top-level
        // spelling. The one-slot fast path must preserve both exact choices.
        let configured = "let a=0,b=1,s=\"a = a + b \";a=a+b;console.log(a,s)";
        let configured_emission =
            ScoredJavaScriptEmission::measure(configured.to_string(), config.javascript.cost_model)
                .unwrap();
        let configured_plan = test_javascript_plan(0, config.js_options());
        let contexts = test_javascript_contexts(&ir);
        let expected = finalize_javascript_candidates(
            vec![
                JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
                    configured_emission.clone(),
                    configured_plan,
                ),
            ],
            configured,
            configured_plan.identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            1,
        )
        .unwrap();

        let emissions_before = javascript_candidate_emission_count();
        let analyses_before = javascript_integer_analysis_count();
        let selected = select_javascript_candidate(
            0,
            &ir,
            &config,
            false,
            &OptimizationProfile::default(),
            1,
            usize::MAX,
            Some(ScoredJavaScriptEmissionSeed {
                emission: configured_emission,
                options: config.js_options(),
            }),
            Vec::new(),
        )
        .unwrap();

        assert_eq!(javascript_candidate_emission_count(), emissions_before);
        assert_eq!(javascript_integer_analysis_count(), analyses_before);
        assert_eq!(selected.code, expected.code);
        assert_eq!(selected.transfer_cost, expected.transfer_cost);
        assert_eq!(selected.metrics, expected.metrics);
        assert_eq!(selected.performance, expected.performance);
        assert_eq!(selected.peephole_rewrites, expected.peephole_rewrites);
    }

    #[test]
    fn byte_exhausted_terminal_search_preserves_the_exact_incumbent() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(vec![JavaScriptOptimization::ParsedPeephole]);

        let configured = "let a=0,b=1,s=\"a = a + b \";a=a+b;console.log(a,s)";
        config.javascript.candidate_byte_budget = configured.len();
        let configured_emission =
            ScoredJavaScriptEmission::measure(configured.to_string(), config.javascript.cost_model)
                .unwrap();
        let configured_plan = test_javascript_plan(0, config.js_options());
        let contexts = test_javascript_contexts(&ir);
        let candidate = JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
            configured_emission,
            configured_plan,
        );
        let expected = finalize_javascript_candidates(
            vec![candidate.clone()],
            configured,
            configured_plan.identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            8,
        )
        .unwrap();
        let selected = finalize_javascript_candidates_with_terminal_objective_challengers(
            vec![candidate],
            configured,
            configured_plan.identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            8,
            false,
        )
        .unwrap();

        assert_eq!(selected.code, expected.code);
        assert_eq!(selected.transfer_cost, expected.transfer_cost);
        assert_eq!(selected.metrics, expected.metrics);
        assert_eq!(selected.performance, expected.performance);
        assert_eq!(selected.peephole_rewrites, expected.peephole_rewrites);
        assert_eq!(selected.candidates_evaluated, expected.candidates_evaluated);
        assert_eq!(selected.terminal_scope_naming_challengers, 0);
        assert_eq!(selected.terminal_string_pooling_challengers, 0);
    }

    #[test]
    fn one_slot_full_search_skips_probe_helper_interaction_work() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "pure int helper(int value){return value+1;}extern int input();print(helper(input()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = javascript_oracle_config();
        config.optimization.inlining = Some(false);
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.candidate_limit = 1;
        config.javascript.candidate_beam_width = 1;
        config.javascript.compression = Some(vec![
            CompressionDecision::PureHelperInlining,
            CompressionDecision::IdentifierMangling,
            CompressionDecision::StandardGrammarElision,
        ]);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let (selected, emissions, analyses) = pool.install(|| {
            let emissions_before = javascript_candidate_emission_count();
            let analyses_before = javascript_integer_analysis_count();
            let selected = optimize_and_select_javascript(ir, &config, false).unwrap();
            (
                selected,
                javascript_candidate_emission_count() - emissions_before,
                javascript_integer_analysis_count() - analyses_before,
            )
        });

        assert!(!selected.javascript.is_empty());
        assert_eq!(emissions, 1, "K=1 emitted a probe interaction");
        assert_eq!(analyses, 1, "K=1 repeated its configured IR analysis");
    }

    #[test]
    fn declaration_leaves_consume_one_structural_plan_slot() {
        let source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        let variants = top_level_declaration_variants(source.to_string());
        assert_eq!(variants.len(), MAX_DECLARATION_VARIANTS);
        let preferred = variants.last().unwrap().clone();
        let calls = std::cell::Cell::new(0usize);
        let (selected, score) = best_declaration_variant_by(source, |candidate| {
            calls.set(calls.get() + 1);
            Ok::<usize, ()>(usize::from(candidate != preferred))
        })
        .unwrap();
        assert_eq!(selected, preferred);
        assert_eq!(score, 0);
        assert_eq!(calls.get(), MAX_DECLARATION_VARIANTS);

        let options = crate::codegen_ir_js::IrJsOptions::default();
        let mut candidates = vec![
            JavaScriptEmissionCandidate::new_declaration_plan(
                source.to_string(),
                test_javascript_plan(0, options),
                CompressionCostModel::Raw,
            )
            .unwrap(),
            JavaScriptEmissionCandidate::new_declaration_plan(
                "var q=0;function g(){var b=2;return b}console.log(g()+q)".to_string(),
                test_javascript_plan(1, options),
                CompressionCostModel::Raw,
            )
            .unwrap(),
        ];
        retain_objective_stratified_candidates(&mut candidates, 2, CompressionCostModel::Raw)
            .unwrap();
        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .all(|candidate| candidate.declaration_plan));
        assert_eq!(candidates[0].code(), source);
    }

    #[test]
    fn explicit_compiler_threads_use_a_local_pool_and_omitted_threads_preserve_current_pool() {
        let outer = rayon::ThreadPoolBuilder::new()
            .num_threads(3)
            .build()
            .unwrap();
        let inherited = outer.install(|| {
            install_configured_compiler_pool(&CompilerResourceConfig::default(), || {
                Ok(rayon::current_num_threads())
            })
            .unwrap()
        });
        let mut explicit = CompilerResourceConfig::default();
        explicit.threads = std::num::NonZeroUsize::new(2);
        let local = outer.install(|| {
            install_configured_compiler_pool(&explicit, || Ok(rayon::current_num_threads()))
                .unwrap()
        });

        assert_eq!(inherited, 3);
        assert_eq!(local, 2);
    }

    #[test]
    fn compiler_resource_counts_preserve_exact_selected_javascript() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int bump(int value){int next=value+1;return next;}print(bump(2));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut serial_config = javascript_oracle_config();
        serial_config.javascript.candidate_search = CandidateSearch::Always;
        serial_config.javascript.candidate_limit = 8;
        serial_config.javascript.candidate_byte_budget = 32 * 1024;
        serial_config.javascript.candidate_beam_width = 4;
        serial_config.javascript.optimizations = Some(vec![
            JavaScriptOptimization::EntropyCrossScopeReuse,
            JavaScriptOptimization::ParsedPeephole,
        ]);
        serial_config.compiler.resources.threads = std::num::NonZeroUsize::new(1);
        serial_config.compiler.resources.codec_workers = std::num::NonZeroUsize::new(1).unwrap();
        let mut parallel_config = serial_config.clone();
        parallel_config.compiler.resources.threads = std::num::NonZeroUsize::new(4);
        parallel_config.compiler.resources.codec_workers = std::num::NonZeroUsize::new(4).unwrap();

        let serial = optimize_and_select_javascript(ir.clone(), &serial_config, false).unwrap();
        let parallel = optimize_and_select_javascript(ir, &parallel_config, false).unwrap();
        let mut serial_metrics = serial.selection_metrics;
        let mut parallel_metrics = parallel.selection_metrics;
        serial_metrics.compiler_time_micros = 0;
        parallel_metrics.compiler_time_micros = 0;

        assert_eq!(parallel.javascript, serial.javascript);
        assert_eq!(parallel.plan_identity, serial.plan_identity);
        assert_eq!(parallel.optimization_reports, serial.optimization_reports);
        assert_eq!(parallel_metrics, serial_metrics);
    }

    #[test]
    fn finalizer_batches_are_contiguous_ordered_and_worker_bounded() {
        let maximum_workers = ProjectConfig::default()
            .compiler
            .resources
            .codec_workers
            .get();
        for item_count in 0..=17 {
            let batches = into_bounded_contiguous_batches(
                (0..item_count).collect::<Vec<_>>(),
                maximum_workers,
            );
            assert!(batches.len() <= maximum_workers);
            assert!(batches.iter().all(|batch| !batch.is_empty()));
            assert_eq!(
                batches.into_iter().flatten().collect::<Vec<_>>(),
                (0..item_count).collect::<Vec<_>>()
            );
        }

        assert_eq!(
            into_bounded_contiguous_batches(vec![0, 1, 2], 0),
            vec![vec![0, 1, 2]]
        );
    }

    #[test]
    fn terminal_codec_pool_caps_nested_remap_parallelism() {
        let outer = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let mut config = ProjectConfig::default();
        config.compiler.resources.codec_workers = std::num::NonZeroUsize::new(2).unwrap();

        let observed = outer
            .install(|| {
                install_terminal_javascript_codec_pool(&config, || {
                    Ok::<_, CompileError>(
                        (0..64)
                            .into_par_iter()
                            .map(|_| rayon::current_num_threads())
                            .collect::<Vec<_>>(),
                    )
                })
            })
            .unwrap();

        assert_eq!(observed.len(), 64);
        assert!(observed.into_iter().all(|workers| workers == 2));
    }

    #[test]
    fn parallel_brotli_finalizer_matches_serial_exact_result() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(vec![JavaScriptOptimization::ParsedPeephole]);
        let options = config.js_options();
        let baseline = "let a=0;a=a+1;console.log(a)";
        let plans = [
            test_javascript_plan(0, options),
            test_javascript_plan(1, options),
            test_javascript_plan(2, options),
        ];
        let candidates = [
            baseline,
            "let b=0;b=b+1;console.log(b)",
            "let c=0;c=c+2;console.log(c)",
        ]
        .into_iter()
        .zip(plans)
        .map(|(code, plan)| {
            JavaScriptEmissionCandidate::new_declaration_plan(
                code.to_string(),
                plan,
                CompressionCostModel::Brotli,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
        let contexts = test_javascript_contexts(&ir);
        let profile = OptimizationProfile::default();
        let mut serial_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let mut parallel_budget = TerminalCodecProbeBudget::new(usize::MAX);

        let serial = finalize_javascript_candidates_with_parallelism(
            candidates.clone(),
            baseline,
            plans[0].identity,
            &config,
            &contexts,
            &profile,
            candidates.len(),
            false,
            &mut serial_budget,
        )
        .unwrap();
        let parallel = finalize_javascript_candidates_with_parallelism(
            candidates,
            baseline,
            plans[0].identity,
            &config,
            &contexts,
            &profile,
            plans.len(),
            true,
            &mut parallel_budget,
        )
        .unwrap();

        assert_same_selected_javascript_candidate(&serial, &parallel);
    }

    #[test]
    fn parallel_brotli_finalizer_preserves_exact_tie_breaking() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(Vec::new());
        let options = config.js_options();
        let baseline = "console.log(9)";
        let candidates = ["console.log(2)", baseline, "console.log(1)"]
            .into_iter()
            .map(|code| {
                JavaScriptEmissionCandidate::new(
                    1,
                    code.to_string(),
                    options,
                    CompressionCostModel::Brotli,
                )
            })
            .collect::<Vec<_>>();
        let configured_identity = candidates[1].identity();
        let contexts = test_javascript_contexts(&ir);
        let profile = OptimizationProfile::default();
        let mut serial_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let mut parallel_budget = TerminalCodecProbeBudget::new(usize::MAX);

        let serial = finalize_javascript_candidates_with_parallelism(
            candidates.clone(),
            baseline,
            configured_identity,
            &config,
            &contexts,
            &profile,
            candidates.len(),
            false,
            &mut serial_budget,
        )
        .unwrap();
        let parallel = finalize_javascript_candidates_with_parallelism(
            candidates,
            baseline,
            configured_identity,
            &config,
            &contexts,
            &profile,
            3,
            true,
            &mut parallel_budget,
        )
        .unwrap();

        assert_eq!(serial.code, "console.log(1)");
        assert_same_selected_javascript_candidate(&serial, &parallel);
    }

    #[test]
    fn parallel_brotli_finalizer_matches_serial_errors() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(Vec::new());
        let options = config.js_options();
        let candidates = ["'unterminated", "/*"]
            .into_iter()
            .map(|code| {
                JavaScriptEmissionCandidate::new(
                    1,
                    code.to_string(),
                    options,
                    CompressionCostModel::Brotli,
                )
            })
            .collect::<Vec<_>>();
        let missing_configured_identity = JavaScriptPlanIdentity {
            context_id: 0,
            ordinal: usize::MAX,
        };
        let contexts = test_javascript_contexts(&ir);
        let profile = OptimizationProfile::default();
        let baseline = "console.log(1)";
        let mut serial_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let mut parallel_budget = TerminalCodecProbeBudget::new(usize::MAX);

        let serial = finalize_javascript_candidates_with_parallelism(
            candidates.clone(),
            baseline,
            missing_configured_identity,
            &config,
            &contexts,
            &profile,
            candidates.len(),
            false,
            &mut serial_budget,
        )
        .unwrap_err();
        let parallel = finalize_javascript_candidates_with_parallelism(
            candidates,
            baseline,
            missing_configured_identity,
            &config,
            &contexts,
            &profile,
            2,
            true,
            &mut parallel_budget,
        )
        .unwrap_err();

        assert_eq!(serial.to_string(), parallel.to_string());
        assert_eq!(serial.span(), parallel.span());
        assert!(serial
            .to_string()
            .contains("startup limits rejected every JavaScript candidate"));
    }

    #[test]
    fn finalizer_reuses_exact_selected_model_declaration_scores() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.optimizations = Some(Vec::new());
        let source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        let plan = test_javascript_plan(0, config.js_options());
        let candidate = JavaScriptEmissionCandidate::new_declaration_plan(
            source.to_string(),
            plan,
            config.javascript.cost_model,
        )
        .unwrap();
        let measurements_before = javascript_codec_measurement_count();
        let contexts = test_javascript_contexts(&ir);

        let selected = finalize_javascript_candidates(
            vec![candidate],
            source,
            plan.identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            1,
        )
        .unwrap();

        assert_eq!(
            javascript_codec_measurement_count(),
            measurements_before,
            "the finalizer should consume the plan's exact score ledger"
        );
        assert_eq!(selected.transfer_cost, selected.code.len());
    }

    #[test]
    fn optional_candidate_raw_cap_is_inclusive_and_skips_codec_work() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let raw_size_cap = 4;
        let measurements_before = javascript_codec_measurement_count();

        let undersized = measure_optional_javascript_candidate(
            "abc".to_string(),
            test_javascript_plan(0, options),
            CompressionCostModel::Brotli,
            raw_size_cap,
        )
        .unwrap();
        let exact = measure_optional_javascript_candidate(
            "abcd".to_string(),
            test_javascript_plan(1, options),
            CompressionCostModel::Brotli,
            raw_size_cap,
        )
        .unwrap();
        let measurements_after_fitting = javascript_codec_measurement_count();
        let oversized = measure_optional_javascript_candidate(
            "abcde".to_string(),
            test_javascript_plan(2, options),
            CompressionCostModel::Brotli,
            raw_size_cap,
        )
        .unwrap();

        assert_eq!(undersized.unwrap().raw_size, raw_size_cap - 1);
        assert_eq!(exact.unwrap().raw_size, raw_size_cap);
        assert!(oversized.is_none());
        assert_eq!(
            measurements_after_fitting - measurements_before,
            2,
            "each fitting one-spelling plan should be measured once"
        );
        assert_eq!(
            javascript_codec_measurement_count(),
            measurements_after_fitting,
            "a plan that cannot fit the arena must not reach the codec"
        );
    }

    #[test]
    fn selected_score_batch_applies_raw_cap_before_incumbent_lookup() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let code = "console.log(1)";
        let incumbent = JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
            ScoredJavaScriptEmission::measure(code.to_string(), CompressionCostModel::Brotli)
                .unwrap(),
            test_javascript_plan(0, options),
        );
        let measurements_before = javascript_codec_measurement_count();
        let request = selected_model_score_request_with_raw_cap(
            test_javascript_plan(1, options),
            code.to_string(),
            CompressionCostModel::Brotli,
            JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            code.len() - 1,
        );

        let results = measure_selected_model_emission_batch(
            request.into_iter().collect(),
            std::slice::from_ref(&incumbent),
        );

        assert!(results.is_empty());
        assert_eq!(javascript_codec_measurement_count(), measurements_before);
    }

    #[test]
    fn oversized_initial_base_can_spawn_a_shorter_quote_descendant_with_stable_ordinal() {
        let raw_size_cap = 3;
        let mut registry = JavaScriptPlanRegistry::default();
        let configured_options = crate::codegen_ir_js::IrJsOptions::default();
        let configured_plan = registry.register(0, configured_options).unwrap();
        let base_options = crate::codegen_ir_js::IrJsOptions {
            string_quote: crate::codegen_ir_js::StringQuote::Single,
            ..configured_options
        };
        let base_plan = registry.register(0, base_options).unwrap();
        let base_emission =
            measure_initial_javascript_emission("'long'".to_string(), CompressionCostModel::Raw)
                .unwrap();
        assert!(base_emission.code.len() > raw_size_cap);

        let descendant_options = crate::codegen_ir_js::IrJsOptions {
            string_quote: crate::codegen_ir_js::StringQuote::Template,
            ..base_options
        };
        let descendant_plan = registry.register(0, descendant_options).unwrap();
        let descendant = measure_optional_javascript_candidate(
            "`x`".to_string(),
            descendant_plan,
            CompressionCostModel::Raw,
            raw_size_cap,
        )
        .unwrap()
        .unwrap();
        let later_options = crate::codegen_ir_js::IrJsOptions {
            struct_method_shorthand: !descendant_options.struct_method_shorthand,
            ..descendant_options
        };
        let later_plan = registry.register(0, later_options).unwrap();

        assert_eq!(configured_plan.identity.ordinal, 0);
        assert_eq!(base_plan.identity.ordinal, 1);
        assert_eq!(descendant.identity(), descendant_plan.identity);
        assert_eq!(descendant_plan.identity.ordinal, 2);
        assert_eq!(later_plan.identity.ordinal, 3);
    }

    #[test]
    fn objective_stratified_frontier_is_deterministic_and_bounded() {
        let costs = [[1, 9, 9], [9, 1, 9], [9, 9, 1], [2, 2, 8], [3, 3, 2]];
        let mut candidates = costs
            .into_iter()
            .enumerate()
            .map(|(index, objective_costs)| {
                let mut candidate = JavaScriptEmissionCandidate::new(
                    objective_costs[1],
                    format!("candidate{index}"),
                    crate::codegen_ir_js::IrJsOptions::default(),
                    CompressionCostModel::Gzip,
                );
                candidate.objective_costs = objective_costs.map(Some);
                candidate
            })
            .collect::<Vec<_>>();

        let first =
            objective_stratified_candidate_indices(&mut candidates, 4, CompressionCostModel::Gzip)
                .unwrap();
        let second =
            objective_stratified_candidate_indices(&mut candidates, 4, CompressionCostModel::Gzip)
                .unwrap();
        assert_eq!(first, vec![1, 0, 2, 3]);
        assert_eq!(second, first);
        assert_eq!(first.len(), 4);
        assert_eq!(first[0], 1, "the configured objective gets first rank");

        retain_objective_stratified_candidates(&mut candidates, 4, CompressionCostModel::Gzip)
            .unwrap();
        assert_eq!(candidates.len(), 4);
    }

    #[test]
    fn objective_ranking_shares_missing_codec_score_across_equal_context_bytes() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let code = "console.log(1)";
        let candidates = [0, 1]
            .map(|context_id| {
                JavaScriptEmissionCandidate::new_declaration_plan(
                    code.to_string(),
                    JavaScriptEmissionPlan {
                        identity: JavaScriptPlanIdentity {
                            context_id,
                            ordinal: 0,
                        },
                        options,
                    },
                    CompressionCostModel::Gzip,
                )
                .unwrap()
            })
            .to_vec();
        let identities = candidates
            .iter()
            .map(JavaScriptEmissionCandidate::identity)
            .collect::<Vec<_>>();
        let mut cold = candidates.clone();
        let mut shared = candidates;

        let cold_measurements_before = javascript_codec_measurement_count();
        for candidate in &mut cold {
            candidate
                .objective_cost(CompressionCostModel::Brotli)
                .unwrap();
        }
        assert_eq!(
            javascript_codec_measurement_count() - cold_measurements_before,
            2,
            "independent declaration plans each require the missing codec"
        );
        let cold_rankings =
            objective_ranked_candidate_indices(&mut cold, CompressionCostModel::Gzip).unwrap();

        let shared_measurements_before = javascript_codec_measurement_count();
        let shared_rankings =
            objective_ranked_candidate_indices(&mut shared, CompressionCostModel::Gzip).unwrap();
        assert_eq!(
            javascript_codec_measurement_count() - shared_measurements_before,
            1,
            "equal bytes and declaration spellings should share one exact missing score"
        );
        assert_eq!(shared_rankings, cold_rankings);
        assert_eq!(
            shared
                .iter()
                .map(|candidate| candidate.objective_costs)
                .collect::<Vec<_>>(),
            cold.iter()
                .map(|candidate| candidate.objective_costs)
                .collect::<Vec<_>>()
        );
        assert_eq!(shared.len(), 2);
        assert_eq!(
            shared
                .iter()
                .map(JavaScriptEmissionCandidate::identity)
                .collect::<Vec<_>>(),
            identities
        );
        assert_ne!(identities[0].context_id, identities[1].context_id);
    }

    #[test]
    fn alternate_gzip_scoring_flattens_one_declaration_family_across_workers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Barrier;

        let source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        assert_eq!(
            top_level_declaration_variants(source.to_string()).len(),
            MAX_DECLARATION_VARIANTS
        );
        let mut candidates = vec![test_brotli_declaration_candidate(0, source)];
        let active = AtomicUsize::new(0);
        let maximum_active = AtomicUsize::new(0);
        let starts = AtomicUsize::new(0);
        let first_wave = Barrier::new(MAX_DECLARATION_VARIANTS);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_DECLARATION_VARIANTS)
            .build()
            .unwrap();

        pool.install(|| {
            populate_missing_gzip_objectives_for_brotli_candidates_by(
                &mut candidates,
                rayon::current_num_threads(),
                |source| {
                    let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum_active.fetch_max(now_active, Ordering::SeqCst);
                    starts.fetch_add(1, Ordering::SeqCst);
                    first_wave.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(source.len())
                },
            );
        });

        assert_eq!(starts.load(Ordering::SeqCst), MAX_DECLARATION_VARIANTS);
        assert_eq!(
            maximum_active.load(Ordering::SeqCst),
            MAX_DECLARATION_VARIANTS
        );
        assert!(
            candidates[0].objective_costs[objective_index(CompressionCostModel::Gzip)].is_some()
        );
    }

    #[test]
    fn alternate_gzip_scoring_deduplicates_leaves_and_matches_serial_ranking() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let var_source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        let let_source = format!("let {}", &var_source[4..]);
        let mut var_variants = top_level_declaration_variants(var_source.to_string());
        let mut let_variants = top_level_declaration_variants(let_source.clone());
        var_variants.sort();
        let_variants.sort();
        assert_eq!(var_variants, let_variants);
        assert_eq!(var_variants.len(), MAX_DECLARATION_VARIANTS);

        let candidates = vec![
            test_brotli_declaration_candidate(7, var_source),
            test_brotli_declaration_candidate(11, &let_source),
        ];
        let identities = candidates
            .iter()
            .map(JavaScriptEmissionCandidate::identity)
            .collect::<Vec<_>>();
        let mut serial = candidates.clone();
        let mut parallel = candidates;
        let score = |source: &str| {
            Ok(source.bytes().fold(17usize, |state, byte| {
                state.wrapping_mul(16777619).wrapping_add(usize::from(byte))
            }))
        };
        populate_missing_gzip_objectives_for_brotli_candidates_by(&mut serial, 1, score);

        let calls = AtomicUsize::new(0);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_DECLARATION_VARIANTS)
            .build()
            .unwrap();
        pool.install(|| {
            populate_missing_gzip_objectives_for_brotli_candidates_by(
                &mut parallel,
                rayon::current_num_threads(),
                |source| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    score(source)
                },
            );
        });

        assert_eq!(
            calls.load(Ordering::SeqCst),
            MAX_DECLARATION_VARIANTS,
            "overlapping var/let families must share each exact gzip leaf"
        );
        assert_eq!(
            parallel
                .iter()
                .map(|candidate| candidate.objective_costs)
                .collect::<Vec<_>>(),
            serial
                .iter()
                .map(|candidate| candidate.objective_costs)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            parallel
                .iter()
                .map(JavaScriptEmissionCandidate::identity)
                .collect::<Vec<_>>(),
            identities
        );
        let serial_rankings =
            objective_ranked_candidate_indices(&mut serial, CompressionCostModel::Brotli).unwrap();
        let parallel_rankings =
            objective_ranked_candidate_indices(&mut parallel, CompressionCostModel::Brotli)
                .unwrap();
        assert_eq!(parallel_rankings, serial_rankings);
    }

    #[test]
    fn alternate_gzip_leaf_failure_marks_the_complete_family_without_losing_identities() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        let variants = top_level_declaration_variants(source.to_string());
        assert_eq!(variants.len(), MAX_DECLARATION_VARIANTS);
        let failing = variants[2].clone();
        let mut candidates = vec![
            test_brotli_declaration_candidate(3, source),
            test_brotli_declaration_candidate(5, source),
        ];
        let identities = candidates
            .iter()
            .map(JavaScriptEmissionCandidate::identity)
            .collect::<Vec<_>>();
        let calls = AtomicUsize::new(0);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_DECLARATION_VARIANTS)
            .build()
            .unwrap();

        pool.install(|| {
            populate_missing_gzip_objectives_for_brotli_candidates_by(
                &mut candidates,
                rayon::current_num_threads(),
                |candidate| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    if candidate == failing {
                        Err("diagnostic gzip scorer unavailable".to_string())
                    } else {
                        Ok(candidate.len())
                    }
                },
            );
        });

        assert_eq!(calls.load(Ordering::SeqCst), MAX_DECLARATION_VARIANTS);
        assert!(candidates.iter().all(|candidate| {
            candidate.objective_costs[objective_index(CompressionCostModel::Gzip)]
                == Some(usize::MAX)
        }));
        assert_eq!(
            candidates
                .iter()
                .map(JavaScriptEmissionCandidate::identity)
                .collect::<Vec<_>>(),
            identities
        );
    }

    #[test]
    fn selected_brotli_batch_shares_one_exact_ledger_without_collapsing_contexts() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let code = "console.log(1)";
        let requests = [7, 11]
            .map(|context_id| SelectedModelEmissionScoreRequest {
                owner: JavaScriptEmissionPlan {
                    identity: JavaScriptPlanIdentity {
                        context_id,
                        ordinal: 0,
                    },
                    options,
                },
                code: code.to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            })
            .into_iter()
            .collect();
        let measurements_before = javascript_codec_measurement_count();

        let mut candidates = measure_selected_model_emission_batch(requests, &[])
            .into_iter()
            .map(|result| {
                JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
                    result.emission.unwrap(),
                    result.owner,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            javascript_codec_measurement_count() - measurements_before,
            1,
            "two exact one-spelling Brotli plans should run one q11 ledger measurement"
        );
        assert_eq!(candidates.len(), 2);
        assert_ne!(candidates[0].identity(), candidates[1].identity());
        assert_eq!(candidates[0].code(), candidates[1].code());
        assert_eq!(candidates[0].transfer_cost, candidates[1].transfer_cost);
        let selected_objective = objective_index(CompressionCostModel::Brotli);
        assert_eq!(
            candidates[0].objective_costs[selected_objective],
            candidates[1].objective_costs[selected_objective]
        );
        let rankings =
            objective_ranked_candidate_indices(&mut candidates, CompressionCostModel::Brotli)
                .unwrap();
        assert_eq!(rankings[0], vec![0, 1]);
        assert_eq!(candidates.len(), 2, "ranking must preserve both contexts");
    }

    #[test]
    fn selected_brotli_batch_copies_the_complete_declaration_ledger() {
        let source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        assert_eq!(
            top_level_declaration_variants(source.to_string()).len(),
            MAX_DECLARATION_VARIANTS
        );
        let requests = [3usize, 5]
            .map(|owner| SelectedModelEmissionScoreRequest {
                owner,
                code: source.to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            })
            .into_iter()
            .collect();
        let measurements_before = javascript_codec_measurement_count();

        let results = measure_selected_model_emission_batch_by(requests, &[], 1, |code, model| {
            compressed_size(code.as_bytes(), model)
        });

        assert_eq!(
            javascript_codec_measurement_count() - measurements_before,
            MAX_DECLARATION_VARIANTS,
            "one four-spelling ledger should be measured, not one ledger per identity"
        );
        let emissions = results
            .into_iter()
            .map(|result| result.emission.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(emissions.len(), 2);
        assert_eq!(
            emissions[0].declaration_scores.known_mask,
            emissions[1].declaration_scores.known_mask
        );
        assert_eq!(
            emissions[0].declaration_scores.variant_count,
            MAX_DECLARATION_VARIANTS as u8
        );
        assert_eq!(
            emissions[0].declaration_scores.costs,
            emissions[1].declaration_scores.costs
        );
        let selected_spelling = |emission: &ScoredJavaScriptEmission| {
            top_level_declaration_variants(emission.code.clone())
                .into_iter()
                .enumerate()
                .min_by(|(left_index, left), (right_index, right)| {
                    (
                        emission.declaration_scores.costs[*left_index],
                        left.len(),
                        left.as_str(),
                    )
                        .cmp(&(
                            emission.declaration_scores.costs[*right_index],
                            right.len(),
                            right.as_str(),
                        ))
                })
                .unwrap()
                .1
        };
        assert_eq!(
            selected_spelling(&emissions[0]),
            selected_spelling(&emissions[1])
        );
    }

    #[test]
    fn selected_score_batch_parallelizes_one_plans_declaration_leaves() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Barrier;

        let source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        assert_eq!(
            declaration_score_variants(
                source,
                JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            )
            .len(),
            MAX_DECLARATION_VARIANTS
        );
        let requests = vec![SelectedModelEmissionScoreRequest {
            owner: 7usize,
            code: source.to_string(),
            model: CompressionCostModel::Brotli,
            semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
        }];
        let active = AtomicUsize::new(0);
        let maximum_active = AtomicUsize::new(0);
        let starts = AtomicUsize::new(0);
        let first_wave = Barrier::new(MAX_DECLARATION_VARIANTS);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_DECLARATION_VARIANTS)
            .build()
            .unwrap();

        let results = pool.install(|| {
            measure_selected_model_emission_batch_by(
                requests,
                &[],
                rayon::current_num_threads(),
                |code, _| {
                    let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum_active.fetch_max(now_active, Ordering::SeqCst);
                    starts.fetch_add(1, Ordering::SeqCst);
                    first_wave.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(code.len())
                },
            )
        });

        assert_eq!(starts.load(Ordering::SeqCst), MAX_DECLARATION_VARIANTS);
        assert_eq!(
            maximum_active.load(Ordering::SeqCst),
            MAX_DECLARATION_VARIANTS
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].owner, 7);
        let scores = results[0].emission.as_ref().unwrap().declaration_scores;
        assert_eq!(scores.variant_count, MAX_DECLARATION_VARIANTS as u8);
        assert_eq!(scores.known_mask, (1 << MAX_DECLARATION_VARIANTS) - 1);
    }

    #[test]
    fn selected_score_batch_matches_serial_ledger_and_owner_order() {
        let build_requests = || {
            [
                "var o=0;function f(){var a=1;return a}console.log(f()+o)",
                "var p=1;function g(){var b=2;return b}console.log(g()+p)",
            ]
            .into_iter()
            .enumerate()
            .map(|(owner, code)| SelectedModelEmissionScoreRequest {
                owner,
                code: code.to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            })
            .collect::<Vec<_>>()
        };
        let score = |code: &str, model: CompressionCostModel| {
            Ok(code.bytes().fold(objective_index(model), |total, byte| {
                total.wrapping_mul(16777619).wrapping_add(usize::from(byte))
            }))
        };
        let serial = measure_selected_model_emission_batch_by(build_requests(), &[], 1, score);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .build()
            .unwrap();
        let parallel = pool.install(|| {
            measure_selected_model_emission_batch_by(
                build_requests(),
                &[],
                rayon::current_num_threads(),
                score,
            )
        });
        let summarize = |results: Vec<SelectedModelEmissionScoreResult<usize>>| {
            results
                .into_iter()
                .map(|result| {
                    let scores = result.emission.unwrap().declaration_scores;
                    (
                        result.owner,
                        scores.model,
                        scores.semantics,
                        scores.variant_count,
                        scores.costs,
                        scores.known_mask,
                    )
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(summarize(parallel), summarize(serial));
    }

    #[test]
    fn selected_score_batch_reports_the_first_declaration_leaf_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let source = "var o=0;function f(){var a=1;return a}console.log(f()+o)";
        let variants = declaration_score_variants(
            source,
            JavaScriptDeclarationScoreSemantics::DeclarationPlan,
        );
        assert_eq!(variants.len(), MAX_DECLARATION_VARIANTS);
        let first_failure = variants[1].clone();
        let later_failure = variants[3].clone();
        let requests = [3usize, 5]
            .map(|owner| SelectedModelEmissionScoreRequest {
                owner,
                code: source.to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            })
            .into_iter()
            .collect();
        let calls = AtomicUsize::new(0);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_DECLARATION_VARIANTS)
            .build()
            .unwrap();

        let results = pool.install(|| {
            measure_selected_model_emission_batch_by(
                requests,
                &[],
                rayon::current_num_threads(),
                |code, _| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    if code == first_failure {
                        Err("first declaration leaf failed".to_string())
                    } else if code == later_failure {
                        Err("later declaration leaf failed".to_string())
                    } else {
                        Ok(code.len())
                    }
                },
            )
        });

        assert_eq!(calls.load(Ordering::SeqCst), MAX_DECLARATION_VARIANTS);
        assert_eq!(
            results
                .iter()
                .map(|result| result.owner)
                .collect::<Vec<_>>(),
            vec![3, 5]
        );
        assert!(results.into_iter().all(|result| {
            let message = result.emission.unwrap_err().to_string();
            message.contains("first declaration leaf failed")
                && !message.contains("later declaration leaf failed")
        }));
    }

    #[test]
    fn selected_score_batch_reports_configured_root_failure_before_optional_results() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ProbeOwner {
            Root,
            RootInteraction,
            NonRootConfigured,
        }

        // Sort order deliberately schedules both optional groups before the
        // root group. Result reconstruction must nevertheless leave the root
        // first so its hard failure is the one observed by the coordinator.
        let requests = vec![
            SelectedModelEmissionScoreRequest {
                owner: ProbeOwner::Root,
                code: "z-configured-root".to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            },
            SelectedModelEmissionScoreRequest {
                owner: ProbeOwner::RootInteraction,
                code: "a-root-interaction".to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            },
            SelectedModelEmissionScoreRequest {
                owner: ProbeOwner::NonRootConfigured,
                code: "b-nonroot-configured".to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            },
        ];
        let results = measure_selected_model_emission_batch_by(requests, &[], 1, |code, _| {
            Err(format!("{code} scorer failed"))
        });

        assert_eq!(
            results
                .iter()
                .map(|result| result.owner)
                .collect::<Vec<_>>(),
            vec![
                ProbeOwner::Root,
                ProbeOwner::RootInteraction,
                ProbeOwner::NonRootConfigured,
            ]
        );
        let error = match take_required_first_selected_model_emission(results, |owner| {
            *owner == ProbeOwner::Root
        }) {
            Ok(_) => panic!("the configured root failure must remain hard"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains("z-configured-root scorer failed"));
        assert!(!message.contains("a-root-interaction scorer failed"));
        assert!(!message.contains("b-nonroot-configured scorer failed"));
    }

    #[test]
    fn configured_root_probe_batch_matches_serial_and_uses_full_codec_pool() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Barrier;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ProbeOwner {
            Root,
            RootInteraction,
            NonRootConfigured,
        }

        let sources = [
            "var o=0;function f(){var a=1;return a}console.log(f()+o)",
            "var p=1;function g(){var b=2;return b}console.log(g()+p)",
            "var q=2;function h(){var c=3;return c}console.log(h()+q)",
        ];
        for source in sources {
            assert_eq!(
                declaration_score_variants(
                    source,
                    JavaScriptDeclarationScoreSemantics::DeclarationPlan,
                )
                .len(),
                MAX_DECLARATION_VARIANTS
            );
        }
        let build_requests = || {
            [
                ProbeOwner::Root,
                ProbeOwner::RootInteraction,
                ProbeOwner::NonRootConfigured,
            ]
            .into_iter()
            .zip(sources)
            .map(|(owner, code)| SelectedModelEmissionScoreRequest {
                owner,
                code: code.to_string(),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            })
            .collect::<Vec<_>>()
        };
        let score = |code: &str, model: CompressionCostModel| {
            Ok(code.bytes().fold(objective_index(model), |total, byte| {
                total.wrapping_mul(16_777_619).wrapping_add(byte.into())
            }))
        };
        let serial = measure_selected_model_emission_batch_by(build_requests(), &[], 1, score);

        let expected_calls = sources.len() * MAX_DECLARATION_VARIANTS;
        let active = AtomicUsize::new(0);
        let maximum_active = AtomicUsize::new(0);
        let calls = AtomicUsize::new(0);
        let first_wave = Barrier::new(8);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .build()
            .unwrap();
        let parallel = pool.install(|| {
            measure_selected_model_emission_batch_by(
                build_requests(),
                &[],
                rayon::current_num_threads(),
                |code, model| {
                    let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum_active.fetch_max(now_active, Ordering::SeqCst);
                    let call = calls.fetch_add(1, Ordering::SeqCst);
                    if call < 8 {
                        first_wave.wait();
                    }
                    let result = score(code, model);
                    active.fetch_sub(1, Ordering::SeqCst);
                    result
                },
            )
        });

        assert_eq!(calls.load(Ordering::SeqCst), expected_calls);
        assert_eq!(maximum_active.load(Ordering::SeqCst), 8);
        let summarize = |results: Vec<SelectedModelEmissionScoreResult<ProbeOwner>>| {
            let (root, optional) = take_required_first_selected_model_emission(results, |owner| {
                *owner == ProbeOwner::Root
            })
            .unwrap();
            let root_scores = root.declaration_scores;
            let mut summary = vec![(
                ProbeOwner::Root,
                root.code,
                root_scores.variant_count,
                root_scores.costs,
                root_scores.known_mask,
            )];
            summary.extend(optional.map(|result| {
                let emission = result.emission.unwrap();
                let scores = emission.declaration_scores;
                (
                    result.owner,
                    emission.code,
                    scores.variant_count,
                    scores.costs,
                    scores.known_mask,
                )
            }));
            summary
        };
        assert_eq!(summarize(parallel), summarize(serial));
    }

    #[test]
    fn selected_score_reuse_key_includes_bytes_model_and_declaration_semantics() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let calls = AtomicUsize::new(0);
        let requests = vec![
            (
                "same",
                CompressionCostModel::Brotli,
                JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            ),
            (
                "same",
                CompressionCostModel::Brotli,
                JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            ),
            (
                "different",
                CompressionCostModel::Brotli,
                JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            ),
            (
                "same",
                CompressionCostModel::Gzip,
                JavaScriptDeclarationScoreSemantics::DeclarationPlan,
            ),
            (
                "same",
                CompressionCostModel::Brotli,
                JavaScriptDeclarationScoreSemantics::ExactSpelling,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(
            |(owner, (code, model, semantics))| SelectedModelEmissionScoreRequest {
                owner,
                code: code.to_string(),
                model,
                semantics,
            },
        )
        .collect();

        let results = measure_selected_model_emission_batch_by(
            requests,
            &[],
            rayon::current_num_threads(),
            |code, _| {
                let cost = calls.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(cost.saturating_add(code.len()))
            },
        );

        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert_eq!(results.len(), 5);
        let scores = results
            .into_iter()
            .map(|result| result.emission.unwrap().declaration_scores.costs[0])
            .collect::<Vec<_>>();
        assert_eq!(scores[0], scores[1], "the one exact key should share");
        assert_ne!(scores[0], scores[2], "different bytes must not share");
        assert_ne!(scores[0], scores[3], "different models must not share");
        assert_ne!(
            scores[0], scores[4],
            "different declaration semantics must not share"
        );
    }

    #[test]
    fn selected_score_batch_respects_worker_cap_and_preserves_owner_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Barrier;

        let requests = (0..4)
            .map(|owner| SelectedModelEmissionScoreRequest {
                owner,
                code: format!("console.log({owner})"),
                model: CompressionCostModel::Brotli,
                semantics: JavaScriptDeclarationScoreSemantics::ExactSpelling,
            })
            .collect::<Vec<_>>();
        let active = AtomicUsize::new(0);
        let maximum_active = AtomicUsize::new(0);
        let starts = AtomicUsize::new(0);
        let first_wave = Barrier::new(2);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let results = pool.install(|| {
            measure_selected_model_emission_batch_by(requests, &[], 2, |code, _| {
                let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum_active.fetch_max(now_active, Ordering::SeqCst);
                let start = starts.fetch_add(1, Ordering::SeqCst);
                if start < 2 {
                    first_wave.wait();
                }
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(code.len())
            })
        });

        assert_eq!(starts.load(Ordering::SeqCst), 4);
        assert_eq!(maximum_active.load(Ordering::SeqCst), 2);
        assert_eq!(
            results
                .into_iter()
                .map(|result| result.owner)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn selected_score_batch_reuses_only_a_matching_incumbent_and_propagates_failures() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let options = crate::codegen_ir_js::IrJsOptions::default();
        let incumbent_plan = test_javascript_plan(0, options);
        let incumbent = JavaScriptEmissionCandidate::new_declaration_plan_with_scores(
            ScoredJavaScriptEmission::measure("same".to_string(), CompressionCostModel::Brotli)
                .unwrap(),
            incumbent_plan,
        );
        let incumbent_cost = incumbent.emission.declaration_scores.costs[0];
        let calls = AtomicUsize::new(0);
        let matching = SelectedModelEmissionScoreRequest {
            owner: 1usize,
            code: "same".to_string(),
            model: CompressionCostModel::Brotli,
            semantics: JavaScriptDeclarationScoreSemantics::DeclarationPlan,
        };
        let reused = measure_selected_model_emission_batch_by(
            vec![matching],
            std::slice::from_ref(&incumbent),
            rayon::current_num_threads(),
            |_, _| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(99)
            },
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            reused[0]
                .emission
                .as_ref()
                .unwrap()
                .declaration_scores
                .costs[0],
            incumbent_cost
        );

        let mismatched_semantics = SelectedModelEmissionScoreRequest {
            owner: 2usize,
            code: "same".to_string(),
            model: CompressionCostModel::Brotli,
            semantics: JavaScriptDeclarationScoreSemantics::ExactSpelling,
        };
        let failures = measure_selected_model_emission_batch_by(
            vec![
                mismatched_semantics,
                SelectedModelEmissionScoreRequest {
                    owner: 3usize,
                    code: "same".to_string(),
                    model: CompressionCostModel::Brotli,
                    semantics: JavaScriptDeclarationScoreSemantics::ExactSpelling,
                },
            ],
            std::slice::from_ref(&incumbent),
            rayon::current_num_threads(),
            |_, _| {
                calls.fetch_add(1, Ordering::SeqCst);
                Err("selected Brotli scorer unavailable".to_string())
            },
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(failures.len(), 2);
        assert!(failures.into_iter().all(|result| result
            .emission
            .unwrap_err()
            .to_string()
            .contains("selected Brotli scorer unavailable")));
    }

    #[test]
    fn live_frontier_keeps_configured_and_distinct_but_deduplicates_identical_code() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let configured_plan = test_javascript_plan(0, options);
        let identical_plan = test_javascript_plan(
            1,
            crate::codegen_ir_js::IrJsOptions {
                compact_boolean_literals: !options.compact_boolean_literals,
                ..options
            },
        );
        let distinct_plan = test_javascript_plan(2, options);
        let mut candidates = vec![
            JavaScriptEmissionCandidate::new_declaration_plan(
                "same".to_string(),
                configured_plan,
                CompressionCostModel::Raw,
            )
            .unwrap(),
            JavaScriptEmissionCandidate::new_declaration_plan(
                "same".to_string(),
                identical_plan,
                CompressionCostModel::Raw,
            )
            .unwrap(),
            JavaScriptEmissionCandidate::new_declaration_plan(
                "distinct".to_string(),
                distinct_plan,
                CompressionCostModel::Raw,
            )
            .unwrap(),
        ];

        deduplicate_live_javascript_candidate_frontier(&mut candidates);

        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.identity() == configured_plan.identity));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.identity() == identical_plan.identity));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.identity() == distinct_plan.identity));
    }

    #[test]
    fn final_selection_preserves_identity_across_equal_two_context_artifacts() {
        let bump = Bump::new();
        let program = parse_source(&bump, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let root_plan = JavaScriptEmissionPlan {
            identity: JavaScriptPlanIdentity {
                context_id: 0,
                ordinal: 0,
            },
            options,
        };
        let other_plan = JavaScriptEmissionPlan {
            identity: JavaScriptPlanIdentity {
                context_id: 1,
                ordinal: 0,
            },
            options,
        };
        let mut candidates = vec![
            JavaScriptEmissionCandidate::new_declaration_plan(
                "console.log(1)".to_string(),
                root_plan,
                CompressionCostModel::Raw,
            )
            .unwrap(),
            JavaScriptEmissionCandidate::new_declaration_plan(
                "console.log(1)".to_string(),
                other_plan,
                CompressionCostModel::Raw,
            )
            .unwrap(),
        ];
        deduplicate_live_javascript_candidate_frontier(&mut candidates);
        assert_eq!(candidates.len(), 2);

        let contexts = JavaScriptEmissionContexts {
            root_configured_context_id: 0,
            contexts: vec![
                JavaScriptEmissionContext::new(0, &ir, None, None, false),
                JavaScriptEmissionContext::new(1, &ir, None, None, false),
            ],
            plan_registry: Mutex::new(JavaScriptPlanRegistry::default()),
            emissions_attempted: AtomicUsize::new(0),
        };
        let mut config = ProjectConfig::default();
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.optimizations = Some(Vec::new());
        let selected = finalize_javascript_candidates(
            candidates,
            "console.log(1)",
            root_plan.identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            2,
        )
        .unwrap();

        assert_eq!(selected.plan_identity, root_plan.identity);
    }

    #[test]
    fn oversized_optional_proposal_does_not_perturb_fitting_identity() {
        fn candidate(
            context_id: usize,
            ordinal: usize,
            code: &str,
            fully_scored: bool,
        ) -> JavaScriptEmissionCandidate {
            let mut candidate = JavaScriptEmissionCandidate::new(
                code.len(),
                code.to_string(),
                crate::codegen_ir_js::IrJsOptions::default(),
                CompressionCostModel::Brotli,
            );
            candidate.plan.identity = JavaScriptPlanIdentity {
                context_id,
                ordinal,
            };
            if fully_scored {
                candidate.objective_costs = [Some(code.len()); 3];
            }
            candidate
        }

        let mut arena = AggregateJavaScriptPlanArena::new(
            candidate(0, 0, "root", true),
            Vec::new(),
            3,
            12,
            CompressionCostModel::Brotli,
        )
        .unwrap();
        assert_eq!(arena.optional_proposal_width(), 2);
        assert_eq!(arena.optional_raw_size_cap(), 8);
        let fitting_identity = JavaScriptPlanIdentity {
            context_id: 1,
            ordinal: 1,
        };
        let measurements_before = javascript_codec_measurement_count();

        arena
            .merge_optional(vec![
                candidate(1, 0, "oversized", false),
                candidate(
                    fitting_identity.context_id,
                    fitting_identity.ordinal,
                    "fits",
                    true,
                ),
            ])
            .unwrap();

        assert_eq!(javascript_codec_measurement_count(), measurements_before);
        assert!(arena
            .iter()
            .any(|candidate| candidate.identity() == fitting_identity));
        assert!(!arena.iter().any(|candidate| {
            candidate.identity()
                == (JavaScriptPlanIdentity {
                    context_id: 1,
                    ordinal: 0,
                })
        }));
    }

    #[test]
    fn oversized_precomputed_proposal_conflicting_with_pinned_root_is_rejected() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let root_identity = JavaScriptPlanIdentity {
            context_id: 0,
            ordinal: 0,
        };
        let mut root = JavaScriptEmissionCandidate::new(
            4,
            "root".to_string(),
            options,
            CompressionCostModel::Brotli,
        );
        root.plan.identity = root_identity;
        let mut arena =
            AggregateJavaScriptPlanArena::new(root, Vec::new(), 2, 4, CompressionCostModel::Brotli)
                .unwrap();
        let mut conflicting = JavaScriptEmissionCandidate::new(
            9,
            "oversized".to_string(),
            options,
            CompressionCostModel::Brotli,
        );
        conflicting.plan.identity = root_identity;

        let error = arena
            .merge_precomputed_optional(vec![conflicting])
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("conflicting bytes for one pinned plan identity"),
            "{error}"
        );
        assert_eq!(arena.retained_plan_count(), 1);
        assert_eq!(arena.iter().next().unwrap().code(), "root");
    }

    #[test]
    fn exact_precomputed_duplicate_of_pinned_root_is_inert_without_codec_work() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let root_identity = JavaScriptPlanIdentity {
            context_id: 0,
            ordinal: 0,
        };
        let mut root = JavaScriptEmissionCandidate::new(
            4,
            "root".to_string(),
            options,
            CompressionCostModel::Brotli,
        );
        root.plan.identity = root_identity;
        let duplicate = root.clone();
        let mut arena =
            AggregateJavaScriptPlanArena::new(root, Vec::new(), 2, 4, CompressionCostModel::Brotli)
                .unwrap();
        let measurements_before = javascript_codec_measurement_count();

        arena.merge_precomputed_optional(vec![duplicate]).unwrap();

        assert_eq!(javascript_codec_measurement_count(), measurements_before);
        assert_eq!(arena.retained_plan_count(), 1);
        assert_eq!(arena.iter().next().unwrap().identity(), root_identity);
    }

    #[test]
    fn wrong_model_precomputed_duplicate_of_pinned_root_is_rejected() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let root_identity = JavaScriptPlanIdentity {
            context_id: 0,
            ordinal: 0,
        };
        let mut root = JavaScriptEmissionCandidate::new(
            4,
            "root".to_string(),
            options,
            CompressionCostModel::Brotli,
        );
        root.plan.identity = root_identity;
        let mut duplicate = JavaScriptEmissionCandidate::new(
            4,
            "root".to_string(),
            options,
            CompressionCostModel::Gzip,
        );
        duplicate.plan.identity = root_identity;
        let mut arena =
            AggregateJavaScriptPlanArena::new(root, Vec::new(), 2, 4, CompressionCostModel::Brotli)
                .unwrap();

        let error = arena
            .merge_precomputed_optional(vec![duplicate])
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("aggregate JavaScript arena received a different codec score ledger"),
            "{error}"
        );
        assert_eq!(arena.retained_plan_count(), 1);
    }

    #[test]
    fn oversized_precomputed_interaction_and_new_emission_are_equally_inert() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let mut root = JavaScriptEmissionCandidate::new(
            4,
            "root".to_string(),
            options,
            CompressionCostModel::Brotli,
        );
        root.plan.identity = JavaScriptPlanIdentity {
            context_id: 0,
            ordinal: 0,
        };
        let mut arena =
            AggregateJavaScriptPlanArena::new(root, Vec::new(), 2, 8, CompressionCostModel::Brotli)
                .unwrap();
        let interaction_plan = JavaScriptEmissionPlan {
            identity: JavaScriptPlanIdentity {
                context_id: 1,
                ordinal: 0,
            },
            options,
        };
        let mut interaction = JavaScriptEmissionCandidate::new(
            1,
            "large".to_string(),
            options,
            CompressionCostModel::Brotli,
        );
        interaction.plan = interaction_plan;
        let measurements_before = javascript_codec_measurement_count();

        arena.merge_precomputed_optional(vec![interaction]).unwrap();
        let newly_emitted = measure_optional_javascript_candidate(
            "large".to_string(),
            interaction_plan,
            CompressionCostModel::Brotli,
            arena.optional_raw_size_cap(),
        )
        .unwrap();

        assert!(newly_emitted.is_none());
        assert_eq!(javascript_codec_measurement_count(), measurements_before);
        assert_eq!(arena.retained_plan_count(), 1);
        assert!(!arena
            .iter()
            .any(|candidate| candidate.identity() == interaction_plan.identity));
    }

    #[test]
    fn oversized_precomputed_seed_still_validates_its_codec_ledger() {
        let bump = Bump::new();
        let program = parse_source(&bump, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(Vec::new());
        let configured_options = config.js_options();
        let configured = ScoredJavaScriptEmissionSeed {
            emission: ScoredJavaScriptEmission::with_exact_test_score(
                "root".to_string(),
                CompressionCostModel::Brotli,
                4,
            ),
            options: configured_options,
        };
        let invalid = ScoredJavaScriptEmissionSeed {
            emission: ScoredJavaScriptEmission::with_exact_test_score(
                "oversized".to_string(),
                CompressionCostModel::Gzip,
                1,
            ),
            options: crate::codegen_ir_js::IrJsOptions {
                struct_method_shorthand: !configured_options.struct_method_shorthand,
                ..configured_options
            },
        };

        let error = select_javascript_candidate(
            0,
            &ir,
            &config,
            false,
            &OptimizationProfile::default(),
            2,
            5,
            Some(configured),
            vec![invalid],
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("seeded JavaScript candidate uses a different codec score ledger"),
            "{error}"
        );
    }

    #[test]
    fn aggregate_plan_arena_pins_configured_floor_and_skips_oversized_ranked_pins() {
        fn candidate(
            context_id: usize,
            ordinal: usize,
            code: &str,
            objective_costs: [usize; 3],
        ) -> JavaScriptEmissionCandidate {
            let mut candidate = JavaScriptEmissionCandidate::new(
                objective_costs[1],
                code.to_string(),
                crate::codegen_ir_js::IrJsOptions::default(),
                CompressionCostModel::Gzip,
            );
            candidate.plan.identity = JavaScriptPlanIdentity {
                context_id,
                ordinal,
            };
            candidate.objective_costs = objective_costs.map(Some);
            candidate
        }

        let configured = candidate(0, 0, "root", [4, 4, 4]);
        let arena = AggregateJavaScriptPlanArena::new(
            configured,
            vec![
                candidate(1, 0, "1234567", [99, 1, 1]),
                candidate(1, 1, "aaa", [2, 2, 2]),
                candidate(1, 2, "bbb", [3, 3, 3]),
            ],
            4,
            10,
            CompressionCostModel::Gzip,
        )
        .unwrap();

        assert_eq!(arena.effective_plan_count_cap, 4);
        assert_eq!(arena.effective_code_byte_cap, 10);
        assert_eq!(arena.retained_plan_count(), 3);
        assert_eq!(arena.retained_code_bytes(), 10);
        assert_eq!(arena.optional_plan_count_cap, 1);
        assert_eq!(arena.optional_code_byte_cap, 0);
        let retained = arena.into_candidates();
        assert_eq!(retained[0].code(), "root");
        assert!(retained.iter().any(|candidate| candidate.code() == "aaa"));
        assert!(retained.iter().any(|candidate| candidate.code() == "bbb"));
        assert!(!retained
            .iter()
            .any(|candidate| candidate.code() == "1234567"));

        let configured = candidate(0, 0, "configured", [10, 10, 10]);
        let floor = AggregateJavaScriptPlanArena::new(
            configured,
            vec![candidate(1, 0, "x", [1, 1, 1])],
            0,
            0,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        assert_eq!(floor.effective_plan_count_cap, 1);
        assert_eq!(floor.effective_code_byte_cap, "configured".len());
        assert_eq!(floor.retained_plan_count(), 1);
        assert_eq!(floor.retained_code_bytes(), "configured".len());
        assert_eq!(floor.optional_plan_count_cap, 0);
        assert_eq!(floor.optional_code_byte_cap, 0);

        let mut optional = AggregateJavaScriptPlanArena::new(
            candidate(0, 0, "root", [4, 4, 4]),
            Vec::new(),
            4,
            16,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        assert_eq!(optional.optional_proposal_width(), 3);
        assert_eq!(optional.optional_raw_size_cap(), 12);
        optional
            .merge_optional(vec![
                candidate(1, 0, "1234567890123", [99, 1, 1]),
                candidate(2, 0, "aaaaaa", [2, 2, 2]),
                candidate(3, 0, "bbbbbb", [3, 3, 3]),
            ])
            .unwrap();
        assert_eq!(optional.retained_plan_count(), 3);
        assert_eq!(optional.retained_code_bytes(), 16);
        assert!(!optional
            .iter()
            .any(|candidate| candidate.code() == "1234567890123"));
        assert!(optional
            .iter()
            .any(|candidate| candidate.code() == "aaaaaa"));
        assert!(optional
            .iter()
            .any(|candidate| candidate.code() == "bbbbbb"));

        // Five seed-sized pins leave a four-byte tail. Even an effectively
        // unbounded configured count must allocate from the five actual input
        // pins and retain exactly one discovery proposal for that nonzero
        // tail, rather than reserving `usize::MAX` slots or disabling search.
        let mut tail = AggregateJavaScriptPlanArena::new(
            candidate(0, 0, "seed", [4, 4, 4]),
            (1..5)
                .map(|context_id| candidate(context_id, 0, "seed", [4, 4, 4]))
                .collect(),
            usize::MAX,
            24,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        assert_eq!(tail.retained_plan_count(), 5);
        assert_eq!(tail.retained_code_bytes(), 20);
        assert_eq!(tail.optional_code_byte_cap, 4);
        assert_eq!(tail.optional_proposal_width(), 1);
        assert!(tail
            .merge_optional(vec![
                candidate(5, 0, "a", [1, 1, 1]),
                candidate(6, 0, "b", [2, 2, 2]),
            ])
            .is_err());
        let mut precomputed = AggregateJavaScriptPlanArena::new(
            candidate(0, 0, "seed", [4, 4, 4]),
            Vec::new(),
            3,
            8,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        assert_eq!(precomputed.optional_proposal_width(), 1);
        precomputed
            .merge_precomputed_optional(vec![
                candidate(1, 0, "large", [99, 1, 1]),
                candidate(2, 0, "fits", [4, 2, 2]),
            ])
            .unwrap();
        assert!(!precomputed
            .iter()
            .any(|candidate| candidate.identity().context_id == 1));
        assert!(precomputed
            .iter()
            .any(|candidate| candidate.identity().context_id == 2));

        let mut projections = vec![
            candidate(3, 0, "worse", [5, 5, 5]),
            candidate(1, 0, "best", [4, 1, 1]),
            candidate(2, 0, "later", [5, 2, 2]),
        ];
        projections.sort_by(compare_javascript_seed_admission);
        assert_eq!(projections[0].identity().context_id, 1);
        let mut projection_arena = AggregateJavaScriptPlanArena::new(
            candidate(0, 0, "root", [4, 4, 4]),
            Vec::new(),
            2,
            8,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        assert_eq!(projection_arena.optional_proposal_width(), 1);
        for projection in projections {
            projection_arena.admit_ranked_pin(projection).unwrap();
        }
        assert_eq!(projection_arena.optional_proposal_width(), 0);
        assert!(projection_arena
            .iter()
            .any(|candidate| candidate.identity().context_id == 1));
        assert_eq!(
            optional_entropy_objective_cost_result(
                Err::<usize, _>("unselected gzip unavailable"),
                CompressionCostModel::Gzip,
                CompressionCostModel::Brotli,
            )
            .unwrap(),
            usize::MAX,
        );
        assert!(optional_entropy_objective_cost_result(
            Err::<usize, _>("selected Brotli unavailable"),
            CompressionCostModel::Brotli,
            CompressionCostModel::Brotli,
        )
        .is_err());

        assert_eq!(entropy_source_limit(12, 1), 1);
        assert_eq!(entropy_source_limit(2, 8), 2);
        let entropy_budget = entropy_mapping_trial_budget(2);
        let first_trials = entropy_trials_for_next_source(2, 1_024, entropy_budget, 2);
        let second_trials =
            entropy_trials_for_next_source(2, 10_000, entropy_budget - first_trials, 1);
        assert_eq!(entropy_budget, 128);
        assert!(first_trials + second_trials <= entropy_budget);
        assert_eq!(first_trials, 64);
        assert_eq!(second_trials, 16);
    }

    #[test]
    fn aggregate_plan_arena_keeps_equal_bytes_from_distinct_contexts() {
        fn candidate(context_id: usize, code: &str) -> JavaScriptEmissionCandidate {
            let mut candidate = JavaScriptEmissionCandidate::new(
                code.len(),
                code.to_string(),
                crate::codegen_ir_js::IrJsOptions::default(),
                CompressionCostModel::Raw,
            );
            candidate.plan.identity = JavaScriptPlanIdentity {
                context_id,
                ordinal: 0,
            };
            candidate
        }

        let mut arena = AggregateJavaScriptPlanArena::new(
            candidate(0, "root"),
            Vec::new(),
            3,
            12,
            CompressionCostModel::Raw,
        )
        .unwrap();
        arena
            .merge_optional(vec![candidate(1, "same"), candidate(2, "same")])
            .unwrap();

        let retained = arena.into_candidates();
        assert_eq!(retained.len(), 3);
        assert_eq!(
            retained
                .iter()
                .filter(|candidate| candidate.code() == "same")
                .count(),
            2
        );
    }

    #[test]
    fn aggregate_plan_arena_preserves_the_best_plan_before_local_regime_diversity() {
        fn candidate(
            context_id: usize,
            code: &str,
            local_name_coalescing: bool,
            objective_cost: usize,
        ) -> JavaScriptEmissionCandidate {
            let options = crate::codegen_ir_js::IrJsOptions {
                local_name_coalescing,
                ..crate::codegen_ir_js::IrJsOptions::default()
            };
            let mut candidate = JavaScriptEmissionCandidate::new(
                code.len(),
                code.to_string(),
                options,
                CompressionCostModel::Gzip,
            );
            candidate.plan.identity = JavaScriptPlanIdentity {
                context_id,
                ordinal: 0,
            };
            candidate.objective_costs = [Some(objective_cost); 3];
            candidate
        }

        let mut arena = AggregateJavaScriptPlanArena::new(
            candidate(0, "root", true, 2),
            Vec::new(),
            3,
            12,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        arena
            .merge_precomputed_optional(vec![
                candidate(1, "best", true, 1),
                candidate(2, "keep", false, 9),
            ])
            .unwrap();

        let retained = arena.into_candidates();
        assert_eq!(retained.len(), 3);
        assert!(retained
            .iter()
            .any(|candidate| !candidate.options().local_name_coalescing));
        assert!(retained
            .iter()
            .any(|candidate| candidate.identity().context_id == 1));

        let mut tight = AggregateJavaScriptPlanArena::new(
            candidate(0, "root", true, 2),
            Vec::new(),
            2,
            8,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        tight
            .merge_precomputed_optional(vec![
                candidate(1, "best", true, 1),
                candidate(2, "keep", false, 9),
            ])
            .unwrap();
        let tight = tight.into_candidates();
        assert!(tight
            .iter()
            .any(|candidate| candidate.identity().context_id == 1));
        assert!(!tight
            .iter()
            .any(|candidate| candidate.identity().context_id == 2));
    }

    #[test]
    fn aggregate_plan_arena_does_not_reserve_byte_identical_local_regimes() {
        fn candidate(
            context_id: usize,
            ordinal: usize,
            code: &str,
            local_name_coalescing: bool,
            objective_cost: usize,
        ) -> JavaScriptEmissionCandidate {
            let options = crate::codegen_ir_js::IrJsOptions {
                local_name_coalescing,
                ..crate::codegen_ir_js::IrJsOptions::default()
            };
            let mut candidate = JavaScriptEmissionCandidate::new(
                code.len(),
                code.to_string(),
                options,
                CompressionCostModel::Gzip,
            );
            candidate.plan.identity = JavaScriptPlanIdentity {
                context_id,
                ordinal,
            };
            candidate.objective_costs = [Some(objective_cost); 3];
            candidate
        }

        let mut arena = AggregateJavaScriptPlanArena::new(
            candidate(0, 0, "root", true, 4),
            Vec::new(),
            3,
            12,
            CompressionCostModel::Gzip,
        )
        .unwrap();
        arena
            .merge_precomputed_optional(vec![
                candidate(1, 0, "best", true, 1),
                candidate(1, 1, "best", false, 2),
                candidate(2, 0, "next", true, 3),
            ])
            .unwrap();

        let retained = arena.into_candidates();
        assert_eq!(retained.len(), 3);
        assert!(retained
            .iter()
            .any(|candidate| candidate.identity().context_id == 2));
        assert!(!retained
            .iter()
            .any(|candidate| !candidate.options().local_name_coalescing));
    }

    #[test]
    fn javascript_plan_identity_is_scoped_to_its_context() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let mut registry = JavaScriptPlanRegistry::default();

        let first = registry.register(7, options).unwrap();
        assert_eq!(first.identity.ordinal, 0);
        assert!(registry.register(7, options).is_none());

        let other_context = registry.register(11, options).unwrap();
        assert_eq!(other_context.identity.ordinal, 0);
        assert_ne!(first.identity, other_context.identity);

        let second = registry
            .register(
                7,
                crate::codegen_ir_js::IrJsOptions {
                    compact_boolean_literals: !options.compact_boolean_literals,
                    ..options
                },
            )
            .unwrap();
        assert_eq!(second.identity.ordinal, 1);
    }

    #[test]
    fn structural_plan_budget_is_charged_before_emission_and_preserves_terminal_slots() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let mut registry = JavaScriptPlanRegistry::default();
        let root = registry.register(0, options).unwrap();
        registry.set_optional_limit(2, 0, false, 0, 0);

        let first = registry
            .register(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    compact_boolean_literals: !options.compact_boolean_literals,
                    ..options
                },
            )
            .unwrap();
        let second = registry
            .register(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    pool_strings: !options.pool_strings,
                    ..options
                },
            )
            .unwrap();
        assert_eq!(registry.plans.len(), 3);
        assert!(registry
            .register(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    local_name_coalescing: !options.local_name_coalescing,
                    ..options
                },
            )
            .is_none());
        assert!(registry.limit_reached);
        assert_eq!(registry.structural_work_used, 2);
        assert_eq!(registry.plans.len(), 3);

        // Terminal challengers debit their own four-plan tail rather than the
        // exhausted structural ledger.
        let terminal = registry
            .register_terminal(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    precise_cross_scope_shadowing: !options.precise_cross_scope_shadowing,
                    ..options
                },
            )
            .unwrap();
        assert_eq!(terminal.identity.ordinal, 3);
        assert_eq!(root.identity.ordinal, 0);
        assert_eq!(first.identity.ordinal, 1);
        assert_eq!(second.identity.ordinal, 2);
    }

    #[test]
    fn priority_plan_reserve_is_free_for_duplicates_and_never_exceeds_the_hard_cap() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let mut registry = JavaScriptPlanRegistry::default();
        registry.register_terminal(0, options).unwrap();
        registry.set_optional_limit(4, 0, false, 2, 0);

        let first_regular = crate::codegen_ir_js::IrJsOptions {
            compact_boolean_literals: !options.compact_boolean_literals,
            ..options
        };
        let second_regular = crate::codegen_ir_js::IrJsOptions {
            pool_strings: !options.pool_strings,
            ..options
        };
        assert!(registry.register(0, first_regular).is_some());
        assert!(registry.register(0, second_regular).is_some());
        assert_eq!(registry.structural_work_used, 2);
        assert_eq!(registry.remaining_structural_work(), 0);
        assert!(registry
            .register(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    local_name_coalescing: !options.local_name_coalescing,
                    ..options
                },
            )
            .is_none());
        assert_eq!(registry.structural_work_used, 2);

        // A duplicate never consumes the protected slice.
        assert!(registry.register_priority(0, first_regular).is_none());
        assert_eq!(registry.structural_work_used, 2);

        assert!(registry
            .register_priority(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    precise_cross_scope_shadowing: !options.precise_cross_scope_shadowing,
                    ..options
                },
            )
            .is_some());
        assert!(registry
            .register_priority(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    struct_method_shorthand: !options.struct_method_shorthand,
                    ..options
                },
            )
            .is_some());
        assert_eq!(registry.structural_work_used, 4);
        assert!(registry
            .register_priority(
                0,
                crate::codegen_ir_js::IrJsOptions {
                    inline_single_use_functions: !options.inline_single_use_functions,
                    ..options
                },
            )
            .is_none());
        assert_eq!(registry.structural_work_used, 4);
        assert_eq!(registry.plans.len(), 5);
        assert!(registry.limit_reached);
    }

    #[test]
    fn priority_families_share_the_reserved_slice_and_release_unused_work() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let option = |minimum| crate::codegen_ir_js::IrJsOptions {
            string_pool_minimum_savings: minimum,
            ..options
        };
        let mut registry = JavaScriptPlanRegistry::default();
        registry.register_terminal(0, options).unwrap();
        registry.set_optional_limit(32, 0, false, 11, 2);

        for minimum in 1_000..1_021 {
            assert!(registry.register(0, option(minimum)).is_some());
        }
        assert_eq!(registry.structural_work_used, 21);
        assert!(registry.register(0, option(1_021)).is_none());

        assert_eq!(registry.begin_priority_family(), 6);
        let before_duplicate = registry.structural_work_used;
        assert!(registry.register_priority(0, option(1_000)).is_none());
        assert_eq!(registry.structural_work_used, before_duplicate);
        for minimum in 2_000..2_006 {
            assert!(registry.register_priority(0, option(minimum)).is_some());
        }
        assert!(registry.register_priority(0, option(2_006)).is_none());
        registry.end_priority_family();

        assert_eq!(registry.begin_priority_family(), 5);
        let joint_arrow = crate::codegen_ir_js::IrJsOptions {
            inline_single_use_functions: true,
            function_spelling: crate::codegen_ir_js::FunctionSpelling::Arrow,
            ..options
        };
        let joint_function = crate::codegen_ir_js::IrJsOptions {
            inline_single_use_functions: true,
            function_spelling: crate::codegen_ir_js::FunctionSpelling::Function,
            ..options
        };
        assert!(registry.register_priority(0, joint_arrow).is_some());
        assert!(registry.register_priority(0, joint_function).is_some());
        for minimum in 3_000..3_003 {
            assert!(registry.register_priority(0, option(minimum)).is_some());
        }
        registry.end_priority_family();
        assert_eq!(registry.structural_work_used, 32);
        assert_eq!(registry.priority_work_reserve, 0);
        assert_eq!(registry.plans.len(), 33);
        assert!(registry.find(0, joint_arrow).is_some());
        assert!(registry.find(0, joint_function).is_some());

        // If the last family contains only an already-registered identity,
        // its unused slice returns to ordinary work instead of disappearing.
        let mut duplicates_only = JavaScriptPlanRegistry::default();
        duplicates_only.register_terminal(0, options).unwrap();
        duplicates_only.set_optional_limit(8, 0, false, 3, 1);
        assert_eq!(duplicates_only.begin_priority_family(), 3);
        assert!(duplicates_only.register_priority(0, options).is_none());
        duplicates_only.end_priority_family();
        assert_eq!(duplicates_only.structural_work_used, 0);
        assert_eq!(duplicates_only.priority_work_reserve, 0);
        assert!(duplicates_only.register(0, option(4_000)).is_some());
    }

    #[test]
    fn level_eight_priority_reserve_cannot_withhold_unconsumable_work() {
        // Level 8 admits 192 optional proposals with a four-plan beam. Five
        // active priority families can consume at most one fair beam apiece,
        // so protecting the old fixed twelve-family capacity stranded 28
        // permits after ordinary enumeration had already stopped.
        let proposal_limit = 192;
        let beam_width = 4;
        let priority_family_count = 5;
        let priority_reserve =
            priority_candidate_proposal_reserve(proposal_limit, beam_width, priority_family_count);
        assert_eq!(priority_reserve, 20);
        // The one-third cap still controls a small explicit proposal budget.
        assert_eq!(
            priority_candidate_proposal_reserve(32, beam_width, priority_family_count),
            11
        );

        let options = crate::codegen_ir_js::IrJsOptions::default();
        let option = |minimum| crate::codegen_ir_js::IrJsOptions {
            string_pool_minimum_savings: minimum,
            ..options
        };
        let mut registry = JavaScriptPlanRegistry::default();
        registry.register_terminal(0, options).unwrap();
        registry.set_optional_limit(
            proposal_limit,
            0,
            false,
            priority_reserve,
            priority_family_count,
        );

        for minimum in 10_000..10_172 {
            assert!(registry.register(0, option(minimum)).is_some());
        }
        assert_eq!(registry.structural_work_used, 172);
        assert_eq!(registry.remaining_structural_work(), 0);

        for family in 0..priority_family_count {
            assert_eq!(registry.begin_priority_family(), beam_width);
            for offset in 0..beam_width {
                let minimum = 20_000 + family * beam_width + offset;
                assert!(registry.register_priority(0, option(minimum)).is_some());
            }
            registry.end_priority_family();
        }
        assert_eq!(registry.structural_work_used, proposal_limit);
        assert_eq!(registry.priority_work_reserve, 0);
        assert_eq!(registry.plans.len(), proposal_limit + 1);
    }

    #[test]
    fn bounded_candidate_merge_scores_proposals_before_evicting_the_old_frontier() {
        fn candidate(code: &str, cost: usize) -> JavaScriptEmissionCandidate {
            let mut candidate = JavaScriptEmissionCandidate::new(
                cost,
                code.to_string(),
                crate::codegen_ir_js::IrJsOptions::default(),
                CompressionCostModel::Brotli,
            );
            candidate.objective_costs = [Some(cost); 3];
            candidate
        }

        let mut candidates = vec![
            candidate("old-a", 10),
            candidate("old-b", 11),
            candidate("old-c", 12),
        ];
        let proposals = vec![
            candidate("proposal-a", 99),
            candidate("proposal-b", 98),
            candidate("proposal-c", 97),
        ];

        merge_javascript_candidate_frontiers(
            &mut candidates,
            proposals,
            3,
            CompressionCostModel::Brotli,
        )
        .unwrap();

        assert_eq!(
            candidates
                .iter()
                .map(JavaScriptEmissionCandidate::code)
                .collect::<Vec<_>>(),
            ["old-a", "old-b", "old-c"]
        );
    }

    #[test]
    fn bounded_variant_sampling_spans_spelling_columns() {
        use crate::codegen_ir_js::{IrJsOptions, MutationSpelling};

        let groups = (0..4)
            .map(|local_name_reserve| {
                [
                    MutationSpelling::Prefix,
                    MutationSpelling::Postfix,
                    MutationSpelling::Compound,
                ]
                .map(|mutation_spelling| IrJsOptions {
                    local_name_reserve,
                    mutation_spelling,
                    ..IrJsOptions::default()
                })
                .to_vec()
            })
            .collect::<Vec<_>>();

        let sampled = bounded_javascript_variant_options(groups, 3);

        assert_eq!(sampled.len(), 3);
        assert_eq!(sampled[0].mutation_spelling, MutationSpelling::Prefix);
        assert_eq!(sampled[1].mutation_spelling, MutationSpelling::Postfix);
        assert_eq!(sampled[2].mutation_spelling, MutationSpelling::Compound);
    }

    #[test]
    fn alternate_objective_failure_cannot_fail_the_selected_build() {
        let mut candidate = JavaScriptEmissionCandidate::new(
            1,
            "0".to_string(),
            crate::codegen_ir_js::IrJsOptions::default(),
            CompressionCostModel::Raw,
        );
        let alternate_error = crate::codegen_js::CodegenError::new(
            Span::empty(0),
            "diagnostic Brotli scorer unavailable",
        )
        .into();
        retain_objective_cost_result(
            &mut candidate,
            CompressionCostModel::Brotli,
            CompressionCostModel::Raw,
            Err(alternate_error),
        )
        .unwrap();
        assert_eq!(
            candidate.objective_costs[objective_index(CompressionCostModel::Brotli)],
            Some(usize::MAX)
        );

        let selected_error = crate::codegen_js::CodegenError::new(
            Span::empty(0),
            "selected Brotli scorer unavailable",
        )
        .into();
        assert!(retain_objective_cost_result(
            &mut candidate,
            CompressionCostModel::Brotli,
            CompressionCostModel::Brotli,
            Err(selected_error),
        )
        .is_err());

        assert_eq!(
            optional_entropy_objective_cost_result(
                Err::<usize, _>("unselected gzip unavailable"),
                CompressionCostModel::Gzip,
                CompressionCostModel::Brotli,
            )
            .unwrap(),
            usize::MAX,
        );
        assert!(optional_entropy_objective_cost_result(
            Err::<usize, _>("selected Brotli unavailable"),
            CompressionCostModel::Brotli,
            CompressionCostModel::Brotli,
        )
        .is_err());
    }

    #[test]
    fn configured_baseline_survives_a_one_candidate_final_frontier() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.optimizations = Some(Vec::new());
        let baseline = "console.log(123456789)";
        let options = config.js_options();
        let candidates = vec![
            JavaScriptEmissionCandidate::new(
                1,
                "0".to_string(),
                options,
                CompressionCostModel::Raw,
            ),
            JavaScriptEmissionCandidate::new(
                baseline.len(),
                baseline.to_string(),
                options,
                CompressionCostModel::Raw,
            ),
        ];
        let configured_identity = candidates[1].identity();
        let contexts = test_javascript_contexts(&ir);
        let selected = finalize_javascript_candidates(
            candidates,
            baseline,
            configured_identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            1,
        )
        .unwrap();
        assert_eq!(selected.code, baseline);
        assert_eq!(selected.candidates_evaluated, 1);
    }

    #[test]
    fn one_plan_final_frontier_selects_its_codec_best_declaration_leaf() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(Vec::new());
        let baseline = "var b={left:2,right:3,middle:4},a={...b,right:12};b.left=22;console.log(a.left??0);console.log(a.right??0);console.log(a.none??-1);console.log(Object.keys(a).join(\",\"));console.log(JSON.stringify(a))";
        let winning_leaf = format!("let {}", &baseline[4..]);
        assert!(
            compressed_size(winning_leaf.as_bytes(), config.javascript.cost_model,).unwrap()
                < compressed_size(baseline.as_bytes(), config.javascript.cost_model).unwrap()
        );

        let plan = test_javascript_plan(0, config.js_options());
        let candidates = vec![JavaScriptEmissionCandidate::new_declaration_plan(
            baseline.to_string(),
            plan,
            config.javascript.cost_model,
        )
        .unwrap()];
        let contexts = test_javascript_contexts(&ir);
        let selected = finalize_javascript_candidates(
            candidates,
            baseline,
            plan.identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            1,
        )
        .unwrap();

        assert_eq!(selected.code, winning_leaf);
        assert_eq!(selected.candidates_evaluated, 1);
    }

    #[test]
    fn entropy_probe_preparation_matches_one_and_four_threads_exactly() {
        fn prepare(code: String, _options: crate::codegen_ir_js::IrJsOptions) -> Option<String> {
            (code != "fail").then(|| format!("{code}-prepared"))
        }

        let sources = vec![
            test_entropy_source_candidate(10, "first"),
            test_entropy_source_candidate(11, "fail"),
            test_entropy_source_candidate(12, "third"),
            test_entropy_source_candidate(13, "fourth"),
        ];
        let run = |threads, sources| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| prepare_entropy_source_requests_by(sources, 8, prepare))
        };
        let serial = run(1, sources.clone());
        let parallel = run(4, sources);

        assert_eq!(parallel, serial);
        assert_eq!(
            parallel
                .iter()
                .map(|(plan, probe, trials)| {
                    (plan.identity.context_id, probe.as_str(), *trials)
                })
                .collect::<Vec<_>>(),
            vec![
                (10, "first-prepared", 128),
                (12, "third-prepared", 192),
                (13, "fourth-prepared", 192),
            ]
        );
    }

    #[test]
    fn entropy_probe_preparation_uses_the_active_parallel_pool() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let sources = (0..8)
            .map(|context_id| test_entropy_source_candidate(context_id, "probe"))
            .collect::<Vec<_>>();
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let requests = pool.install(|| {
            prepare_entropy_source_requests_by(sources, 8, {
                let barrier = Arc::clone(&barrier);
                let active = Arc::clone(&active);
                let maximum = Arc::clone(&maximum);
                move |code, _options| {
                    let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(running, Ordering::SeqCst);
                    barrier.wait();
                    active.fetch_sub(1, Ordering::SeqCst);
                    Some(code)
                }
            })
        });

        assert_eq!(requests.len(), 8);
        assert_eq!(maximum.load(Ordering::SeqCst), 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn identifier_alphabet_search_remains_serial_and_preserves_order_and_errors() {
        let options = crate::codegen_ir_js::IrJsOptions::default();
        let plan = |context_id| JavaScriptEmissionPlan {
            identity: JavaScriptPlanIdentity {
                context_id,
                ordinal: 0,
            },
            options,
        };
        let requests = vec![
            (
                plan(10),
                "var F=[1,2,3];console.log(F.join('-'))".to_string(),
                4,
            ),
            (plan(11), "'unterminated".to_string(), 4),
            (
                plan(12),
                "var G=[4,5,6];console.log(G.join('-'))".to_string(),
                4,
            ),
        ];
        let expected_source_order = requests
            .iter()
            .map(|(_, probe, _)| probe.clone())
            .collect::<Vec<_>>();
        let mut observed_source_order = Vec::new();
        let dry_run =
            search_identifier_alphabet_groups_by(requests.clone(), |probe, _baseline, _trials| {
                observed_source_order.push(probe.to_string());
                if probe == "'unterminated" {
                    Err(())
                } else {
                    Ok(Vec::new())
                }
            });
        assert_eq!(observed_source_order, expected_source_order);
        assert_eq!(dry_run.len(), 2, "the same invalid source must be skipped");

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let searched = pool
            .install(|| search_identifier_alphabet_groups(requests, CompressionCostModel::Brotli));

        assert_eq!(searched.len(), 2, "the same invalid source must be skipped");
        assert!(searched.iter().all(|group| !group.is_empty()));
        assert!(searched[0].iter().all(|(context, _)| *context == 10));
        assert!(searched[1].iter().all(|(context, _)| *context == 12));
    }

    #[test]
    fn unused_letter_binding_remap_can_leave_the_initial_live_character_set() {
        let code = "var F=[1,2,3,4,5];console.log(F==F),F.reverse(),console.log(F.join('-'))";
        let baseline = compressed_size(code.as_bytes(), CompressionCostModel::Brotli).unwrap();
        let mut codec_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let remapped = best_unused_letter_binding_remaps(
            code,
            CompressionCostModel::Brotli,
            &mut codec_budget,
        )
        .unwrap()
        .expect("an unused letter should beat F");
        assert!(
            remapped.1 < baseline,
            "baseline={baseline}, remapped={}",
            remapped.1
        );
        assert!(!remapped.0.contains("var F="), "{}", remapped.0);
        assert!(single_character_name_is_clear_binding(code, b'F').unwrap());
    }

    #[test]
    fn unused_letter_binding_remap_can_recover_a_two_name_brotli_interaction() {
        let code = "let a=b=>10+b|0;console.log(a(3));console.log(a(8))";
        let baseline = compressed_size(code.as_bytes(), CompressionCostModel::Brotli).unwrap();
        assert!(
            resolved_one_byte_binding_count(code)
                > resolved_one_byte_binding_count(
                    "let $=$=>10+$|0;console.log($(3));console.log($(8))",
                ),
        );
        let mut codec_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let remapped = best_two_binding_unused_letter_remap(
            code,
            CompressionCostModel::Brotli,
            &mut codec_budget,
        )
        .unwrap()
        .unwrap_or_else(|| (code.to_string(), baseline));
        assert!(
            remapped.1 <= 52,
            "baseline={baseline}, remapped={} code={}",
            remapped.1,
            remapped.0,
        );
        assert_eq!(
            remapped.0,
            "var g=l=>10+l|0;console.log(g(3));console.log(g(8))"
        );
    }

    #[test]
    fn exact_two_binding_search_has_a_fixed_trial_ceiling() {
        let pairs = exact_two_binding_replacement_pairs(ONE_BYTE_IDENTIFIER_STARTS);
        assert_eq!(pairs.len(), EXACT_TWO_BINDING_MAX_PAIR_TRIALS);
        assert_eq!(EXACT_TWO_BINDING_MAX_PAIR_TRIALS, 56);
        assert_eq!(
            EXACT_TWO_BINDING_MAX_PAIR_TRIALS * MAX_DECLARATION_VARIANTS,
            224,
        );

        let mut level_nine = ProjectConfig::default();
        level_nine.javascript.candidate_search = CandidateSearch::Always;
        level_nine.javascript.optimization_level = 9;
        assert!(!exact_two_binding_terminal_search_enabled(&level_nine));
        let mut level_twelve = level_nine;
        level_twelve.javascript.optimization_level = 12;
        assert!(exact_two_binding_terminal_search_enabled(&level_twelve));

        let three_bindings = "let a=1,b=2,c=3;console.log(a+b+c)";
        assert_eq!(resolved_one_byte_binding_count(three_bindings), 3);
        let mut codec_budget = TerminalCodecProbeBudget::new(usize::MAX);
        assert!(
            best_two_binding_unused_letter_remap(
                three_bindings,
                CompressionCostModel::Brotli,
                &mut codec_budget,
            )
            .unwrap()
            .is_none(),
            "artifacts outside the exact-two-binding gate must skip pair trials",
        );
    }

    #[test]
    fn unused_binding_remaps_never_rename_an_ambient_one_byte_host() {
        let code = "let a=b=>10+b|0;X(a),console.log(a(3));console.log(a(8))";
        let resolved = single_character_resolved_binding_identifiers(code).unwrap();
        assert!(resolved.contains(&b'a'));
        assert!(resolved.contains(&b'b'));
        assert!(!resolved.contains(&b'X'));

        let baseline = compressed_size(code.as_bytes(), CompressionCostModel::Brotli).unwrap();
        let mut exact_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let (remapped, remapped_cost) = best_two_binding_unused_letter_remap(
            code,
            CompressionCostModel::Brotli,
            &mut exact_budget,
        )
        .unwrap()
        .expect("the exact pair neighborhood should contain a strict Brotli win");
        assert!(remapped_cost < baseline, "{baseline} -> {remapped_cost}");
        assert!(remapped.contains("X("), "{remapped}");
        let mut greedy_budget = TerminalCodecProbeBudget::new(usize::MAX);
        if let Some((remapped, _)) = best_unused_letter_binding_remaps(
            code,
            CompressionCostModel::Brotli,
            &mut greedy_budget,
        )
        .unwrap()
        {
            assert!(remapped.contains("X("), "{remapped}");
        }
    }

    #[test]
    fn higher_effort_retains_the_lower_effort_two_binding_brotli_winner() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "func(int)->int makeAdder(int base){return (int value)=>base+value;}func(int)->int add=makeAdder(10);print(add(3));print(add(8));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut base = javascript_oracle_config();
        base.javascript.priority = JavaScriptPriority::SizeFirst;
        base.javascript.cost_model = CompressionCostModel::Brotli;
        base.javascript.candidate_search = CandidateSearch::Always;
        base.javascript.candidate_limit = 1536;
        base.javascript.candidate_beam_width = 12;
        base.javascript.local_name_reserve = 48;
        base.optimization.inlining = Some(true);
        base.optimization.identical_function_folding = Some(true);
        base.optimization.function_subsumption = Some(true);
        base.optimization.scalar_replacement = Some(true);
        base.mangle.identifiers = Some(true);
        base.mangle.properties = Some(true);
        base.mangle.exports = Some(true);

        let mut outputs = Vec::new();
        for level in [6, 9, 12, 15] {
            let mut config = base.clone();
            config.javascript.optimization_level = level;
            let selected = optimize_and_select_javascript(ir.clone(), &config, false).unwrap();
            outputs.push((
                level,
                selected.selection_metrics.transfer_bytes,
                selected.javascript,
            ));
        }
        for pair in outputs.windows(2) {
            assert!(
                pair[1].1 <= pair[0].1,
                "level {}={} regressed from level {}={}\n{}\n{}",
                pair[1].0,
                pair[1].1,
                pair[0].0,
                pair[0].1,
                pair[0].2,
                pair[1].2,
            );
        }
        assert_eq!(
            outputs.last().map(|row| row.1),
            Some(52),
            "the highest built-in effort tier retains the exact pair winner"
        );
    }

    #[test]
    fn short_binding_remap_can_collapse_a_two_character_local() {
        let code =
            "var ge=[1,2,3,4,5,6,7,8,9];console.log(ge==ge),ge.reverse(),console.log(ge.join('-'))";
        let baseline = compressed_size(code.as_bytes(), CompressionCostModel::Brotli).unwrap();
        let mut codec_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let remapped =
            best_short_binding_remaps(code, CompressionCostModel::Brotli, &mut codec_budget)
                .unwrap()
                .expect("an unused letter should beat ge");
        assert!(
            remapped.1 < baseline,
            "baseline={baseline}, remapped={}",
            remapped.1
        );
        assert!(!remapped.0.contains("var ge="), "{}", remapped.0);
    }

    #[test]
    fn identifier_alphabet_search_can_leave_the_initial_live_character_set() {
        let code = "var F=[1,2,3,4,5];console.log(F==F),F.reverse(),console.log(F.join('-'))";
        let baseline = compressed_size(code.as_bytes(), CompressionCostModel::Brotli).unwrap();
        let alphabets = search_identifier_alphabets(
            code,
            crate::codegen_ir_js::IdentifierAlphabet::canonical(),
            CompressionCostModel::Brotli,
            64,
            4,
        )
        .unwrap();
        assert!(!alphabets.is_empty());
        let mut mapping = std::array::from_fn(|index| index as u8);
        mapping[b'F' as usize] = b'l';
        mapping[b'l' as usize] = b'F';
        let mut ranked = Vec::new();
        rank_identifier_mapping(code, CompressionCostModel::Brotli, 1, mapping, &mut ranked)
            .unwrap();
        let best = ranked[0].objective_costs[objective_index(CompressionCostModel::Brotli)];
        assert!(best < baseline, "baseline={baseline}, best={best}");
    }

    #[test]
    fn final_peephole_keeps_the_exact_codec_baseline_as_a_candidate() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.optimizations = Some(vec![JavaScriptOptimization::ParsedPeephole]);

        let original = "let a=0,b=1,s=\"a = a + b \";a=a+b;console.log(a,s)";
        let optimized = crate::js_peephole::optimize_generated_javascript(original).unwrap();
        assert_eq!(
            optimized.code,
            "let a=0,b=1,s=\"a = a + b \";a+=b,console.log(a,s)"
        );
        assert!(
            compressed_size(original.as_bytes(), CompressionCostModel::Brotli).unwrap()
                < compressed_size(optimized.code.as_bytes(), CompressionCostModel::Brotli).unwrap()
        );

        let candidates = vec![JavaScriptEmissionCandidate::new(
            compressed_size(original.as_bytes(), CompressionCostModel::Brotli).unwrap(),
            original.to_string(),
            config.js_options(),
            CompressionCostModel::Brotli,
        )];
        let configured_identity = candidates[0].identity();
        let contexts = test_javascript_contexts(&ir);
        let selected = finalize_javascript_candidates(
            candidates,
            original,
            configured_identity,
            &config,
            &contexts,
            &OptimizationProfile::default(),
            usize::MAX,
        )
        .unwrap();

        // Final declaration-spelling probes may improve the retained baseline
        // (`let` and `var` are equivalent for this top-level generated
        // artifact), so the exact source string need not win. The important
        // fallback contract is that the independently worse parsed peephole
        // cannot displace a baseline-or-better candidate.
        assert_ne!(selected.code, optimized.code);
        assert!(
            selected.transfer_cost
                <= compressed_size(original.as_bytes(), CompressionCostModel::Brotli).unwrap(),
            "selected={} baseline={}",
            selected.transfer_cost,
            compressed_size(original.as_bytes(), CompressionCostModel::Brotli).unwrap()
        );
        assert_eq!(selected.peephole_rewrites, 0);
    }

    #[test]
    fn candidate_search_keeps_a_valid_structured_for_in_baseline() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern JsValue arguments;export string keys(){string out=\"\";for(string key in arguments){if(arguments[key].truthy()){out=out+key;}}return out;}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.candidate_limit = 8;
        config.javascript.candidate_beam_width = 4;
        config.javascript.candidate_byte_budget = 64 * 1024;
        let output = compile_program_to_js_module_configured(&program, &config).unwrap();

        assert!(output.contains(" in "), "{output}");
        assert!(output.contains("function "), "{output}");
        assert!(!output.contains("JsForIn"), "{output}");
    }

    #[test]
    fn candidate_raw_growth_limit_is_exact_and_tunable() {
        assert!(candidate_raw_size_allowed(100, 100, 0));
        assert!(!candidate_raw_size_allowed(101, 100, 0));
        assert!(candidate_raw_size_allowed(105, 100, 5));
        assert!(!candidate_raw_size_allowed(106, 100, 5));
    }

    #[test]
    fn compressed_cost_models_keep_better_transfer_despite_raw_growth() {
        assert!(optimizer_variant_candidate_allowed(
            CompressionCostModel::Brotli,
            90,
            100,
            120,
            100,
            0,
        ));
        assert!(!optimizer_variant_candidate_allowed(
            CompressionCostModel::Brotli,
            110,
            100,
            120,
            100,
            0,
        ));
        assert!(!optimizer_variant_candidate_allowed(
            CompressionCostModel::Raw,
            90,
            100,
            120,
            100,
            0,
        ));
        assert!(optimizer_variant_candidate_allowed(
            CompressionCostModel::Gzip,
            110,
            100,
            100,
            100,
            0,
        ));
    }

    #[test]
    fn optimizer_search_includes_the_reusable_helper_corner() {
        let config = ProjectConfig::default();
        let configured = config.js_optimizer_options();
        let variants = crate::decision_registry::scored_ir_optimizer_clones(&config, configured);

        assert!(variants.iter().any(|candidate| {
            !candidate.inlining
                && !candidate.constant_parameter_specialization
                && !candidate.call_site_specialization
        }));
        assert!(variants
            .iter()
            .any(|candidate| !candidate.scalar_replacement));
    }

    #[test]
    fn javascript_search_adapts_local_name_reservation() {
        let options = ProjectConfig::default().js_options();
        assert_eq!(
            local_name_reserve_variants(options).map(|variant| variant.local_name_reserve),
            [0, 8, 16, 32]
        );
    }

    #[test]
    fn terminal_scope_naming_challenges_the_actual_winner_with_codec_friendly_prefixes() {
        let mut config = ProjectConfig::default();
        config.javascript.local_name_reserve = 8;
        let configured = config.js_options();
        assert!(configured.mangle_identifiers);
        assert!(configured.cross_scope_name_reuse);

        let parent = crate::codegen_ir_js::IrJsOptions {
            local_name_reserve: 16,
            ..configured
        };
        let variants = terminal_scope_naming_options(parent, configured);
        assert!(variants.iter().any(|variant| {
            variant.precise_cross_scope_shadowing && !variant.reserved_local_name_prefix
        }));
        for reserve in [8, 16, 32] {
            assert!(variants.iter().any(|variant| {
                variant.precise_cross_scope_shadowing
                    && variant.reserved_local_name_prefix
                    && variant.local_name_reserve == reserve
            }));
        }
        assert!(variants.iter().any(|variant| {
            variant.transitive_nested_shadowing && !variant.precise_cross_scope_shadowing
        }));
        assert!(variants.iter().all(|variant| *variant != parent));
    }

    #[test]
    fn terminal_candidate_budget_charges_retained_and_challenger_slots_and_bytes() {
        let mut budget = TerminalJavaScriptCandidateBudget::after_retained(4, 10, 2, 7);
        assert_eq!(
            budget,
            TerminalJavaScriptCandidateBudget {
                remaining_plans: 2,
                remaining_code_bytes: 3,
            }
        );

        // A byte-oversized challenger consumes neither ledger, so a later
        // smaller spelling can still use the tail.
        assert!(!budget.can_admit(4));
        assert_eq!(budget.remaining_plans, 2);
        assert_eq!(budget.remaining_code_bytes, 3);
        assert!(budget.can_admit(2));
        budget.charge(2);
        assert!(budget.can_admit(1));
        budget.charge(1);

        // The final zero-byte spelling is still rejected because the shared
        // structural plan capacity is exhausted independently of source bytes.
        assert!(!budget.can_admit(0));
        assert_eq!(budget.remaining_plans, 0);
        assert_eq!(budget.remaining_code_bytes, 0);

        let smaller_can_fit = TerminalJavaScriptCandidateBudget::after_retained(3, 8, 2, 7);
        assert!(!smaller_can_fit.cannot_admit_any_challenger());
        assert!(smaller_can_fit.can_admit(1));
        let byte_exhausted = TerminalJavaScriptCandidateBudget::after_retained(3, 7, 2, 7);
        assert!(byte_exhausted.cannot_admit_any_challenger());
        let slot_exhausted = TerminalJavaScriptCandidateBudget::after_retained(2, 8, 2, 7);
        assert!(slot_exhausted.cannot_admit_any_challenger());
    }

    #[test]
    fn terminal_family_reserves_are_released_at_most_once() {
        let exact_pair_reserve = 226;
        let challenger_reserve = 20;
        let finalist_reserve = 96;
        let mut budget = TerminalCodecProbeBudget::with_final_reserve(
            384,
            exact_pair_reserve + challenger_reserve + finalist_reserve,
        );

        budget.release_finalist_reserve_once(finalist_reserve);
        budget.release_finalist_reserve_once(finalist_reserve);
        assert_eq!(
            budget.reserved_for_final,
            exact_pair_reserve + challenger_reserve
        );
        budget.release_challenger_reserve_once(challenger_reserve);
        budget.release_challenger_reserve_once(challenger_reserve);
        assert_eq!(budget.reserved_for_final, exact_pair_reserve);
    }

    #[test]
    fn terminal_codec_probe_budget_is_a_hard_compilation_wide_call_ceiling() {
        let measurements_before = javascript_codec_measurement_count();
        let mut budget = TerminalCodecProbeBudget::new(7);
        let mut completed = 0;
        for index in 0..11 {
            if budget
                .compressed_size(
                    format!("let value={index}").as_bytes(),
                    CompressionCostModel::Raw,
                )
                .unwrap()
                .is_some()
            {
                completed += 1;
            }
        }

        assert_eq!(completed, 7);
        assert_eq!(budget.used, 7);
        assert_eq!(budget.codec_calls(), 7);
        assert!(budget.limit_reached);
        assert_eq!(
            javascript_codec_measurement_count() - measurements_before,
            7
        );
        // A later terminal family sees the same exhausted ledger.
        assert!(budget
            .compressed_size(b"let later=1", CompressionCostModel::Raw)
            .unwrap()
            .is_none());
        assert_eq!(budget.used, 7);
    }

    #[test]
    fn generated_javascript_admission_rejects_invalid_code_before_codec() {
        let measurements_before = javascript_codec_measurement_count();
        let mut budget = TerminalCodecProbeBudget::new(2);

        let malformed = budget
            .compressed_size(b"function f(){return [1,2}", CompressionCostModel::Raw)
            .unwrap_err();
        assert!(malformed.to_string().contains("admission failed"));
        let unresolved = budget
            .compressed_size(
                b"function a(){return x}function b(){let x=1}",
                CompressionCostModel::Raw,
            )
            .unwrap_err();
        assert!(unresolved
            .to_string()
            .contains("unresolved generated identifier"));

        assert_eq!(budget.codec_calls(), 0);
        assert_eq!(javascript_codec_measurement_count(), measurements_before);
    }

    #[test]
    fn declaration_variants_are_admitted_before_scoring() {
        let measurements_before = javascript_codec_measurement_count();
        let error = ScoredJavaScriptEmission::measure(
            "function f(){return [1,2}".to_string(),
            CompressionCostModel::Raw,
        )
        .unwrap_err();

        assert!(error.to_string().contains("admission failed"));
        assert_eq!(javascript_codec_measurement_count(), measurements_before);
    }

    #[test]
    fn observed_export_names_must_match_the_typed_abi() {
        let manifest = crate::compilation_contract::JavaScriptAbiManifest {
            world: "reusable-library",
            exports: vec![crate::compilation_contract::JavaScriptExportAbi {
                name: "expected".to_string(),
                kind: crate::compilation_contract::JavaScriptExportKind::Global,
                arity: None,
                constructible: None,
                methods: Vec::new(),
            }],
            export_names_may_mangle: false,
            foreign_imports: Vec::new(),
            public_aggregate_abi: "named",
            stable_aggregate_fields: Vec::new(),
            stable_extern_fields: Vec::new(),
        };

        let error = validate_observed_javascript_artifact(
            "let a=1;export{a as actual}",
            "let a=1;export{a as actual}",
            &manifest,
            0,
        )
        .unwrap_err();
        assert!(error.to_string().contains("export ABI mismatch"));
    }

    #[test]
    fn observed_javascript_must_retain_lowering_obligations() {
        let manifest = crate::compilation_contract::JavaScriptAbiManifest {
            world: "closed-application",
            exports: Vec::new(),
            export_names_may_mangle: false,
            foreign_imports: Vec::new(),
            public_aggregate_abi: "named",
            stable_aggregate_fields: Vec::new(),
            stable_extern_fields: Vec::new(),
        };

        let error =
            validate_observed_javascript_artifact("let a=1", "let a=1", &manifest, 1).unwrap_err();
        assert!(error.to_string().contains("lowering-obligation mismatch"));
        validate_observed_javascript_artifact("let a=value|0", "let a=value|0", &manifest, 1)
            .unwrap();
    }

    #[test]
    fn final_javascript_cannot_introduce_an_unclassified_static_property() {
        let manifest = crate::compilation_contract::JavaScriptAbiManifest {
            world: "closed-application",
            exports: Vec::new(),
            export_names_may_mangle: false,
            foreign_imports: Vec::new(),
            public_aggregate_abi: "named",
            stable_aggregate_fields: Vec::new(),
            stable_extern_fields: Vec::new(),
        };

        let error = validate_observed_javascript_artifact(
            "let a=o.safe+o.changed",
            "let a=o.safe",
            &manifest,
            0,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unclassified static properties"));
    }

    #[test]
    fn candidate_search_off_skips_terminal_live_letter_work() {
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.optimization_level = 15;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.javascript.terminal_codec_probe_limit = Some(99);
        assert_eq!(config.javascript.effective_terminal_codec_probe_limit(), 0);

        // This deliberately is not valid JavaScript. Entering any remap
        // neighborhood would parse it and fail; the zero-budget guard must
        // retain it without even constructing live-letter swap proposals.
        let selected = ScoredJavaScriptCandidate {
            plan_identity: JavaScriptPlanIdentity {
                context_id: 0,
                ordinal: 0,
            },
            transfer_cost: 19,
            startup_score: 0,
            code: "not valid JavaScript @".to_string(),
            metrics: JavaScriptSyntaxMetrics::default(),
            peephole_rewrites: 0,
            performance: JavaScriptPerformanceMetrics::default(),
            rank: (0, 0),
            has_explicit_lowering_obligations: false,
            admission: test_artifact_admission("not valid JavaScript @"),
        };
        let measurements_before = javascript_codec_measurement_count();
        let mut budget =
            TerminalCodecProbeBudget::new(config.javascript.effective_terminal_codec_probe_limit());
        let retained =
            apply_unused_letter_binding_remaps(selected.clone(), &config, true, &mut budget)
                .expect("zero terminal budget retains the incumbent");
        let retained = apply_terminal_binding_coordinate_descent(retained, &config, &mut budget)
            .expect("zero terminal budget skips coordinate descent");

        assert_eq!(retained.code, selected.code);
        assert_eq!(budget.used, 0);
        assert_eq!(
            javascript_codec_measurement_count() - measurements_before,
            0
        );
    }

    #[test]
    fn selected_canonical_peephole_uses_only_reserved_terminal_work() {
        let code = "function f(a){var x;x=a;return x}console.log(f(1))";
        let optimized = crate::js_peephole::optimize_generated_javascript(code).unwrap();
        assert!(optimized.code.len() < code.len(), "{}", optimized.code);

        let mut config = ProjectConfig::default();
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.candidate_search = CandidateSearch::Always;
        config.mangle.identifiers = Some(false);
        let metrics = analyze_generated_javascript(code).unwrap();
        let selected = SelectedJavaScriptCandidate {
            plan_identity: JavaScriptPlanIdentity {
                context_id: 0,
                ordinal: 0,
            },
            code: code.to_string(),
            transfer_cost: code.len(),
            baseline_transfer: code.len(),
            has_explicit_lowering_obligations: false,
            startup_score: 0,
            metrics,
            baseline_metrics: metrics,
            performance: JavaScriptPerformanceMetrics::default(),
            baseline_performance: JavaScriptPerformanceMetrics::default(),
            candidates_evaluated: 1,
            terminal_codec_probes: 94,
            terminal_work_units: 94,
            terminal_codec_probe_limit: 96,
            terminal_codec_probe_limit_reached: false,
            peephole_rewrites: 0,
            terminal_scope_naming_challengers: 0,
            terminal_scope_naming_selected: false,
            terminal_scope_naming_incumbent_bytes: None,
            terminal_scope_naming_best_bytes: None,
            terminal_string_pooling_challengers: 0,
            terminal_string_pooling_selected: false,
            terminal_string_pooling_incumbent_bytes: None,
            terminal_string_pooling_best_bytes: None,
            admission: test_artifact_admission(code),
        };
        let cleaned = apply_selected_canonical_peephole(selected, &config).unwrap();
        assert_eq!(cleaned.code, optimized.code);
        assert_eq!(cleaned.transfer_cost, optimized.code.len());
        assert!(cleaned.peephole_rewrites > 0);
        assert_eq!(cleaned.terminal_work_units, 96);
        assert_eq!(cleaned.terminal_codec_probes, 95);

        let skipped = SelectedJavaScriptCandidate {
            plan_identity: JavaScriptPlanIdentity {
                context_id: 0,
                ordinal: 0,
            },
            code: code.to_string(),
            transfer_cost: code.len(),
            baseline_transfer: code.len(),
            has_explicit_lowering_obligations: false,
            startup_score: 0,
            metrics,
            baseline_metrics: metrics,
            performance: JavaScriptPerformanceMetrics::default(),
            baseline_performance: JavaScriptPerformanceMetrics::default(),
            candidates_evaluated: 1,
            terminal_codec_probes: 0,
            terminal_work_units: 0,
            terminal_codec_probe_limit: 0,
            terminal_codec_probe_limit_reached: true,
            peephole_rewrites: 0,
            terminal_scope_naming_challengers: 0,
            terminal_scope_naming_selected: false,
            terminal_scope_naming_incumbent_bytes: None,
            terminal_scope_naming_best_bytes: None,
            terminal_string_pooling_challengers: 0,
            terminal_string_pooling_selected: false,
            terminal_string_pooling_incumbent_bytes: None,
            terminal_string_pooling_best_bytes: None,
            admission: test_artifact_admission(code),
        };
        let skipped = apply_selected_canonical_peephole(skipped, &config).unwrap();
        assert_eq!(skipped.code, code);
        assert_eq!(skipped.peephole_rewrites, 0);
    }

    #[test]
    fn search_off_finalization_scores_the_artifact_it_returns() {
        let code = "function f(a){var x;x=a;return x}console.log(f(1))";
        let metrics = analyze_generated_javascript(code).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.mangle.identifiers = Some(false);
        let selected = SelectedJavaScriptCandidate {
            plan_identity: JavaScriptPlanIdentity {
                context_id: 0,
                ordinal: 0,
            },
            code: code.to_string(),
            transfer_cost: code.len(),
            baseline_transfer: code.len(),
            has_explicit_lowering_obligations: false,
            startup_score: 0,
            metrics,
            baseline_metrics: metrics,
            performance: JavaScriptPerformanceMetrics::default(),
            baseline_performance: JavaScriptPerformanceMetrics::default(),
            candidates_evaluated: 1,
            terminal_codec_probes: 0,
            terminal_work_units: 0,
            terminal_codec_probe_limit: 0,
            terminal_codec_probe_limit_reached: false,
            peephole_rewrites: 0,
            terminal_scope_naming_challengers: 0,
            terminal_scope_naming_selected: false,
            terminal_scope_naming_incumbent_bytes: None,
            terminal_scope_naming_best_bytes: None,
            terminal_string_pooling_challengers: 0,
            terminal_string_pooling_selected: false,
            terminal_string_pooling_incumbent_bytes: None,
            terminal_string_pooling_best_bytes: None,
            admission: test_artifact_admission(code),
        };

        let selected = apply_search_off_declaration_peephole(selected, &config).unwrap();
        assert!(selected.code.len() < code.len(), "{}", selected.code);
        assert_eq!(selected.transfer_cost, selected.code.len());
        assert_eq!(selected.terminal_codec_probes, 1);
    }

    #[test]
    fn source_written_i32_normalization_survives_every_javascript_objective() {
        for cost_model in [
            CompressionCostModel::Raw,
            CompressionCostModel::Gzip,
            CompressionCostModel::Brotli,
        ] {
            for candidate_search in [CandidateSearch::Off, CandidateSearch::Always] {
                let arena = Bump::new();
                let program =
                    parse_source(&arena, "export int normalize(int value){return value|0;}")
                        .unwrap();
                let mut config = javascript_oracle_config();
                config.javascript.cost_model = cost_model;
                config.javascript.priority = JavaScriptPriority::SizeFirst;
                config.javascript.candidate_search = candidate_search;
                let output = compile_program_to_js_module_configured(&program, &config).unwrap();
                assert!(
                    output.contains("|0"),
                    "{cost_model:?}/{candidate_search:?} erased source `|0`: {output}"
                );
            }
        }

        let arena = Bump::new();
        let constant = parse_source(&arena, "export int normalize(){return 7|0;}").unwrap();
        let output =
            compile_program_to_js_module_configured(&constant, &javascript_oracle_config())
                .unwrap();
        assert!(
            output.contains("7|0"),
            "constant folding erased source `|0`: {output}"
        );
    }

    #[test]
    fn only_canonical_source_i32_normalization_creates_an_obligation() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int normalize(int value){int a=value|0;int b=0|value;b|=0;return a+b;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let obligations = ir
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                instruction.lowering_obligation
                    == crate::ir::LoweringObligation::PreserveJavaScriptBitOrZero
            })
            .count();
        assert_eq!(obligations, 1);
    }

    #[test]
    fn dead_source_written_i32_normalization_does_not_keep_dead_code_alive() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "int normalize(int value){return value|0;}print(1);").unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.strip_console = false;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("|0"), "{output}");
    }

    #[test]
    fn terminal_cleanup_reopens_canonical_peephole_on_unprepared_finalist() {
        let code = "function f(a){var x;x=a;return x}console.log(f(1))";
        let optimized = crate::js_peephole::optimize_generated_javascript(code).unwrap();
        assert!(optimized.code.len() < code.len(), "{}", optimized.code);

        let mut config = ProjectConfig::default();
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.candidate_search = CandidateSearch::Always;
        config.mangle.identifiers = Some(false);
        assert!(config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole));
        let metrics = analyze_generated_javascript(code).unwrap();
        let selected = ScoredJavaScriptCandidate {
            plan_identity: JavaScriptPlanIdentity {
                context_id: 0,
                ordinal: 0,
            },
            transfer_cost: code.len(),
            startup_score: 0,
            code: code.to_string(),
            metrics,
            peephole_rewrites: 0,
            performance: JavaScriptPerformanceMetrics::default(),
            rank: (0, 0),
            has_explicit_lowering_obligations: false,
            admission: test_artifact_admission(code),
        };
        // One work unit discovers the canonical leaf and one exact score
        // admits it. No later cleanup family has budget to rediscover it.
        let mut codec_budget = TerminalCodecProbeBudget::new(2);
        let cleaned = apply_late_javascript_cleanup(selected, &config, 0, &mut codec_budget)
            .expect("terminal cleanup succeeds");

        assert_eq!(cleaned.code, optimized.code);
        assert_eq!(cleaned.transfer_cost, optimized.code.len());
        assert_eq!(codec_budget.used, 2);
    }

    #[test]
    fn zero_structural_proposal_budget_skips_optional_emission_before_codegen() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int choose(int value){if(value>0){return value+1;}return value-1;}print(choose(3));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.optimization_level = 15;
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.candidate_proposal_limit = Some(0);
        config.javascript.terminal_codec_probe_limit = Some(0);

        let selected = optimize_and_select_javascript(ir, &config, false).unwrap();
        assert!(!selected.javascript.is_empty());
        assert_eq!(selected.selection_metrics.candidate_proposal_limit, 0);
        assert!(selected.selection_metrics.candidate_proposal_limit_reached);
        assert_eq!(selected.selection_metrics.emissions_attempted, 0);
        assert_eq!(selected.selection_metrics.terminal_work_units, 0);
        assert_eq!(selected.selection_metrics.terminal_codec_probes, 0);
        assert!(!selected
            .selection_metrics
            .starved_emission_families
            .is_empty());
    }

    #[test]
    fn parsed_peephole_leaves_share_the_terminal_codec_budget() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1);").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.optimizations = Some(vec![JavaScriptOptimization::ParsedPeephole]);
        let source = "let a=0,b=1;a=a+b;console.log(a)";

        let make_candidate = || {
            JavaScriptEmissionCandidate::new_declaration_plan(
                source.to_string(),
                JavaScriptEmissionPlan {
                    identity: JavaScriptPlanIdentity {
                        context_id: 0,
                        ordinal: 0,
                    },
                    options: config.js_options(),
                },
                CompressionCostModel::Raw,
            )
            .unwrap()
        };

        let zero_candidate = make_candidate();
        let zero_identity = zero_candidate.identity();
        let zero_contexts = test_javascript_contexts(&ir);
        let measurements_before = javascript_codec_measurement_count();
        let mut zero_budget = TerminalCodecProbeBudget::new(0);
        let zero = finalize_javascript_candidates_with_parallelism(
            vec![zero_candidate],
            source,
            zero_identity,
            &config,
            &zero_contexts,
            &OptimizationProfile::default(),
            1,
            false,
            &mut zero_budget,
        )
        .unwrap();
        assert_eq!(zero_budget.used, 0);
        assert_eq!(zero_budget.codec_calls(), 0);
        assert_eq!(zero.peephole_rewrites, 0);
        assert_eq!(
            javascript_codec_measurement_count() - measurements_before,
            0
        );

        let one_candidate = make_candidate();
        let one_identity = one_candidate.identity();
        let one_contexts = test_javascript_contexts(&ir);
        let measurements_before = javascript_codec_measurement_count();
        // One work unit admits parsed preparation and one admits its exact
        // codec score. The work ledger is deliberately stricter than the
        // actual-codec counter.
        let mut one_budget = TerminalCodecProbeBudget::new(2);
        let one = finalize_javascript_candidates_with_parallelism(
            vec![one_candidate],
            source,
            one_identity,
            &config,
            &one_contexts,
            &OptimizationProfile::default(),
            1,
            false,
            &mut one_budget,
        )
        .unwrap();
        assert_eq!(one_budget.used, 2);
        assert_eq!(one_budget.codec_calls(), 1);
        assert!(one.peephole_rewrites > 0, "{}", one.code);
        assert_eq!(
            javascript_codec_measurement_count() - measurements_before,
            1
        );
    }

    #[test]
    fn terminal_challenger_emission_obeys_the_shared_slot_and_byte_tail() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "int add(int value){return value+1;}print(add(2));").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let config = ProjectConfig::default();
        let parent = config.js_options();
        let options = terminal_scope_naming_options(parent, parent);
        assert!(!options.is_empty());
        let contexts = test_javascript_contexts(&ir);

        // Reserve terminal capacity before structural retention. Without this
        // slice, a byte-full structural arena leaves zero room for the naming
        // challenger even after those structural candidates have been ranked.
        let root_plan = contexts
            .register_plan(0, parent)
            .expect("the configured root is registered first");
        let root_code = contexts
            .emit(root_plan.identity.context_id, false, root_plan.options)
            .unwrap();
        let root_size = root_code.len();
        let root = JavaScriptEmissionCandidate::new_declaration_plan(
            root_code,
            root_plan,
            CompressionCostModel::Raw,
        )
        .unwrap();
        let mut reserved_arena = AggregateJavaScriptPlanArena::new_with_terminal_reserve(
            root,
            Vec::new(),
            5,
            root_size * 5,
            CompressionCostModel::Raw,
            2,
        )
        .unwrap();
        let structural_width = reserved_arena.optional_proposal_width();
        assert_eq!(structural_width, 2);
        let structural = (0..structural_width)
            .map(|index| {
                let mut candidate = JavaScriptEmissionCandidate::new(
                    root_size,
                    char::from(b'x' + index as u8).to_string().repeat(root_size),
                    parent,
                    CompressionCostModel::Raw,
                );
                candidate.plan.identity = JavaScriptPlanIdentity {
                    context_id: index + 1,
                    ordinal: 0,
                };
                candidate
            })
            .collect();
        reserved_arena.merge_optional(structural).unwrap();
        let retained = reserved_arena.into_candidates();
        let retained_bytes = retained.iter().map(|candidate| candidate.raw_size).sum();
        let mut reserved_tail = TerminalJavaScriptCandidateBudget::after_retained(
            5,
            root_size * 5,
            retained.len(),
            retained_bytes,
        );
        assert_eq!(reserved_tail.remaining_plans, 2);
        assert_eq!(reserved_tail.remaining_code_bytes, root_size * 2);
        let mut reserved_codec_budget = TerminalCodecProbeBudget::new(16);
        let reserved_challengers = emit_terminal_javascript_challengers(
            options.clone(),
            0,
            false,
            CompressionCostModel::Raw,
            &contexts,
            &mut reserved_tail,
            &mut reserved_codec_budget,
        );
        assert!(!reserved_challengers.is_empty());

        let mut byte_tight = TerminalJavaScriptCandidateBudget {
            remaining_plans: 1,
            remaining_code_bytes: 0,
        };
        let mut byte_codec_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let rejected = emit_terminal_javascript_challengers(
            options.clone(),
            0,
            false,
            CompressionCostModel::Raw,
            &contexts,
            &mut byte_tight,
            &mut byte_codec_budget,
        );
        assert!(rejected.is_empty());
        assert_eq!(byte_tight.remaining_plans, 1);

        let mut slot_tight = TerminalJavaScriptCandidateBudget {
            remaining_plans: 1,
            remaining_code_bytes: usize::MAX,
        };
        let mut slot_codec_budget = TerminalCodecProbeBudget::new(usize::MAX);
        let retained = emit_terminal_javascript_challengers(
            options,
            0,
            false,
            CompressionCostModel::Raw,
            &contexts,
            &mut slot_tight,
            &mut slot_codec_budget,
        );
        assert_eq!(retained.len(), 1);
        assert_eq!(slot_tight.remaining_plans, 0);
        assert!(slot_tight.remaining_code_bytes < usize::MAX);
    }

    #[test]
    fn terminal_string_pooling_challenges_the_actual_winner_with_sparse_thresholds() {
        let configured = ProjectConfig::default().js_options();
        assert!(configured.pool_strings);
        let parent = crate::codegen_ir_js::IrJsOptions {
            pool_strings: true,
            string_pool_minimum_savings: 128,
            ..configured
        };
        let variants = terminal_string_pooling_options(parent, configured);
        assert!(variants.iter().any(|variant| !variant.pool_strings));
        for threshold in [16, 32, 64, 96, 192, 256, 384, 512, 768, 1024] {
            assert!(variants.iter().any(|variant| {
                variant.pool_strings && variant.string_pool_minimum_savings == threshold
            }));
        }
        assert!(variants.iter().all(|variant| *variant != parent));

        let disabled = crate::codegen_ir_js::IrJsOptions {
            pool_strings: false,
            ..configured
        };
        assert!(terminal_string_pooling_options(parent, disabled).is_empty());
    }

    #[test]
    fn parsed_peephole_is_independently_configurable() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int state=0;void increment(){state=state+1;}increment();print(state);",
        )
        .unwrap();
        let mut disabled = javascript_oracle_config();
        disabled.optimization.preset = OptimizationPreset::None;
        disabled.javascript.candidate_search = CandidateSearch::Off;
        disabled.javascript.optimizations = Some(Vec::new());
        disabled.javascript.compression = Some(Vec::new());
        disabled.mangle.identifiers = Some(false);
        let plain = compile_program_to_js_configured(&program, &disabled).unwrap();
        assert!(plain.contains("state=state+1|0"), "{plain}");
        assert!(!plain.contains("state++"), "{plain}");

        let mut enabled = disabled;
        // `candidate_search = "off"` is a hard zero-work policy even when an
        // exact feature allowlist is present. Enable bounded search here so
        // the test isolates the parsed-peephole feature gate itself.
        enabled.javascript.candidate_search = CandidateSearch::Always;
        enabled.javascript.optimizations = Some(vec![JavaScriptOptimization::ParsedPeephole]);
        let optimized = compile_program_to_js_configured(&program, &enabled).unwrap();
        assert!(optimized.contains("state++"), "{optimized}");
        assert!(!optimized.contains("state=state+1|0"), "{optimized}");

        for javascript in [&plain, &optimized] {
            let runtime = std::process::Command::new("node")
                .arg("-e")
                .arg(javascript)
                .output()
                .expect("Node.js is required for JavaScript runtime parity tests");
            assert!(
                runtime.status.success(),
                "node failed with {}:\n{}\n{javascript}",
                runtime.status,
                String::from_utf8_lossy(&runtime.stderr)
            );
            assert_eq!(
                String::from_utf8(runtime.stdout).expect("node stdout is UTF-8"),
                "1\n",
                "{javascript}"
            );
        }
    }

    #[test]
    fn search_off_module_merges_adjacent_generated_declarations() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            concat!(
                "export int[] copyNames(int[] items){",
                "int[] out=[];",
                "int n=items.length;",
                "int i=0;",
                "while(i<n){out.push(items[i]);i=i+1;}",
                "return out;",
                "}",
            ),
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.optimization_level = 12;
        config.mangle.identifiers = Some(false);
        let output = compile_program_to_js_module_configured(&program, &config).unwrap();
        assert!(output.contains("++") || output.contains("+=1"), "{output}");
        assert!(
            !output.contains("=[];var ") && !output.contains("=[];let "),
            "search-off should still merge adjacent declarations:\n{output}"
        );
    }

    #[test]
    fn parsed_peephole_applies_without_candidate_search() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int state=read();void increment(){state=state+1;}increment();print(state);",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.optimization_level = 12;
        config.mangle.identifiers = Some(false);
        assert!(config.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole));
        let optimized = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            !optimized.contains("state=state+1|0"),
            "configured peephole must still rewrite unit updates when search is off:\n{optimized}"
        );
        assert!(
            optimized.contains("read()+1") || optimized.contains("++"),
            "{optimized}"
        );
        let runtime = std::process::Command::new("node")
            .arg("-e")
            .arg(format!("function read(){{return 0}};{optimized}"))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            runtime.status.success(),
            "node failed with {}:\n{}\n{optimized}",
            runtime.status,
            String::from_utf8_lossy(&runtime.stderr)
        );
        assert_eq!(
            String::from_utf8(runtime.stdout).expect("node stdout is UTF-8"),
            "1\n",
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
    fn absolute_javascript_nesting_limit_rejects_every_oversized_candidate() {
        let arena = Bump::new();
        let program = parse_source(&arena, "extern int read();print(read()+read());").unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.optimizations = Some(Vec::new());
        config.javascript.compression = Some(Vec::new());
        config.javascript.startup.max_nesting = Some(1);
        let error = compile_program_to_js_configured(&program, &config).unwrap_err();
        assert!(error.to_string().contains("startup limits"), "{error}");
    }

    #[test]
    fn codec_layout_windows_match_the_exact_encoders() {
        assert_eq!(codec_history_window(CompressionCostModel::Gzip), 32 * 1024);
        assert_eq!(
            codec_history_window(CompressionCostModel::Brotli),
            4 * 1024 * 1024
        );
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
            optimize_and_select_javascript(ir, &javascript_oracle_config(), false).unwrap();

        assert_eq!(
            selected.selection_metrics.syntax.bytes,
            selected.javascript.len()
        );
        assert!(selected.selection_metrics.syntax.tokens > 0);
        assert!(selected.selection_metrics.transfer_bytes > 0);
        assert!(selected.selection_metrics.candidates_evaluated > 0);
        assert!(selected.selection_metrics.performance.score > 0);

        let arena = Bump::new();
        let program =
            parse_source(&arena, "export int square(int value){return value*value;}").unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let selected = optimize_and_select_javascript(ir, &ProjectConfig::default(), true).unwrap();
        assert!(
            selected.javascript.contains("export{"),
            "{}",
            selected.javascript
        );
        assert_eq!(
            selected.selection_metrics.syntax.bytes,
            selected.javascript.len()
        );
        assert!(selected.selection_metrics.transfer_bytes > 1);
        assert_eq!(selected.selection_metrics.codec, "brotli");
        assert!(selected
            .selection_metrics
            .removed_compression_families
            .is_empty());
        assert!(selected.selection_metrics.layout_searched);
        assert!(
            selected
                .selection_metrics
                .scored_emission_families
                .iter()
                .any(|family| family == "named-aggregate-layout"),
            "{:?}",
            selected.selection_metrics.scored_emission_families
        );
        assert!(
            selected
                .selection_metrics
                .cartesian_emission_axes
                .iter()
                .any(|axis| axis == "string-array-packing"),
            "{:?}",
            selected.selection_metrics.cartesian_emission_axes
        );
        assert!(
            selected
                .selection_metrics
                .ir_variants_searched
                .iter()
                .any(|variant| variant == "keep-object"),
            "{:?}",
            selected.selection_metrics.ir_variants_searched
        );
        assert!(selected.selection_metrics.source_operations > 0);
    }

    #[test]
    fn lowered_source_operations_carry_node_ids() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int mask(int value){return value|0;}print(mask(9));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let (source, generated) = ir.operation_provenance_counts();
        assert!(source > 0, "source={source} generated={generated}");
        assert_eq!(generated, 0);
        let mut saw_bit_or_zero = false;
        for instruction in ir
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
        {
            assert_eq!(instruction.origin, crate::ir::OperationOrigin::Source);
            assert!(instruction.node_id.is_some());
            if instruction.lowering_obligation
                == crate::ir::LoweringObligation::PreserveJavaScriptBitOrZero
            {
                saw_bit_or_zero = true;
            }
        }
        assert!(saw_bit_or_zero);
    }

    #[test]
    fn javascript_priorities_rank_transfer_and_runtime_shape_independently() {
        let mut config = ProjectConfig::default();
        config.javascript.priority = crate::config::JavaScriptPriority::SizeFirst;
        assert!(
            javascript_candidate_rank(&config, 90, 100, 120, 100)
                < javascript_candidate_rank(&config, 100, 100, 80, 100)
        );
        assert!(
            javascript_candidate_rank(&config, 30_000, 30_000, 200, 100)
                < javascript_candidate_rank(&config, 30_001, 30_000, 50, 100),
            "size-first must not hide byte differences behind ratio quantization"
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
    fn raw_objective_selects_a_shared_helper_over_duplicated_inlining() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int mix(int value){return value^(value<<value);}int[] values=[1,2,3,4,5,6,7,8];print(mix(values[0]));print(mix(values[1]));print(mix(values[2]));print(mix(values[3]));print(mix(values[4]));print(mix(values[5]));print(mix(values[6]));print(mix(values[7]));",
        )
        .unwrap();
        let sharing_compression = vec![
            CompressionDecision::IdentifierMangling,
            CompressionDecision::EntropyAwareMangling,
            CompressionDecision::QuoteStyleSelection,
            CompressionDecision::StringPooling,
            CompressionDecision::SizeAwareInlining,
            CompressionDecision::SafeIntegerCoercionElision,
            CompressionDecision::CompactBooleanLiterals,
            CompressionDecision::StandardGrammarElision,
            CompressionDecision::StructuredClosureInlining,
            CompressionDecision::StringArrayPacking,
            CompressionDecision::ScalarPhiCopies,
            CompressionDecision::PhiAffinityCoalescing,
            CompressionDecision::IrInliningVariants,
        ];
        let mut selected_config = javascript_oracle_config();
        selected_config.javascript.cost_model = CompressionCostModel::Raw;
        selected_config.javascript.compression = Some(sharing_compression.clone());
        let selected = compile_program_to_js_configured(&program, &selected_config).unwrap();
        let mut inline_only = selected_config.clone();
        inline_only
            .javascript
            .compression
            .as_mut()
            .unwrap()
            .retain(|decision| *decision != CompressionDecision::IrInliningVariants);
        let inlined = compile_program_to_js_configured(&program, &inline_only).unwrap();
        let mut no_inlining = selected_config.clone();
        no_inlining.optimization.inlining = Some(false);
        let outlined = compile_program_to_js_configured(&program, &no_inlining).unwrap();

        assert_eq!(selected.len(), outlined.len(), "{selected}\n{outlined}");
        assert!(selected.len() < inlined.len(), "{selected}\n{inlined}");
        assert_eq!(selected.matches("=>").count(), 1, "{selected}");
    }

    #[test]
    fn compressor_search_selects_eager_pure_helper_substitution_when_smaller() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "pure int increment(int value){return value+1;}pure int summarize(int value){int incremented=increment(value);if(incremented<0){return incremented;}if(incremented==0){return incremented+2;}return incremented+incremented;}int run(int value){return summarize(value);}extern int input();print(run(input()));",
        )
        .unwrap();
        let mut enabled = javascript_oracle_config();
        enabled.optimization.inlining = Some(false);
        enabled.javascript.cost_model = CompressionCostModel::Raw;
        enabled.javascript.candidate_search = CandidateSearch::Always;
        enabled.javascript.candidate_limit = 128;
        enabled.javascript.candidate_beam_width = 8;
        enabled.javascript.compression = Some(vec![
            CompressionDecision::PureHelperInlining,
            CompressionDecision::IdentifierMangling,
            CompressionDecision::StandardGrammarElision,
            CompressionDecision::SafeIntegerCoercionElision,
            CompressionDecision::CompactBooleanLiterals,
        ]);
        let mut disabled = enabled.clone();
        disabled
            .javascript
            .compression
            .as_mut()
            .unwrap()
            .retain(|decision| *decision != CompressionDecision::PureHelperInlining);

        let selected = compile_program_to_js_configured(&program, &enabled).unwrap();
        let retained = compile_program_to_js_configured(&program, &disabled).unwrap();

        assert!(selected.len() < retained.len(), "{selected}\n{retained}");
        assert_eq!(selected.matches("+1").count(), 1, "{selected}");
        assert!(selected.contains(")("), "{selected}");
    }

    #[test]
    fn outlined_ir_probe_carries_its_helper_interaction_into_a_small_finalist_budget() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                extern string algorithmString(int index);
                extern int algorithmInt(int index);
                pure int prefixClass(string label) {
                    if (label.startsWith("cache:")) { return 11; }
                    if (label.startsWith("view:")) { return 17; }
                    return 3;
                }
                pure int suffixClass(string label) {
                    if (label.endsWith(":hot")) { return 13; }
                    if (label.endsWith(":cold")) { return -5; }
                    return 1;
                }
                pure int keywordClass(string label) {
                    if (label == "render") { return 29; }
                    if (label == "hydrate") { return 31; }
                    return 5;
                }
                pure int eventScore(string label, int base) {
                    int token = prefixClass(label) * 3;
                    token += suffixClass(label) * 5;
                    return base + (token + keywordClass(label)) * 13;
                }
                int first = eventScore(algorithmString(0), algorithmInt(0));
                int second = eventScore(algorithmString(1), algorithmInt(1));
                print(first + second);
            "#,
        )
        .unwrap();

        let mut selected_outlined_objectives = 0;
        let mut selected_outlined_helper_interactions = 0;
        for cost_model in [
            CompressionCostModel::Raw,
            CompressionCostModel::Gzip,
            CompressionCostModel::Brotli,
        ] {
            let mut enabled = javascript_oracle_config();
            enabled.javascript.cost_model = cost_model;
            enabled.javascript.candidate_search = CandidateSearch::Always;
            // Fewer than the late helper family's 13 required slots proves
            // that the interaction code retained by the IR probe itself is
            // available to the terminal selector.
            enabled.javascript.candidate_limit = 8;
            enabled.javascript.candidate_beam_width = 2;
            // This fixture measures the outline probe's carried helper
            // interaction, which only appears in the three-address spelling it
            // was written against.
            enabled.javascript.operand_order_fusion = false;
            enabled.javascript.iife_private_callee_clusters = false;
            enabled.javascript.nested_once_run_helpers = false;
            enabled.javascript.compression = Some(vec![
                CompressionDecision::PureHelperInlining,
                CompressionDecision::RegionOutlining,
                CompressionDecision::IdentifierMangling,
                CompressionDecision::StandardGrammarElision,
                CompressionDecision::SafeIntegerCoercionElision,
                CompressionDecision::CompactBooleanLiterals,
            ]);
            let mut without_outline = enabled.clone();
            without_outline.optimization.region_outlining = Some(false);

            let semantics = analyze(&program).unwrap();
            let ir = lower_to_control_flow(&program, &semantics).unwrap();
            let selected = optimize_and_select_javascript(ir, &enabled, false).unwrap();
            let semantics = analyze(&program).unwrap();
            let ir = lower_to_control_flow(&program, &semantics).unwrap();
            let baseline = optimize_and_select_javascript(ir, &without_outline, false).unwrap();

            assert!(
                selected.selection_metrics.transfer_bytes
                    <= baseline.selection_metrics.transfer_bytes,
                "{cost_model:?}: enabled={} baseline={}\n{}\n{}",
                selected.selection_metrics.transfer_bytes,
                baseline.selection_metrics.transfer_bytes,
                selected.javascript,
                baseline.javascript
            );
            let selected_outline = selected
                .optimization_reports
                .iter()
                .any(|report| report.pass_name == "repeated-region-outlining" && report.changed);
            if selected_outline {
                assert_ne!(
                    selected.plan_identity.context_id, 0,
                    "the non-root outline context must survive final identity/report mapping"
                );
            }
            selected_outlined_objectives += usize::from(selected_outline);
            selected_outlined_helper_interactions += usize::from(
                selected_outline
                    && selected.selection_metrics.syntax.functions
                        < baseline.selection_metrics.syntax.functions,
            );
            assert!(selected.selection_metrics.candidates_evaluated <= 8);
        }
        assert!(
            selected_outlined_objectives != 0,
            "at least one exact objective must select the carried outline/helper interaction"
        );
        assert!(
            selected_outlined_helper_interactions != 0,
            "the selected outlined artifact must also remove eligible helper declarations"
        );
    }

    #[test]
    fn deferred_inlining_probe_carries_single_static_use_into_a_two_slot_budget() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                extern string algorithmString(int index);
                extern int algorithmInt(int index);
                pure int prefixClass(string label) {
                    if (label.startsWith("cache:")) { return 11; }
                    if (label.startsWith("view:")) { return 17; }
                    if (label.startsWith("data:")) { return 23; }
                    return 3;
                }
                pure int suffixClass(string label) {
                    if (label.endsWith(":hot")) { return 13; }
                    if (label.endsWith(":cold")) { return -5; }
                    if (label.endsWith(":idle")) { return 7; }
                    return 1;
                }
                pure int tokenClass(string label) {
                    return prefixClass(label) * 3 + suffixClass(label) * 5;
                }
                pure int absoluteValue(int value) {
                    if (value < 0) { return -value; }
                    return value;
                }
                pure int clampMagnitude(int value) {
                    if (value > 500) { return 500; }
                    return value;
                }
                pure int normalizedMagnitude(int value) {
                    return clampMagnitude(absoluteValue(value));
                }
                pure int eventScore(int value, string label) {
                    return normalizedMagnitude(value) + tokenClass(label) * 13;
                }
                print(eventScore(algorithmInt(0), algorithmString(0)));
                print(eventScore(algorithmInt(1), algorithmString(1)));
            "#,
        )
        .unwrap();

        let mut selected_config = javascript_oracle_config();
        selected_config.optimization.inlining = Some(false);
        selected_config.optimization.region_outlining = Some(false);
        selected_config.javascript.cost_model = CompressionCostModel::Brotli;
        selected_config.javascript.candidate_search = CandidateSearch::Always;
        selected_config.javascript.candidate_limit = 2;
        // Preserve a deliberately tiny retained frontier while allowing the
        // late single-use structural coordinate to be proposed.
        selected_config.javascript.candidate_proposal_limit = Some(128);
        selected_config.javascript.candidate_beam_width = 1;
        selected_config.javascript.nested_once_run_helpers = false;
        selected_config.javascript.compression = Some(vec![
            CompressionDecision::PureHelperInlining,
            CompressionDecision::StructuredClosureInlining,
            CompressionDecision::IdentifierMangling,
            CompressionDecision::StandardGrammarElision,
            CompressionDecision::SafeIntegerCoercionElision,
            CompressionDecision::CompactBooleanLiterals,
        ]);
        let mut configured_only = selected_config.clone();
        configured_only.javascript.candidate_limit = 1;

        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let selected = optimize_and_select_javascript(ir, &selected_config, false).unwrap();
        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let baseline = optimize_and_select_javascript(ir, &configured_only, false).unwrap();

        assert!(
            selected.selection_metrics.transfer_bytes < baseline.selection_metrics.transfer_bytes,
            "selected={} baseline={}\n{}\n{}",
            selected.selection_metrics.transfer_bytes,
            baseline.selection_metrics.transfer_bytes,
            selected.javascript,
            baseline.javascript,
        );
        assert!(
            selected.selection_metrics.syntax.functions
                < baseline.selection_metrics.syntax.functions,
            "{}\n{}",
            selected.javascript,
            baseline.javascript,
        );
        assert!(selected.selection_metrics.candidates_evaluated <= 2);
    }

    #[test]
    fn compressor_scores_helper_and_dense_table_choices_as_one_cartesian_family() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "pure string label(int value){int group=value&3;if(group==0){return \"dictionary-stable\";}if(group==1){return \"dictionary-warm\";}if(group==2){return \"dictionary-cold\";}return \"dictionary-retry\";}string run(int value){return label(value);}extern int input();print(run(input()));",
        )
        .unwrap();
        let mut joint = javascript_oracle_config();
        joint.optimization.inlining = Some(false);
        joint.javascript.cost_model = CompressionCostModel::Raw;
        joint.javascript.candidate_search = CandidateSearch::Always;
        joint.javascript.candidate_limit = 128;
        joint.javascript.candidate_proposal_limit = Some(384);
        joint.javascript.candidate_beam_width = 8;
        joint.javascript.compression = Some(vec![
            CompressionDecision::PureHelperInlining,
            CompressionDecision::DenseStringReturnTables,
            CompressionDecision::IdentifierMangling,
            CompressionDecision::StandardGrammarElision,
            CompressionDecision::SafeIntegerCoercionElision,
            CompressionDecision::CompactBooleanLiterals,
        ]);
        let mut helper_only = joint.clone();
        helper_only
            .javascript
            .compression
            .as_mut()
            .unwrap()
            .retain(|decision| *decision != CompressionDecision::DenseStringReturnTables);
        let mut table_only = joint.clone();
        table_only
            .javascript
            .compression
            .as_mut()
            .unwrap()
            .retain(|decision| *decision != CompressionDecision::PureHelperInlining);

        let selected = compile_program_to_js_configured(&program, &joint).unwrap();
        let substituted = compile_program_to_js_configured(&program, &helper_only).unwrap();
        let table = compile_program_to_js_configured(&program, &table_only).unwrap();

        assert!(selected.contains("[\"dictionary-stable\""), "{selected}");
        assert_eq!(selected.matches("=>").count(), 1, "{selected}");
        assert!(
            selected.len() < substituted.len(),
            "{selected}\n{substituted}"
        );
        assert!(selected.len() < table.len(), "{selected}\n{table}");
    }

    #[test]
    fn module_search_keeps_helpers_named_but_still_scores_dense_tables() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "pure string label(int value){int group=value&3;if(group==0){return \"stable\";}if(group==1){return \"warm\";}if(group==2){return \"cold\";}return \"retry\";}export string run(int value){return label(value);}",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.optimization.inlining = Some(false);
        config.javascript.cost_model = CompressionCostModel::Raw;
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.candidate_limit = 128;
        config.javascript.candidate_proposal_limit = Some(384);
        config.javascript.candidate_beam_width = 8;
        config.javascript.compression = Some(vec![
            CompressionDecision::PureHelperInlining,
            CompressionDecision::DenseStringReturnTables,
            CompressionDecision::IdentifierMangling,
            CompressionDecision::StandardGrammarElision,
        ]);

        let selected = compile_program_to_js_module_configured(&program, &config).unwrap();

        assert!(
            selected.contains("[\"stable\",\"warm\",\"cold\",\"retry\"]["),
            "{selected}"
        );
        assert_eq!(selected.matches("=>").count(), 1, "{selected}");
        assert_eq!(selected.matches("function ").count(), 1, "{selected}");
        assert!(selected.contains("export{"), "{selected}");
    }

    #[test]
    fn gzip_search_can_keep_distinct_locals_when_coalescing_adds_syntax() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue input=JS.object();JsValue node=JS.and(input,input[\"nodeName\"]);if(node is string){print(node);}else{print(\"none\");}",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.cost_model = CompressionCostModel::Gzip;
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.candidate_limit = 1536;
        config.javascript.candidate_beam_width = 12;

        let semantics = analyze(&program).unwrap();
        let ir = lower_to_control_flow(&program, &semantics).unwrap();
        let selected = optimize_and_select_javascript(ir, &config, false).unwrap();
        let mut coalesced = config.clone();
        coalesced.javascript.candidate_search = CandidateSearch::Off;
        let coalesced = compile_program_to_js_configured(&program, &coalesced).unwrap();
        let coalesced_gzip =
            compressed_size(coalesced.as_bytes(), CompressionCostModel::Gzip).unwrap();
        let mut forced = config.clone();
        forced.javascript.candidate_search = CandidateSearch::Off;
        forced.javascript.local_name_coalescing = false;
        let forced = compile_program_to_js_configured(&program, &forced).unwrap();
        let forced_gzip = compressed_size(forced.as_bytes(), CompressionCostModel::Gzip).unwrap();

        assert!(
            !selected.selection_metrics.decisions.local_name_coalescing,
            "selected:\n{}\nforced ({forced_gzip} bytes):\n{forced}",
            selected.javascript,
        );
        assert!(
            selected.selection_metrics.transfer_bytes < coalesced_gzip,
            "selected gzip={}\n{}\ncoalesced gzip={coalesced_gzip}\n{coalesced}",
            selected.selection_metrics.transfer_bytes,
            selected.javascript
        );
        assert!(
            selected.selection_metrics.transfer_bytes <= forced_gzip,
            "selected gzip={}\n{}\nforced gzip={forced_gzip}\n{forced}",
            selected.selection_metrics.transfer_bytes,
            selected.javascript,
        );
        let runtime = std::process::Command::new("node")
            .arg("-e")
            .arg(&selected.javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            runtime.status.success(),
            "node failed with {}:\n{}\n{}",
            runtime.status,
            String::from_utf8_lossy(&runtime.stderr),
            selected.javascript,
        );
        assert_eq!(
            String::from_utf8(runtime.stdout).expect("node stdout is UTF-8"),
            "none\n",
            "{}",
            selected.javascript,
        );
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
        enabled.javascript.candidate_proposal_limit = Some(128);
        enabled.mangle.identifiers = Some(false);
        enabled.mangle.properties = Some(false);
        enabled.mangle.exports = Some(false);
        enabled.mangle.pool_strings = Some(false);
        let mut source_order = enabled.clone();
        source_order.javascript.optimizations = Some(Vec::new());
        source_order.javascript.candidate_search = CandidateSearch::Off;

        let selected = compile_program_to_js_configured(&program, &enabled).unwrap();
        let baseline = compile_program_to_js_configured(&program, &source_order).unwrap();
        let mut gzip_enabled = enabled.clone();
        gzip_enabled.javascript.cost_model = CompressionCostModel::Gzip;
        let gzip_selected = compile_program_to_js_configured(&program, &gzip_enabled).unwrap();

        assert_eq!(selected.len(), baseline.len());
        assert!(
            compressed_size(selected.as_bytes(), CompressionCostModel::Brotli).unwrap()
                < compressed_size(baseline.as_bytes(), CompressionCostModel::Brotli).unwrap(),
            "selected:\n{selected}\nsource order:\n{baseline}"
        );
        assert_eq!(gzip_selected.len(), baseline.len());
        assert!(
            compressed_size(gzip_selected.as_bytes(), CompressionCostModel::Gzip).unwrap()
                < compressed_size(baseline.as_bytes(), CompressionCostModel::Gzip).unwrap(),
            "selected:\n{gzip_selected}\nsource order:\n{baseline}"
        );
    }

    #[test]
    fn codec_scoring_selects_proven_private_function_subsumption() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            include_str!("../benchmarks/function-subsumption/fixture.lil"),
        )
        .unwrap();
        let mut enabled = ProjectConfig::default();
        enabled.optimization.inlining = Some(false);
        enabled.optimization.call_site_specialization = Some(false);
        enabled.optimization.capture_signature_cloning = Some(false);
        enabled.optimization.identical_function_folding = Some(false);
        enabled.optimization.parameterized_function_merging = Some(false);
        enabled.optimization.pipeline_fusion = Some(false);
        enabled.optimization.partial_escape_sinking = Some(false);
        enabled.optimization.region_outlining = Some(false);
        enabled.optimization.expression_superopt = Some(false);
        enabled.optimization.path_sensitive_propagation = Some(false);
        enabled.javascript.optimizations =
            Some(vec![JavaScriptOptimization::IrFunctionSubsumptionVariants]);
        enabled.javascript.candidate_search = CandidateSearch::Always;
        enabled.javascript.candidate_limit = 8;
        let mut disabled = enabled.clone();
        disabled.javascript.optimizations = Some(Vec::new());
        disabled.optimization.function_subsumption = Some(false);

        let selected = compile_program_to_js_configured(&program, &enabled).unwrap();
        let baseline = compile_program_to_js_configured(&program, &disabled).unwrap();

        assert!(selected.len() < baseline.len(), "{selected}\n{baseline}");
        assert!(
            compressed_size(selected.as_bytes(), CompressionCostModel::Brotli).unwrap()
                < compressed_size(baseline.as_bytes(), CompressionCostModel::Brotli).unwrap()
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
        let mut performance = javascript_oracle_config();
        performance.javascript.priority = crate::config::JavaScriptPriority::PerformanceFirst;
        let mut realistic = javascript_oracle_config();
        realistic.javascript.priority =
            crate::config::JavaScriptPriority::RealisticPerformanceFirst;
        let mut balanced = javascript_oracle_config();
        balanced.javascript.priority = crate::config::JavaScriptPriority::Balanced;
        let mut size = javascript_oracle_config();
        size.javascript.priority = crate::config::JavaScriptPriority::SizeFirst;

        let performance = compile_program_all_configured(&program, &performance).unwrap();
        let realistic = compile_program_all_configured(&program, &realistic).unwrap();
        let balanced = compile_program_all_configured(&program, &balanced).unwrap();
        let size = compile_program_all_configured(&program, &size).unwrap();

        assert_ne!(performance.javascript, size.javascript);
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
    fn nested_arrows_emit_captured_parameter_defaults() {
        let output = compile_source(
            "func(int)->int factory(int defaultSize){return (int size=defaultSize)=>size;}print(factory(21)(7));",
        )
        .unwrap();
        assert!(output.contains("console.log"), "{output}");
        assert!(
            output.contains("7"),
            "explicit calls must keep the argument: {output}"
        );
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
        assert!(
            output.contains(".push(1)") || output.contains("console.log(1)"),
            "the mutation may remain explicit or fold with its fresh array: {output}"
        );
        assert!(!output.contains("console.log(0)"));
    }

    #[test]
    fn keeps_array_length_stable_across_fill() {
        let output = compile_source(
            "int[] values=[1,2,3];values.fill(0);print(values[0]);print(values.length);",
        )
        .unwrap();
        assert!(output.contains(".fill(0)"), "{output}");
        assert!(output.contains("console.log(3)"), "{output}");
    }

    #[test]
    fn inlines_disjoint_top_level_control_flow_regions() {
        let output = compile_source(
            "int gcd(int a,int b){while(b!=0){int next=a%b;a=b;b=next;}return a;}int fib(int count){int a=0;int b=1;for(int i=0;i<count;i++){int next=a+b;a=b;b=next;}return a;}print(gcd(21,14));print(fib(8));",
        )
        .unwrap();
        assert!(!output.contains("function"));
        assert!(!output.contains("switch("));
        assert_eq!(
            output.matches("while(").count() + output.matches("for(").count(),
            2
        );
    }

    #[test]
    fn compiles_source_to_native_c() {
        let output = compile_source_to_c("int value=40+2;print(value);").unwrap();
        assert!(output.contains("int main(void)"));
        assert!(output.contains("printf(\"%d\\n\""));
    }

    #[test]
    fn compiles_mutable_capture_cells_to_native_c() {
        let output = compile_source_to_c(
            "int run(int seed){auto next=()=>{seed+=1;return seed;};next();return next();}print(run(40));",
        )
        .unwrap();

        assert!(output.contains("malloc(sizeof*l"), "{output}");
        assert!(output.contains("->c0=l"), "{output}");
        assert!(output.contains("*l"), "{output}");
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
    fn explicit_constructor_export_preserves_named_class_identity() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export constructor Scale as PublicScale;class Scale{int factor;init(int factor){this.factor=factor;}int apply(int value){return value*this.factor;}}",
        )
        .unwrap();
        for cost_model in [
            CompressionCostModel::Raw,
            CompressionCostModel::Gzip,
            CompressionCostModel::Brotli,
        ] {
            for candidate_search in [CandidateSearch::Off, CandidateSearch::Always] {
                let mut config = javascript_oracle_config();
                config.javascript.cost_model = cost_model;
                config.javascript.candidate_search = candidate_search;
                config.mangle.exports = Some(false);
                let javascript = compile_program_to_js_module_configured(&program, &config)
                    .unwrap_or_else(|error| panic!("{cost_model:?}/{candidate_search:?}: {error}"));

                assert!(javascript.contains("class Scale"), "{javascript}");
                assert!(
                    javascript.contains("export{Scale as PublicScale}"),
                    "{javascript}"
                );
                assert!(!javascript.contains("$init"), "{javascript}");
                assert!(!javascript.contains("var factor"), "{javascript}");
                let output = std::process::Command::new("node")
                    .arg("--input-type=module")
                    .arg("-e")
                    .arg(format!(
                        "{javascript};let value=new Scale(3);process.stdout.write([Scale.name,Scale.length,value.factor,value.apply(4)].join(':'))"
                    ))
                    .output()
                    .expect("Node.js is required for JavaScript runtime parity tests");
                assert!(
                    output.status.success(),
                    "{cost_model:?}/{candidate_search:?}: node failed with {}:\n{}\n{javascript}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
                    "Scale:1:3:12",
                    "{cost_model:?}/{candidate_search:?}: {javascript}"
                );
            }
        }
    }

    #[test]
    fn constructor_export_preserves_explicit_inheritance() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export constructor Child;class Base{int value;init(int value){this.value=value;}int read(){return this.value;}}class Child extends Base{int extra;init(int value,int extra){super(value);this.extra=extra;}int total(){return this.value+this.extra;}}",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.mangle.exports = Some(false);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();

        assert!(javascript.contains("class Base"), "{javascript}");
        assert!(
            javascript.contains("class Child extends Base"),
            "{javascript}"
        );
        assert!(javascript.contains("super("), "{javascript}");
        let output = std::process::Command::new("node")
            .arg("--input-type=module")
            .arg("-e")
            .arg(format!(
                "{javascript};let value=new Child(3,4);process.stdout.write([Child.name,Child.length,value instanceof Base,value instanceof Child,value.read(),value.total()].join(':'))"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "Child:2:true:true:3:7",
            "{javascript}"
        );
    }

    #[test]
    fn constructor_export_synthesizes_only_the_published_default_constructor() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export constructor Empty;class Empty{int value;bool ready;string label;int[] items;int ping(){return 1;}}class Dissolved{int value;}",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.mangle.exports = Some(false);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();

        assert!(javascript.contains("class Empty"), "{javascript}");
        assert!(!javascript.contains("class Dissolved"), "{javascript}");
        let output = std::process::Command::new("node")
            .arg("--input-type=module")
            .arg("-e")
            .arg(format!(
                "{javascript};let value=new Empty;process.stdout.write([Empty.name,Empty.length,value.value,value.ready,value.label,value.items.length,value.ping()].join(':'))"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "Empty:0:0:false::0:1",
            "{javascript}"
        );
    }

    #[test]
    fn constructor_export_with_fields_respects_the_javascript_syntax_floor() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export constructor Box;class Box{int value;init(int value){this.value=value;}}",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.ecmascript = crate::js_syntax_target::EcmaScriptEdition::Es2015;
        let error = compile_program_to_js_module_configured(&program, &config).unwrap_err();
        assert!(
            error.to_string().contains("public class fields requires"),
            "{error}"
        );
    }

    #[test]
    fn ordinary_object_literal_preserves_javascript_prototype_semantics() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void inspect(JsValue value);JsValue value=object{alpha:1,\"__proto__\":2};value[\"beta\"]=3;inspect(value);",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.assume_pristine_builtins = false;
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!javascript.contains("JS.object"), "{javascript}");
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "let alphaSets=0,betaSets=0;Object.defineProperty(Object.prototype,'alpha',{{configurable:true,set(){{alphaSets++}}}});Object.defineProperty(Object.prototype,'beta',{{configurable:true,set(){{betaSets++}}}});function inspect(value){{process.stdout.write([Object.getPrototypeOf(value)===Object.prototype,Object.hasOwn(value,'alpha'),Object.hasOwn(value,'__proto__'),Object.hasOwn(value,'beta'),value.alpha,value.__proto__,alphaSets,betaSets].join(':'))}}try{{{javascript}}}finally{{delete Object.prototype.alpha;delete Object.prototype.beta}}"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "true:true:true:false:1:2:0:1",
            "{javascript}"
        );
    }

    #[test]
    fn owned_plain_object_proof_forwards_only_proven_own_reads() {
        let arena = Bump::new();
        let owned = parse_source(
            &arena,
            "extern int read();JsValue value=object{alpha:read()};print(value[\"alpha\"]);",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.assume_pure_property_reads = false;
        let javascript = compile_program_to_js_configured(&owned, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "let calls=0;function read(){{calls++;return 7}};{javascript};process.stdout.write(String(calls))"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(output.status.success(), "{javascript}");
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "7\n1",
            "{javascript}"
        );

        let arena = Bump::new();
        let missing = parse_source(
            &arena,
            "JsValue value=object{alpha:1};print(value[\"missing\"]);",
        )
        .unwrap();
        let javascript = compile_program_to_js_configured(&missing, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "Object.defineProperty(Object.prototype,'missing',{{configurable:true,get(){{return 9}}}});try{{{javascript}}}finally{{delete Object.prototype.missing}}"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(output.status.success(), "{javascript}");
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "9\n",
            "{javascript}"
        );

        let arena = Bump::new();
        let escaped = parse_source(
            &arena,
            "extern void escape(JsValue value);JsValue value=object{alpha:1};escape(value);print(value[\"alpha\"]);",
        )
        .unwrap();
        let javascript = compile_program_to_js_configured(&escaped, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "function escape(value){{Object.defineProperty(value,'alpha',{{get(){{return 11}}}})}};{javascript}"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(output.status.success(), "{javascript}");
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "11\n",
            "{javascript}"
        );
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
    fn drops_globals_only_read_by_dead_functions() {
        let output = compile_source("int secret=7;void unused(){print(secret);}print(1);").unwrap();
        assert_eq!(output, "console.log(1)");
    }

    #[test]
    fn drops_unread_scheduler_store_and_host_value() {
        let output = compile_source(
            "func(func()->void)->void scheduler=(func()->void callback)=>{};\
             void enable(func(func()->void)->void next){scheduler=next;}\
             extern void host(func()->void callback);\
             enable(host);\
             print(1);",
        )
        .unwrap();
        assert_eq!(output, "console.log(1)", "{output}");
        assert!(!output.contains("host"), "{output}");
    }

    #[test]
    fn keeps_effectful_initializer_when_global_is_unread() {
        let output = compile_source("extern int bump();int unused=bump();print(1);").unwrap();
        assert!(output.contains("bump()"), "{output}");
        assert!(output.contains("console.log(1)"), "{output}");
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
    fn extern_class_members_stay_exact_when_owned_fields_reuse_the_name() {
        let source = r#"
            class Lexer {
                bool gfm;
                bool breaks;
                init(bool gfm = true, bool breaks = false) {
                    this.gfm = gfm;
                    this.breaks = breaks;
                }
            }
            extern class Options {
                JsValue gfm;
                JsValue breaks;
            }
            export JsValue getDefaults() {
                Options opt = JS.assume(JS.object());
                opt.gfm = true;
                opt.breaks = false;
                return opt;
            }
            export bool readGfm(JsValue opt) {
                Options incoming = JS.assume(opt);
                if (JS.isUndefined(incoming.gfm)) {
                    return true;
                }
                return incoming.gfm.truthy();
            }
            export bool lexGfm(Lexer lexer) {
                return lexer.gfm;
            }
        "#;
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = crate::config::CandidateSearch::Off;
        config.mangle.identifiers = Some(true);
        config.mangle.properties = Some(true);
        config.mangle.extern_fields = Some(true);
        let output = compile_program_to_js_module_configured(&program, &config).unwrap();
        let contains_exact_property = |property: &str| {
            output.contains(&format!(".{property}"))
                || output.contains(&format!("{{{property}:"))
                || output.contains(&format!(",{property}:"))
        };
        assert!(
            contains_exact_property("gfm") && contains_exact_property("breaks"),
            "extern class option keys must stay exact under property mangling:\n{output}"
        );
        assert!(
            !output.contains("{n:!0") && !output.contains("{n:true"),
            "owned Lexer field names must not steal the public option keys:\n{output}"
        );
    }

    #[test]
    fn closed_world_can_release_extern_class_fields_without_renaming_host_length() {
        let source = r#"
            extern class Options {
                JsValue gfm;
                JsValue breaks;
            }
            export JsValue getDefaults() {
                Options opt = JS.assume(JS.object());
                opt.gfm = true;
                opt.breaks = false;
                return opt;
            }
            export int hostLength(string src) {
                return src.length;
            }
        "#;
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = crate::config::CandidateSearch::Off;
        config.mangle.identifiers = Some(true);
        config.mangle.properties = Some(true);
        config.mangle.extern_fields = Some(false);
        let output = compile_program_to_js_module_configured(&program, &config).unwrap();
        assert!(
            !output.contains(".gfm") && !output.contains(".breaks"),
            "closed-world extern fields must enter property mangling:\n{output}"
        );
        assert!(
            output.contains(".length"),
            "host members such as string.length must stay exact:\n{output}"
        );
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
    fn strip_console_removes_print_and_keeps_effectful_arguments() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1+2);").unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = true;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("console.log"), "{output}");

        let arena = Bump::new();
        let program = parse_source(&arena, "extern int read();print(read());").unwrap();
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("console.log"), "{output}");
        assert!(output.contains("read("), "{output}");

        let arena = Bump::new();
        let program = parse_source(&arena, "print(40+2);").unwrap();
        config.javascript.strip_console = false;
        let kept = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(kept.contains("console.log"), "{kept}");
    }

    #[test]
    fn known_host_externs_lower_to_javascript_builtins() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern JsValue createEmptyObject();\
             extern string typeOf(JsValue value);\
             extern float mathRound(float value);\
             extern float read();\
             print(typeOf(createEmptyObject()));\
             print(mathRound(read()));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("createEmptyObject"), "{output}");
        assert!(
            output.contains("\"object\"")
                || output.contains("'object'")
                || (output.contains("typeof") && output.contains("{}")),
            "{output}"
        );
        assert!(output.contains("Math.round"), "{output}");
        assert!(!output.contains("mathRound"), "{output}");
    }

    #[test]
    fn frequent_host_math_uses_a_mangled_builtin_alias() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern float mathRound(float value);\
             extern float a();\
             extern float b();\
             extern float c();\
             extern float d();\
             extern float e();\
             print(mathRound(a())+mathRound(b())+mathRound(c())+mathRound(d())+mathRound(e()));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(output.contains("Math.round"), "{output}");
        assert!(!output.contains("mathRound"), "{output}");
        assert!(
            output.contains("=Math.round") || output.matches("Math.round").count() >= 5,
            "{output}"
        );
    }

    #[test]
    fn frequent_typeof_uses_a_shared_helper() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern string typeOf(JsValue value);\
             extern JsValue a();\
             extern JsValue b();\
             extern JsValue c();\
             extern JsValue d();\
             extern JsValue e();\
             print(typeOf(a()));\
             print(typeOf(b()));\
             print(typeOf(c()));\
             print(typeOf(d()));\
             print(typeOf(e()));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("typeOf("), "{output}");
        assert_eq!(output.matches("typeof ").count(), 1, "{output}");
    }

    #[test]
    fn default_config_strips_console() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1+2);").unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("console.log"), "{output}");
        assert!(config.javascript.strip_console);
    }

    #[test]
    fn host_throw_emits_javascript_throw() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void throwError(string msg);throwError(\"bad\");",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(output.contains("throw new Error("), "{output}");
        assert!(!output.contains("throwError("), "{output}");
    }

    #[test]
    fn host_array_push_and_has_own_drop_typescript_wrappers() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern float arrayPush(JsValue arr, JsValue item);\
             extern bool objectHasOwn(JsValue obj, string key);\
             extern JsValue getProp(JsValue obj, string key);\
             extern JsValue arr();\
             extern JsValue obj();\
             extern JsValue item();\
             arrayPush(arr(), item());\
             print(objectHasOwn(obj(), \"k\"));\
             print(getProp(obj(), \"foo\"));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("arrayPush"), "{output}");
        assert!(!output.contains("objectHasOwn"), "{output}");
        assert!(!output.contains("getProp"), "{output}");
        assert!(
            output.contains("push") && output.contains(".call("),
            "{output}"
        );
        assert!(output.contains("Object.hasOwn("), "{output}");
        assert!(!output.contains("hasOwnProperty"), "{output}");
        assert!(!output.contains(".call.bind("), "{output}");
        assert!(
            output.contains(".foo") || output.contains("[\"foo\"]") || output.contains("['foo']"),
            "{output}"
        );
    }

    #[test]
    fn known_host_window_and_timeout_drop_typescript_wrappers() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern JsValue windowSelf();\
             extern JsValue windowDocument();\
             extern void scheduleTimeout(JsValue fn);\
             extern JsValue scheduleTimeoutMs(JsValue fn, float ms);\
             extern void clearTimeoutId(JsValue id);\
             extern JsValue newDOMParser();\
             extern JsValue newXMLHttpRequest();\
             extern JsValue arrayFlat(JsValue array);\
             extern JsValue fn();\
             extern JsValue arr();\
             extern JsValue id();\
             print(windowSelf());\
             print(windowDocument());\
             scheduleTimeout(fn());\
             print(scheduleTimeoutMs(fn(), 16.0));\
             clearTimeoutId(id());\
             print(newDOMParser());\
             print(newXMLHttpRequest());\
             print(arrayFlat(arr()));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("windowSelf"), "{output}");
        assert!(!output.contains("windowDocument"), "{output}");
        assert!(!output.contains("scheduleTimeout"), "{output}");
        assert!(!output.contains("clearTimeoutId"), "{output}");
        assert!(!output.contains("newDOMParser"), "{output}");
        assert!(!output.contains("newXMLHttpRequest"), "{output}");
        assert!(!output.contains("arrayFlat"), "{output}");
        assert!(output.contains("typeof window"), "{output}");
        assert!(output.contains("globalThis"), "{output}");
        assert!(output.contains("document"), "{output}");
        assert!(!output.contains(".document"), "{output}");
        assert!(output.contains("setTimeout("), "{output}");
        assert!(output.contains("clearTimeout("), "{output}");
        assert!(output.contains("new DOMParser"), "{output}");
        assert!(output.contains("new XMLHttpRequest"), "{output}");
        assert!(
            output.contains(".flat.call(") || output.contains("prototype.flat"),
            "{output}"
        );
    }

    #[test]
    fn known_host_function_and_window_predicates_share_compact_aliases() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool isFunctionValue(JsValue obj);\
             extern bool isWindowValue(JsValue obj);\
             extern JsValue value();\
             print(isFunctionValue(value()));\
             print(isWindowValue(value()));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("isFunctionValue"), "{output}");
        assert!(!output.contains("isWindowValue"), "{output}");
        assert!(output.contains("typeof"), "{output}");
        assert!(output.contains("nodeType"), "{output}");
        assert!(output.contains(".window"), "{output}");
    }

    #[test]
    fn known_host_predicates_fold_on_fresh_objects() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool isFunctionValue(JsValue obj);\
             extern bool isWindowValue(JsValue obj);\
             if (isFunctionValue(JS.object())) { print(1); } else { print(0); }\
             if (isWindowValue(JS.undefined())) { print(1); } else { print(0); }\
             print(JS.typeOf(JS.object()));\
             print(JS.typeOf(JS.undefined()));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("typeof"), "{output}");
        assert!(!output.contains("nodeType"), "{output}");
        assert!(output.contains("console.log(0)"), "{output}");
        assert!(
            output.contains("\"object\"") || output.contains("'object'"),
            "{output}"
        );
        assert!(
            output.contains("\"undefined\"") || output.contains("'undefined'"),
            "{output}"
        );
    }

    #[test]
    fn known_host_iterator_and_console_drop_typescript_wrappers() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void defineIterator(JsValue obj, JsValue iterator);\
             extern JsValue getArrayIterator();\
             extern void consoleWarn3(JsValue a, JsValue b, JsValue c);\
             extern JsValue requestAnimationFrameOrNull(JsValue fn);\
             extern void defineConfigurable(JsValue obj, string key, JsValue value);\
             extern JsValue fn();\
             JsValue object = JS.object();\
             defineIterator(object, getArrayIterator());\
             defineConfigurable(object, \"keep\", JS.object());\
             consoleWarn3(fn(), fn(), fn());\
             print(requestAnimationFrameOrNull(fn()));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("defineIterator"), "{output}");
        assert!(!output.contains("getArrayIterator"), "{output}");
        assert!(!output.contains("consoleWarn3"), "{output}");
        assert!(!output.contains("requestAnimationFrameOrNull"), "{output}");
        assert!(!output.contains("defineConfigurable"), "{output}");
        assert!(output.contains("Symbol.iterator"), "{output}");
        assert!(
            output.contains("({}")
                || output.contains("[{")
                || output.contains("var ")
                || output.contains("let "),
            "object-literal iterator assignment must not parse as a block:\n{output}"
        );
        assert!(output.contains("console.warn("), "{output}");
        assert!(output.contains("requestAnimationFrame"), "{output}");
        assert!(output.contains("Object.defineProperty("), "{output}");
    }

    #[test]
    fn object_literal_symbol_assign_is_not_a_block() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void defineIterator(JsValue obj, JsValue iterator);\
             extern JsValue getArrayIterator();\
             defineIterator(JS.object(), getArrayIterator());",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(output.contains("Symbol.iterator"), "{output}");
        assert!(
            !output.contains(";{}[") && !output.starts_with("{}["),
            "{output}"
        );
    }

    #[test]
    fn js_construct_emits_new_without_a_host_wrapper() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "print(JS.typeOf(JS.construct(JS.method0((JsValue self) => self))));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            output.contains("new ") || output.contains("new("),
            "{output}"
        );
        assert!(!output.contains("JS.construct"), "{output}");
        assert!(!output.contains("createJQuery"), "{output}");
        let runtime = std::process::Command::new("node")
            .arg("-e")
            .arg(&output)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            runtime.status.success(),
            "node failed with {}:\n{}\n{output}",
            runtime.status,
            String::from_utf8_lossy(&runtime.stderr)
        );
        assert_eq!(
            String::from_utf8(runtime.stdout).expect("node stdout is UTF-8"),
            "object\n",
            "{output}"
        );
    }

    #[test]
    fn nested_js_closure_keeps_copied_value_after_source_write() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue snapshot(){\
               JsValue value=1;\
               JsValue oldValue=value;\
               JsValue nested=JS.method0((JsValue _n)=>{value=2;return JS.undefined();});\
               JS.call(nested,JS.undefined());\
               print(oldValue);\
               print(value);\
               return JS.undefined();\
             }\
             snapshot();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\n2\n",
            "{javascript}"
        );
    }

    #[test]
    fn keeps_copied_property_load_across_a_mutating_method_call() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue probe(){\
               JsValue self=JS.object();\
               self[\"x\"]=1;\
               self[\"mutate\"]=JS.method0((JsValue s)=>{s[\"x\"]=2;return JS.undefined();});\
               bool was=JS.strictEqual(self[\"x\"],1);\
               JS.call(self[\"mutate\"],self);\
               print(was);\
               print(self[\"x\"]);\
               return JS.undefined();\
             }\
             probe();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "true\n2\n",
            "{javascript}"
        );
    }

    #[test]
    fn keeps_copied_property_compare_across_a_mutating_call_before_a_branch() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue probe(){\
               JsValue self=JS.object();\
               self[\"x\"]=1;\
               self[\"compute\"]=JS.method0((JsValue s)=>{s[\"x\"]=2;return 3;});\
               bool was=JS.strictEqual(self[\"x\"],1);\
               JsValue newValue=JS.call(self[\"compute\"],self);\
               if (was) { print(1); } else { print(0); }\
               print(newValue);\
               print(self[\"x\"]);\
               return JS.undefined();\
             }\
             probe();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\n3\n2\n",
            "{javascript}"
        );
    }

    #[test]
    fn keeps_toint_property_compare_across_a_js_call() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue probe(){\
               JsValue self=JS.object();\
               self[\"x\"]=1;\
               self[\"compute\"]=JS.method0((JsValue s)=>{s[\"x\"]=2;return 3;});\
               bool was=JS.number(self[\"x\"]).toInt()==1;\
               JsValue newValue=JS.call(self[\"compute\"],self);\
               if (was) { print(1); } else { print(0); }\
               print(newValue);\
               print(self[\"x\"]);\
               return JS.undefined();\
             }\
             probe();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\n3\n2\n",
            "{javascript}"
        );
    }

    #[test]
    fn keeps_toint_property_compare_across_a_js_call_when_used_in_a_later_or() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int NOT_TRACKING=-1;\
             int toInt(JsValue value){return JS.number(value).toInt();}\
             JsValue call1(JsValue fn,JsValue self,JsValue a0){return JS.call(fn,self,a0);}\
             JsValue Computed=JS.method0((JsValue self)=>self);\
             JsValue probe(){\
               Computed[\"prototype\"][\"trackAndCompute\"]=JS.method0((JsValue self)=>{\
                 JsValue oldValue=self[\"value_\"];\
                 bool wasSuspended=toInt(self[\"dependenciesState_\"])==NOT_TRACKING;\
                 JsValue newValue=call1(self[\"computeValue_\"],self,true);\
                 bool changed=wasSuspended||JS.strictEqual(oldValue,newValue);\
                 if(changed){print(1);}else{print(0);}\
                 print(newValue);\
                 return changed;\
               });\
               JsValue self=JS.construct(Computed);\
               self[\"dependenciesState_\"]=NOT_TRACKING;\
               self[\"value_\"]=\"old\";\
               self[\"computeValue_\"]=JS.method1((JsValue s,JsValue keep)=>{\
                 s[\"dependenciesState_\"]=0;\
                 return \"df\";\
               });\
               JS.call(self[\"trackAndCompute\"],self);\
               return JS.undefined();\
             }\
             probe();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.optimization_level = 8;
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\ndf\n",
            "{javascript}"
        );
    }

    #[test]
    fn keeps_a_captured_changed_flag_across_a_js_track_call() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue call1(JsValue fn,JsValue self,JsValue a0){return JS.call(fn,self,a0);}\
             JsValue Reaction=JS.method0((JsValue self)=>self);\
             JsValue reaction(JsValue expression,JsValue effect){\
               JsValue currentValue=JS.undefined();\
               JsValue firstTimeFlag=true;\
               JsValue changedFlag=false;\
               JsValue r=JS.undefined();\
               JsValue exprTrack=()=>{\
                 JsValue nextValue=JS.call(expression,JS.undefined(),r);\
                 if(firstTimeFlag.truthy()){\
                   changedFlag=true;\
                 }else{\
                   changedFlag=!JS.strictEqual(currentValue,nextValue);\
                 }\
                 currentValue=nextValue;\
                 return JS.undefined();\
               };\
               JsValue reactionRunner=()=>{\
                 changedFlag=false;\
                 call1(r[\"track\"],r,exprTrack);\
                 if(firstTimeFlag.truthy()){\
                   print(\"first\");\
                 }else if(changedFlag.truthy()){\
                   JS.call(effect,JS.undefined(),currentValue);\
                 }else{\
                   print(\"skip\");\
                 }\
                 firstTimeFlag=false;\
                 return JS.undefined();\
               };\
               r=JS.construct(Reaction);\
               r[\"track\"]=JS.method1((JsValue self,JsValue fn)=>{\
                 JS.call(fn,self);\
                 return JS.undefined();\
               });\
               return reactionRunner;\
             }\
             JsValue box=JS.object();\
             box[\"n\"]=1;\
             JsValue run=reaction(()=>box[\"n\"],(JsValue value)=>{\
               print(value);\
               return JS.undefined();\
             });\
             JS.call(run,JS.undefined());\
             box[\"n\"]=2;\
             JS.call(run,JS.undefined());\
             JS.call(run,JS.undefined());",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.optimization_level = 15;
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "first\n2\nskip\n",
            "{javascript}"
        );
    }

    #[test]
    fn keeps_a_captured_changed_flag_across_a_known_track_method_in_production() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue trackDerived(JsValue _self, JsValue fn){\
               return JS.call(fn, JS.undefined());\
             }\
             JsValue Reaction=JS.method0((JsValue self)=>self);\
             Reaction[\"prototype\"][\"track\"]=JS.method1((JsValue self, JsValue fn)=>{\
               trackDerived(self, fn);\
               return JS.undefined();\
             });\
             JsValue reaction(JsValue expression, JsValue effect){\
               JsValue currentValue=JS.undefined();\
               JsValue firstTimeFlag=true;\
               JsValue changedFlag=false;\
               JsValue r=JS.undefined();\
               JsValue exprTrack=()=>{\
                 JsValue nextValue=JS.call(expression,JS.undefined(),r);\
                 if(firstTimeFlag.truthy()){\
                   changedFlag=true;\
                 }else{\
                   changedFlag=!JS.strictEqual(currentValue,nextValue);\
                 }\
                 currentValue=nextValue;\
                 return JS.undefined();\
               };\
               JsValue reactionRunner=()=>{\
                 changedFlag=false;\
                 JS.call(r[\"track\"],r,exprTrack);\
                 if(firstTimeFlag.truthy()){\
                   print(\"first\");\
                 }else if(changedFlag.truthy()){\
                   JS.call(effect,JS.undefined(),currentValue);\
                 }else{\
                   print(\"skip\");\
                 }\
                 firstTimeFlag=false;\
                 return JS.undefined();\
               };\
               r=JS.construct(Reaction);\
               return reactionRunner;\
             }\
             JsValue box=JS.object();\
             box[\"n\"]=1;\
             JsValue run=reaction(()=>box[\"n\"],(JsValue value)=>{\
               print(value);\
               return JS.undefined();\
             });\
             JS.call(run,JS.undefined());\
             box[\"n\"]=2;\
             JS.call(run,JS.undefined());\
             JS.call(run,JS.undefined());",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Production;
        config.javascript.optimization_level = 15;
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.strip_console = false;
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "first\n2\nskip\n",
            "{javascript}"
        );
    }

    #[test]
    fn declares_a_loop_carried_binding_before_a_prototype_walk() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue walk(JsValue adm, JsValue key, JsValue annotation){\
               if(JS.strictEqual(annotation,true)){annotation=adm[\"defaultAnnotation_\"];}\
               if(JS.strictEqual(annotation,false)){return JS.undefined();}\
               JsValue source=adm[\"target_\"];\
               while(source.truthy()&&!JS.strictEqual(source,JS.undefined())){\
                 if(source[key].truthy()){break;}\
                 source=source[\"proto\"];\
               }\
               return source;\
             }\
             JsValue proto=JS.object();\
             proto[\"key\"]=JS.undefined();\
             proto[\"proto\"]=JS.undefined();\
             JsValue target=JS.object();\
             target[\"key\"]=1;\
             target[\"proto\"]=proto;\
             JsValue adm=JS.object();\
             adm[\"target_\"]=target;\
             adm[\"defaultAnnotation_\"]=JS.object();\
             if(JS.strictEqual(walk(adm,\"key\",true),target)){print(1);}else{print(0);}",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.javascript.optimization_level = 15;
        config.javascript.strip_console = false;
        config.mangle.identifiers = Some(true);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let script = format!("\"use strict\";{javascript}");
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&script)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\n",
            "{javascript}"
        );
    }

    #[test]
    fn module_level_assignment_is_not_shadowed_inside_setter() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue pred = JS.undefined();\
             void setPred(string name, JsValue value) {\
               if (name == \"x\") {\
                 pred = value;\
                 return;\
               }\
               pred = JS.undefined();\
             }\
             bool isMatch(JsValue thing) {\
               return JS.call(pred, JS.undefined(), thing).truthy();\
             }\
             setPred(\"x\", JS.method1((JsValue _this, JsValue x) => {\
               return JS.strictEqual(x, 1);\
             }));\
             if (isMatch(1)) { print(1); } else { print(0); }",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Production;
        config.javascript.optimization_level = 15;
        config.javascript.strip_console = false;
        config.mangle.identifiers = Some(true);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\n",
            "{javascript}"
        );
    }

    #[test]
    fn method_has_temp_does_not_clobber_a_live_value_argument() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue call1(JsValue fn, JsValue self, JsValue a) {\
               return JS.call(fn, self, a);\
             }\
             JsValue Ctor = JS.method0((JsValue self) => self);\
             Ctor[\"prototype\"][\"has\"] = JS.method1((JsValue self, JsValue value) => {\
               return false;\
             });\
             Ctor[\"prototype\"][\"add\"] = JS.method1((JsValue self, JsValue value) => {\
               if (self[\"flag\"].truthy()) {\
                 JsValue change = JS.object();\
                 change[\"newValue\"] = value;\
                 if (!change.truthy()) {\
                   return self;\
                 }\
                 value = change[\"newValue\"];\
               }\
               JsValue hasFn = self[\"has\"];\
               self[\"touched\"] = self;\
               if (!call1(hasFn, self, value).truthy()) {\
                 JS.call(self[\"run\"], JS.undefined(), JS.method0((JsValue _s) => {\
                   self[\"added\"] = value;\
                   value = self[\"added\"];\
                   return JS.undefined();\
                 }));\
               }\
               return self;\
             });\
             JsValue obj = JS.construct(Ctor);\
             obj[\"run\"] = JS.method1((JsValue _s, JsValue fn) => {\
               return JS.call(fn, JS.undefined());\
             });\
             JS.call(obj[\"add\"], obj, 7);\
             print(obj[\"added\"]);",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Production;
        config.javascript.optimization_level = 15;
        config.javascript.strip_console = false;
        config.mangle.identifiers = Some(true);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "7\n",
            "{javascript}"
        );
    }

    #[test]
    fn inner_temps_do_not_clobber_module_bindings_used_as_exports() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue when = JS.methodRest((JsValue _s, JsValue args) => {\
               return args[0];\
             });\
             JsValue shallow = \"observable.shallow\";\
             JsValue parseFlag(JsValue target, JsValue args) {\
               bool flag = false;\
               if (JS.number(args[\"length\"]).toInt() > 2) {\
                 flag = args[2].truthy();\
               }\
               JsValue run = JS.method0((JsValue _s) => {\
                 if (flag) { return target; }\
                 return args[0];\
               });\
               JS.call(run, JS.undefined());\
               return flag;\
             }\
             JsValue args = JS.array();\
             JS.push(args, 1);\
             JS.push(args, 2);\
             JS.push(args, true);\
             parseFlag(JS.object(), args);\
             if (JS.typeOf(when) == \"function\") { print(1); } else { print(0); }\
             print(shallow);",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Production;
        config.javascript.optimization_level = 15;
        config.javascript.strip_console = false;
        config.javascript.pool_numeric_literals = true;
        config.javascript.local_name_reserve = 48;
        config.javascript.stable_local_names = true;
        config.mangle.identifiers = Some(true);
        config.mangle.pool_strings = Some(true);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\nobservable.shallow\n",
            "{javascript}"
        );
    }

    #[test]
    fn snapshot_of_a_mutable_capture_survives_production_indirect_store() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void install(){\
               JsValue current=1;\
               JsValue bump=JS.method0((JsValue _s)=>{\
                 current=2;\
                 return JS.undefined();\
               });\
               JsValue track=JS.method1((JsValue _s,JsValue fn)=>{\
                 JS.call(fn,JS.undefined());\
                 return JS.undefined();\
               });\
               JsValue run=JS.method0((JsValue _s)=>{\
                 JsValue oldValue=current;\
                 JS.call(track,JS.undefined(),bump);\
                 print(oldValue);\
                 print(current);\
                 return JS.undefined();\
               });\
               JS.call(run,JS.undefined());\
             }\
             install();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Production;
        config.javascript.optimization_level = 15;
        config.javascript.strip_console = false;
        config.javascript.pool_numeric_literals = true;
        config.javascript.local_name_reserve = 48;
        config.javascript.stable_local_names = true;
        config.javascript.function_spelling =
            Some(crate::codegen_ir_js::FunctionSpelling::Function);
        config.mangle.identifiers = Some(true);
        config.mangle.pool_strings = Some(true);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "1\n2\n",
            "{javascript}"
        );
    }

    #[test]
    fn production_nested_call_temp_does_not_reuse_a_module_callee_register() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue action = JS.undefined();\
             JsValue call1(JsValue fn, JsValue self, JsValue a) {\
               return JS.call(fn, self, a);\
             }\
             JsValue undef() {\
               return JS.undefined();\
             }\
             void run() {\
               JsValue gen = JS.object();\
               gen[\"next\"] = JS.method1((JsValue g, JsValue x) => {\
                 JsValue result = JS.object();\
                 result[\"value\"] = x;\
                 result[\"done\"] = true;\
                 result[\"then\"] = undef();\
                 return result;\
               });\
               JsValue nextStep = undef();\
               JsValue onFulfilled = JS.method1((JsValue _f, JsValue v) => {\
                 try {\
                   string stepName = \"s\";\
                   JsValue ret = call1(JS.call(action, undef(), stepName, gen[\"next\"]), gen, v);\
                   call1(nextStep, undef(), ret);\
                 } catch (JsValue e) {\
                   print(\"err\");\
                 }\
                 return undef();\
               });\
               nextStep = JS.method1((JsValue _n, JsValue ret) => {\
                 print(ret[\"value\"]);\
                 return undef();\
               });\
               call1(onFulfilled, undef(), 7);\
             }\
             action = JS.method2((JsValue _s, JsValue name, JsValue fn) => {\
               return fn;\
             });\
             run();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Production;
        config.javascript.optimization_level = 15;
        config.javascript.strip_console = false;
        config.javascript.pool_numeric_literals = true;
        config.javascript.local_name_reserve = 48;
        config.javascript.stable_local_names = true;
        config.javascript.function_spelling =
            Some(crate::codegen_ir_js::FunctionSpelling::Function);
        config.mangle.identifiers = Some(true);
        config.mangle.pool_strings = Some(true);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "7\n",
            "{javascript}"
        );
    }

    #[test]
    fn nested_js_closure_stores_captured_outer_binding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "JsValue captured(){\
               JsValue rejector=JS.undefined();\
               JsValue inner=JS.method1((JsValue _s,JsValue reject)=>{rejector=reject;return JS.undefined();});\
               JS.call(inner,JS.undefined(),7);\
               print(rejector);\
               return JS.undefined();\
             }\
             captured();",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "7\n",
            "{javascript}"
        );
    }

    #[test]
    fn repeated_window_roots_share_one_binding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern JsValue windowSelf();\
             print(windowSelf());\
             print(windowSelf());\
             print(windowSelf());\
             print(windowSelf());\
             print(windowSelf());",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert_eq!(output.matches("typeof window").count(), 1, "{output}");
        assert!(output.contains("globalThis"), "{output}");
        assert!(!output.contains("windowSelf"), "{output}");
    }

    #[test]
    fn static_host_function_taken_as_value_needs_no_bind() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool objectHasOwn(JsValue obj, string key);\
             extern JsValue obj();\
             func(JsValue, string)->bool has = objectHasOwn;\
             print(has(obj(), \"k\"));\
             print(objectHasOwn(obj(), \"z\"));",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(!output.contains("objectHasOwn"), "{output}");
        assert!(output.contains("=Object.hasOwn"), "{output}");
        assert!(!output.contains(".bind("), "{output}");
        assert!(!output.contains("hasOwnProperty"), "{output}");
    }

    #[test]
    fn user_defined_host_alias_name_is_not_rewritten() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int objectHasOwn(int value, int offset){return value+offset;}\
             print(objectHasOwn(3, 4));",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        config.mangle.identifiers = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();

        assert!(!javascript.contains("Object.hasOwn"), "{javascript}");
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "7\n",
            "{javascript}"
        );
    }

    #[test]
    fn object_has_own_direct_and_detached_calls_preserve_runtime_order() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool objectHasOwn(JsValue obj, string key);\
             extern JsValue nextObject();\
             extern string nextKey();\
             func(JsValue, string)->bool detached = objectHasOwn;\
             print(objectHasOwn(nextObject(), nextKey()));\
             print(detached(nextObject(), nextKey()));",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.javascript.candidate_search = CandidateSearch::Off;
        config.optimization.inlining = Some(false);
        config.mangle.identifiers = Some(false);
        let javascript = compile_program_to_js_configured(&program, &config).unwrap();

        assert!(javascript.contains("Object.hasOwn"), "{javascript}");
        assert!(!javascript.contains(".bind("), "{javascript}");
        assert!(!javascript.contains("hasOwnProperty"), "{javascript}");
        let harness = format!(
            "let trace=[];\
             function nextObject(){{trace.push('object');return {{owned:1}}}}\
             function nextKey(){{trace.push('key');return 'owned'}}\
             {javascript};\
             process.stdout.write('trace='+trace.join(','));"
        );
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&harness)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{harness}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "true\ntrue\ntrace=object,key,object,key",
            "{harness}"
        );
    }

    #[test]
    fn brotli_selects_direct_object_has_own_but_retains_a_detached_alias() {
        let mut calls = String::new();
        for index in 0..24 {
            calls.push_str(&format!(
                "print(objectHasOwn(object(),\"stable-property-{index:02}\"));"
            ));
        }
        let source = format!(
            "extern bool objectHasOwn(JsValue object,string key);\
             extern JsValue object();{calls}"
        );
        let arena = Bump::new();
        let program = parse_source(&arena, &source).unwrap();
        let mut baseline_config = javascript_oracle_config();
        baseline_config.optimization.inlining = Some(false);
        baseline_config.javascript.priority = JavaScriptPriority::SizeFirst;
        baseline_config.javascript.cost_model = CompressionCostModel::Brotli;
        baseline_config.javascript.candidate_search = CandidateSearch::Off;
        baseline_config.javascript.optimizations = Some(Vec::new());
        baseline_config.javascript.function_spelling =
            Some(crate::codegen_ir_js::FunctionSpelling::Arrow);
        baseline_config.javascript.compression = Some(vec![
            CompressionDecision::HostAliasSpelling,
            CompressionDecision::IdentifierMangling,
            CompressionDecision::StandardGrammarElision,
        ]);
        let baseline = compile_program_to_js_configured(&program, &baseline_config).unwrap();
        assert!(baseline.contains("=Object.hasOwn"), "{baseline}");

        let mut search_config = baseline_config.clone();
        search_config.javascript.candidate_search = CandidateSearch::Always;
        search_config.javascript.candidate_limit = 64;
        search_config.javascript.candidate_beam_width = 8;
        let selected = compile_program_to_js_configured(&program, &search_config).unwrap();
        assert!(!selected.contains("=Object.hasOwn"), "{selected}");
        assert_eq!(selected.matches("Object.hasOwn(").count(), 24, "{selected}");
        assert!(
            compressed_size(selected.as_bytes(), CompressionCostModel::Brotli).unwrap()
                < compressed_size(baseline.as_bytes(), CompressionCostModel::Brotli).unwrap(),
            "selected={selected}\nbaseline={baseline}"
        );

        let detached_source = format!(
            "extern bool objectHasOwn(JsValue object,string key);\
             extern JsValue object();\
             extern void accept(func(JsValue,string)->bool callback);\
             accept(objectHasOwn);{calls}"
        );
        let arena = Bump::new();
        let detached_program = parse_source(&arena, &detached_source).unwrap();
        let retained = compile_program_to_js_configured(&detached_program, &search_config).unwrap();
        assert!(retained.contains("=Object.hasOwn"), "{retained}");
        assert!(!retained.contains("Object.hasOwn("), "{retained}");
    }

    #[test]
    fn host_undefined_global_name_is_not_value_proof() {
        let arena = Bump::new();
        let program = parse_source(&arena, "extern JsValue UNDEFINED;print(UNDEFINED);").unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.javascript.candidate_search = CandidateSearch::Off;
        let output = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(output.contains("UNDEFINED"), "{output}");
        assert!(!output.contains("void 0"), "{output}");
    }

    #[test]
    fn applies_fine_grained_optimizer_and_mangling_config() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1+2*3);").unwrap();
        let mut config = ProjectConfig::default();
        config.javascript.strip_console = false;
        config.optimization.preset = crate::config::OptimizationPreset::None;
        config.optimization.constant_folding = Some(false);
        // Keep this assertion scoped to the IR optimizer option. The terminal
        // JavaScript optimizer is an independent layer and may legally fold
        // the same expression after emission when it is enabled.
        config.javascript.optimizations = Some(Vec::new());
        config.mangle.identifiers = Some(false);
        let unoptimized = compile_program_to_js_configured(&program, &config).unwrap();
        assert_ne!(unoptimized, "console.log(7)");
        assert!(unoptimized.contains("2*3"));
        config.javascript.optimizations = None;

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int horizontal;int vertical;}extern void send(Point point);Point point=Point{1,2};send(point);",
        )
        .unwrap();
        config.optimization.preset = crate::config::OptimizationPreset::Maximum;
        config.mangle.properties = Some(false);
        let preserved = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            preserved.contains("horizontal:") && preserved.contains("vertical:"),
            "{preserved}"
        );
        config.mangle.properties = Some(true);
        config.mangle.exports = Some(false);
        let escape_owned = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            escape_owned.contains("horizontal:") && escape_owned.contains("vertical:"),
            "dynamic-boundary aggregate fields remain open-world stable: {escape_owned}"
        );
        let arena = Bump::new();
        let exported = parse_source(
            &arena,
            "struct Point{int horizontal;int vertical;}export int sum(Point point){return point.horizontal+point.vertical;}",
        )
        .unwrap();
        let stable_export = compile_program_to_js_module_configured(&exported, &config).unwrap();
        assert!(
            stable_export.contains("horizontal") && stable_export.contains("vertical"),
            "ESM-exported aggregate field names stay stable: {stable_export}"
        );
        config.mangle.exports = Some(true);
        let mangled = compile_program_to_js_configured(&program, &config).unwrap();
        assert!(
            mangled.contains("horizontal:") && mangled.contains("vertical:"),
            "{mangled}"
        );
    }

    #[test]
    fn configured_bundle_uses_requested_local_pool_without_changing_output() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-bundle-resource-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("library.lil"),
            "export int read(){return 41;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(&main, "import {read} from \"./library\";print(read()+1);").unwrap();
        let mut serial_config = javascript_oracle_config();
        serial_config.bundle.mode = BundleMode::PreserveModules;
        serial_config.optimization.inlining = Some(false);
        serial_config.mangle.identifiers = Some(false);
        serial_config.compiler.resources.threads = std::num::NonZeroUsize::new(1);
        let mut parallel_config = serial_config.clone();
        parallel_config.compiler.resources.threads = std::num::NonZeroUsize::new(3);

        let (serial, serial_threads) =
            compile_path_to_js_bundle_configured_observing_pool(&main, &serial_config, "entry.js")
                .unwrap();
        let (parallel, parallel_threads) = compile_path_to_js_bundle_configured_observing_pool(
            &main,
            &parallel_config,
            "entry.js",
        )
        .unwrap();

        assert_eq!(serial_threads, 1);
        assert_eq!(parallel_threads, 3);
        assert_eq!(parallel, serial);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn configured_all_bundle_lowers_once_and_matches_separate_outputs() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-all-bundle-single-lowering-test-{}",
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
        let mut config = javascript_oracle_config();
        config.bundle.mode = BundleMode::PreserveModules;
        config.optimization.inlining = Some(false);
        config.mangle.identifiers = Some(false);

        let expected_javascript =
            compile_path_to_js_bundle_configured(&main, &config, "main.js").unwrap();
        let expected_c = compile_path_to_c_configured(&main, &config).unwrap();
        reset_configured_module_lowering_count();

        let artifacts =
            compile_path_all_to_js_bundle_configured(&main, &config, "main.js").unwrap();

        assert_eq!(configured_module_lowering_count(), 1);
        assert_eq!(artifacts.javascript, expected_javascript);
        assert_eq!(artifacts.c, expected_c);
        assert_eq!(artifacts.javascript.manifest.entry, "main.js");
        std::fs::remove_dir_all(directory).unwrap();
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
        let mut config = javascript_oracle_config();
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
        assert_eq!(bundle.manifest.objective.javascript_codec, "brotli");
        assert_eq!(bundle.manifest.objective_fingerprint.len(), 64);
        assert!(bundle.manifest.selected_transfer_bytes > 0);
        assert_eq!(
            bundle.manifest.chunks[0].selected_transfer_bytes,
            bundle.manifest.chunks[0].brotli_bytes
        );
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
    fn preserve_modules_disables_ownerless_region_outlining() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-preserve-outlining-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("library.lil"),
            "pure int privateScore(int first,int second){int a=first+1;int b=a*3;int c=b-2;int d=c^7;int e=second+1;int f=e*3;int g=f-2;int h=g^7;return d+h;}export int read(){return privateScore(1,2);}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(&main, "import {read} from \"./library\";print(read());").unwrap();
        let mut config = javascript_oracle_config();
        config.bundle.mode = BundleMode::PreserveModules;
        config.optimization.inlining = Some(false);
        config.optimization.region_outlining = Some(true);
        config.mangle.identifiers = Some(false);

        let bundle = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap();

        assert_eq!(bundle.files.len(), 2);
        let entry = bundle
            .files
            .iter()
            .find(|file| file.file_name == "entry.js")
            .expect("entry chunk");
        let dependency = bundle
            .files
            .iter()
            .find(|file| file.file_name != "entry.js")
            .expect("dependency chunk");
        assert!(entry.code.contains("from\"./"), "{}", entry.code);
        assert!(
            !dependency.code.contains("from\"./entry.js\""),
            "{}",
            dependency.code
        );
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
            "export int shared(int value){if(value==0){return 101;}if(value==1){return 103;}if(value==2){return 107;}if(value==3){return 109;}if(value==4){return 113;}if(value==5){return 127;}if(value==6){return 131;}if(value==7){return 137;}if(value==8){return 139;}if(value==9){return 149;}return value*151+157;}",
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
        let mut config = javascript_oracle_config();
        config.bundle.mode = BundleMode::Split;
        config.bundle.min_chunk_bytes = 1;
        config.bundle.max_chunks = 1;
        config.bundle.shared_min_imports = 2;
        config.bundle.cost.raw_weight = 1;
        config.bundle.cost.gzip_weight = 0;
        config.bundle.cost.brotli_weight = 0;
        config.bundle.cost.request_overhead_bytes = 0;
        config.bundle.cost.dependency_depth_penalty_bytes = 0;
        config.bundle.cost.cache_reuse_discount_percent = 100;
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
    fn split_does_not_force_a_costlier_first_shared_chunk() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-costly-first-shared-chunk-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("shared.lil"),
            "export int shared(int value){return value+1;}",
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
        let mut config = javascript_oracle_config();
        config.bundle.mode = BundleMode::Split;
        config.bundle.min_chunk_bytes = 1;
        config.bundle.max_chunks = 1;
        config.bundle.shared_min_imports = 2;
        config.bundle.cost.raw_weight = 1;
        config.bundle.cost.gzip_weight = 0;
        config.bundle.cost.brotli_weight = 0;
        config.bundle.cost.request_overhead_bytes = 1_000_000;
        config.bundle.cost.dependency_depth_penalty_bytes = 0;
        config.bundle.cost.cache_reuse_discount_percent = 0;
        config.optimization.inlining = Some(false);

        let bundle = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap();
        assert_eq!(bundle.files.len(), 1);
        assert!(bundle.manifest.chunks.is_empty());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn split_rejects_more_mandatory_lazy_chunks_than_max_chunks() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-mandatory-lazy-chunk-limit-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("first.lil"),
            "export int answer(){return 1;}",
        )
        .unwrap();
        std::fs::write(
            directory.join("second.lil"),
            "export int answer(){return 2;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import(\"./first\").then((auto first)=>print(first.answer()));import(\"./second\").then((auto second)=>print(second.answer()));",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.bundle.mode = BundleMode::Split;
        config.bundle.min_chunk_bytes = 1;
        config.bundle.max_chunks = 1;

        let error = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap_err();
        std::fs::remove_dir_all(directory).unwrap();

        assert!(error.message.contains("`bundle.max_chunks` is 1"));
        assert!(error.message.contains("requires 2 mandatory lazy chunks"));
        assert!(error.message.contains("increase `bundle.max_chunks`"));
    }

    #[test]
    fn split_emits_with_the_winning_joint_chunk_symbol_options() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-joint-chunk-symbol-output-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("feature.lil"),
            "export int transform(int value){int doubled=value*2;int adjusted=doubled+3;return adjusted*5;}export int fallback(int value){return value-7;}",
        )
        .unwrap();
        let main = directory.join("main.lil");
        std::fs::write(
            &main,
            "import(\"./feature\").then((auto feature)=>print(feature.transform(5)));",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.bundle.mode = BundleMode::Split;
        config.bundle.min_chunk_bytes = 1;
        config.bundle.max_chunks = 1;
        config.bundle.cost.raw_weight = 1;
        config.bundle.cost.gzip_weight = 0;
        config.bundle.cost.brotli_weight = 0;
        config.bundle.cost.request_overhead_bytes = 0;
        config.bundle.cost.dependency_depth_penalty_bytes = 0;
        config.bundle.cost.cache_reuse_discount_percent = 0;
        config.javascript.local_name_reserve = 100;
        config.optimization.inlining = Some(false);

        let modules = discover_modules_configured(&main, &config).unwrap();
        let arena = Bump::new();
        let programs = parse_modules(&arena, &modules).unwrap();
        let linked = link_modules(&arena, &modules, &programs).unwrap();
        let semantics = analyze(&linked).unwrap();
        let mut ir = lower_to_control_flow(&linked, &semantics).unwrap();
        let guidance = load_optimization_guidance(
            &config,
            config.javascript_optimization_configured(
                JavaScriptOptimization::ProfileGuidedOptimization,
            ),
        )
        .unwrap();
        prepare_javascript_ir(&mut ir, &config);
        optimize_control_flow_with_guidance(
            &mut ir,
            &config.js_optimizer_options(),
            true,
            &guidance,
        )
        .unwrap();
        let selected = plan_javascript_chunks(&ir, &modules, &config, "entry.js").unwrap();
        assert_eq!(selected.options.local_name_reserve, 0);
        assert_ne!(selected.options, config.js_options());
        let plan = IrJsChunkPlan {
            entry_file: "entry.js".to_string(),
            chunks: selected
                .chunks
                .iter()
                .map(|chunk| IrJsChunkSpec {
                    file_name: chunk.file_name.clone(),
                    functions: chunk.functions.clone(),
                    lazy_module: chunk.lazy_module,
                })
                .collect(),
        };
        let expected =
            emit_optimized_ir_js_chunks_with_options(&ir, &selected.options, &plan).unwrap();
        let expected_files = expected
            .into_iter()
            .map(|chunk| JavaScriptBundleFile {
                file_name: chunk.file_name,
                code: chunk.code,
            })
            .collect::<Vec<_>>();

        let bundle = compile_path_to_js_bundle_configured(&main, &config, "entry.js").unwrap();
        std::fs::remove_dir_all(directory).unwrap();

        assert_eq!(bundle.files, expected_files);
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
        let mut config = javascript_oracle_config();
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
        let mut config = javascript_oracle_config();
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
        let comparison = ["state==1", "1==state"]
            .into_iter()
            .find_map(|comparison| output.find(comparison))
            .unwrap_or_else(|| panic!("comparison must remain: {output}"));
        let store = output
            .find("state=2")
            .unwrap_or_else(|| panic!("write must remain: {output}"));
        assert!(comparison < store, "{output}");
        let runtime = std::process::Command::new("node")
            .arg("-e")
            .arg(&output)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            runtime.status.success(),
            "node failed with {}:\n{}\n{output}",
            runtime.status,
            String::from_utf8_lossy(&runtime.stderr)
        );
        assert_eq!(
            String::from_utf8(runtime.stdout).expect("node stdout is UTF-8"),
            "true\n",
            "{output}"
        );
    }

    #[test]
    fn private_global_store_survives_cross_function_conditional_selection() {
        let source = "int state=0;extern bool flag();int choose(){state=1;int value=2;int result=0;if(flag()){result=value;}return result;}print(choose());print(state);";
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let state = ir
            .globals
            .iter()
            .find(|global| global.name == "state")
            .expect("private state global is lowered")
            .symbol;

        let mut config = javascript_oracle_config();
        // This regression is independent of global propagation/internalization:
        // the cross-function write must remain observable even when that family
        // is disabled.
        config.optimization.global_optimization = Some(false);
        crate::optimizer::optimize_control_flow_with_options(
            &mut ir,
            &config.js_optimizer_options(),
            false,
        )
        .unwrap();

        let retained_update = ir.functions.iter().any(|function| {
            let ones = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match (instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Const(crate::ir::ConstValue::Int(1))) => Some(out),
                    _ => None,
                })
                .collect::<Vec<_>>();
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    matches!(
                        instruction.op,
                        ControlFlowOp::StoreGlobal { global, value }
                            if global == state && ones.contains(&value)
                    )
                })
        });
        assert!(retained_update, "optimized IR dropped `state = 1`: {ir:#?}");

        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        for (flag, expected) in [(false, "0\n1\n"), (true, "2\n1\n")] {
            let output = std::process::Command::new("node")
                .arg("-e")
                .arg(format!("function flag(){{return {flag}}};{javascript}"))
                .output()
                .expect("Node.js is required for JavaScript runtime parity tests");
            assert!(
                output.status.success(),
                "flag={flag}: node failed with {}:\n{}\n{javascript}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
                expected,
                "flag={flag}: {javascript}"
            );
        }
    }

    #[test]
    fn collapses_unobserved_byte_array_buffer_construction() {
        let output = compile_source(
            "ArrayBuffer buffer=new ArrayBuffer(16);Uint8Array bytes=new Uint8Array(buffer);bytes[0]=7;print(bytes[0]);",
        )
        .unwrap();
        assert!(output.contains("new Uint8Array(16)"), "{output}");
        assert!(!output.contains("new ArrayBuffer"), "{output}");
    }

    #[test]
    fn folds_fixed_typed_array_and_subarray_lengths() {
        let output = compile_source(
            "ArrayBuffer buffer=new ArrayBuffer(16);Int32Array words=new Int32Array(buffer);Uint8Array bytes=new Uint8Array(12);Uint8Array slice=bytes.subarray(3,9);print(words.length+slice.length);",
        )
        .unwrap();
        assert!(output.contains("console.log(10)"), "{output}");
        assert!(!output.contains(".length"), "{output}");
    }

    #[test]
    fn compiles_nested_capturing_closures_after_inlining() {
        let source = "class Box{int value;init(int value){this.value=value;}void increment(){this.value+=1;}}extern void accept(func()->void callback);void run(){Box box=new Box(0);accept(()=>{accept(()=>box.increment());});}run();";
        let output = compile_source(source).unwrap();

        assert!(output.contains("accept("), "{output}");
    }

    #[test]
    fn compiles_mutable_captures_as_shared_lexical_bindings() {
        let output = compile_source(
            "int run(int seed){auto next=()=>{seed+=1;return seed;};next();return next();}print(run(40));",
        )
        .unwrap();

        assert!(output.contains("+1"), "{output}");
        assert!(!output.contains("seed:"), "{output}");
    }

    #[test]
    fn every_production_objective_invokes_empty_parameter_sibling_closures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void run(){int value=1;func()->int increment=()=>{value++;return value;};func()->int read=()=>value;print(increment());print(read());}run();",
        )
        .unwrap();

        for cost_model in [
            CompressionCostModel::Raw,
            CompressionCostModel::Gzip,
            CompressionCostModel::Brotli,
        ] {
            let mut config = javascript_oracle_config();
            config.javascript.candidate_search = CandidateSearch::Always;
            config.javascript.optimization_level = 15;
            config.javascript.cost_model = cost_model;
            config.mangle.identifiers = Some(true);
            let javascript = compile_program_to_js_configured(&program, &config).unwrap();
            let output = std::process::Command::new("node")
                .arg("-e")
                .arg(&javascript)
                .output()
                .expect("Node.js is required for JavaScript runtime parity tests");
            assert!(
                output.status.success(),
                "{cost_model:?}: node failed with {}:\n{}\n{javascript}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
                "2\n2\n",
                "{cost_model:?}: {javascript}"
            );
        }
    }

    #[test]
    fn every_production_objective_preserves_async_function_boundaries() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "async int immediate(){return 1;}async int delayed(){return await Task.resolve(2);}immediate().then((int value)=>print(value));delayed().then((int value)=>print(value));",
        )
        .unwrap();

        for cost_model in [
            CompressionCostModel::Raw,
            CompressionCostModel::Gzip,
            CompressionCostModel::Brotli,
        ] {
            let mut config = javascript_oracle_config();
            config.optimization.preset = OptimizationPreset::Maximum;
            config.javascript.candidate_search = CandidateSearch::Always;
            config.javascript.optimization_level = 15;
            config.javascript.cost_model = cost_model;
            config.mangle.identifiers = Some(true);
            let javascript = compile_program_to_js_configured(&program, &config).unwrap();
            assert!(
                javascript.matches("async").count() >= 2,
                "{cost_model:?}: {javascript}"
            );
            let output = std::process::Command::new("node")
                .arg("-e")
                .arg(&javascript)
                .output()
                .expect("Node.js is required for JavaScript runtime parity tests");
            assert!(
                output.status.success(),
                "{cost_model:?}: node failed with {}:\n{}\n{javascript}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
                "1\n2\n",
                "{cost_model:?}: {javascript}"
            );
        }
    }

    #[test]
    fn brotli_objective_carries_async_literal_movement_into_name_search() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "async int resolveValue(int value){int resolved=await Task.resolve(value+5);return resolved*2;}resolveValue(3).then((int value)=>print(value));",
        )
        .unwrap();
        let mut config = javascript_oracle_config();
        config.optimization.preset = OptimizationPreset::Maximum;
        config.javascript.candidate_search = CandidateSearch::Always;
        config.javascript.optimization_level = 15;
        config.javascript.cost_model = CompressionCostModel::Brotli;
        config.mangle.identifiers = Some(true);
        config.mangle.properties = Some(true);
        config.mangle.exports = Some(true);

        let javascript = compile_program_to_js_configured(&program, &config).unwrap();
        let retained_binding =
            "var a=async ()=>await Promise.resolve(8)*2|0;a().then(a=>{console.log(a)})";
        assert!(
            compressed_size(javascript.as_bytes(), CompressionCostModel::Brotli).unwrap()
                < compressed_size(retained_binding.as_bytes(), CompressionCostModel::Brotli)
                    .unwrap(),
            "{javascript}"
        );
        assert!(!javascript.contains("var "), "{javascript}");
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("node stdout is UTF-8"),
            "16\n",
            "{javascript}"
        );
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

        let runtime = std::process::Command::new("node")
            .arg("-e")
            .arg(format!(
                "let callback;function retain(value){{callback=value}}{output};console.log(callback());console.log(callback())"
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            runtime.status.success(),
            "node failed with {}:\n{}\n{output}",
            runtime.status,
            String::from_utf8_lossy(&runtime.stderr)
        );
        assert_eq!(
            String::from_utf8(runtime.stdout).expect("node stdout is UTF-8"),
            "1\n2\n",
            "{output}"
        );
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
    fn reports_missing_exports_and_links_static_module_cycles() {
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
            "import {right} from \"./right\";export int leaf(){return 3;}export int left(){return right()+1;}",
        )
        .unwrap();
        std::fs::write(
            directory.join("right.lil"),
            "import {leaf} from \"./left\";export int right(){return leaf()+3;}",
        )
        .unwrap();
        std::fs::write(&main, "import {left} from \"./left\";print(left());").unwrap();
        assert_eq!(compile_path(&main).unwrap(), "console.log(7)");
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn compile_cycle_fixture(name: &str, files: &[(&str, &str)]) -> Result<String, ModuleError> {
        let directory =
            std::env::temp_dir().join(format!("lilscript-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        for (file, source) in files {
            std::fs::write(directory.join(file), source).unwrap();
        }
        let result = compile_path(&directory.join("a.lil"));
        std::fs::remove_dir_all(directory).unwrap();
        result
    }

    #[test]
    fn links_two_module_cycle_with_exported_value() {
        let javascript = compile_cycle_fixture(
            "two-module-value-cycle-test",
            &[
                (
                    "a.lil",
                    "import {readLater} from \"./b\";export int entry(){return readLater();}export int later=7;print(entry());",
                ),
                (
                    "b.lil",
                    "import {later} from \"./a\";export int readLater(){return later;}",
                ),
            ],
        )
        .unwrap();

        assert_eq!(node_stdout(&javascript), "7\n", "{javascript}");
    }

    #[test]
    fn uses_imported_constructor_value_inside_exported_function_in_cycle() {
        let javascript = compile_cycle_fixture(
            "imported-constructor-value-cycle-test",
            &[
                (
                    "a.lil",
                    "import {Context,cycleValue} from \"./b\";export JsValue create(JsValue value){return JS.construct(Context,value);}print(JS.number(create(7)[\"value\"]).toInt()+cycleValue());",
                ),
                (
                    "b.lil",
                    "import {create} from \"./a\";export constructor Context;class Context{JsValue value;init(JsValue value){this.value=value;}}export int cycleValue(){if(JS.typeOf(create)==\"function\")return 1;return 0;}",
                ),
            ],
        )
        .unwrap();

        assert_eq!(node_stdout(&javascript), "8\n", "{javascript}");
    }

    #[test]
    fn cyclic_imports_observe_live_export_updates() {
        let javascript = compile_cycle_fixture(
            "live-cycle-binding-test",
            &[
                (
                    "a.lil",
                    "import {readAfterBump} from \"./b\";export int count=1;export void bump(){count+=1;}print(readAfterBump());",
                ),
                (
                    "b.lil",
                    "import {count,bump} from \"./a\";export int readAfterBump(){bump();return count;}",
                ),
            ],
        )
        .unwrap();

        assert_eq!(node_stdout(&javascript), "2\n", "{javascript}");
    }

    #[test]
    fn resolves_reexports_through_a_static_cycle() {
        let javascript = compile_cycle_fixture(
            "cyclic-reexport-test",
            &[
                (
                    "a.lil",
                    "import {value} from \"./b\";export {value};export int read(){return value;}print(read());",
                ),
                (
                    "b.lil",
                    "import {read} from \"./a\";import {value} from \"./c\";export {value};export int through(){return read();}",
                ),
                ("c.lil", "export int value=9;"),
            ],
        )
        .unwrap();

        assert_eq!(node_stdout(&javascript), "9\n", "{javascript}");
    }

    #[test]
    fn initializes_each_module_once_in_a_static_cycle() {
        let javascript = compile_cycle_fixture(
            "cycle-once-test",
            &[
                (
                    "a.lil",
                    "import {read} from \"./b\";int initialized=0;int initialize(){initialized+=1;return initialized;}export int value=initialize();print(read()+initialized);",
                ),
                (
                    "b.lil",
                    "import {value} from \"./a\";export int read(){return value;}",
                ),
            ],
        )
        .unwrap();

        assert_eq!(node_stdout(&javascript), "2\n", "{javascript}");
    }

    #[test]
    fn permits_deferred_cyclic_module_value_reads() {
        let javascript = compile_cycle_fixture(
            "deferred-cycle-read-test",
            &[
                (
                    "a.lil",
                    "import {reader} from \"./b\";export int later=11;print(reader());",
                ),
                (
                    "b.lil",
                    "import {later} from \"./a\";export func()->int reader=()=>later;",
                ),
            ],
        )
        .unwrap();

        assert_eq!(node_stdout(&javascript), "11\n", "{javascript}");
    }

    #[test]
    fn rejects_eager_cyclic_module_value_reads() {
        let error = compile_cycle_fixture(
            "eager-cycle-read-test",
            &[
                (
                    "a.lil",
                    "import {eager} from \"./b\";export int later=7;print(eager);",
                ),
                (
                    "b.lil",
                    "import {later} from \"./a\";export int eager=later;",
                ),
            ],
        )
        .unwrap_err();
        assert!(
            error.message.contains("cannot eagerly read module binding"),
            "{error}"
        );
        assert!(error.path.ends_with("b.lil"), "{error}");

        let same_module = compile_cycle_fixture(
            "same-module-forward-read-test",
            &[(
                "a.lil",
                "int read(){return later;}int later=7;print(read());",
            )],
        )
        .unwrap_err();
        assert!(
            same_module.message.contains("before its declaration"),
            "{same_module}"
        );
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

#[cfg(test)]
mod function_scope_tests {
    use super::*;
    use crate::config::JavaScriptPriority;
    use crate::parser::parse_source;
    use bumpalo::Bump;

    fn run_module(javascript: &str, probe: &str) -> String {
        let encoded = base64_encode(javascript.as_bytes());
        let output = std::process::Command::new("node")
            .arg("--input-type=module")
            .arg("-e")
            .arg(format!(
                "import * as m from \"data:text/javascript;base64,{encoded}\";const {{{names}}}=m;{probe}",
                names = crate::js_peephole::generated_javascript_export_names(javascript)
                    .unwrap()
                    .join(",")
            ))
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn base64_encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let word = chunk.iter().enumerate().fold(0u32, |acc, (index, byte)| acc | (u32::from(*byte) << (16 - 8 * index)));
            for position in 0..4 {
                if position <= chunk.len() {
                    out.push(char::from(ALPHABET[((word >> (18 - 6 * position)) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn function_scope_wraps_the_module_internals_and_keeps_the_public_api() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<string> cache = record{};int misses = 0;export string look(string key){string? hit = cache[key];if (hit != null) return hit;misses = misses + 1;string made = key + \"!\";cache[key] = made;return made;}export int missed(){return misses;}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.exports = Some(false);
        let plain = compile_program_to_js_module_configured(&program, &config).unwrap();
        config.javascript.function_scope = Some(true);
        let wrapped = compile_program_to_js_module_configured(&program, &config).unwrap();

        assert!(!plain.starts_with("var _"), "{plain}");
        assert_eq!(
            crate::js_peephole::wrap_module_internals_in_function_scope(&plain).unwrap().as_ref().map(|_| ()),
            Ok(()),
            "{plain}"
        );
        assert!(wrapped.starts_with("var _a,_b;(function(){"), "{wrapped}");
        assert!(wrapped.ends_with("})();export{_a as look,_b as missed}") || wrapped.ends_with("})();export{_a as missed,_b as look}"), "{wrapped}");
        // identity-observable facts stay what the unwrapped module had: names, arity, values
        let probe = "process.stdout.write([look('a'),look('b'),look('a'),missed(),look.name===missed.name,look.length,typeof look.prototype].join(':'))";
        assert_eq!(run_module(&plain, probe), run_module(&wrapped, probe));
        assert!(run_module(&wrapped, probe).starts_with("a!:b!:a!:2:false:1:"), "{wrapped}");
    }

    #[test]
    fn a_record_read_tested_for_null_stays_undefined_when_absent() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<string> cache = record{};export string look(string key){string? hit = cache[key];if (hit != null) return hit;cache[key] = key + \"!\";return cache[key] ?? \"\";}export string? raw(string key){return cache[key];}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.exports = Some(false);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        // the tested read is spelled bare with a strict undefined test …
        assert!(javascript.contains("!==void 0") || javascript.contains("===void 0"), "{javascript}");
        // … while the read that escapes as a `string?` keeps its null normalization
        assert!(javascript.contains("??null"), "{javascript}");
        let probe = "process.stdout.write([look('a'),look('a'),String(raw('a')),String(raw('zz'))].join(':'))";
        assert_eq!(run_module(&javascript, probe), "a!:a!:a!:null");
    }

    #[test]
    fn a_record_read_that_is_never_tested_keeps_its_null_normalization() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<string> cache = record{};export string seed(string key){cache[key] = key + \"!\";return key;}export JsValue peek(string key){string? hit = cache[key];return JS.assume(hit);}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.exports = Some(false);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        // no test anywhere, so absence keeps its `null` spelling rather than leaking `undefined`
        assert!(javascript.contains("??null"), "{javascript}");
        let probe = "seed('a');process.stdout.write([String(peek('a')),String(peek('zz'))].join(':'))";
        assert_eq!(run_module(&javascript, probe), "a!:null");
    }

    #[test]
    fn an_unnormalized_map_get_tests_strictly_for_undefined_unless_truthiness_is_cheaper() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Map<string, string[]> buckets = new Map<string, string[]>();export int count(string key){string[]? bucket = buckets.get(key);if (bucket == null) { string[] made = [key]; buckets.set(key, made); return 1; }bucket.push(key);return bucket.length;}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.exports = Some(false);
        let size_first = compile_program_to_js_module_configured(&program, &config).unwrap();
        config.javascript.priority = JavaScriptPriority::PerformanceFirst;
        let performance = compile_program_to_js_module_configured(&program, &config).unwrap();
        // neither spelling normalizes the read, and the performance spelling tests `undefined` strictly
        assert!(!size_first.contains("??null") && !performance.contains("??null"), "{size_first}\n{performance}");
        assert!(performance.contains("===void 0") || performance.contains("!==void 0"), "{performance}");
        assert!(!size_first.contains("void 0"), "{size_first}");
        let probe = "process.stdout.write([count('a'),count('b'),count('a'),count('a')].join(':'))";
        assert_eq!(run_module(&size_first, probe), "1:1:2:3");
        assert_eq!(run_module(&performance, probe), "1:1:2:3");
    }

    #[test]
    fn a_string_element_read_compared_against_a_truthy_value_drops_its_hole_guard() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export int matched(JsValue args, string[] expected){int slot = 0;int index = 0;int length = args.length.toInt();while (index < length) {JsValue value = args[index];if (value.truthy()) {if (!JS.strictEqual(value, expected[slot])) return index;slot = slot + 1;}index = index + 1;}return -1;}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.exports = Some(false);
        let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
        assert!(!javascript.contains("||\"\""), "{javascript}");
        // the guard is unobservable, not absent from the semantics: a short `expected` still
        // reports a mismatch, exactly as the guarded spelling did
        let probe = "process.stdout.write([matched(['a','b'],['a','b']),matched(['a','b'],['a','z']),matched(['a','b'],['a']),matched([0,'b'],['b'])].join(':'))";
        assert_eq!(run_module(&javascript, probe), "-1:1:1:-1");
    }

    #[test]
    fn a_string_element_read_keeps_its_guard_where_the_difference_shows() {
        let arena = Bump::new();
        for source in [
            // compared against a value that is not known truthy
            "export bool same(string[] expected, string probe){return JS.strictEqual(probe, expected[3]);}",
            // concatenated, not compared
            "export string tail(string[] expected){return \"x\" + expected[3];}",
            // returned
            "export string at(string[] expected){return expected[3];}",
        ] {
            let program = parse_source(&arena, source).unwrap();
            let mut config = ProjectConfig::default();
            config.mangle.exports = Some(false);
            let javascript = compile_program_to_js_module_configured(&program, &config).unwrap();
            assert!(javascript.contains("||\"\""), "{source}\n{javascript}");
        }
    }

    #[test]
    fn truthy_nullable_checks_follow_the_priority_unless_configured() {
        let mut config = ProjectConfig::default();
        assert!(config.js_options().truthy_nullable_checks);
        config.javascript.priority = JavaScriptPriority::PerformanceFirst;
        assert!(!config.js_options().truthy_nullable_checks);
        config.javascript.truthy_nullable_checks = Some(true);
        assert!(config.js_options().truthy_nullable_checks);
        config.javascript.priority = JavaScriptPriority::SizeFirst;
        config.javascript.truthy_nullable_checks = Some(false);
        assert!(!config.js_options().truthy_nullable_checks);
    }

    #[test]
    fn a_nullable_object_test_is_strict_under_performance_first() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Entry { string r; Entry? n; init(string r) { this.r = r; this.n = null; } }Entry? last = null;export string step(string r){Entry? prior = last;if (prior != null) { Entry? next = prior.n; if (next != null) return next.r; }Entry made = new Entry(r);if (prior != null) prior.n = made;last = made;return made.r;}",
        )
        .unwrap();
        let mut config = ProjectConfig::default();
        config.mangle.exports = Some(false);
        let size_first = compile_program_to_js_module_configured(&program, &config).unwrap();
        config.javascript.priority = JavaScriptPriority::PerformanceFirst;
        let performance = compile_program_to_js_module_configured(&program, &config).unwrap();
        // performance-first spells the nullable object test as a null comparison, never as
        // truthiness; the strict `!==null` needs a provenance proof and is a later step
        assert!(performance.contains("null!=") || performance.contains("!=null"), "{performance}");
        assert!(!performance.contains("if(a)") && !performance.contains("if(c)"), "{performance}");
        assert!(size_first.len() <= performance.len(), "{size_first}\n{performance}");
        let probe = "process.stdout.write([step('a'),step('b'),step('c'),step('d')].join(':'))";
        assert_eq!(run_module(&size_first, probe), run_module(&performance, probe));
    }
}
