//! Native toolchain controls are separate from the JavaScript size objective.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeObjective {
    #[default]
    Speed,
    Size,
    Balanced,
}
impl NativeObjective {
    pub fn optimization_flag(self) -> &'static str {
        match self { Self::Speed => "-O3", Self::Size => "-Os", Self::Balanced => "-O2" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeSanitizer { Address, Undefined }
impl NativeSanitizer {
    pub fn flag(self) -> &'static str {
        match self { Self::Address => "-fsanitize=address", Self::Undefined => "-fsanitize=undefined" }
    }
}

/// External C compiler settings; these never relax checked source semantics.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TargetNativeConfig {
    /// One executable, never a shell fragment. Relative paths use config_dir.
    pub compiler: Option<PathBuf>,
    /// `speed` (default): -O3; `size`: -Os; `balanced`: -O2. No semantic relaxation.
    pub objective: NativeObjective,
    /// Clang-compatible target triple and sysroot; omitted means host defaults.
    pub triple: Option<String>,
    /// Optional target SDK root, relative to the TOML file; never auto-downloaded.
    pub sysroot: Option<PathBuf>,
    /// False by default. Add debug information; larger artifacts, easier debugging.
    pub debug_info: bool,
    /// False by default. Enable -Wall -Wextra -Werror for generated and provider C.
    pub warnings_as_errors: bool,
    /// False by default. Cross-translation-unit optimization costs more link work.
    pub link_time_optimization: bool,
    /// Empty by default. Address/undefined checks cost code, memory and runtime.
    pub sanitizers: Vec<NativeSanitizer>,
    /// 4096 allocations by default, scaled with the live heap. Smaller values
    /// reclaim cycles sooner at more CPU cost; zero collects only explicitly
    /// and at shutdown. Ordinary zero-count reclamation is always immediate.
    pub cycle_collection_threshold: u32,
}
impl Default for TargetNativeConfig {
    fn default() -> Self {
        Self {
            compiler: None,
            objective: NativeObjective::Speed,
            triple: None,
            sysroot: None,
            debug_info: false,
            warnings_as_errors: false,
            link_time_optimization: false,
            sanitizers: Vec::new(),
            cycle_collection_threshold: 4096,
        }
    }
}
impl TargetNativeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.compiler.iter().chain(&self.sysroot).any(|path| path.to_str().is_none()) {
            return Err("target.native compiler/sysroot controls must use UTF-8 paths, as in TOML".into());
        }
        if self.compiler.as_ref().is_some_and(|p| p.as_os_str().is_empty()) {
            return Err("target.native.compiler must name one executable".into());
        }
        if self.triple.as_ref().is_some_and(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))) {
            return Err("target.native.triple must be a nonempty target triple".into());
        }
        if self.sysroot.as_ref().is_some_and(|p| p.as_os_str().is_empty()) {
            return Err("target.native.sysroot must name a directory".into());
        }
        for (index, sanitizer) in self.sanitizers.iter().enumerate() {
            if self.sanitizers[..index].contains(sanitizer) {
                return Err("target.native.sanitizers must not contain duplicates".into());
            }
        }
        Ok(())
    }
}
