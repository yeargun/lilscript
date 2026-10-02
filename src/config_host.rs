//! Host binding names are ABI contracts, independent of optimization effort.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// `[host]`: explicit ABI names and native provider sources; independent of effort.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostConfig {
    /// Empty by default. Declared extern name to a JavaScript global/property path. Member reads
    /// remain observable and are evaluated at each original use.
    pub javascript: BTreeMap<String, String>,
    /// Empty by default. Declared extern function to a callback ABI v3 C provider symbol.
    pub native: BTreeMap<String, String>,
    /// Empty by default. Provider translation units, relative to the configuration directory.
    /// These are consumed only by native delivery, never by optimization.
    pub native_sources: Vec<PathBuf>,
}
impl HostConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        for (name, path) in &self.javascript {
            if !crate::js::identifier(name)
                || matches!(name.as_str(), "this" | "arguments" | "eval")
            {
                return Err(format!(
                    "host.javascript key `{name}` must name an ordinary extern declaration"
                ));
            }
            let mut parts = path.split('.');
            let root = parts.next().unwrap_or("");
            if !crate::js::identifier(root)
                || matches!(root, "arguments" | "eval")
                || !parts.all(crate::js::identifier_name)
            {
                return Err(format!("host.javascript.{name} must be a global identifier followed by named properties"));
            }
            if path.split('.').any(|part| part == "eval") {
                return Err(format!(
                    "host.javascript.{name} cannot bind eval; use a declared host wrapper"
                ));
            }
        }
        let mut links = BTreeSet::new();
        for (name, link) in &self.native {
            if !crate::js::identifier(name) || !crate::catalog::native_link_identifier(link) {
                return Err(format!("host.native.{name} must name a callback ABI v3 provider in the host_ namespace"));
            }
            if !links.insert(link) {
                return Err(format!("duplicate native provider `{link}`"));
            }
        }
        if self
            .native_sources
            .iter()
            .any(|path| path.as_os_str().is_empty())
        {
            return Err("host.native_sources cannot contain an empty path".into());
        }
        Ok(())
    }
}
