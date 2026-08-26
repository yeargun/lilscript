use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::codegen_ir_js::{
    ControlFlowSpelling, FunctionLayout, FunctionSpelling, HostAliasSpelling, IdentifierAlphabet,
    IrJsOptions, LoopSpelling, MutationSpelling, PhiAffinityMode, StateMachineSpelling,
    StringQuote,
};
use crate::codegen_native::NativeOptions;
use crate::js_syntax_target::{resolve_ecmascript_target, EcmaScriptEdition, JsSyntaxFeature};
use crate::optimizer::OptimizationOptions;
use crate::profile::{JavaScriptPerformanceWeights, OptimizationProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PublicAggregateAbi {
    #[default]
    Named,
    Positional,
}

/// Runtime layout for class and struct instances. `Positional` emits array slots, which is the
/// smallest possible output. `Named` emits hidden-class objects, which cost fewer bytes per
/// instance at runtime because V8 stores named properties inline instead of behind a separate
/// elements backing store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AggregateLayout {
    #[default]
    Positional,
    Named,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompilerConfig {
    pub resources: CompilerResourceConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompilerResourceConfig {
    /// Total Rayon workers for one configured JavaScript compilation. When
    /// omitted, Rayon keeps its process/global `RAYON_NUM_THREADS` or host
    /// default rather than creating a per-compilation pool.
    pub threads: Option<NonZeroUsize>,
    /// Maximum terminal Brotli plan finalizers. The effective value is also
    /// capped by the active Rayon pool.
    pub codec_workers: NonZeroUsize,
}

impl Default for CompilerResourceConfig {
    fn default() -> Self {
        Self {
            threads: None,
            codec_workers: NonZeroUsize::new(4).expect("four is nonzero"),
        }
    }
}

impl CompilerResourceConfig {
    pub fn effective_codec_workers(&self, active_threads: usize) -> usize {
        self.codec_workers.get().min(active_threads.max(1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub package: Option<PackageMetadata>,
    pub dependencies: BTreeMap<String, DependencyConfig>,
    pub compiler: CompilerConfig,
    pub optimization: OptimizationConfig,
    pub javascript: JavaScriptConfig,
    pub mangle: MangleConfig,
    pub bundle: BundleConfig,
    pub profile: OptimizationProfileConfig,
    pub native: NativeConfig,
    pub lint: LintConfig,
    pub format: FormatConfig,
    #[serde(skip)]
    pub config_dir: Option<PathBuf>,
}

impl ProjectConfig {
    pub fn optimizer_options(&self) -> OptimizationOptions {
        self.optimization.resolve()
    }

    pub fn native_profile_guided_optimization(&self) -> bool {
        self.optimization.profile_guided.unwrap_or(matches!(
            self.optimization.preset,
            OptimizationPreset::Maximum
        ))
    }

    pub fn js_profile_guided_optimization(&self) -> bool {
        self.native_profile_guided_optimization()
            && self.javascript_optimization_configured(
                JavaScriptOptimization::ProfileGuidedOptimization,
            )
    }

    pub fn js_optimizer_options(&self) -> OptimizationOptions {
        let mut options = self.optimization.resolve();
        options.specialize_tagged_constants = self
            .optimization
            .specialize_tagged_constants
            .unwrap_or(true);
        options.call_site_specialization &= self
            .javascript
            .optimization_enabled(JavaScriptOptimization::CallSiteSpecialization, None);
        options.capture_signature_cloning &= self
            .javascript
            .optimization_enabled(JavaScriptOptimization::CaptureSignatureCloning, None);
        options.identical_function_folding &= self
            .javascript
            .optimization_enabled(JavaScriptOptimization::IdenticalFunctionFolding, None);
        options.function_subsumption &= self
            .javascript
            .optimization_enabled(JavaScriptOptimization::IrFunctionSubsumptionVariants, None);
        let compress = self.compress_pass_options();
        options.pipeline_fusion = compress.pipeline_fusion;
        options.partial_escape_sinking = compress.partial_escape_sinking;
        options.region_outlining = compress.region_outlining;
        options.expression_superopt = compress.expression_superopt;
        options.path_sensitive_propagation = compress.path_sensitive_propagation;
        options.parameterized_function_merging = self.js_parameterized_function_merging_enabled();
        if !options.inlining {
            return options;
        }
        let policy = self.javascript.priority.policy();
        options.inline_instruction_limit = self
            .javascript
            .inline_instruction_limit
            .unwrap_or(policy.inline_instruction_limit);
        options.inline_control_flow_limit = self
            .javascript
            .inline_control_flow_limit
            .unwrap_or(policy.inline_control_flow_limit);
        options.inline_growth_limit =
            self.javascript
                .max_inline_growth
                .map(Some)
                .unwrap_or_else(|| {
                    self.javascript
                        .compression_enabled(CompressionDecision::SizeAwareInlining)
                        .then_some(policy.max_inline_growth)
                });
        options
    }

    pub fn js_function_subsumption_variants_enabled(&self) -> bool {
        if self.optimization.function_subsumption == Some(false)
            || !self
                .javascript
                .optimization_enabled(JavaScriptOptimization::IrFunctionSubsumptionVariants, None)
        {
            return false;
        }
        self.optimization.function_subsumption == Some(true)
            || self.javascript.optimizations.is_some()
            || matches!(self.javascript.priority, JavaScriptPriority::SizeFirst)
    }

    pub fn load_optimization_profile(&self) -> Result<OptimizationProfile, String> {
        let mut profile = if let Some(path) = &self.profile.path {
            let path = if path.is_absolute() {
                path.clone()
            } else {
                self.config_dir
                    .as_deref()
                    .unwrap_or_else(|| Path::new("."))
                    .join(path)
            };
            let source = fs::read_to_string(&path).map_err(|error| {
                format!(
                    "failed to read optimization profile `{}`: {error}",
                    path.display()
                )
            })?;
            serde_json::from_str::<OptimizationProfile>(&source).map_err(|error| {
                format!("invalid optimization profile `{}`: {error}", path.display())
            })?
        } else {
            OptimizationProfile::default()
        };
        profile.merge(&OptimizationProfile {
            version: 1,
            functions: self.profile.functions.clone(),
            loops: self.profile.loops.clone(),
        });
        profile.validate()?;
        Ok(profile)
    }

    pub const fn javascript_performance_weights(&self) -> JavaScriptPerformanceWeights {
        JavaScriptPerformanceWeights {
            deoptimization: self.javascript.performance.deoptimization_weight,
            allocation: self.javascript.performance.allocation_weight,
            indirect_call: self.javascript.performance.indirect_call_weight,
            hot_code: self.javascript.performance.hot_code_weight,
        }
    }

    pub const fn native_options(&self) -> NativeOptions {
        NativeOptions {
            partial_escape_analysis: self.native.partial_escape_analysis,
            stack_allocation: self.native.stack_allocation,
            region_allocation: self.native.region_allocation,
            stack_array_element_limit: self.native.stack_array_element_limit,
        }
    }

    pub fn js_options(&self) -> IrJsOptions {
        IrJsOptions {
            mangle_identifiers: self.mangle.identifiers.unwrap_or_else(|| {
                self.javascript
                    .compression_enabled(CompressionDecision::IdentifierMangling)
            }),
            mangle_properties: self.mangle.properties.unwrap_or_else(|| {
                self.javascript
                    .compression_enabled(CompressionDecision::PropertyMangling)
            }),
            mangle_exports: self.mangle.exports.unwrap_or_else(|| {
                self.javascript
                    .compression_enabled(CompressionDecision::ExportMangling)
            }),
            mangle_extern_fields: self.mangle.extern_fields.unwrap_or(true),
            public_aggregate_fields: matches!(
                self.javascript.public_aggregate_abi,
                PublicAggregateAbi::Named
            ),
            named_aggregate_fields: matches!(
                self.javascript.aggregate_layout,
                AggregateLayout::Named
            ),
            pool_strings: self.mangle.pool_strings.unwrap_or_else(|| {
                self.javascript
                    .compression_enabled(CompressionDecision::StringPooling)
            }),
            string_pool_minimum_savings: match self.javascript.cost_model {
                CompressionCostModel::Raw => 1,
                CompressionCostModel::Gzip => 4,
                CompressionCostModel::Brotli => 8,
            },
            pool_identifier_strings: !matches!(
                self.javascript.cost_model,
                CompressionCostModel::Brotli
            ),
            pool_numeric_literals: self.javascript.pool_numeric_literals,
            ordinary_record_literals: false,
            elide_safe_integer_coercions: !self.javascript.keep_integer_coercions(),
            elide_length_tonumber: self
                .javascript
                .compression_enabled(CompressionDecision::LengthToNumberElision),
            compact_boolean_literals: self
                .javascript
                .compression_enabled(CompressionDecision::CompactBooleanLiterals),
            elide_block_terminal_semicolons: self
                .javascript
                .compression_enabled(CompressionDecision::StandardGrammarElision),
            elide_new_parentheses: self
                .javascript
                .compression_enabled(CompressionDecision::StandardGrammarElision),
            elide_call_chain_parentheses: self
                .javascript
                .compression_enabled(CompressionDecision::StandardGrammarElision),
            inline_structured_closures: self
                .javascript
                .compression_enabled(CompressionDecision::StructuredClosureInlining),
            struct_method_shorthand: self.javascript.struct_method_shorthand.unwrap_or(true),
            truthy_nullable_checks: true,
            pack_string_arrays: self
                .javascript
                .compression_enabled(CompressionDecision::StringArrayPacking)
                && !matches!(self.javascript.cost_model, CompressionCostModel::Brotli),
            regex_literals: self.javascript.assume_pristine_builtins
                && self
                    .javascript
                    .compression_enabled(CompressionDecision::RegexLiterals),
            unused_catch_binding_elision: self
                .javascript
                .compression_enabled(CompressionDecision::UnusedCatchBindingElision)
                && self
                    .javascript
                    .resolved_ecmascript()
                    .allows(JsSyntaxFeature::OptionalCatchBinding),
            compact_generator_star: self
                .javascript
                .compression_enabled(CompressionDecision::CompactGeneratorStar),
            // Candidate search can introduce this whole-program
            // representation when structured-function compression is enabled.
            // Keeping the configured baseline named preserves predictable
            // development output and lets the exact transfer codec decide.
            inline_single_use_functions: false,
            inline_exclusive_closures: true,
            iife_private_callee_clusters: self.javascript.iife_private_callee_clusters,
            nested_once_run_helpers: self.javascript.nested_once_run_helpers,
            batch_property_assigns: false,
            batch_property_assign_minimum: 2,
            // Fresh-literal factory substitution changes complete-artifact
            // repetition history, so candidate search scores it separately.
            inline_fresh_empty_array_factories: false,
            // Complete constructor-literal fusion is scored as a distinct
            // local-codegen candidate; the configured baseline stays dense and
            // source-shaped for predictable output and codec comparison.
            constructor_initializer_fusion: false,
            pure_helper_inlining: crate::codegen_ir_js::PureHelperInliningPolicy::None,
            // The configured artifact remains source-shaped. Candidate search
            // introduces the dense representation only when its dedicated
            // compression decision is enabled and exact codec scoring wins.
            dense_string_return_tables: false,
            host_alias_spelling: HostAliasSpelling::Shared,
            callee_default_arguments: self
                .javascript
                .compression_enabled(CompressionDecision::CalleeDefaultArguments),
            scalar_phi_copies: self
                .javascript
                .compression_enabled(CompressionDecision::ScalarPhiCopies),
            local_name_coalescing: self.javascript.local_name_coalescing,
            phi_affinity_mode: if self
                .javascript
                .compression_enabled(CompressionDecision::PhiAffinityCoalescing)
            {
                PhiAffinityMode::Grouped
            } else {
                PhiAffinityMode::Conservative
            },
            control_flow_spelling: ControlFlowSpelling::Auto,
            state_machine_spelling: StateMachineSpelling::Switch,
            conditional_expressions: self
                .javascript
                .optimization_enabled(JavaScriptOptimization::ConditionalExpressionVariants, None),
            expression_phi_regions: self
                .javascript
                .optimization_enabled(JavaScriptOptimization::ExpressionPhiRegionVariants, None),
            // Statement-authored phi recovery is raw-positive in the common
            // case, but may disrupt Brotli's larger-context repetitions. Use
            // a codec-aware canonical state and let candidate search score the
            // opposite state over the complete artifact.
            local_phi_expression_regions: self.javascript.optimization_enabled(
                JavaScriptOptimization::LocalPhiExpressionRegionVariants,
                None,
            ) && self
                .javascript
                .local_phi_expression_regions
                .unwrap_or(!matches!(
                    self.javascript.cost_model,
                    CompressionCostModel::Brotli
                )),
            phi_edge_value_forwarding: self
                .javascript
                .optimization_enabled(JavaScriptOptimization::PhiEdgeValueForwardingVariants, None)
                && !matches!(self.javascript.cost_model, CompressionCostModel::Brotli),
            operand_order_fusion: self.javascript.operand_order_fusion,
            aggregate_operand_order_fusion: self.javascript.aggregate_operand_order_fusion,
            sink_entry_function_declarations: self.javascript.sink_entry_function_declarations,
            comma_expressions: false,
            update_loop_layout: true,
            cross_scope_name_reuse: self
                .javascript
                .optimization_enabled(JavaScriptOptimization::EntropyCrossScopeReuse, None),
            // Keep the mandatory emission conservative. Production search
            // scores exact transitive nested-function shadowing separately.
            transitive_nested_shadowing: false,
            // The precise regime is an aggressive scored proposal. Keep the
            // configured/pinned emission conservative so an incomplete
            // transitive-reference proof can be rejected without making the
            // compiler's mandatory fallback invalid.
            precise_cross_scope_shadowing: false,
            reserved_local_name_prefix: false,
            local_name_reserve: self.javascript.local_name_reserve,
            stable_local_names: self.javascript.stable_local_names,
            frequency_order_local_names: false,
            entropy_property_names: self
                .javascript
                .optimization_enabled(JavaScriptOptimization::EntropyPropertyAssignment, None),
            function_layout: FunctionLayout::Source,
            function_layout_exact_limit: self.javascript.function_layout_exact_limit,
            function_spelling: self.javascript.function_spelling.unwrap_or(
                if matches!(self.javascript.cost_model, CompressionCostModel::Brotli) {
                    FunctionSpelling::Function
                } else {
                    FunctionSpelling::Arrow
                },
            ),
            public_function_arrows: matches!(
                self.javascript.function_spelling,
                Some(FunctionSpelling::Arrow)
            ),
            loop_spelling: LoopSpelling::Auto,
            mutation_spelling: MutationSpelling::Assignment,
            identifier_alphabet: IdentifierAlphabet::canonical(),
            string_quote: StringQuote::Double,
            pool_window_roots: true,
            bare_window_root: false,
            alias_array_prototype_methods: !matches!(
                self.javascript.cost_model,
                CompressionCostModel::Brotli
            ),
            ecmascript: self.javascript.resolved_ecmascript(),
            indexed_char_at: false,
            effect_ternary: true,
        }
    }

    pub fn entropy_aware_mangling_enabled(&self) -> bool {
        self.javascript
            .compression_enabled(CompressionDecision::EntropyAwareMangling)
    }

    pub fn quote_style_selection_enabled(&self) -> bool {
        self.javascript
            .compression_enabled(CompressionDecision::QuoteStyleSelection)
    }

    pub fn single_use_function_expression_candidates_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self
                .javascript
                .compression_enabled(CompressionDecision::StructuredClosureInlining)
    }

    pub fn pure_helper_inlining_candidates_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self
                .javascript
                .compression_enabled(CompressionDecision::PureHelperInlining)
    }

    pub fn dense_string_return_table_candidates_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self
                .javascript
                .compression_enabled(CompressionDecision::DenseStringReturnTables)
    }

    pub fn host_alias_spelling_candidates_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self
                .javascript
                .compression_enabled(CompressionDecision::HostAliasSpelling)
    }

    pub fn ir_inlining_variants_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::IrInliningVariants,
                Some(CompressionDecision::IrInliningVariants),
            )
    }

    pub fn ir_closure_factory_variants_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::IrClosureFactoryVariants,
                Some(CompressionDecision::IrClosureFactoryVariants),
            )
    }

    pub fn ir_phase_ordering_variants_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::IrPhaseOrderingVariants,
                Some(CompressionDecision::IrPhaseOrderingVariants),
            )
    }

    pub fn javascript_optimization_enabled(&self, feature: JavaScriptOptimization) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(feature, None)
    }

    pub fn js_scalar_phi_copy_variants_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::SsaDestructionVariants,
                Some(CompressionDecision::ScalarPhiCopies),
            )
    }

    pub fn js_phi_affinity_variants_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::SsaDestructionVariants,
                Some(CompressionDecision::PhiAffinityCoalescing),
            )
    }

    /// The full coalesced/uncoalesced spelling is another bounded
    /// SSA-destruction variant. It intentionally shares the existing
    /// phi-affinity allowlist for backward-compatible configuration, while
    /// callers can distinguish it from choosing an affinity mode.
    pub fn js_local_name_coalescing_variants_enabled(&self) -> bool {
        self.js_phi_affinity_variants_enabled()
    }

    pub fn javascript_optimization_configured(&self, feature: JavaScriptOptimization) -> bool {
        self.javascript.optimization_enabled(feature, None)
    }

    pub fn loop_spelling_selection_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::StructuralLoopVariants,
                Some(CompressionDecision::LoopSpellingSelection),
            )
    }

    pub fn mutation_spelling_selection_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::CompoundMutationVariants,
                Some(CompressionDecision::MutationSpellingSelection),
            )
    }

    pub fn indexed_char_at_candidates_enabled(&self) -> bool {
        self.javascript
            .search_compression_enabled(CompressionDecision::IndexedCharAt)
    }

    pub fn effect_ternary_candidates_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self
                .javascript
                .compression_enabled(CompressionDecision::EffectTernary)
    }

    pub fn compress_pass_options(&self) -> crate::compress_passes::CompressPassOptions {
        let allow_profile_defaults = self.optimization.preset != OptimizationPreset::None;
        crate::compress_passes::CompressPassOptions {
            pipeline_fusion: self
                .optimization
                .pipeline_fusion
                .unwrap_or(allow_profile_defaults)
                && self
                    .javascript
                    .compression_enabled(CompressionDecision::ArrayPipelineFusion),
            partial_escape_sinking: self
                .optimization
                .partial_escape_sinking
                .unwrap_or(allow_profile_defaults)
                && self
                    .javascript
                    .compression_enabled(CompressionDecision::PartialEscapeSinking),
            // Default off: helpers often win raw while losing gzip/Brotli. A
            // codec-scored search may reintroduce outlining only when the
            // compression policy permits that decision.
            region_outlining: self.optimization.region_outlining.unwrap_or(false)
                && self
                    .javascript
                    .compression_enabled(CompressionDecision::RegionOutlining),
            expression_superopt: self
                .optimization
                .expression_superopt
                .unwrap_or(allow_profile_defaults)
                && self
                    .javascript
                    .compression_enabled(CompressionDecision::ExpressionSuperoptimization),
            path_sensitive_propagation: self
                .optimization
                .path_sensitive_propagation
                .unwrap_or(allow_profile_defaults)
                && self
                    .javascript
                    .compression_enabled(CompressionDecision::PathSensitivePropagation),
        }
    }

    pub fn js_joint_chunk_symbol_search_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::JointChunkSymbolSearch,
                Some(CompressionDecision::JointChunkSymbolSearch),
            )
    }

    pub fn js_region_outlining_candidate_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.optimization.region_outlining != Some(false)
            && self
                .javascript
                .compression_enabled(CompressionDecision::RegionOutlining)
    }

    pub fn js_joint_representation_search_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::JointRepresentationSearch,
                Some(CompressionDecision::JointRepresentationSearch),
            )
    }

    pub fn js_default_argument_variants_enabled(&self) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(
                JavaScriptOptimization::DefaultArgumentVariants,
                Some(CompressionDecision::CalleeDefaultArguments),
            )
    }

    pub fn js_parameterized_function_merging_enabled(&self) -> bool {
        let allow_profile_defaults = self.optimization.preset != OptimizationPreset::None;
        self.optimization
            .parameterized_function_merging
            .unwrap_or(allow_profile_defaults)
            && self
                .javascript
                .compression_enabled(CompressionDecision::ParameterizedFunctionMerging)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.bundle.min_chunk_bytes == 0 {
            return Err("`bundle.min_chunk_bytes` must be greater than zero".to_string());
        }
        if let Some(package) = &self.package {
            validate_package_name(&package.name)?;
            semver::Version::parse(&package.version)
                .map_err(|error| format!("invalid `package.version`: {error}"))?;
            if package.abi != crate::package::LILSCRIPT_ABI_VERSION {
                return Err(format!(
                    "`package.abi` is {}, but this compiler supports ABI {}",
                    package.abi,
                    crate::package::LILSCRIPT_ABI_VERSION
                ));
            }
            if package.entry.as_os_str().is_empty() {
                return Err("`package.entry` must not be empty".to_string());
            }
        }
        for (name, dependency) in &self.dependencies {
            validate_package_name(name)?;
            if dependency.path.as_os_str().is_empty() {
                return Err(format!("dependency `{name}` has an empty path"));
            }
            semver::VersionReq::parse(&dependency.version).map_err(|error| {
                format!("dependency `{name}` has invalid version requirement: {error}")
            })?;
            if dependency.abi != crate::package::LILSCRIPT_ABI_VERSION {
                return Err(format!(
                    "dependency `{name}` requests ABI {}, but this compiler supports ABI {}",
                    dependency.abi,
                    crate::package::LILSCRIPT_ABI_VERSION
                ));
            }
        }
        if self.bundle.max_chunks == 0 {
            return Err("`bundle.max_chunks` must be greater than zero".to_string());
        }
        if self.bundle.shared_min_imports < 2 {
            return Err("`bundle.shared_min_imports` must be at least 2".to_string());
        }
        if self.bundle.cost.raw_weight == 0
            && self.bundle.cost.gzip_weight == 0
            && self.bundle.cost.brotli_weight == 0
        {
            return Err("`bundle.cost` must enable at least one byte-cost weight".to_string());
        }
        if self.bundle.cost.preload_request_discount_percent > 100 {
            return Err(
                "`bundle.cost.preload_request_discount_percent` must be at most 100".to_string(),
            );
        }
        if self.bundle.cost.cache_reuse_discount_percent > 100 {
            return Err(
                "`bundle.cost.cache_reuse_discount_percent` must be at most 100".to_string(),
            );
        }
        if let Some(decisions) = &self.javascript.compression {
            let mut unique = HashSet::with_capacity(decisions.len());
            for decision in decisions {
                if !unique.insert(*decision) {
                    return Err(format!(
                        "`javascript.compression` contains duplicate `{}`",
                        decision.name()
                    ));
                }
            }
        }
        resolve_ecmascript_target(self.javascript.ecmascript, &self.javascript.browsers)?;
        if self.javascript.candidate_limit == 0 {
            return Err("`javascript.candidate_limit` must be greater than zero".to_string());
        }
        if self.javascript.candidate_byte_budget == 0 {
            return Err("`javascript.candidate_byte_budget` must be greater than zero".to_string());
        }
        if self.javascript.candidate_beam_width == 0 {
            return Err("`javascript.candidate_beam_width` must be greater than zero".to_string());
        }
        if self.javascript.max_candidate_raw_growth_percent > 1000 {
            return Err(
                "`javascript.max_candidate_raw_growth_percent` must be at most 1000".to_string(),
            );
        }
        if self.javascript.function_layout_exact_limit > 18 {
            return Err("`javascript.function_layout_exact_limit` must be at most 18".to_string());
        }
        if self.javascript.local_name_reserve > 256 {
            return Err("`javascript.local_name_reserve` must be at most 256".to_string());
        }
        if self.javascript.startup.max_nesting == Some(0) {
            return Err("`javascript.startup.max_nesting` must be greater than zero".to_string());
        }
        if self.javascript.optimization_level > 15 {
            return Err("`javascript.optimization_level` must be between 0 and 15".to_string());
        }
        if let Some(features) = &self.javascript.optimizations {
            let mut unique = HashSet::with_capacity(features.len());
            for feature in features {
                if !unique.insert(*feature) {
                    return Err(format!(
                        "`javascript.optimizations` contains duplicate `{}`",
                        feature.name()
                    ));
                }
            }
        }
        if self.javascript.performance.deoptimization_weight == 0
            && self.javascript.performance.allocation_weight == 0
            && self.javascript.performance.indirect_call_weight == 0
            && self.javascript.performance.hot_code_weight == 0
        {
            return Err(
                "`javascript.performance` must enable at least one cost weight".to_string(),
            );
        }
        if self.javascript.performance.max_regression_percent > 1_000 {
            return Err(
                "`javascript.performance.max_regression_percent` must be at most 1000".to_string(),
            );
        }
        if self.profile.specialization_min_count == 0 {
            return Err("`profile.specialization_min_count` must be greater than zero".to_string());
        }
        if self.profile.max_specializations_per_function == 0 {
            return Err(
                "`profile.max_specializations_per_function` must be greater than zero".to_string(),
            );
        }
        if self.profile.max_clone_instructions == 0 {
            return Err("`profile.max_clone_instructions` must be greater than zero".to_string());
        }
        if self.native.stack_array_element_limit == 0 {
            return Err("`native.stack_array_element_limit` must be greater than zero".to_string());
        }
        OptimizationProfile {
            version: 1,
            functions: self.profile.functions.clone(),
            loops: self.profile.loops.clone(),
        }
        .validate()?;
        for (name, percent) in [
            (
                "parse_overhead_limit_percent",
                self.javascript.startup.parse_overhead_limit_percent,
            ),
            (
                "compile_overhead_limit_percent",
                self.javascript.startup.compile_overhead_limit_percent,
            ),
            (
                "memory_overhead_limit_percent",
                self.javascript.startup.memory_overhead_limit_percent,
            ),
        ] {
            if percent > 1_000 {
                return Err(format!("`javascript.startup.{name}` must be at most 1000"));
            }
        }
        if self.format.line_width < 40 {
            return Err("`format.line_width` must be at least 40".to_string());
        }
        for (rule, severity) in &self.lint.rules {
            if rule.trim().is_empty() {
                return Err("`lint.rules` contains an empty rule name".to_string());
            }
            let _ = severity;
        }
        if let Some(providers) = &self.lint.providers {
            let mut unique = HashSet::with_capacity(providers.len());
            for provider in providers {
                if provider.trim().is_empty() || provider.contains('/') {
                    return Err(
                        "`lint.providers` names must be nonempty namespace identifiers without `/`"
                            .to_string(),
                    );
                }
                if !unique.insert(provider) {
                    return Err(format!("`lint.providers` contains duplicate `{provider}`"));
                }
            }
        }
        Ok(())
    }
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "package name `{name}` must contain only ASCII letters, digits, `-`, or `_`"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub abi: u32,
    pub entry: PathBuf,
}

