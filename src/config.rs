use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::codegen_ir_js::{IdentifierAlphabet, IrJsOptions, PhiAffinityMode, StringQuote};
use crate::optimizer::OptimizationOptions;

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub optimization: OptimizationConfig,
    pub javascript: JavaScriptConfig,
    pub mangle: MangleConfig,
    pub bundle: BundleConfig,
    pub lint: LintConfig,
    pub format: FormatConfig,
}

impl ProjectConfig {
    pub fn optimizer_options(&self) -> OptimizationOptions {
        self.optimization.resolve()
    }

    pub fn js_optimizer_options(&self) -> OptimizationOptions {
        let mut options = self.optimization.resolve();
        options.specialize_tagged_constants = true;
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

    pub fn validate(&self) -> Result<(), String> {
        if self.bundle.min_chunk_bytes == 0 {
            return Err("`bundle.min_chunk_bytes` must be greater than zero".to_string());
        }
        if self.bundle.max_chunks == 0 {
            return Err("`bundle.max_chunks` must be greater than zero".to_string());
        }
        if self.bundle.shared_min_imports < 2 {
            return Err("`bundle.shared_min_imports` must be at least 2".to_string());
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
        if self.format.line_width < 40 {
            return Err("`format.line_width` must be at least 40".to_string());
        }
        for (rule, severity) in &self.lint.rules {
            if rule.trim().is_empty() {
                return Err("`lint.rules` contains an empty rule name".to_string());
            }
            let _ = severity;
        }
        Ok(())
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JavaScriptConfig {
    pub priority: JavaScriptPriority,
    pub compression: Option<Vec<CompressionDecision>>,
    pub inline_instruction_limit: Option<usize>,
    pub inline_control_flow_limit: Option<usize>,
    pub max_inline_growth: Option<usize>,
    pub cost_model: CompressionCostModel,
    pub candidate_search: CandidateSearch,
    pub candidate_limit: usize,
}

impl Default for JavaScriptConfig {
    fn default() -> Self {
        Self {
            priority: JavaScriptPriority::SizeFirst,
            compression: None,
            inline_instruction_limit: None,
            inline_control_flow_limit: None,
            max_inline_growth: None,
            cost_model: CompressionCostModel::Brotli,
            candidate_search: CandidateSearch::Production,
            candidate_limit: 1536,
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
    pub global_optimization: Option<bool>,
    pub inlining: Option<bool>,
    pub scalar_replacement: Option<bool>,
    pub dead_store_elimination: Option<bool>,
    pub dead_code_elimination: Option<bool>,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            preset: OptimizationPreset::Maximum,
            constant_folding: None,
            algebraic_simplification: None,
            common_subexpression_elimination: None,
            global_optimization: None,
            inlining: None,
            scalar_replacement: None,
            dead_store_elimination: None,
            dead_code_elimination: None,
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
            global_optimization: self.global_optimization.unwrap_or(base.global_optimization),
            inlining: self.inlining.unwrap_or(base.inlining),
            scalar_replacement: self.scalar_replacement.unwrap_or(base.scalar_replacement),
            dead_store_elimination: self
                .dead_store_elimination
                .unwrap_or(base.dead_store_elimination),
            dead_code_elimination: self
                .dead_code_elimination
                .unwrap_or(base.dead_code_elimination),
            specialize_tagged_constants: base.specialize_tagged_constants,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BundleConfig {
    pub mode: BundleMode,
    pub min_chunk_bytes: usize,
    pub max_chunks: usize,
    pub shared_min_imports: usize,
}

impl Default for BundleConfig {
    fn default() -> Self {
        Self {
            mode: BundleMode::Single,
            min_chunk_bytes: 16 * 1024,
            max_chunks: 32,
            shared_min_imports: 2,
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
    let config = toml::from_str::<ProjectConfig>(&source).map_err(|error| ConfigError {
        path: path.clone(),
        message: format!("invalid config: {error}"),
    })?;
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
    fn rejects_unknown_and_invalid_settings() {
        assert!(toml::from_str::<ProjectConfig>("[mangle]\nmagic=true").is_err());
        let config = toml::from_str::<ProjectConfig>("[bundle]\nmax_chunks=0").unwrap();
        assert!(config.validate().unwrap_err().contains("max_chunks"));
        let duplicate: ProjectConfig =
            toml::from_str("[javascript]\ncompression=['string-pooling','string-pooling']\n")
                .unwrap();
        assert!(duplicate.validate().unwrap_err().contains("duplicate"));
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
}
