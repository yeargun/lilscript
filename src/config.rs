use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::codegen_ir_js::{
    ControlFlowSpelling, FunctionLayout, IdentifierAlphabet, IrJsOptions, LoopSpelling,
    MutationSpelling, PhiAffinityMode, StateMachineSpelling, StringQuote,
};
use crate::codegen_native::NativeOptions;
use crate::optimizer::OptimizationOptions;
use crate::profile::{JavaScriptPerformanceWeights, OptimizationProfile};

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub package: Option<PackageMetadata>,
    pub dependencies: BTreeMap<String, DependencyConfig>,
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
        options.specialize_tagged_constants = true;
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
            pool_strings: self.mangle.pool_strings.unwrap_or_else(|| {
                self.javascript
                    .compression_enabled(CompressionDecision::StringPooling)
            }),
            elide_safe_integer_coercions: self
                .javascript
                .compression_enabled(CompressionDecision::SafeIntegerCoercionElision),
            compact_boolean_literals: self
                .javascript
                .compression_enabled(CompressionDecision::CompactBooleanLiterals),
            inline_structured_closures: self
                .javascript
                .compression_enabled(CompressionDecision::StructuredClosureInlining),
            pack_string_arrays: self
                .javascript
                .compression_enabled(CompressionDecision::StringArrayPacking),
            scalar_phi_copies: self
                .javascript
                .compression_enabled(CompressionDecision::ScalarPhiCopies),
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
            comma_expressions: false,
            update_loop_layout: true,
            cross_scope_name_reuse: self
                .javascript
                .optimization_enabled(JavaScriptOptimization::EntropyCrossScopeReuse, None),
            entropy_property_names: self
                .javascript
                .optimization_enabled(JavaScriptOptimization::EntropyPropertyAssignment, None),
            function_layout: FunctionLayout::Source,
            loop_spelling: LoopSpelling::Auto,
            mutation_spelling: MutationSpelling::Assignment,
            identifier_alphabet: IdentifierAlphabet::canonical(),
            string_quote: StringQuote::Double,
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

    pub fn javascript_optimization_enabled(&self, feature: JavaScriptOptimization) -> bool {
        self.javascript.candidate_search_enabled()
            && self.javascript.optimization_enabled(feature, None)
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
        if self.javascript.candidate_limit == 0 {
            return Err("`javascript.candidate_limit` must be greater than zero".to_string());
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

    const fn enables_compression(self, decision: CompressionDecision) -> bool {
        match decision {
            CompressionDecision::IdentifierMangling => true,
            CompressionDecision::EntropyAwareMangling => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::QuoteStyleSelection => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::StringPooling => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::SizeAwareInlining => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::SafeIntegerCoercionElision => {
                !matches!(self, Self::PerformanceFirst)
            }
            CompressionDecision::CompactBooleanLiterals => !matches!(self, Self::PerformanceFirst),
            CompressionDecision::StructuredClosureInlining => {
                !matches!(self, Self::PerformanceFirst)
            }
            CompressionDecision::StringArrayPacking => matches!(self, Self::SizeFirst),
            CompressionDecision::ScalarPhiCopies => matches!(self, Self::SizeFirst),
            CompressionDecision::PhiAffinityCoalescing => true,
            CompressionDecision::IrInliningVariants => matches!(self, Self::SizeFirst),
            CompressionDecision::IrClosureFactoryVariants => matches!(self, Self::SizeFirst),
            CompressionDecision::LoopSpellingSelection => matches!(self, Self::SizeFirst),
            CompressionDecision::MutationSpellingSelection => matches!(self, Self::SizeFirst),
            CompressionDecision::PropertyMangling | CompressionDecision::ExportMangling => false,
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
    CompactBooleanLiterals,
    StructuredClosureInlining,
    StringArrayPacking,
    ScalarPhiCopies,
    PhiAffinityCoalescing,
    IrInliningVariants,
    IrClosureFactoryVariants,
    LoopSpellingSelection,
    MutationSpellingSelection,
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
            Self::CompactBooleanLiterals => "compact-boolean-literals",
            Self::StructuredClosureInlining => "structured-closure-inlining",
            Self::StringArrayPacking => "string-array-packing",
            Self::ScalarPhiCopies => "scalar-phi-copies",
            Self::PhiAffinityCoalescing => "phi-affinity-coalescing",
            Self::IrInliningVariants => "ir-inlining-variants",
            Self::IrClosureFactoryVariants => "ir-closure-factory-variants",
            Self::LoopSpellingSelection => "loop-spelling-selection",
            Self::MutationSpellingSelection => "mutation-spelling-selection",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JavaScriptConfig {
    pub priority: JavaScriptPriority,
    pub optimization_level: u8,
    pub optimizations: Option<Vec<JavaScriptOptimization>>,
    pub compression: Option<Vec<CompressionDecision>>,
    pub inline_instruction_limit: Option<usize>,
    pub inline_control_flow_limit: Option<usize>,
    pub max_inline_growth: Option<usize>,
    pub cost_model: CompressionCostModel,
    pub candidate_search: CandidateSearch,
    pub candidate_limit: usize,
    pub startup: StartupCostConfig,
    pub performance: JavaScriptPerformanceConfig,
}

impl Default for JavaScriptConfig {
    fn default() -> Self {
        Self {
            priority: JavaScriptPriority::SizeFirst,
            optimization_level: 15,
            optimizations: None,
            compression: None,
            inline_instruction_limit: None,
            inline_control_flow_limit: None,
            max_inline_growth: None,
            cost_model: CompressionCostModel::Brotli,
            candidate_search: CandidateSearch::Production,
            candidate_limit: 1536,
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
    IrFunctionSubsumptionVariants,
    IrSpecializationVariants,
    StructuralControlFlowVariants,
    SsaDestructionVariants,
    ConditionalExpressionVariants,
    CommaExpressionVariants,
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
}

impl JavaScriptOptimization {
    pub const fn name(self) -> &'static str {
        match self {
            Self::IrInliningVariants => "ir-inlining-variants",
            Self::IrClosureFactoryVariants => "ir-closure-factory-variants",
            Self::IrFunctionSubsumptionVariants => "ir-function-subsumption-variants",
            Self::IrSpecializationVariants => "ir-specialization-variants",
            Self::StructuralControlFlowVariants => "structural-control-flow-variants",
            Self::SsaDestructionVariants => "ssa-destruction-variants",
            Self::ConditionalExpressionVariants => "conditional-expression-variants",
            Self::CommaExpressionVariants => "comma-expression-variants",
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
        }
    }

    const fn minimum_level(self) -> u8 {
        match self {
            Self::ConditionalExpressionVariants => 4,
            Self::UpdateLoopVariants | Self::CompoundMutationVariants => 5,
            Self::CommaExpressionVariants | Self::SsaDestructionVariants => 7,
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
            Self::IrFunctionSubsumptionVariants => 14,
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
    fn compression_enabled(&self, decision: CompressionDecision) -> bool {
        self.compression.as_ref().map_or_else(
            || self.priority.enables_compression(decision),
            |enabled| enabled.contains(&decision),
        )
    }

    pub const fn candidate_search_enabled(&self) -> bool {
        !matches!(self.candidate_search, CandidateSearch::Off)
    }

    fn optimization_enabled(
        &self,
        feature: JavaScriptOptimization,
        legacy: Option<CompressionDecision>,
    ) -> bool {
        if let Some(features) = &self.optimizations {
            return features.contains(&feature);
        }
        self.optimization_level >= feature.minimum_level()
            && legacy.is_none_or(|decision| self.compression_enabled(decision))
    }

    pub fn effective_candidate_limit(&self) -> usize {
        if self.optimizations.is_some() {
            return self.candidate_limit;
        }
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
        self.candidate_limit.min(level_limit)
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
    pub scalar_replacement: Option<bool>,
    pub dead_store_elimination: Option<bool>,
    pub dead_code_elimination: Option<bool>,
    pub call_site_specialization: Option<bool>,
    pub capture_signature_cloning: Option<bool>,
    pub identical_function_folding: Option<bool>,
    pub function_subsumption: Option<bool>,
    pub profile_guided: Option<bool>,
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
            scalar_replacement: None,
            dead_store_elimination: None,
            dead_code_elimination: None,
            call_site_specialization: None,
            capture_signature_cloning: None,
            identical_function_folding: None,
            function_subsumption: None,
            profile_guided: None,
        }
    }
}

impl OptimizationConfig {
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
            specialize_tagged_constants: base.specialize_tagged_constants,
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
        assert!(!config.js_options().mangle_identifiers);
        assert!(config.js_options().mangle_properties);
        assert_eq!(config.bundle.mode, BundleMode::Split);
        assert_eq!(config.bundle.min_chunk_bytes, 4096);
        config.validate().unwrap();
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
        assert!(realistic.js_options().elide_safe_integer_coercions);
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
        assert!(!balanced.js_options().pack_string_arrays);

        let size: ProjectConfig = toml::from_str("[javascript]\npriority='size-first'\n").unwrap();
        assert_eq!(size.js_optimizer_options().inline_growth_limit, Some(16));
        assert!(size.js_options().pool_strings);
        assert!(size.js_options().elide_safe_integer_coercions);
        assert!(size.js_options().inline_structured_closures);
        assert!(size.js_options().pack_string_arrays);
        assert!(size.js_options().scalar_phi_copies);
        assert!(size.ir_inlining_variants_enabled());
        assert!(size.ir_closure_factory_variants_enabled());
        assert!(size.loop_spelling_selection_enabled());
        assert!(size.mutation_spelling_selection_enabled());
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
        assert!(!performance.ir_inlining_variants_enabled());
        assert!(!performance.ir_closure_factory_variants_enabled());
        assert!(!performance.loop_spelling_selection_enabled());
        assert!(!performance.mutation_spelling_selection_enabled());

        let explicit_pooling: ProjectConfig = toml::from_str(
            "[javascript]\npriority='performance-first'\n[mangle]\npool_strings=true\n",
        )
        .unwrap();
        assert!(explicit_pooling.js_options().pool_strings);
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
        assert!(!custom.ir_inlining_variants_enabled());
        assert!(!custom.ir_closure_factory_variants_enabled());
        assert!(!custom.loop_spelling_selection_enabled());
        assert!(!custom.mutation_spelling_selection_enabled());

        let none: ProjectConfig = toml::from_str("[javascript]\ncompression=[]\n").unwrap();
        let none_codegen = none.js_options();
        assert_eq!(none.js_optimizer_options().inline_growth_limit, None);
        assert!(!none_codegen.mangle_identifiers);
        assert!(!none_codegen.mangle_properties);
        assert!(!none_codegen.mangle_exports);
        assert!(!none_codegen.pool_strings);
        assert!(!none_codegen.elide_safe_integer_coercions);
        assert!(!none_codegen.compact_boolean_literals);
        assert!(!none_codegen.pack_string_arrays);
        assert!(!none_codegen.scalar_phi_copies);
        assert_eq!(
            none_codegen.phi_affinity_mode,
            PhiAffinityMode::Conservative
        );
        assert!(!none.entropy_aware_mangling_enabled());

        let explicit_mangle: ProjectConfig = toml::from_str(
            "[javascript]\ncompression=[]\n[mangle]\nidentifiers=true\npool_strings=true\n",
        )
        .unwrap();
        assert!(explicit_mangle.js_options().mangle_identifiers);
        assert!(explicit_mangle.js_options().pool_strings);
    }

    #[test]
    fn resolves_javascript_optimization_levels_and_exact_allowlists() {
        let disabled: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=0\ncandidate_limit=1536\n").unwrap();
        disabled.validate().unwrap();
        assert_eq!(disabled.javascript.effective_candidate_limit(), 1);
        assert!(!disabled.javascript_optimization_configured(
            JavaScriptOptimization::ConditionalExpressionVariants
        ));
        assert!(
            !disabled.javascript_optimization_configured(JavaScriptOptimization::StartupCostGuard)
        );
        assert!(!disabled.js_options().conditional_expressions);
        assert!(!disabled.js_options().cross_scope_name_reuse);

        let standard: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=9\n").unwrap();
        assert_eq!(standard.javascript.effective_candidate_limit(), 384);
        assert!(standard.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole));
        assert!(standard
            .javascript_optimization_configured(JavaScriptOptimization::EntropyPropertyAssignment));
        assert!(!standard
            .javascript_optimization_configured(JavaScriptOptimization::IrInliningVariants));

        let exhaustive: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=13\n").unwrap();
        assert!(exhaustive
            .javascript_optimization_configured(JavaScriptOptimization::FunctionLayoutVariants));
        assert!(exhaustive
            .javascript_optimization_configured(JavaScriptOptimization::IdenticalFunctionFolding));
        assert!(!exhaustive.javascript_optimization_configured(
            JavaScriptOptimization::IrFunctionSubsumptionVariants
        ));

        let level_fourteen: ProjectConfig =
            toml::from_str("[javascript]\noptimization_level=14\n").unwrap();
        assert!(level_fourteen.javascript_optimization_configured(
            JavaScriptOptimization::IrFunctionSubsumptionVariants
        ));
        assert!(level_fourteen.js_function_subsumption_variants_enabled());

        let exact: ProjectConfig = toml::from_str(
            r#"
[javascript]
optimization_level = 0
optimizations = ["parsed-peephole", "do-loop-variants", "function-layout-variants", "ir-function-subsumption-variants"]
"#,
        )
        .unwrap();
        exact.validate().unwrap();
        assert_eq!(exact.javascript.effective_candidate_limit(), 1536);
        assert!(exact.javascript_optimization_configured(JavaScriptOptimization::ParsedPeephole));
        assert!(exact.javascript_optimization_configured(JavaScriptOptimization::DoLoopVariants));
        assert!(exact
            .javascript_optimization_configured(JavaScriptOptimization::FunctionLayoutVariants));
        assert!(exact.javascript_optimization_configured(
            JavaScriptOptimization::IrFunctionSubsumptionVariants
        ));
        assert!(!exact.javascript_optimization_configured(
            JavaScriptOptimization::ConditionalExpressionVariants
        ));

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
profile_guided = false

[javascript]
optimization_level = 15
"#,
        )
        .unwrap();
        let options = config.js_optimizer_options();
        assert!(!options.call_site_specialization);
        assert!(!options.capture_signature_cloning);
        assert!(!config.js_profile_guided_optimization());
        assert!(!config.native_profile_guided_optimization());
    }
}
