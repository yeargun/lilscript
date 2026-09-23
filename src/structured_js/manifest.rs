//! The manifest of a multi-file JavaScript delivery: every delivered file
//! with its transfer sizes, depth from the entry, reachability, deploy cost
//! and cache key, and one build id over the whole delivery.
//!
//! Each file is measured once with the canonical codecs; the deploy cost is
//! `ChunkCostConfig::deploy_cost`, the rule split delivery selects by.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::{BundleMode, ChunkCostConfig, CompressionCostModel};
use crate::stable_hash::StableHashMap;

/// One delivered file, as the manifest measures it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFile {
    pub file_name: String,
    /// Source module names, relative to the entry module's directory.
    pub modules: Vec<String>,
    pub dependencies: Vec<String>,
    /// Lazy chunks this file loads with `import()`.
    pub dynamic_dependencies: Vec<String>,
    /// Loaded only by `import()`.
    pub lazy: bool,
    /// Modules importing this file's module, when split counts them toward
    /// cache reuse; zero otherwise.
    pub importers: usize,
    pub code: String,
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

/// The delivered files and their manifest. `entry` comes first; `codec` is
/// the objective's codec, whose bytes the manifest totals as transfer.
pub fn javascript_bundle(
    entry: ManifestFile,
    chunks: Vec<ManifestFile>,
    preload: Vec<String>,
    mode: BundleMode,
    codec: CompressionCostModel,
    cost: &ChunkCostConfig,
) -> Result<JavaScriptBundle, String> {
    let files = std::iter::once(&entry).chain(&chunks).collect::<Vec<_>>();
    let mut depths = StableHashMap::default();
    depths.insert(entry.file_name.clone(), 0usize);
    let mut changed = true;
    while changed {
        changed = false;
        for file in &files {
            let Some(depth) = depths.get(&file.file_name).copied() else {
                continue;
            };
            for dependency in file.dependencies.iter().chain(&file.dynamic_dependencies) {
                let candidate = depth.saturating_add(1);
                let slot = depths.entry(dependency.clone()).or_insert(candidate);
                if candidate < *slot {
                    *slot = candidate;
                    changed = true;
                }
            }
        }
    }
    let mut reachability: StableHashMap<String, usize> = StableHashMap::default();
    for file in &files {
        let mut dependencies = file
            .dependencies
            .iter()
            .chain(&file.dynamic_dependencies)
            .collect::<Vec<_>>();
        dependencies.sort_unstable();
        dependencies.dedup();
        for dependency in dependencies {
            *reachability.entry(dependency.clone()).or_insert(0) += 1;
        }
    }
    let selected = |raw: usize, gzip: usize, brotli: usize| match codec {
        CompressionCostModel::Raw => raw,
        CompressionCostModel::Gzip => gzip,
        CompressionCostModel::Brotli => brotli,
    };
    let mut deploy_cost = 0u64;
    let mut selected_transfer_bytes = 0usize;
    let mut manifest_chunks = Vec::with_capacity(chunks.len());
    for (index, file) in files.iter().enumerate() {
        let sizes = crate::compression::measure_javascript_transfer_sizes(file.code.as_bytes())?;
        let (gzip_bytes, brotli_bytes) = (sizes.gzip9, sizes.brotli11);
        let depth = depths.get(&file.file_name).copied().unwrap_or(0);
        let file_cost = cost.deploy_cost(
            file.code.len(),
            gzip_bytes,
            brotli_bytes,
            depth,
            index != 0 && preload.contains(&file.file_name),
            reachability
                .get(&file.file_name)
                .copied()
                .unwrap_or(0)
                .max(file.importers),
        );
        deploy_cost = deploy_cost.saturating_add(file_cost);
        let transfer = selected(file.code.len(), gzip_bytes, brotli_bytes);
        selected_transfer_bytes = selected_transfer_bytes.saturating_add(transfer);
        if index == 0 {
            continue;
        }
        manifest_chunks.push(JavaScriptBundleManifestChunk {
            file: file.file_name.clone(),
            modules: file.modules.clone(),
            bytes: file.code.len(),
            gzip_bytes,
            brotli_bytes,
            selected_transfer_bytes: transfer,
            kind: if file.lazy { "lazy" } else { "static" }.to_string(),
            dependencies: file.dependencies.clone(),
            dynamic_dependencies: file.dynamic_dependencies.clone(),
            cache_key: content_hash(file.code.as_bytes()),
            deploy_cost: file_cost,
        });
    }
    let mut hasher = Sha256::new();
    for file in &files {
        hasher.update((file.file_name.len() as u64).to_le_bytes());
        hasher.update(file.file_name.as_bytes());
        hasher.update((file.code.len() as u64).to_le_bytes());
        hasher.update(file.code.as_bytes());
    }
    let build_id = content_hash(&hasher.finalize());
    let objective = JavaScriptBundleObjectiveManifest {
        javascript_codec: codec.name().to_string(),
        raw_weight: cost.raw_weight,
        gzip_weight: cost.gzip_weight,
        brotli_weight: cost.brotli_weight,
        request_overhead_bytes: cost.request_overhead_bytes,
        dependency_depth_penalty_bytes: cost.dependency_depth_penalty_bytes,
        preload_request_discount_percent: cost.preload_request_discount_percent,
        cache_reuse_discount_percent: cost.cache_reuse_discount_percent,
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
    let entry_file = entry.file_name.clone();
    let bundle_files = std::iter::once(entry)
        .chain(chunks)
        .map(|file| JavaScriptBundleFile {
            file_name: file.file_name,
            code: file.code,
        })
        .collect();
    Ok(JavaScriptBundle {
        files: bundle_files,
        manifest: JavaScriptBundleManifest {
            version: 2,
            build_id,
            mode: mode.name().to_string(),
            entry: entry_file,
            preload,
            objective,
            objective_fingerprint,
            selected_transfer_bytes,
            deploy_cost,
            chunks: manifest_chunks,
        },
    })
}

/// Lowercase hexadecimal SHA-256 of `bytes`.
pub fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, dependencies: &[&str], code: &str) -> ManifestFile {
        ManifestFile {
            file_name: name.to_string(),
            modules: vec![format!("{name}.lil")],
            dependencies: dependencies.iter().map(|name| name.to_string()).collect(),
            dynamic_dependencies: Vec::new(),
            lazy: false,
            importers: 0,
            code: code.to_string(),
        }
    }

    #[test]
    fn the_manifest_measures_every_chunk_and_costs_its_depth() {
        let cost = ChunkCostConfig::default();
        let bundle = javascript_bundle(
            file("entry.mjs", &["a.mjs"], "import{a}from\"./a.mjs\";a();"),
            vec![
                file("a.mjs", &["b.mjs"], "import{b}from\"./b.mjs\";export let a=()=>b();"),
                file("b.mjs", &[], "export let b=()=>1;"),
            ],
            Vec::new(),
            BundleMode::Split,
            CompressionCostModel::Brotli,
            &cost,
        )
        .unwrap();
        let manifest = &bundle.manifest;
        assert_eq!(manifest.version, 2);
        assert_eq!(manifest.mode, "split");
        assert_eq!(manifest.entry, "entry.mjs");
        assert_eq!(manifest.objective.javascript_codec, "brotli");
        assert_eq!(bundle.files.len(), 3);
        assert_eq!(manifest.chunks.len(), 2);
        let [a, b] = [&manifest.chunks[0], &manifest.chunks[1]];
        for chunk in [a, b] {
            let code = &bundle.files.iter().find(|f| f.file_name == chunk.file).unwrap().code;
            let sizes = crate::compression::measure_javascript_transfer_sizes(code.as_bytes()).unwrap();
            assert_eq!((chunk.bytes, chunk.gzip_bytes, chunk.brotli_bytes), (sizes.raw, sizes.gzip9, sizes.brotli11));
            assert_eq!(chunk.selected_transfer_bytes, sizes.brotli11);
            assert_eq!(chunk.cache_key, content_hash(code.as_bytes()));
        }
        // One level below the entry pays a request; two levels also pay depth.
        assert_eq!(a.deploy_cost, cost.deploy_cost(a.bytes, a.gzip_bytes, a.brotli_bytes, 1, false, 1));
        assert_eq!(b.deploy_cost, cost.deploy_cost(b.bytes, b.gzip_bytes, b.brotli_bytes, 2, false, 1));
        assert_eq!(manifest.build_id.len(), 64);
    }
}
