use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::codegen_ir_js::IrJsOptions;
use crate::optimizer::OptimizationOptions;

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub optimization: OptimizationConfig,
    pub javascript: JavaScriptConfig,
    pub mangle: MangleConfig,
    pub bundle: BundleConfig,
}

impl ProjectConfig {
    pub fn optimizer_options(&self) -> OptimizationOptions {
        self.optimization.resolve()
    }

    pub fn js_optimizer_options(&self) -> OptimizationOptions {
        let mut options = self.optimization.resolve();
        if !options.inlining {
            return options;
        }
        match self.javascript.priority {
            JavaScriptPriority::PerformanceFirst => {
                options.inline_instruction_limit = 24;
                options.inline_control_flow_limit = 60;
                options.inline_growth_limit = None;
            }
            JavaScriptPriority::Balanced => {}
            JavaScriptPriority::SizeFirst => {
                options.inline_growth_limit = Some(0);
            }
        }
        options
    }

    pub fn js_options(&self) -> IrJsOptions {
        IrJsOptions {
            mangle_identifiers: self.mangle.identifiers,
            mangle_properties: self.mangle.properties,
            mangle_exports: self.mangle.exports,
            pool_strings: self.mangle.pool_strings.unwrap_or(!matches!(
                self.javascript.priority,
                JavaScriptPriority::PerformanceFirst
            )),
        }
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
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaScriptPriority {
    PerformanceFirst,
    #[default]
    Balanced,
    SizeFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JavaScriptConfig {
    pub priority: JavaScriptPriority,
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
            inline_instruction_limit: base.inline_instruction_limit,
            inline_control_flow_limit: base.inline_control_flow_limit,
            inline_growth_limit: base.inline_growth_limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MangleConfig {
    pub identifiers: bool,
    pub properties: bool,
    pub exports: bool,
    pub pool_strings: Option<bool>,
}

impl Default for MangleConfig {
    fn default() -> Self {
        Self {
            identifiers: true,
            properties: false,
            exports: false,
            pool_strings: None,
        }
    }
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

        let balanced = ProjectConfig::default();
        let balanced_optimizer = balanced.js_optimizer_options();
        assert_eq!(balanced_optimizer.inline_instruction_limit, 12);
        assert_eq!(balanced_optimizer.inline_control_flow_limit, 30);
        assert_eq!(balanced_optimizer.inline_growth_limit, None);
        assert!(balanced.js_options().pool_strings);

        let size: ProjectConfig = toml::from_str("[javascript]\npriority='size-first'\n").unwrap();
        assert_eq!(size.js_optimizer_options().inline_growth_limit, Some(0));
        assert!(size.js_options().pool_strings);

        let explicit_pooling: ProjectConfig = toml::from_str(
            "[javascript]\npriority='performance-first'\n[mangle]\npool_strings=true\n",
        )
        .unwrap();
        assert!(explicit_pooling.js_options().pool_strings);
    }

    #[test]
    fn rejects_unknown_and_invalid_settings() {
        assert!(toml::from_str::<ProjectConfig>("[mangle]\nmagic=true").is_err());
        let config = toml::from_str::<ProjectConfig>("[bundle]\nmax_chunks=0").unwrap();
        assert!(config.validate().unwrap_err().contains("max_chunks"));
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
        assert!(!loaded.config.mangle.identifiers);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