impl Default for PackageMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.1.0".to_string(),
            abi: crate::package::LILSCRIPT_ABI_VERSION,
            entry: PathBuf::from("src/lib.lil"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DependencyConfig {
    pub path: PathBuf,
    pub version: String,
    pub abi: u32,
}

impl Default for DependencyConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            version: "*".to_string(),
            abi: crate::package::LILSCRIPT_ABI_VERSION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaScriptPriority {
    PerformanceFirst,
    #[serde(alias = "realisticperf-first", alias = "realistic-perf-first")]
    RealisticPerformanceFirst,
    Balanced,
    #[default]
    SizeFirst,
}

impl JavaScriptPriority {
    const fn policy(self) -> JavaScriptPolicy {
        match self {
            Self::PerformanceFirst => JavaScriptPolicy::new(24, 60, 32),
            Self::RealisticPerformanceFirst => JavaScriptPolicy::new(18, 45, 16),
            Self::Balanced => JavaScriptPolicy::new(12, 30, 4),
            Self::SizeFirst => JavaScriptPolicy::new(12, 30, 16),
        }
    }

    const fn keeps_integer_coercions(self) -> bool {
        matches!(
            self,
            Self::PerformanceFirst | Self::RealisticPerformanceFirst
        )
    }

    const fn enables_compression(self, decision: CompressionDecision) -> bool {
        match decision {
            CompressionDecision::IdentifierMangling => true,
            CompressionDecision::EntropyAwareMangling => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::QuoteStyleSelection => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::StringPooling => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::SizeAwareInlining => !matches!(self, Self::PerformanceFirst),
            // `|0` never helps gzip/Brotli. Size-first and balanced drop
            // proven-redundant coercions. Emission still follows
            // `javascript.integer_coercions` / performance-first, not this
            // allowlist: exact `compression = []` must not reintroduce `|0`.
            CompressionDecision::SafeIntegerCoercionElision => !self.keeps_integer_coercions(),
            CompressionDecision::LengthToNumberElision => matches!(self, Self::SizeFirst),
            CompressionDecision::CompactBooleanLiterals => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::StandardGrammarElision => true,
            CompressionDecision::StructuredClosureInlining => {
                !matches!(self, Self::PerformanceFirst)
            }
            CompressionDecision::PureHelperInlining
            | CompressionDecision::DenseStringReturnTables
            | CompressionDecision::HostAliasSpelling => matches!(self, Self::SizeFirst),
            CompressionDecision::StringArrayPacking => matches!(self, Self::SizeFirst),
            CompressionDecision::RegexLiterals => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::UnusedCatchBindingElision => true,
            CompressionDecision::CompactGeneratorStar => true,
            CompressionDecision::CalleeDefaultArguments => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::ScalarPhiCopies => matches!(self, Self::SizeFirst),
            CompressionDecision::PhiAffinityCoalescing => true,
            CompressionDecision::IrInliningVariants => matches!(self, Self::SizeFirst),
            CompressionDecision::IrClosureFactoryVariants => matches!(self, Self::SizeFirst),
            CompressionDecision::IrPhaseOrderingVariants => matches!(self, Self::SizeFirst),
            CompressionDecision::LoopSpellingSelection => {
                matches!(self, Self::SizeFirst | Self::Balanced)
            }
            CompressionDecision::MutationSpellingSelection | CompressionDecision::IndexedCharAt => {
                matches!(self, Self::SizeFirst)
            }
            CompressionDecision::EffectTernary => false,
            CompressionDecision::PropertyMangling => matches!(self, Self::SizeFirst),
            CompressionDecision::ExportMangling => false,
            CompressionDecision::ArrayPipelineFusion
            | CompressionDecision::PartialEscapeSinking
            | CompressionDecision::RegionOutlining
            | CompressionDecision::JointRepresentationSearch
            | CompressionDecision::JointChunkSymbolSearch
            | CompressionDecision::ParameterizedFunctionMerging => {
                matches!(self, Self::SizeFirst)
            }
            CompressionDecision::ExpressionSuperoptimization
            | CompressionDecision::PathSensitivePropagation => {
                matches!(self, Self::SizeFirst | Self::Balanced)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JavaScriptPolicy {
    inline_instruction_limit: usize,
    inline_control_flow_limit: usize,
    max_inline_growth: usize,
}

impl JavaScriptPolicy {
    const fn new(
        inline_instruction_limit: usize,
        inline_control_flow_limit: usize,
        max_inline_growth: usize,
    ) -> Self {
        Self {
            inline_instruction_limit,
            inline_control_flow_limit,
            max_inline_growth,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompressionDecision {
    IdentifierMangling,
    EntropyAwareMangling,
    QuoteStyleSelection,
    PropertyMangling,
    ExportMangling,
    StringPooling,
    SizeAwareInlining,
    SafeIntegerCoercionElision,
    LengthToNumberElision,
    CompactBooleanLiterals,
    StandardGrammarElision,
    StructuredClosureInlining,
    PureHelperInlining,
    DenseStringReturnTables,
    HostAliasSpelling,
    StringArrayPacking,
    RegexLiterals,
    UnusedCatchBindingElision,
    CompactGeneratorStar,
    CalleeDefaultArguments,
    ScalarPhiCopies,
    PhiAffinityCoalescing,
    IrInliningVariants,
    IrClosureFactoryVariants,
    IrPhaseOrderingVariants,
    LoopSpellingSelection,
    MutationSpellingSelection,
    IndexedCharAt,
    EffectTernary,
    ArrayPipelineFusion,
    PartialEscapeSinking,
    RegionOutlining,
    ExpressionSuperoptimization,
    PathSensitivePropagation,
    JointRepresentationSearch,
    JointChunkSymbolSearch,
    ParameterizedFunctionMerging,
}

impl CompressionDecision {
    const fn name(self) -> &'static str {
        match self {
            Self::IdentifierMangling => "identifier-mangling",
            Self::EntropyAwareMangling => "entropy-aware-mangling",
            Self::QuoteStyleSelection => "quote-style-selection",
            Self::PropertyMangling => "property-mangling",
            Self::ExportMangling => "export-mangling",
            Self::StringPooling => "string-pooling",
            Self::SizeAwareInlining => "size-aware-inlining",
            Self::SafeIntegerCoercionElision => "safe-integer-coercion-elision",
            Self::LengthToNumberElision => "length-to-number-elision",
            Self::CompactBooleanLiterals => "compact-boolean-literals",
            Self::StandardGrammarElision => "standard-grammar-elision",
            Self::StructuredClosureInlining => "structured-closure-inlining",
            Self::PureHelperInlining => "pure-helper-inlining",
            Self::DenseStringReturnTables => "dense-string-return-tables",
            Self::HostAliasSpelling => "host-alias-spelling",
            Self::StringArrayPacking => "string-array-packing",
            Self::RegexLiterals => "regex-literals",
            Self::UnusedCatchBindingElision => "unused-catch-binding-elision",
            Self::CompactGeneratorStar => "compact-generator-star",
            Self::CalleeDefaultArguments => "callee-default-arguments",
            Self::ScalarPhiCopies => "scalar-phi-copies",
            Self::PhiAffinityCoalescing => "phi-affinity-coalescing",
            Self::IrInliningVariants => "ir-inlining-variants",
            Self::IrClosureFactoryVariants => "ir-closure-factory-variants",
            Self::IrPhaseOrderingVariants => "ir-phase-ordering-variants",
            Self::LoopSpellingSelection => "loop-spelling-selection",
            Self::MutationSpellingSelection => "mutation-spelling-selection",
            Self::IndexedCharAt => "indexed-char-at",
            Self::EffectTernary => "effect-ternary",
            Self::ArrayPipelineFusion => "array-pipeline-fusion",
            Self::PartialEscapeSinking => "partial-escape-sinking",
            Self::RegionOutlining => "region-outlining",
            Self::ExpressionSuperoptimization => "expression-superoptimization",
            Self::PathSensitivePropagation => "path-sensitive-propagation",
            Self::JointRepresentationSearch => "joint-representation-search",
            Self::JointChunkSymbolSearch => "joint-chunk-symbol-search",
            Self::ParameterizedFunctionMerging => "parameterized-function-merging",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JavaScriptConfig {
    pub priority: JavaScriptPriority,
    pub ecmascript: EcmaScriptEdition,
    pub browsers: Vec<String>,
    pub optimization_level: u8,
    pub optimizations: Option<Vec<JavaScriptOptimization>>,
    pub compression: Option<Vec<CompressionDecision>>,
    pub pool_numeric_literals: bool,
    /// Keep signed-i32 `|0` even when range analysis proves it redundant.
    /// Omitted: size-first and balanced drop proven `|0` (`|0` does not help
    /// gzip/Brotli); performance-first and realistic-performance-first keep it.
    pub integer_coercions: Option<bool>,
    pub inline_instruction_limit: Option<usize>,
    pub inline_control_flow_limit: Option<usize>,
    pub max_inline_growth: Option<usize>,
    pub cost_model: CompressionCostModel,
    pub candidate_search: CandidateSearch,
    pub candidate_limit: usize,
    pub candidate_byte_budget: usize,
    pub candidate_beam_width: usize,
    /// Maximum optional structural emission plans admitted after the scored
    /// context seeds are installed. Omitted values also honor
    /// `candidate_limit`, so a deliberately tiny retained frontier stays a
    /// tiny-work search. An explicit value decouples attempted work from that
    /// survivor count while remaining bounded by the level/search tier.
    /// Zero keeps only scored seeds and the reserved terminal challenger tail.
    pub candidate_proposal_limit: Option<usize>,
    /// Maximum whole-artifact work units in terminal syntax/name search. A
    /// unit is charged before optional repair/validation and bounds one exact
    /// codec call. Omitted values derive from level and artifact size; zero
    /// disables optional terminal search while retaining the incumbent.
    pub terminal_codec_probe_limit: Option<usize>,
    pub max_candidate_raw_growth_percent: u16,
    pub function_layout_exact_limit: usize,
    pub local_name_reserve: usize,
    pub stable_local_names: bool,
    /// Reuse bindings for noninterfering SSA values in identifier-mangled
    /// output. Candidate search may still score the opposite spelling because
    /// fewer names can require more assignment and parenthesis syntax in the
    /// final JavaScript. Unmangled output preserves source-oriented names and
    /// does not use this switch.
    pub local_name_coalescing: bool,
    /// Wrap exclusive callees of a named root in a once-run IIFE so those
    /// helpers can reuse short names. Off only for oracles that need the
    /// three-address helper spelling their fixture was written against.
    pub iife_private_callee_clusters: bool,
    /// Declare helpers whose every reference lives in one named host as nested
    /// `function` bindings in that host. Off only for oracles that need the
    /// module-scope helper spelling their fixture was written against.
    pub nested_once_run_helpers: bool,
    /// Rebuild a nested expression when a run of single-use producers all feed
    /// one consumer that reads them in production order. Off only for oracles
    /// that need the three-address spelling their fixture was written against.
    pub operand_order_fusion: bool,
    /// Nest single-use calls into array/record literals across inert literals.
    /// Off by default; projects that repeat that shape can opt in.
    pub aggregate_operand_order_fusion: bool,
    /// Declare a defaulted function at its only entry Closure instead of in
    /// the hoisted function group. Off by default.
    pub sink_entry_function_declarations: bool,
    pub function_spelling: Option<FunctionSpelling>,
    /// Spell an object method as `k(){…}` rather than `k:function(){…}`.
    /// Shorthand is shorter, and shorter is not always smaller: measured on
    /// jQuery, turning it off is -94 Brotli *and* -404 raw, because the shapes
    /// the emitter reaches without it repeat better. It is neutral on marked,
    /// mobx, posthog and zod, so the default stands and a port that has
    /// measured its own artifact says otherwise here.
    pub struct_method_shorthand: Option<bool>,
    /// Recover statement-authored local selections as conditional expressions.
    /// The default follows the cost model, because the trade is real and goes
    /// both ways: measured across the ports, forcing it on is jQuery -87 Brotli
    /// and marked +172, zod +201, mobx +58, monaco +50, posthog +22. Candidate
    /// search does carry both states, but the winning one is only cheaper after
    /// terminal cleanup, so a beam that ranks mid-pipeline drops it. A port that
    /// has measured its own artifact says so here.
    pub local_phi_expression_regions: Option<bool>,
    pub public_aggregate_abi: PublicAggregateAbi,
    pub aggregate_layout: AggregateLayout,
    /// Allow representations that bypass ambient JavaScript constructor
    /// bindings. This is false for open-world library output. At present it
    /// gates only `new RegExp(...)` to regular-expression literal candidates.
    pub assume_pristine_builtins: bool,
    /// Drop `print()` / `debugLog` from JavaScript. On by default so production
    /// builds do not ship `console.log`. Test oracles set false. Does not strip
    /// `console.warn` (observable library behavior).
    pub strip_console: bool,
    pub startup: StartupCostConfig,
    pub performance: JavaScriptPerformanceConfig,
}

impl Default for JavaScriptConfig {
    fn default() -> Self {
        Self {
            priority: JavaScriptPriority::SizeFirst,
            ecmascript: EcmaScriptEdition::Es2022,
            browsers: Vec::new(),
            optimization_level: 15,
            optimizations: None,
            compression: None,
            pool_numeric_literals: true,
            integer_coercions: None,
            inline_instruction_limit: None,
            inline_control_flow_limit: None,
            max_inline_growth: None,
            cost_model: CompressionCostModel::Brotli,
            candidate_search: CandidateSearch::Production,
            candidate_limit: 1536,
            candidate_byte_budget: 1024 * 1024,
            candidate_beam_width: 12,
            candidate_proposal_limit: None,
            terminal_codec_probe_limit: None,
            max_candidate_raw_growth_percent: 0,
            function_layout_exact_limit: 13,
            local_name_reserve: 16,
            stable_local_names: true,
            local_name_coalescing: true,
            iife_private_callee_clusters: true,
            nested_once_run_helpers: true,
            operand_order_fusion: true,
            aggregate_operand_order_fusion: false,
            sink_entry_function_declarations: false,
            function_spelling: None,
            struct_method_shorthand: None,
            local_phi_expression_regions: None,
            public_aggregate_abi: PublicAggregateAbi::Named,
            aggregate_layout: AggregateLayout::default(),
            assume_pristine_builtins: false,
            strip_console: true,
            startup: StartupCostConfig::default(),
            performance: JavaScriptPerformanceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaScriptOptimization {
    IrInliningVariants,
    IrClosureFactoryVariants,
    IrPhaseOrderingVariants,
    IrFunctionSubsumptionVariants,
    IrSpecializationVariants,
    StructuralControlFlowVariants,
    SsaDestructionVariants,
    ConditionalExpressionVariants,
    ExpressionPhiRegionVariants,
    LocalPhiExpressionRegionVariants,
    PhiEdgeValueForwardingVariants,
    ConstructorInitializerFusionVariants,
    FreshLiteralFactoryInliningVariants,
    DefaultArgumentVariants,
    CommaExpressionVariants,
    OperandOrderFusionVariants,
    StructuralLoopVariants,
    DoLoopVariants,
    UpdateLoopVariants,
    SwitchLoweringVariants,
    CompoundMutationVariants,
    EntropyCrossScopeReuse,
    EntropyPropertyAssignment,
    ParsedPeephole,
    StartupCostGuard,
    PerformanceShapeModel,
    ProfileGuidedOptimization,
    CallSiteSpecialization,
    CaptureSignatureCloning,
    IdenticalFunctionFolding,
    FunctionLayoutVariants,
    IrCompressPassVariants,
    JointChunkSymbolSearch,
    JointRepresentationSearch,
}

impl JavaScriptOptimization {
    pub const fn name(self) -> &'static str {
        match self {
            Self::IrInliningVariants => "ir-inlining-variants",
            Self::IrClosureFactoryVariants => "ir-closure-factory-variants",
            Self::IrPhaseOrderingVariants => "ir-phase-ordering-variants",
            Self::IrFunctionSubsumptionVariants => "ir-function-subsumption-variants",
            Self::IrSpecializationVariants => "ir-specialization-variants",
            Self::StructuralControlFlowVariants => "structural-control-flow-variants",
            Self::SsaDestructionVariants => "ssa-destruction-variants",
            Self::ConditionalExpressionVariants => "conditional-expression-variants",
            Self::ExpressionPhiRegionVariants => "expression-phi-region-variants",
            Self::LocalPhiExpressionRegionVariants => "local-phi-expression-region-variants",
            Self::PhiEdgeValueForwardingVariants => "phi-edge-value-forwarding-variants",
            Self::ConstructorInitializerFusionVariants => "constructor-initializer-fusion-variants",
            Self::FreshLiteralFactoryInliningVariants => "fresh-literal-factory-inlining-variants",
            Self::DefaultArgumentVariants => "default-argument-variants",
            Self::CommaExpressionVariants => "comma-expression-variants",
            Self::OperandOrderFusionVariants => "operand-order-fusion-variants",
            Self::StructuralLoopVariants => "structural-loop-variants",
            Self::DoLoopVariants => "do-loop-variants",
            Self::UpdateLoopVariants => "update-loop-variants",
            Self::SwitchLoweringVariants => "switch-lowering-variants",
            Self::CompoundMutationVariants => "compound-mutation-variants",
            Self::EntropyCrossScopeReuse => "entropy-cross-scope-reuse",
            Self::EntropyPropertyAssignment => "entropy-property-assignment",
            Self::ParsedPeephole => "parsed-peephole",
            Self::StartupCostGuard => "startup-cost-guard",
            Self::PerformanceShapeModel => "performance-shape-model",
            Self::ProfileGuidedOptimization => "profile-guided-optimization",
            Self::CallSiteSpecialization => "call-site-specialization",
            Self::CaptureSignatureCloning => "capture-signature-cloning",
            Self::IdenticalFunctionFolding => "identical-function-folding",
            Self::FunctionLayoutVariants => "function-layout-variants",
            Self::IrCompressPassVariants => "ir-compress-pass-variants",
            Self::JointChunkSymbolSearch => "joint-chunk-symbol-search",
            Self::JointRepresentationSearch => "joint-representation-search",
        }
    }

    const fn minimum_level(self) -> u8 {
        match self {
            Self::ConditionalExpressionVariants => 4,
            Self::ExpressionPhiRegionVariants => 4,
            Self::LocalPhiExpressionRegionVariants => 4,
            Self::PhiEdgeValueForwardingVariants => 4,
            Self::DefaultArgumentVariants => 7,
            Self::UpdateLoopVariants
            | Self::CompoundMutationVariants
            | Self::ConstructorInitializerFusionVariants
            | Self::FreshLiteralFactoryInliningVariants => 5,
            Self::CommaExpressionVariants | Self::SsaDestructionVariants => 7,
            Self::OperandOrderFusionVariants => 4,
            Self::EntropyCrossScopeReuse | Self::EntropyPropertyAssignment => 8,
            Self::StructuralLoopVariants | Self::ParsedPeephole => 9,
            Self::IrInliningVariants
            | Self::IrSpecializationVariants
            | Self::StructuralControlFlowVariants => 10,
            Self::IrClosureFactoryVariants | Self::DoLoopVariants => 11,
            Self::SwitchLoweringVariants => 12,
            Self::StartupCostGuard => 1,
            Self::PerformanceShapeModel => 3,
            Self::ProfileGuidedOptimization => 10,
            Self::CallSiteSpecialization => 11,
            Self::CaptureSignatureCloning => 12,
            Self::IdenticalFunctionFolding => 13,
            Self::FunctionLayoutVariants => 13,
            Self::JointRepresentationSearch => 13,
            Self::IrFunctionSubsumptionVariants
            | Self::IrPhaseOrderingVariants
            | Self::IrCompressPassVariants
            | Self::JointChunkSymbolSearch => 14,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JavaScriptPerformanceConfig {
    pub deoptimization_weight: u32,
    pub allocation_weight: u32,
    pub indirect_call_weight: u32,
    pub hot_code_weight: u32,
    pub max_regression_percent: u32,
}

impl Default for JavaScriptPerformanceConfig {
    fn default() -> Self {
        Self {
            deoptimization_weight: 32,
            allocation_weight: 12,
            indirect_call_weight: 24,
            hot_code_weight: 1,
            max_regression_percent: 25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptimizationProfileConfig {
    pub path: Option<PathBuf>,
    pub functions: BTreeMap<String, u64>,
    pub loops: BTreeMap<String, u64>,
    pub specialization_min_count: u64,
    pub max_specializations_per_function: usize,
    pub max_clone_instructions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NativeConfig {
    pub partial_escape_analysis: bool,
    pub stack_allocation: bool,
    pub region_allocation: bool,
    pub stack_array_element_limit: usize,
}

impl Default for NativeConfig {
    fn default() -> Self {
        Self {
            partial_escape_analysis: true,
            stack_allocation: true,
            region_allocation: true,
            stack_array_element_limit: 64,
        }
    }
}

impl Default for OptimizationProfileConfig {
    fn default() -> Self {
        Self {
            path: None,
            functions: BTreeMap::new(),
            loops: BTreeMap::new(),
            specialization_min_count: 100,
            max_specializations_per_function: 8,
            max_clone_instructions: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StartupCostConfig {
    pub parse_weight: u32,
    pub compile_weight: u32,
    pub memory_weight: u32,
    pub max_nesting: Option<usize>,
    pub parse_overhead_limit_percent: u32,
    pub compile_overhead_limit_percent: u32,
    pub memory_overhead_limit_percent: u32,
}

impl Default for StartupCostConfig {
    fn default() -> Self {
        Self {
            parse_weight: 1,
            compile_weight: 1,
            memory_weight: 1,
            max_nesting: None,
            parse_overhead_limit_percent: 30,
            compile_overhead_limit_percent: 30,
            memory_overhead_limit_percent: 35,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompressionCostModel {
    Raw,
    Gzip,
    #[default]
    Brotli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateSearch {
    Off,
    #[default]
    Production,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LintSeverity {
    Off,
    Hint,
    #[default]
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LintPreset {
    Minimal,
    #[default]
    Recommended,
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LintConfig {
    pub enabled: bool,
    pub preset: LintPreset,
    pub deny_warnings: bool,
    pub providers: Option<Vec<String>>,
    pub exclude: Vec<String>,
    pub pure_extern_allowlist: Vec<String>,
    pub rules: BTreeMap<String, LintSeverity>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preset: LintPreset::Recommended,
            deny_warnings: false,
            providers: None,
            exclude: Vec::new(),
            pure_extern_allowlist: Vec::new(),
            rules: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NewlineStyle {
    #[default]
    Lf,
    Crlf,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormatConfig {
    pub enabled: bool,
    pub line_width: usize,
    pub newline: NewlineStyle,
    pub organize_imports: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            line_width: 100,
            newline: NewlineStyle::Lf,
            organize_imports: true,
        }
    }
}

impl JavaScriptConfig {
    fn keep_integer_coercions(&self) -> bool {
        self.integer_coercions
            .unwrap_or_else(|| self.priority.keeps_integer_coercions())
    }

    pub fn resolved_ecmascript(&self) -> EcmaScriptEdition {
        resolve_ecmascript_target(self.ecmascript, &self.browsers).unwrap_or(self.ecmascript)
    }

    fn compression_enabled(&self, decision: CompressionDecision) -> bool {
        self.compression.as_ref().map_or_else(
            || self.priority.enables_compression(decision),
            |enabled| enabled.contains(&decision),
        )
    }

    fn search_compression_enabled(&self, decision: CompressionDecision) -> bool {
        self.candidate_search_enabled()
            && match &self.compression {
                None => self.priority.enables_compression(decision),
                Some(enabled) if enabled.is_empty() => false,
                Some(enabled) => {
                    enabled.contains(&decision) || self.priority.enables_compression(decision)
                }
            }
    }

    pub const fn candidate_search_enabled(&self) -> bool {
        !matches!(self.candidate_search, CandidateSearch::Off)
    }

    fn optimization_enabled(
        &self,
        feature: JavaScriptOptimization,
        legacy: Option<CompressionDecision>,
    ) -> bool {
        self.optimizations.as_ref().map_or_else(
            || self.optimization_level >= feature.minimum_level(),
            |features| features.contains(&feature),
        ) && legacy.is_none_or(|decision| self.compression_enabled(decision))
    }

    pub fn effective_candidate_limit(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 1,
            3..=4 => 16,
            5..=6 => 64,
            7..=8 => 192,
            9..=10 => 384,
            11..=12 => 768,
            13..=14 => 1_024,
            _ => usize::MAX,
        };
        let search_limit = match self.candidate_search {
            CandidateSearch::Off => 1,
            CandidateSearch::Production => 384,
            CandidateSearch::Always => usize::MAX,
        };
        self.candidate_limit.min(level_limit).min(search_limit)
    }

    /// The configured byte pool is a ceiling, while the optimization level
    /// supplies a progressively larger default work tier. The configured root
    /// can always exceed this value: the arena raises its effective byte floor
    /// to retain that mandatory incumbent.
    pub fn effective_candidate_byte_budget(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 64 * 1024,
            3..=4 => 128 * 1024,
            5..=6 => 192 * 1024,
            7..=8 => 256 * 1024,
            9..=10 => 384 * 1024,
            11..=12 => 512 * 1024,
            13 => 768 * 1024,
            14 => 896 * 1024,
            _ => usize::MAX,
        };
        let search_limit = match self.candidate_search {
            CandidateSearch::Off => 1,
            CandidateSearch::Production | CandidateSearch::Always => usize::MAX,
        };
        self.candidate_byte_budget
            .min(level_limit)
            .min(search_limit)
    }

    /// Beam width participates in the effort ladder too. Previously every
    /// nonzero level inherited the level-15 width of twelve even when its
    /// candidate cap was intentionally small.
    pub fn effective_candidate_beam_width(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 1,
            3..=4 => 2,
            5..=6 => 3,
            7..=8 => 4,
            9..=10 => 6,
            11..=12 => 8,
            13 => 10,
            14 => 11,
            _ => usize::MAX,
        };
        self.candidate_beam_width
            .min(level_limit)
            .min(self.effective_candidate_limit())
            .max(1)
    }

    /// Hard ceiling for optional structural whole-artifact proposals after
    /// the already-scored IR context seeds have been installed. Survivor and
    /// byte limits cannot provide this guarantee: hundreds of rejected plans
    /// may be emitted before a small survivor frontier is chosen.
    /// The ceiling an explicitly configured proposal budget may not exceed.
    /// The optimization level sets the *default* breadth, so an explicit budget
    /// is allowed past it; the search tier is a different thing and stays hard.
    fn candidate_proposal_tier_ceiling(&self) -> usize {
        match self.candidate_search {
            CandidateSearch::Off => 0,
            CandidateSearch::Production => 384,
            CandidateSearch::Always => usize::MAX,
        }
    }

    fn candidate_proposal_level_limit(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=2 => 0,
            3..=4 => 16,
            5..=6 => 64,
            7..=8 => 192,
            9..=10 => 384,
            11..=12 => 768,
            13..=14 => 1_024,
            _ => 1_536,
        };
        match self.candidate_search {
            CandidateSearch::Off => 0,
            CandidateSearch::Production => level_limit.min(384),
            CandidateSearch::Always => level_limit,
        }
    }

    pub fn effective_candidate_proposal_limit(&self) -> usize {
        let level_limit = self.candidate_proposal_level_limit();
        // A level that turns the search off turns it off for everyone; an
        // explicit budget widens a search that is running, it does not start one.
        if level_limit == 0 {
            return 0;
        }
        self.candidate_proposal_limit.map_or_else(
            || self.effective_candidate_limit().min(level_limit),
            |configured| configured.min(self.candidate_proposal_tier_ceiling()),
        )
    }

    /// Default proposal work scales down for broad artifacts because every
    /// admitted identity can require a complete IR-to-JavaScript emission and
    /// selected-model score. Without an explicit proposal limit, the retained
    /// candidate limit is an additional ceiling. An explicit value can exceed
    /// the survivor count and bypasses artifact scaling, but remains bounded
    /// by the level and search tier.
    pub fn effective_candidate_proposal_limit_for_artifact(&self, raw_size: usize) -> usize {
        let level_limit = self.candidate_proposal_level_limit();
        if level_limit == 0 {
            return 0;
        }
        let artifact_limit = match raw_size {
            0..=16_384 => level_limit,
            16_385..=65_536 => level_limit.div_ceil(4),
            _ => level_limit.div_ceil(12),
        };
        // An explicit proposal budget is a request for a wider search and is
        // honored past the level's default breadth, the same way
        // `terminal_codec_probe_limit` is. Clamping it to the level tier meant a
        // config could ask for four times the proposals and silently receive
        // none of them. The search tier is a separate ceiling and stays hard.
        self.candidate_proposal_limit.map_or_else(
            || self.effective_candidate_limit().min(artifact_limit),
            |configured| configured.min(self.candidate_proposal_tier_ceiling()),
        )
    }

    /// Hard ceiling for optional whole-artifact work after structural
    /// candidates have been ranked. This is deliberately independent of
    /// survivor count: one large survivor can expose thousands of proposals.
    fn terminal_codec_probe_level_limit(&self) -> usize {
        let level_limit = match self.optimization_level {
            0..=7 => 0,
            8 => 24,
            9..=10 => 64,
            11..=12 => 128,
            13 => 192,
            14 => 256,
            _ => 384,
        };
        match self.candidate_search {
            CandidateSearch::Off => 0,
            CandidateSearch::Production | CandidateSearch::Always => level_limit,
        }
    }

    pub fn effective_terminal_codec_probe_limit(&self) -> usize {
        let level_limit = self.terminal_codec_probe_level_limit();
        if level_limit == 0 {
            return 0;
        }
        // An explicit limit is a request for more verification, and it is
        // honored. Measured on jQuery: raising the ceiling from 384 to the
        // configured 1536 is 33 Brotli bytes for 24% more compile time, because
        // terminal search on an 84KB artifact is budget-limited rather than
        // idea-limited. Silently clamping to the level meant a config could ask
        // for four times the search and receive none of it.
        self.terminal_codec_probe_limit.unwrap_or(level_limit)
    }

    /// Default terminal work scales down for broad artifacts because every
    /// syntax validation and Brotli-11 call is whole-artifact work. An explicit
    /// value bypasses that scaling and sets the ceiling itself; the search tier
    /// still gates it to zero when candidate search is off.
    pub fn effective_terminal_codec_probe_limit_for_artifact(&self, raw_size: usize) -> usize {
        let level_limit = self.terminal_codec_probe_level_limit();
        if level_limit == 0 {
            return 0;
        }
        let artifact_limit = match raw_size {
            0..=16_384 => level_limit,
            16_385..=65_536 => level_limit.div_ceil(4),
            _ => level_limit.div_ceil(12),
        };
        self.terminal_codec_probe_limit.unwrap_or(artifact_limit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OptimizationPreset {
    None,
    #[default]
    Maximum,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptimizationConfig {
    pub preset: OptimizationPreset,
    pub constant_folding: Option<bool>,
    pub algebraic_simplification: Option<bool>,
    pub common_subexpression_elimination: Option<bool>,
    pub finite_value_propagation: Option<bool>,
    pub global_optimization: Option<bool>,
    pub inlining: Option<bool>,
    pub inline_closure_factories: Option<bool>,
    pub constant_parameter_specialization: Option<bool>,
    pub specialize_tagged_constants: Option<bool>,
    pub scalar_replacement: Option<bool>,
    pub dead_store_elimination: Option<bool>,
    pub dead_code_elimination: Option<bool>,
    pub call_site_specialization: Option<bool>,
    pub capture_signature_cloning: Option<bool>,
    pub identical_function_folding: Option<bool>,
    pub function_subsumption: Option<bool>,
    pub pipeline_fusion: Option<bool>,
    pub partial_escape_sinking: Option<bool>,
    pub region_outlining: Option<bool>,
    pub expression_superopt: Option<bool>,
    pub path_sensitive_propagation: Option<bool>,
    pub parameterized_function_merging: Option<bool>,
    pub profile_guided: Option<bool>,
    pub for_of_specialize_family: Option<usize>,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            preset: OptimizationPreset::Maximum,
            constant_folding: None,
            algebraic_simplification: None,
            common_subexpression_elimination: None,
            finite_value_propagation: None,
            global_optimization: None,
            inlining: None,
            inline_closure_factories: None,
            constant_parameter_specialization: None,
            specialize_tagged_constants: None,
            scalar_replacement: None,
            dead_store_elimination: None,
            dead_code_elimination: None,
            call_site_specialization: None,
            capture_signature_cloning: None,
            identical_function_folding: None,
            function_subsumption: None,
            pipeline_fusion: None,
            partial_escape_sinking: None,
            region_outlining: None,
            expression_superopt: None,
            path_sensitive_propagation: None,
            parameterized_function_merging: None,
            profile_guided: None,
            for_of_specialize_family: None,
        }
    }
}

impl OptimizationConfig {
    pub fn for_of_specialize_family(&self) -> usize {
        self.for_of_specialize_family.unwrap_or(0)
    }

    pub fn resolve(&self) -> OptimizationOptions {
        let base = match self.preset {
            OptimizationPreset::None => OptimizationOptions::disabled(),
            OptimizationPreset::Maximum => OptimizationOptions::default(),
        };
        OptimizationOptions {
            constant_folding: self.constant_folding.unwrap_or(base.constant_folding),
            algebraic_simplification: self
                .algebraic_simplification
                .unwrap_or(base.algebraic_simplification),
            common_subexpression_elimination: self
                .common_subexpression_elimination
                .unwrap_or(base.common_subexpression_elimination),
            finite_value_propagation: self
                .finite_value_propagation
                .unwrap_or(base.finite_value_propagation),
            global_optimization: self.global_optimization.unwrap_or(base.global_optimization),
            inlining: self.inlining.unwrap_or(base.inlining),
            inline_closure_factories: self
                .inline_closure_factories
                .unwrap_or(base.inline_closure_factories),
            scalar_replacement: self.scalar_replacement.unwrap_or(base.scalar_replacement),
            dead_store_elimination: self
                .dead_store_elimination
                .unwrap_or(base.dead_store_elimination),
            dead_code_elimination: self
                .dead_code_elimination
                .unwrap_or(base.dead_code_elimination),
            constant_parameter_specialization: self
                .constant_parameter_specialization
                .unwrap_or(base.constant_parameter_specialization),
            specialize_tagged_constants: self
                .specialize_tagged_constants
                .unwrap_or(base.specialize_tagged_constants),
            call_site_specialization: self
                .call_site_specialization
                .unwrap_or(base.call_site_specialization),
            capture_signature_cloning: self
                .capture_signature_cloning
                .unwrap_or(base.capture_signature_cloning),
            identical_function_folding: self
                .identical_function_folding
                .unwrap_or(base.identical_function_folding),
            function_subsumption: self
                .function_subsumption
                .unwrap_or(base.function_subsumption),
            pipeline_fusion: self.pipeline_fusion.unwrap_or(base.pipeline_fusion),
            partial_escape_sinking: self
                .partial_escape_sinking
                .unwrap_or(base.partial_escape_sinking),
            region_outlining: self.region_outlining.unwrap_or(base.region_outlining),
            expression_superopt: self.expression_superopt.unwrap_or(base.expression_superopt),
            path_sensitive_propagation: self
                .path_sensitive_propagation
                .unwrap_or(base.path_sensitive_propagation),
            parameterized_function_merging: self
                .parameterized_function_merging
                .unwrap_or(base.parameterized_function_merging),
            inline_instruction_limit: base.inline_instruction_limit,
            inline_control_flow_limit: base.inline_control_flow_limit,
            inline_growth_limit: base.inline_growth_limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MangleConfig {
    pub identifiers: Option<bool>,
    pub properties: Option<bool>,
    pub exports: Option<bool>,
    pub extern_fields: Option<bool>,
    pub pool_strings: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundleMode {
    #[default]
    Single,
    Split,
    PreserveModules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreloadPolicy {
    #[default]
    None,
    Entry,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChunkCostConfig {
    pub raw_weight: u32,
    pub gzip_weight: u32,
    pub brotli_weight: u32,
    pub request_overhead_bytes: usize,
    pub dependency_depth_penalty_bytes: usize,
    pub preload_request_discount_percent: u32,
    pub cache_reuse_discount_percent: u32,
}

impl Default for ChunkCostConfig {
    fn default() -> Self {
        Self {
            raw_weight: 0,
            gzip_weight: 1,
            brotli_weight: 2,
            request_overhead_bytes: 1_000,
            dependency_depth_penalty_bytes: 160,
            preload_request_discount_percent: 70,
            cache_reuse_discount_percent: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BundleConfig {
    pub mode: BundleMode,
    pub min_chunk_bytes: usize,
    pub max_chunks: usize,
    pub shared_min_imports: usize,
    pub preload: PreloadPolicy,
    pub cost: ChunkCostConfig,
}

impl Default for BundleConfig {
    fn default() -> Self {
        Self {
            mode: BundleMode::Single,
            min_chunk_bytes: 16 * 1024,
            max_chunks: 32,
            shared_min_imports: 2,
            preload: PreloadPolicy::None,
            cost: ChunkCostConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: ProjectConfig,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ConfigError {}

pub fn load_project_config(
    input: &Path,
    explicit: Option<&Path>,
) -> Result<LoadedConfig, ConfigError> {
    let path = explicit.map(Path::to_path_buf).or_else(|| discover(input));
    let Some(path) = path else {
        return Ok(LoadedConfig {
            config: ProjectConfig::default(),
            path: None,
        });
    };
    let source = fs::read_to_string(&path).map_err(|error| ConfigError {
        path: path.clone(),
        message: format!("failed to read config: {error}"),
    })?;
    let mut config = toml::from_str::<ProjectConfig>(&source).map_err(|error| ConfigError {
        path: path.clone(),
        message: format!("invalid config: {error}"),
    })?;
    config.config_dir = path
        .parent()
        .and_then(|directory| directory.canonicalize().ok());
    config.validate().map_err(|message| ConfigError {
        path: path.clone(),
        message,
    })?;
    Ok(LoadedConfig {
        config,
        path: Some(path),
    })
}

fn discover(input: &Path) -> Option<PathBuf> {
    let start = if input.is_dir() {
        input
    } else {
        input.parent().unwrap_or_else(|| Path::new("."))
    };
    let mut directory = start.canonicalize().ok()?;
    loop {
        let candidate = directory.join("lilscript.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !directory.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_compiler_resource_limits() {
        let defaults = ProjectConfig::default();
        assert_eq!(defaults.compiler.resources.threads, None);
        assert_eq!(defaults.compiler.resources.codec_workers.get(), 4);

        let configured: ProjectConfig =
            toml::from_str("[compiler.resources]\nthreads=12\ncodec_workers=8\n").unwrap();
        assert_eq!(configured.compiler.resources.threads.unwrap().get(), 12);
        assert_eq!(configured.compiler.resources.codec_workers.get(), 8);
        assert_eq!(configured.compiler.resources.effective_codec_workers(6), 6);
        assert_eq!(configured.compiler.resources.effective_codec_workers(16), 8);

        assert!(toml::from_str::<ProjectConfig>(
            "[compiler.resources]\nthreads=0\ncodec_workers=4\n"
        )
        .is_err());
        assert!(
            toml::from_str::<ProjectConfig>("[compiler.resources]\ncodec_workers=0\n").is_err()
        );
    }

    #[test]
    fn resolves_presets_and_fine_grained_overrides() {
        let config: ProjectConfig = toml::from_str(
            r#"
[optimization]
preset = "none"
constant_folding = true
finite_value_propagation = true
inline_closure_factories = true

[javascript]
priority = "size-first"

[mangle]
identifiers = false
properties = true
exports = true

[bundle]
mode = "split"
min_chunk_bytes = 4096
max_chunks = 8
shared_min_imports = 3
"#,
        )
        .unwrap();
        let optimizer = config.optimizer_options();
        assert!(optimizer.constant_folding);
        assert!(optimizer.finite_value_propagation);
        assert!(optimizer.inline_closure_factories);
        assert!(!optimizer.inlining);
        assert_eq!(config.javascript.priority, JavaScriptPriority::SizeFirst);
        assert!(config.javascript.strip_console);
        assert!(!config.js_options().mangle_identifiers);
        assert!(config.js_options().mangle_properties);
        assert_eq!(config.bundle.mode, BundleMode::Split);
        assert_eq!(config.bundle.min_chunk_bytes, 4096);
        config.validate().unwrap();
    }

    #[test]
    fn parses_javascript_strip_console() {
        let enabled: ProjectConfig = toml::from_str("[javascript]\nstrip_console=true\n").unwrap();
        assert!(enabled.javascript.strip_console);
        let disabled: ProjectConfig =
            toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
        assert!(!disabled.javascript.strip_console);
        assert!(ProjectConfig::default().javascript.strip_console);
    }

    #[test]
    fn regex_literals_require_an_explicit_pristine_builtin_contract() {
        let open_world: ProjectConfig =
            toml::from_str("[javascript]\npriority='size-first'\ncompression=['regex-literals']\n")
                .unwrap();
        assert!(!open_world.js_options().regex_literals);

        let pristine: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['regex-literals']\nassume_pristine_builtins=true\n",
        )
        .unwrap();
        assert!(pristine.js_options().regex_literals);
        assert!(!ProjectConfig::default().javascript.assume_pristine_builtins);
    }

    #[test]
    fn maps_javascript_priorities_to_concrete_policies() {
        let performance: ProjectConfig =
            toml::from_str("[javascript]\npriority='performance-first'\n").unwrap();
        let performance_optimizer = performance.js_optimizer_options();
        assert_eq!(performance_optimizer.inline_instruction_limit, 24);
        assert_eq!(performance_optimizer.inline_control_flow_limit, 60);
        assert_eq!(performance_optimizer.inline_growth_limit, None);
        assert!(!performance.js_options().pool_strings);
        assert!(!performance.js_options().elide_safe_integer_coercions);
        assert!(!performance.js_options().elide_length_tonumber);
        assert!(
            ProjectConfig::default()
                .js_options()
                .elide_safe_integer_coercions
        );

        let realistic: ProjectConfig =
            toml::from_str("[javascript]\npriority='realistic-performance-first'\n").unwrap();
        let realistic_optimizer = realistic.js_optimizer_options();
        assert_eq!(
            realistic.javascript.priority,
            JavaScriptPriority::RealisticPerformanceFirst
        );
        assert_eq!(realistic_optimizer.inline_instruction_limit, 18);
        assert_eq!(realistic_optimizer.inline_control_flow_limit, 45);
        assert_eq!(realistic_optimizer.inline_growth_limit, Some(16));
        assert!(realistic.js_options().mangle_identifiers);
        assert!(realistic.entropy_aware_mangling_enabled());
        assert!(realistic.js_options().pool_strings);
        assert!(!realistic.js_options().elide_safe_integer_coercions);
        assert!(realistic.js_options().compact_boolean_literals);
        assert!(!realistic.js_options().pack_string_arrays);
        assert_eq!(realistic.javascript.candidate_limit, 1536);
        assert_eq!(
            realistic.js_options().phi_affinity_mode,
            PhiAffinityMode::Grouped
        );

        let alias: ProjectConfig =
            toml::from_str("[javascript]\npriority='realisticperf-first'\n").unwrap();
        assert_eq!(
            alias.javascript.priority,
            JavaScriptPriority::RealisticPerformanceFirst
        );

        let balanced: ProjectConfig =
            toml::from_str("[javascript]\npriority='balanced'\n").unwrap();
        let balanced_optimizer = balanced.js_optimizer_options();
        assert_eq!(balanced_optimizer.inline_instruction_limit, 12);
        assert_eq!(balanced_optimizer.inline_control_flow_limit, 30);
        assert_eq!(balanced_optimizer.inline_growth_limit, Some(4));
        assert!(balanced.js_options().pool_strings);
        assert!(balanced.js_options().elide_safe_integer_coercions);
        assert!(!balanced.js_options().pack_string_arrays);

        let size: ProjectConfig = toml::from_str("[javascript]\npriority='size-first'\n").unwrap();
        assert_eq!(size.js_optimizer_options().inline_growth_limit, Some(16));
        assert!(size.js_options().pool_strings);
        assert!(size.js_options().elide_safe_integer_coercions);
        assert!(size.js_options().elide_length_tonumber);
        assert!(!balanced.js_options().elide_length_tonumber);
        assert!(size.js_options().inline_structured_closures);
        assert!(!size.js_options().pack_string_arrays);
        assert_eq!(size.js_options().string_pool_minimum_savings, 8);
        assert!(!size.js_options().pool_identifier_strings);
        assert!(size.js_options().scalar_phi_copies);
        assert!(size.js_options().mangle_properties);
        assert!(!size.js_options().mangle_exports);
        assert!(size.ir_inlining_variants_enabled());
        assert!(size.pure_helper_inlining_candidates_enabled());
        assert!(size.dense_string_return_table_candidates_enabled());
        assert!(size.host_alias_spelling_candidates_enabled());
        assert!(size.ir_closure_factory_variants_enabled());
        assert!(size.ir_phase_ordering_variants_enabled());
        assert!(size.loop_spelling_selection_enabled());
        assert!(size.mutation_spelling_selection_enabled());
        assert!(size.indexed_char_at_candidates_enabled());
        assert!(!size.effect_ternary_candidates_enabled());
        assert!(size.js_joint_chunk_symbol_search_enabled());
        assert!(size.js_joint_representation_search_enabled());
        assert!(size.js_parameterized_function_merging_enabled());
        let size_passes = size.compress_pass_options();
        assert!(size_passes.pipeline_fusion);
        assert!(size_passes.partial_escape_sinking);
        assert!(!size_passes.region_outlining);
        assert!(size_passes.expression_superopt);
        assert!(size_passes.path_sensitive_propagation);
        assert_eq!(
            size.js_options().phi_affinity_mode,
            PhiAffinityMode::Grouped
        );
        assert_eq!(
            ProjectConfig::default().javascript.priority,
            JavaScriptPriority::SizeFirst
        );

        assert!(!performance.entropy_aware_mangling_enabled());
        assert!(!performance.js_options().compact_boolean_literals);
        assert!(performance.js_options().elide_block_terminal_semicolons);
        assert!(!performance.js_options().mangle_properties);
        assert!(!performance.ir_inlining_variants_enabled());
        assert!(!performance.pure_helper_inlining_candidates_enabled());
        assert!(!performance.dense_string_return_table_candidates_enabled());
        assert!(!performance.host_alias_spelling_candidates_enabled());
        assert!(!performance.ir_closure_factory_variants_enabled());
        assert!(!performance.ir_phase_ordering_variants_enabled());
        assert!(!performance.loop_spelling_selection_enabled());
        assert!(!performance.mutation_spelling_selection_enabled());
        assert!(!performance.indexed_char_at_candidates_enabled());
        assert!(!performance.effect_ternary_candidates_enabled());
        assert!(!performance.js_joint_chunk_symbol_search_enabled());
        assert!(!performance.js_joint_representation_search_enabled());
        assert!(!performance.js_parameterized_function_merging_enabled());
        let performance_passes = performance.compress_pass_options();
        assert!(!performance_passes.pipeline_fusion);
        assert!(!performance_passes.partial_escape_sinking);
        assert!(!performance_passes.region_outlining);
        assert!(!performance_passes.expression_superopt);
        assert!(!performance_passes.path_sensitive_propagation);

        assert!(!balanced.js_options().mangle_properties);
        assert!(balanced.loop_spelling_selection_enabled());
        assert!(!balanced.mutation_spelling_selection_enabled());
        assert!(!balanced.indexed_char_at_candidates_enabled());
        assert!(!balanced.effect_ternary_candidates_enabled());
        let balanced_passes = balanced.compress_pass_options();
        assert!(!balanced_passes.pipeline_fusion);
        assert!(!balanced_passes.partial_escape_sinking);
        assert!(!balanced_passes.region_outlining);
        assert!(balanced_passes.expression_superopt);
        assert!(balanced_passes.path_sensitive_propagation);
        assert!(!balanced.js_joint_chunk_symbol_search_enabled());
        assert!(!balanced.js_joint_representation_search_enabled());
        assert!(!balanced.js_parameterized_function_merging_enabled());

        let explicit_pooling: ProjectConfig = toml::from_str(
            "[javascript]\npriority='performance-first'\n[mangle]\npool_strings=true\n",
        )
        .unwrap();
        assert!(explicit_pooling.js_options().pool_strings);

        let keep_coercions: ProjectConfig =
            toml::from_str("[javascript]\npriority='size-first'\ninteger_coercions=true\n")
                .unwrap();
        assert!(!keep_coercions.js_options().elide_safe_integer_coercions);

        let keep_balanced: ProjectConfig =
            toml::from_str("[javascript]\npriority='balanced'\ninteger_coercions=true\n").unwrap();
        assert!(!keep_balanced.js_options().elide_safe_integer_coercions);

        let drop_on_performance: ProjectConfig =
            toml::from_str("[javascript]\npriority='performance-first'\ninteger_coercions=false\n")
                .unwrap();
        assert!(
            drop_on_performance
                .js_options()
                .elide_safe_integer_coercions
        );
    }

    #[test]
    fn applies_an_exact_custom_compression_decision_set() {
        let custom: ProjectConfig = toml::from_str(
            r#"
[javascript]
priority = "performance-first"
compression = [
  "string-pooling",
  "size-aware-inlining",
  "property-mangling",
]
inline_instruction_limit = 7
inline_control_flow_limit = 9
max_inline_growth = 3
local_name_coalescing = false
"#,
        )
        .unwrap();
        custom.validate().unwrap();
        let optimizer = custom.js_optimizer_options();
        let codegen = custom.js_options();

        assert_eq!(optimizer.inline_instruction_limit, 7);
        assert_eq!(optimizer.inline_control_flow_limit, 9);
        assert_eq!(optimizer.inline_growth_limit, Some(3));
        assert!(!codegen.mangle_identifiers);
        assert!(codegen.mangle_properties);
        assert!(!codegen.mangle_exports);
        assert!(codegen.pool_strings);
        assert!(!codegen.elide_safe_integer_coercions);
        assert!(!codegen.elide_block_terminal_semicolons);
        assert!(!codegen.elide_new_parentheses);
        assert!(!codegen.elide_call_chain_parentheses);
        assert!(!codegen.compact_generator_star);
        assert!(!codegen.local_name_coalescing);
        assert!(!custom.ir_inlining_variants_enabled());
        assert!(!custom.ir_closure_factory_variants_enabled());
        assert!(!custom.ir_phase_ordering_variants_enabled());
        assert!(!custom.loop_spelling_selection_enabled());
        assert!(!custom.mutation_spelling_selection_enabled());
        assert!(!custom.indexed_char_at_candidates_enabled());
        assert!(!custom.effect_ternary_candidates_enabled());

        let size_overlay: ProjectConfig = toml::from_str(
            "[javascript]\npriority='size-first'\ncompression=['identifier-mangling']\n",
        )
        .unwrap();
        assert!(size_overlay.indexed_char_at_candidates_enabled());
        assert!(!size_overlay.js_options().indexed_char_at);
        assert!(!size_overlay.effect_ternary_candidates_enabled());

        let none: ProjectConfig = toml::from_str("[javascript]\ncompression=[]\n").unwrap();
        let none_codegen = none.js_options();
        assert_eq!(none.js_optimizer_options().inline_growth_limit, None);
        assert!(!none_codegen.mangle_identifiers);
        assert!(!none_codegen.mangle_properties);
        assert!(!none_codegen.mangle_exports);
        assert!(!none_codegen.pool_strings);
        assert!(none_codegen.elide_safe_integer_coercions);
        assert!(!none_codegen.compact_boolean_literals);
        assert!(!none_codegen.elide_block_terminal_semicolons);
        assert!(!none_codegen.elide_new_parentheses);
        assert!(!none_codegen.elide_call_chain_parentheses);
        assert!(!none_codegen.pack_string_arrays);
        assert!(!none_codegen.compact_generator_star);
        assert!(!none_codegen.scalar_phi_copies);
        assert!(!none.indexed_char_at_candidates_enabled());
        assert!(!none.effect_ternary_candidates_enabled());
        assert!(!none.js_options().indexed_char_at);
        assert!(none.js_options().effect_ternary);
        assert_eq!(
            none_codegen.phi_affinity_mode,
            PhiAffinityMode::Conservative
        );
        assert!(!none.entropy_aware_mangling_enabled());
        assert!(!none.js_region_outlining_candidate_enabled());
        assert!(ProjectConfig::default().js_region_outlining_candidate_enabled());

        let hard_disabled_outlining: ProjectConfig = toml::from_str(
            "[optimization]\nregion_outlining=false\n[javascript]\ncompression=['region-outlining']\n",
        )
        .unwrap();
        assert!(!hard_disabled_outlining.js_region_outlining_candidate_enabled());

        let explicit_mangle: ProjectConfig = toml::from_str(
            "[javascript]\ncompression=[]\n[mangle]\nidentifiers=true\npool_strings=true\n",
        )
        .unwrap();
        assert!(explicit_mangle.js_options().mangle_identifiers);
        assert!(explicit_mangle.js_options().pool_strings);
        assert!(explicit_mangle.js_options().mangle_extern_fields);

        let closed: ProjectConfig = toml::from_str("[mangle]\nextern_fields=false\n").unwrap();
        assert!(!closed.js_options().mangle_extern_fields);
        assert!(ProjectConfig::default().js_options().mangle_extern_fields);
    }

    #[test]
    fn resolves_javascript_ecmascript_and_browser_floors() {
        let defaults = ProjectConfig::default();
        assert_eq!(defaults.javascript.ecmascript, EcmaScriptEdition::Es2022);
        assert_eq!(defaults.js_options().ecmascript, EcmaScriptEdition::Es2022);
        assert!(!defaults.js_options().indexed_char_at);
        assert!(defaults.js_options().effect_ternary);
        assert!(defaults.indexed_char_at_candidates_enabled());
        assert!(!defaults.effect_ternary_candidates_enabled());

        assert!(toml::from_str::<ProjectConfig>("[javascript]\necmascript='es2014'\n").is_err());

        let unknown_browser: ProjectConfig =
            toml::from_str("[javascript]\nbrowsers=['opera80']\n").unwrap();
        assert!(unknown_browser.validate().is_err());

        let intersected: ProjectConfig = toml::from_str(
            "[javascript]\necmascript='es2022'\nbrowsers=['chrome80','firefox78']\n",
        )
        .unwrap();
        intersected.validate().unwrap();
        assert_eq!(
            intersected.javascript.resolved_ecmascript(),
            EcmaScriptEdition::Es2020
        );
        assert_eq!(
            intersected.js_options().ecmascript,
            EcmaScriptEdition::Es2020
        );

        let balanced_listed: ProjectConfig = toml::from_str(
            "[javascript]\npriority='balanced'\ncompression=['indexed-char-at','effect-ternary']\n",
        )
        .unwrap();
        assert!(balanced_listed.indexed_char_at_candidates_enabled());
        assert!(balanced_listed.effect_ternary_candidates_enabled());
        assert!(!balanced_listed.js_options().indexed_char_at);
        assert!(balanced_listed.js_options().effect_ternary);

        let omitted_balanced: ProjectConfig =
            toml::from_str("[javascript]\npriority='balanced'\n").unwrap();
        assert_eq!(
            omitted_balanced.js_options().effect_ternary,
            balanced_listed.js_options().effect_ternary
        );
        assert_eq!(
            omitted_balanced.js_options().indexed_char_at,
            balanced_listed.js_options().indexed_char_at
        );
    }

    #[test]
    fn paired_variant_searches_require_both_exact_allowlists() {
        fn states(config: &ProjectConfig) -> [bool; 11] {
            [
                config.ir_inlining_variants_enabled(),
                config.ir_closure_factory_variants_enabled(),
                config.ir_phase_ordering_variants_enabled(),
                config.loop_spelling_selection_enabled(),
                config.mutation_spelling_selection_enabled(),
                config.js_joint_chunk_symbol_search_enabled(),
                config.js_joint_representation_search_enabled(),
                config.js_default_argument_variants_enabled(),
                config.js_scalar_phi_copy_variants_enabled(),
                config.js_phi_affinity_variants_enabled(),
                config.js_local_name_coalescing_variants_enabled(),
            ]
        }

        let all_optimizations = vec![
            JavaScriptOptimization::IrInliningVariants,
            JavaScriptOptimization::IrClosureFactoryVariants,
            JavaScriptOptimization::IrPhaseOrderingVariants,
            JavaScriptOptimization::StructuralLoopVariants,
            JavaScriptOptimization::CompoundMutationVariants,
            JavaScriptOptimization::JointChunkSymbolSearch,
            JavaScriptOptimization::JointRepresentationSearch,
            JavaScriptOptimization::DefaultArgumentVariants,
            JavaScriptOptimization::SsaDestructionVariants,
        ];
        let all_compression = vec![
            CompressionDecision::IrInliningVariants,
            CompressionDecision::IrClosureFactoryVariants,
            CompressionDecision::IrPhaseOrderingVariants,
            CompressionDecision::LoopSpellingSelection,
            CompressionDecision::MutationSpellingSelection,
            CompressionDecision::JointChunkSymbolSearch,
            CompressionDecision::JointRepresentationSearch,
            CompressionDecision::CalleeDefaultArguments,
            CompressionDecision::ScalarPhiCopies,
            CompressionDecision::PhiAffinityCoalescing,
        ];

        let mut enabled = ProjectConfig::default();
        enabled.javascript.optimizations = Some(all_optimizations.clone());
        enabled.javascript.compression = Some(all_compression.clone());
        assert_eq!(states(&enabled), [true; 11]);

        let mut no_compression = enabled.clone();
        no_compression.javascript.compression = Some(Vec::new());
        assert_eq!(states(&no_compression), [false; 11]);

        let mut no_optimizations = enabled.clone();
        no_optimizations.javascript.optimizations = Some(Vec::new());
        assert_eq!(states(&no_optimizations), [false; 11]);

        let mut exact_empty = ProjectConfig::default();
        exact_empty.javascript.optimizations = Some(Vec::new());
        exact_empty.javascript.compression = Some(Vec::new());
        assert_eq!(states(&exact_empty), [false; 11]);

        let mut mixed = ProjectConfig::default();
        mixed.javascript.optimizations = Some(vec![
            JavaScriptOptimization::IrInliningVariants,
            JavaScriptOptimization::DefaultArgumentVariants,
            JavaScriptOptimization::SsaDestructionVariants,
        ]);
        mixed.javascript.compression = Some(vec![
            CompressionDecision::IrClosureFactoryVariants,
            CompressionDecision::CalleeDefaultArguments,
            CompressionDecision::ScalarPhiCopies,
        ]);
        assert_eq!(
            states(&mixed),
            [false, false, false, false, false, false, false, true, true, false, false,]
        );

        let mut legacy = ProjectConfig::default();
        legacy.javascript.optimizations = None;
        legacy.javascript.compression = Some(all_compression);
        assert_eq!(states(&legacy), [true; 11]);
    }

    #[test]
    fn gates_helper_and_dense_table_search_with_independent_decisions() {
        let helper_only: ProjectConfig =
            toml::from_str("[javascript]\ncompression=['pure-helper-inlining']\n").unwrap();
        assert!(helper_only.pure_helper_inlining_candidates_enabled());
        assert!(!helper_only.dense_string_return_table_candidates_enabled());
        assert!(!helper_only.single_use_function_expression_candidates_enabled());

        let table_only: ProjectConfig =
            toml::from_str("[javascript]\ncompression=['dense-string-return-tables']\n").unwrap();
        assert!(!table_only.pure_helper_inlining_candidates_enabled());
        assert!(table_only.dense_string_return_table_candidates_enabled());
        assert!(!table_only.single_use_function_expression_candidates_enabled());

        let legacy_closure_only: ProjectConfig =
            toml::from_str("[javascript]\ncompression=['structured-closure-inlining']\n").unwrap();
        assert!(!legacy_closure_only.pure_helper_inlining_candidates_enabled());
        assert!(!legacy_closure_only.dense_string_return_table_candidates_enabled());
        assert!(legacy_closure_only.single_use_function_expression_candidates_enabled());
    }

    #[test]
    fn gates_host_alias_spelling_with_search_and_the_exact_compression_set() {
        let enabled: ProjectConfig =
            toml::from_str("[javascript]\ncompression=['host-alias-spelling']\n").unwrap();
        assert!(enabled.host_alias_spelling_candidates_enabled());
        assert_eq!(
            enabled.js_options().host_alias_spelling,
            HostAliasSpelling::Shared
        );

        let exact_empty: ProjectConfig = toml::from_str("[javascript]\ncompression=[]\n").unwrap();
        assert!(!exact_empty.host_alias_spelling_candidates_enabled());

        let search_off: ProjectConfig = toml::from_str(
            "[javascript]\ncandidate_search='off'\ncompression=['host-alias-spelling']\n",
        )
        .unwrap();
        assert!(!search_off.host_alias_spelling_candidates_enabled());

        let explicit_override: ProjectConfig = toml::from_str(
            "[javascript]\npriority='performance-first'\ncompression=['host-alias-spelling']\n",
        )
        .unwrap();
        assert!(explicit_override.host_alias_spelling_candidates_enabled());
    }

    #[test]
    fn resolves_javascript_optimization_levels_and_exact_allowlists() {
        let disabled: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=0\ncandidate_limit=1536\n").unwrap();
        disabled.validate().unwrap();
        assert_eq!(disabled.javascript.candidate_beam_width, 12);
        assert_eq!(disabled.javascript.candidate_byte_budget, 1024 * 1024);
        assert_eq!(disabled.javascript.candidate_proposal_limit, None);
        assert_eq!(disabled.javascript.terminal_codec_probe_limit, None);
        assert_eq!(disabled.javascript.max_candidate_raw_growth_percent, 0);
        assert_eq!(disabled.javascript.function_layout_exact_limit, 13);
        assert_eq!(disabled.javascript.local_name_reserve, 16);
        assert!(disabled.javascript.stable_local_names);
        assert_eq!(disabled.javascript.effective_candidate_limit(), 1);
        assert_eq!(disabled.javascript.effective_candidate_beam_width(), 1);
        assert_eq!(
            disabled.javascript.effective_candidate_byte_budget(),
            64 * 1024
        );
        assert_eq!(
            disabled.javascript.effective_terminal_codec_probe_limit(),
            0
        );
        assert_eq!(disabled.javascript.effective_candidate_proposal_limit(), 0);
        assert!(!disabled.javascript_optimization_configured(
            JavaScriptOptimization::ConditionalExpressionVariants
        ));
        assert!(
            !disabled.javascript_optimization_configured(JavaScriptOptimization::StartupCostGuard)
        );
        assert!(!disabled.js_options().conditional_expressions);
        assert!(!disabled.js_options().expression_phi_regions);
        assert!(!disabled.js_options().local_phi_expression_regions);
        assert!(!disabled.js_options().phi_edge_value_forwarding);
        assert!(!disabled.js_options().constructor_initializer_fusion);
        assert!(!disabled.js_options().inline_fresh_empty_array_factories);
        assert!(!disabled.javascript_optimization_configured(
            JavaScriptOptimization::ConstructorInitializerFusionVariants
        ));
        assert!(!disabled.javascript_optimization_configured(
            JavaScriptOptimization::FreshLiteralFactoryInliningVariants
        ));
        assert!(!disabled.js_options().cross_scope_name_reuse);

        let standard: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=9\n").unwrap();
        assert_eq!(standard.javascript.effective_candidate_limit(), 384);
        assert_eq!(standard.javascript.effective_candidate_beam_width(), 6);
        assert_eq!(
            standard.javascript.effective_candidate_byte_budget(),
            384 * 1024
        );
        assert_eq!(
            standard.javascript.effective_terminal_codec_probe_limit(),
            64
        );
        assert_eq!(
            standard.javascript.effective_candidate_proposal_limit(),
            384
        );
        assert!(standard.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole));
        assert!(standard
            .javascript_optimization_configured(JavaScriptOptimization::EntropyPropertyAssignment));
        assert!(standard.javascript_optimization_configured(
            JavaScriptOptimization::ConstructorInitializerFusionVariants
        ));
        assert!(standard.javascript_optimization_configured(
            JavaScriptOptimization::FreshLiteralFactoryInliningVariants
        ));
        assert!(!standard.js_options().constructor_initializer_fusion);
        assert!(!standard.js_options().inline_fresh_empty_array_factories);
        assert!(!standard
            .javascript_optimization_configured(JavaScriptOptimization::IrInliningVariants));
        assert!(!standard.js_options().local_phi_expression_regions);
        assert!(!standard.js_options().phi_edge_value_forwarding);

        let shorthand_off: ProjectConfig =
            toml::from_str("[javascript]\nstruct_method_shorthand=false\n").unwrap();
        assert!(!shorthand_off.js_options().struct_method_shorthand);
        let shorthand_default: ProjectConfig = toml::from_str("[javascript]\n").unwrap();
        assert!(shorthand_default.js_options().struct_method_shorthand);

        // The cost model picks the default, and a port that has measured its
        // own artifact overrides it in either direction.
        let forced_on: ProjectConfig = toml::from_str(
            "[javascript]\noptimization_level=9\ncost_model='brotli'\nlocal_phi_expression_regions=true\n",
        )
        .unwrap();
        assert!(forced_on.js_options().local_phi_expression_regions);
        let forced_off: ProjectConfig = toml::from_str(
            "[javascript]\noptimization_level=9\ncost_model='gzip'\nlocal_phi_expression_regions=false\n",
        )
        .unwrap();
        assert!(!forced_off.js_options().local_phi_expression_regions);

        let gzip: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=9\ncost_model='gzip'\n").unwrap();
        assert!(gzip.js_options().local_phi_expression_regions);
        assert!(gzip.js_options().phi_edge_value_forwarding);

        let exhaustive: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=13\n").unwrap();
        assert!(exhaustive
            .javascript_optimization_configured(JavaScriptOptimization::FunctionLayoutVariants));
        assert!(exhaustive
            .javascript_optimization_configured(JavaScriptOptimization::IdenticalFunctionFolding));
        assert!(exhaustive
            .javascript_optimization_configured(JavaScriptOptimization::JointRepresentationSearch));
        assert!(!exhaustive.javascript_optimization_configured(
            JavaScriptOptimization::IrFunctionSubsumptionVariants
        ));
        assert!(!exhaustive
            .javascript_optimization_configured(JavaScriptOptimization::IrCompressPassVariants));
        assert!(!exhaustive
            .javascript_optimization_configured(JavaScriptOptimization::JointChunkSymbolSearch));

        let level_fourteen: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=14\n").unwrap();
        assert_eq!(
            level_fourteen.javascript.effective_candidate_beam_width(),
            11
        );
        assert_eq!(
            level_fourteen.javascript.effective_candidate_byte_budget(),
            896 * 1024
        );
        assert_eq!(
            level_fourteen
                .javascript
                .effective_terminal_codec_probe_limit(),
            256
        );
        let level_fifteen: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=15\n").unwrap();
        assert_eq!(
            level_fifteen
                .javascript
                .effective_terminal_codec_probe_limit_for_artifact(16 * 1024),
            384
        );
        assert_eq!(
            level_fifteen
                .javascript
                .effective_terminal_codec_probe_limit_for_artifact(32 * 1024),
            96
        );
        assert_eq!(
            level_fifteen
                .javascript
                .effective_terminal_codec_probe_limit_for_artifact(100 * 1024),
            32
        );
        assert_eq!(
            level_fifteen
                .javascript
                .effective_candidate_proposal_limit_for_artifact(16 * 1024),
            384
        );
        assert_eq!(
            level_fifteen
                .javascript
                .effective_candidate_proposal_limit_for_artifact(32 * 1024),
            96
        );
        assert_eq!(
            level_fifteen
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            32
        );
        let mut bounded_override = level_fifteen.clone();
        bounded_override.javascript.terminal_codec_probe_limit = Some(17);
        bounded_override.javascript.candidate_proposal_limit = Some(23);
        assert_eq!(
            bounded_override
                .javascript
                .effective_terminal_codec_probe_limit_for_artifact(100 * 1024),
            17
        );
        assert_eq!(
            bounded_override
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            23
        );
        bounded_override.javascript.terminal_codec_probe_limit = Some(999);
        bounded_override.javascript.candidate_proposal_limit = Some(999);
        assert_eq!(
            bounded_override
                .javascript
                .effective_terminal_codec_probe_limit_for_artifact(100 * 1024),
            999,
            "an explicit terminal ceiling is honored, not clamped to the level tier"
        );
        assert_eq!(
            bounded_override
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            384,
            "production search remains a hard ceiling for an explicit proposal budget"
        );
        assert!(level_fourteen.javascript_optimization_configured(
            JavaScriptOptimization::IrFunctionSubsumptionVariants
        ));
        assert!(level_fourteen
            .javascript_optimization_configured(JavaScriptOptimization::IrCompressPassVariants));
        assert!(level_fourteen
            .javascript_optimization_configured(JavaScriptOptimization::JointChunkSymbolSearch));
        assert!(level_fourteen.js_function_subsumption_variants_enabled());

        let exact: ProjectConfig = toml::from_str(
            r#"
[javascript]
optimization_level = 0
  optimizations = ["parsed-peephole", "do-loop-variants", "function-layout-variants", "ir-function-subsumption-variants", "constructor-initializer-fusion-variants", "fresh-literal-factory-inlining-variants"]
"#,
        )
        .unwrap();
        exact.validate().unwrap();
        assert_eq!(exact.javascript.effective_candidate_limit(), 1);
        assert_eq!(exact.javascript.effective_candidate_beam_width(), 1);
        assert_eq!(
            exact.javascript.effective_candidate_byte_budget(),
            64 * 1024
        );
        assert_eq!(exact.javascript.effective_terminal_codec_probe_limit(), 0);
        assert_eq!(exact.javascript.effective_candidate_proposal_limit(), 0);
        assert!(exact.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole));
        assert!(exact.javascript_optimization_configured(JavaScriptOptimization::DoLoopVariants));
        assert!(exact
            .javascript_optimization_configured(JavaScriptOptimization::FunctionLayoutVariants));
        assert!(exact.javascript_optimization_configured(
            JavaScriptOptimization::IrFunctionSubsumptionVariants
        ));
        assert!(exact.javascript_optimization_configured(
            JavaScriptOptimization::ConstructorInitializerFusionVariants
        ));
        assert!(exact.javascript_optimization_configured(
            JavaScriptOptimization::FreshLiteralFactoryInliningVariants
        ));
        assert!(!exact.javascript_optimization_configured(
            JavaScriptOptimization::ConditionalExpressionVariants
        ));
        assert!(!exact.javascript_optimization_configured(
            JavaScriptOptimization::ExpressionPhiRegionVariants
        ));
        assert!(!exact.javascript_optimization_configured(
            JavaScriptOptimization::LocalPhiExpressionRegionVariants
        ));
        assert!(!exact.javascript_optimization_configured(
            JavaScriptOptimization::PhiEdgeValueForwardingVariants
        ));

        let mut exact_always = exact.clone();
        exact_always.javascript.candidate_search = CandidateSearch::Always;
        assert_eq!(exact_always.javascript.effective_candidate_limit(), 1);
        assert_eq!(exact_always.javascript.effective_candidate_beam_width(), 1);
        assert_eq!(
            exact_always.javascript.effective_candidate_byte_budget(),
            64 * 1024
        );
        assert_eq!(
            exact_always
                .javascript
                .effective_terminal_codec_probe_limit(),
            0
        );

        exact_always.javascript.terminal_codec_probe_limit = Some(17);
        exact_always.javascript.candidate_proposal_limit = Some(23);
        assert_eq!(
            exact_always
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            0,
            "an explicit proposal value cannot bypass the level-zero tier"
        );
        assert_eq!(
            exact_always
                .javascript
                .effective_terminal_codec_probe_limit(),
            0,
            "an explicit terminal value cannot bypass the level-zero tier"
        );
        assert_eq!(
            exact_always
                .javascript
                .effective_terminal_codec_probe_limit_for_artifact(100 * 1024),
            0
        );
        exact_always.javascript.candidate_search = CandidateSearch::Off;
        assert_eq!(
            exact_always.javascript.effective_candidate_proposal_limit(),
            0,
            "candidate_search=off is a hard stop even with an explicit proposal cap"
        );
        assert_eq!(
            exact_always
                .javascript
                .effective_terminal_codec_probe_limit(),
            0,
            "candidate_search=off is a hard stop even with an explicit lab cap"
        );

        let constructible: ProjectConfig =
            toml::from_str("[javascript]\nfunction_spelling='function'\n").unwrap();
        assert_eq!(
            constructible.js_options().function_spelling,
            FunctionSpelling::Function
        );

        let opaque_handles: ProjectConfig =
            toml::from_str("[javascript]\npublic_aggregate_abi='positional'\n").unwrap();
        assert!(!opaque_handles.js_options().public_aggregate_fields);
        assert!(
            ProjectConfig::default()
                .js_options()
                .public_aggregate_fields
        );

        let exact_layout: ProjectConfig =
            toml::from_str("[javascript]\nfunction_layout_exact_limit=18\n").unwrap();
        exact_layout.validate().unwrap();
        assert_eq!(exact_layout.js_options().function_layout_exact_limit, 18);

        let balanced: ProjectConfig =
            toml::from_str("[javascript]\npriority='balanced'\noptimization_level=15\n").unwrap();
        assert!(!balanced.js_function_subsumption_variants_enabled());
        let explicit_balanced: ProjectConfig = toml::from_str(
            "[javascript]\npriority='balanced'\noptimization_level=0\noptimizations=['ir-function-subsumption-variants']\n",
        )
        .unwrap();
        assert!(explicit_balanced.js_function_subsumption_variants_enabled());
        let hard_disabled: ProjectConfig = toml::from_str(
            "[optimization]\nfunction_subsumption=false\n[javascript]\noptimizations=['ir-function-subsumption-variants']\n",
        )
        .unwrap();
        assert!(!hard_disabled.js_function_subsumption_variants_enabled());
    }

    #[test]
    fn proposal_defaults_follow_survivor_limits_but_explicit_work_is_independent() {
        let mut config: ProjectConfig = toml::from_str(
            "[javascript]\noptimization_level=15\ncandidate_search='always'\ncandidate_limit=2\n",
        )
        .unwrap();
        assert_eq!(config.javascript.effective_candidate_limit(), 2);
        assert_eq!(config.javascript.effective_candidate_proposal_limit(), 2);
        assert_eq!(
            config
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            2,
            "an omitted proposal limit preserves the expected tiny-work policy"
        );

        config.javascript.candidate_proposal_limit = Some(23);
        assert_eq!(config.javascript.effective_candidate_proposal_limit(), 23);
        assert_eq!(
            config
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            23,
            "an explicit lab budget can exceed survivor count and bypass artifact scaling"
        );
        config.javascript.candidate_proposal_limit = Some(1);
        assert_eq!(config.javascript.effective_candidate_proposal_limit(), 1);
        assert_eq!(
            config
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            1
        );

        config.javascript.optimization_level = 0;
        config.javascript.candidate_proposal_limit = Some(23);
        assert_eq!(config.javascript.effective_candidate_proposal_limit(), 0);
        assert_eq!(
            config
                .javascript
                .effective_candidate_proposal_limit_for_artifact(100 * 1024),
            0,
            "an explicit proposal ceiling cannot bypass level zero"
        );
    }

    #[test]
    fn rejects_unknown_and_invalid_settings() {
        assert!(toml::from_str::<ProjectConfig>("[mangle]\nmagic=true").is_err());
        let config = toml::from_str::<ProjectConfig>("[bundle]\nmax_chunks=0").unwrap();
        assert!(config.validate().unwrap_err().contains("max_chunks"));
        let duplicate: ProjectConfig =
            toml::from_str("[javascript]\ncompression=['string-pooling','string-pooling']\n")
                .unwrap();
        assert!(duplicate.validate().unwrap_err().contains("duplicate"));
        let invalid_level: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=16\n").unwrap();
        assert!(invalid_level
            .validate()
            .unwrap_err()
            .contains("between 0 and 15"));
        let zero_beam: ProjectConfig =
            toml::from_str("[javascript]\ncandidate_beam_width=0\n").unwrap();
        assert!(zero_beam
            .validate()
            .unwrap_err()
            .contains("candidate_beam_width"));
        let zero_byte_budget: ProjectConfig =
            toml::from_str("[javascript]\ncandidate_byte_budget=0\n").unwrap();
        assert!(zero_byte_budget
            .validate()
            .unwrap_err()
            .contains("candidate_byte_budget"));
        let excessive_raw_growth: ProjectConfig =
            toml::from_str("[javascript]\nmax_candidate_raw_growth_percent=1001\n").unwrap();
        assert!(excessive_raw_growth
            .validate()
            .unwrap_err()
            .contains("max_candidate_raw_growth_percent"));
        let excessive_layout_search: ProjectConfig =
            toml::from_str("[javascript]\nfunction_layout_exact_limit=19\n").unwrap();
        assert!(excessive_layout_search
            .validate()
            .unwrap_err()
            .contains("function_layout_exact_limit"));
        let excessive_local_reserve: ProjectConfig =
            toml::from_str("[javascript]\nlocal_name_reserve=257\n").unwrap();
        assert!(excessive_local_reserve
            .validate()
            .unwrap_err()
            .contains("local_name_reserve"));
        let zero_nesting: ProjectConfig =
            toml::from_str("[javascript.startup]\nmax_nesting=0\n").unwrap();
        assert!(zero_nesting.validate().unwrap_err().contains("max_nesting"));
        let duplicate_optimization: ProjectConfig =
            toml::from_str("[javascript]\noptimizations=['parsed-peephole','parsed-peephole']\n")
                .unwrap();
        assert!(duplicate_optimization
            .validate()
            .unwrap_err()
            .contains("duplicate"));
    }

    #[test]
    fn discovers_the_nearest_project_config() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-config-discovery-test-{}",
            std::process::id()
        ));
        let nested = directory.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            directory.join("lilscript.toml"),
            "[mangle]\nidentifiers=false\n",
        )
        .unwrap();
        let loaded = load_project_config(&nested.join("main.lil"), None).unwrap();

        assert_eq!(
            loaded.path,
            Some(directory.join("lilscript.toml").canonicalize().unwrap())
        );
        assert!(!loaded.config.js_options().mangle_identifiers);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn loads_and_overlays_versioned_profile_data() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-profile-config-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("profile.json"),
            r#"{"version":1,"functions":{"render":40},"loops":{"render#0":80}}"#,
        )
        .unwrap();
        let config_path = directory.join("lilscript.toml");
        std::fs::write(
            &config_path,
            "[profile]\npath='profile.json'\n[profile.functions]\nrender=100\n",
        )
        .unwrap();
        let input = directory.join("main.lil");
        std::fs::write(&input, "print(1);").unwrap();

        let loaded = load_project_config(&input, Some(&config_path)).unwrap();
        let profile = loaded.config.load_optimization_profile().unwrap();
        assert_eq!(profile.functions.get("render"), Some(&100));
        assert_eq!(profile.loops.get("render#0"), Some(&80));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn validates_performance_native_profile_and_provider_controls() {
        let config: ProjectConfig = toml::from_str(
            r#"
[javascript.performance]
deoptimization_weight = 10
allocation_weight = 5
indirect_call_weight = 20
hot_code_weight = 1
max_regression_percent = 15

[profile]
specialization_min_count = 50
max_specializations_per_function = 3
max_clone_instructions = 40

[native]
partial_escape_analysis = true
stack_allocation = true
region_allocation = true
stack_array_element_limit = 32

[lint]
providers = ["correctness", "web"]
[lint.rules]
"web/eager-host-access" = "hint"
"#,
        )
        .unwrap();
        config.validate().unwrap();
        assert_eq!(config.profile.specialization_min_count, 50);
        assert_eq!(config.native_options().stack_array_element_limit, 32);
        assert_eq!(config.lint.providers.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn global_optimizer_disables_override_javascript_effort_features() {
        let config: ProjectConfig = toml::from_str(
            r#"
[optimization]
preset = "maximum"
call_site_specialization = false
capture_signature_cloning = false
constant_parameter_specialization = false
specialize_tagged_constants = false
profile_guided = false

[javascript]
optimization_level = 15
"#,
        )
        .unwrap();
        let options = config.js_optimizer_options();
        assert!(!options.call_site_specialization);
        assert!(!options.capture_signature_cloning);
        assert!(!options.constant_parameter_specialization);
        assert!(!options.specialize_tagged_constants);
        assert!(!config.js_profile_guided_optimization());
        assert!(!config.native_profile_guided_optimization());
    }
}
