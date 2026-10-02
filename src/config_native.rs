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

#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Deserialize,Serialize)]
#[serde(rename_all="kebab-case")]
pub enum NativeArtifact { #[default] Executable, SharedLibrary, Object }
impl NativeArtifact {pub fn is_library(self)->bool {self!=Self::Executable}}

/// External C compiler settings; these never relax checked source semantics.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TargetNativeConfig {
    /// One executable, never a shell fragment. Relative paths use config_dir.
    pub compiler: Option<PathBuf>,
    /// `speed` (default): -O3; `size`: -Os; `balanced`: -O2. No semantic relaxation.
    pub objective: NativeObjective,
    /// Executable by default; shared-library and object retain public exports
    /// and expose explicit initialization/queue/shutdown instead of main.
    pub artifact: NativeArtifact,
    /// Stable public namespace for library types, runtime API and exports.
    /// Changing it requires rebuilding native clients; default is `lil`.
    pub symbol_prefix: String,
    /// Clang-compatible target triple and sysroot; omitted means host defaults.
    pub triple: Option<String>,
    /// Optional target SDK root, relative to the TOML file; never auto-downloaded.
    pub sysroot: Option<PathBuf>,
    /// Optional linked WebAssembly stack reservation, in bytes (16-byte
    /// aligned). Requires an explicit wasm target triple; SDK default otherwise.
    pub wasm_stack_size: Option<u32>,
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
    /// True: eligible once-created immutable graphs use static C storage.
    /// False retains ordinary owned initialization, useful for size/startup comparisons.
    pub static_data: bool,
    /// True: synchronous closures whose complete use set stays in their
    /// creation region borrow stack environments and eligible captured cells.
    /// Saves allocation/RC work at the cost of activation stack storage.
    pub stack_storage: bool,
    /// True: closed generic bodies called with one concrete scalar/handle
    /// instantiation use typed storage. Public/polymorphic uses keep tagging.
    pub generic_specialization: bool,
    /// 262144 bytes by default. Bounds libregexp parser stack use; at least
    /// 16384. Smaller values reject deeply nested patterns sooner. Exhaustion
    /// raises RangeError. Reserve this plus the caller stack in a cross profile.
    pub regex_stack_limit: u32,
    /// Zero by default (unlimited). Maximum libregexp polling intervals per
    /// test/search/replace operation, shared by all matches in a replace. A
    /// positive limit bounds adversarial backtracking at the cost of a catchable
    /// RangeError; it never changes an exhausted operation into a failed match.
    pub regex_poll_limit: u64,
}
impl Default for TargetNativeConfig {
    fn default() -> Self {
        Self {
            compiler: None,
            objective: NativeObjective::Speed,
            artifact: NativeArtifact::Executable,
            symbol_prefix: "lil".into(),
            triple: None,
            sysroot: None,
            wasm_stack_size: None,
            debug_info: false,
            warnings_as_errors: false,
            link_time_optimization: false,
            sanitizers: Vec::new(),
            cycle_collection_threshold: 4096,
            static_data: true,
            stack_storage: true,
            generic_specialization: true,
            regex_stack_limit: 262144,
            regex_poll_limit: 0,
        }
    }
}
impl TargetNativeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(bytes)=self.wasm_stack_size {
            if bytes==0 || bytes%16!=0 || !self.triple.as_deref().is_some_and(|t|t.starts_with("wasm32-") || t.starts_with("wasm64-")) || self.artifact==NativeArtifact::Object {
                return Err("target.native.wasm_stack_size requires a linked wasm32/wasm64 target and a positive, 16-byte-aligned byte count".into());
            }
        }
        if !crate::native_symbols::public_prefix(&self.symbol_prefix) {return Err("target.native.symbol_prefix must be an ASCII identifier starting with a letter, outside the reserved ls and host namespaces".into());}
        if self.artifact==NativeArtifact::Object && self.link_time_optimization {return Err("target.native.link_time_optimization requires a linked artifact".into());}
        if self.regex_stack_limit < 16384 {
            return Err("target.native.regex_stack_limit must be at least 16384 bytes".into());
        }
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
