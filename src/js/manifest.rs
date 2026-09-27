//! Manifest v3 of a multi-file JavaScript delivery (plan M3.3, design §10):
//! per output, every entry with its closure and rows, and every file with
//! its role, label, source modules, links, bytes, objective-codec bytes and
//! SHA-256; `side_effects` lists the files whose loading runs code, ready
//! for `package.json`.
//!
//! It reads the sizes the search measured on the delivered bytes and never
//! encodes again: a file the objective never scored (level 0 records
//! estimates only) reports `null`.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::config::CompressionCostModel;
use crate::js::delivery::DeliveredLayout;
use crate::js::selection::Sizes;

/// One delivered file as the manifest reads it.
pub struct ManifestFile<'a> {
    pub name: &'a str,
    pub code: &'a str,
    pub sizes: Sizes,
}

/// One output of a build: its files in plan order and its layout.
pub struct ManifestOutput<'a> {
    pub files: Vec<ManifestFile<'a>>,
    pub layout: &'a DeliveredLayout,
}

fn codec_size(sizes: &Sizes, codec: CompressionCostModel) -> Option<usize> {
    match codec {
        CompressionCostModel::Raw => Some(sizes.raw),
        CompressionCostModel::Gzip => sizes.gzip9,
        CompressionCostModel::Brotli => sizes.brotli11,
    }
}

fn codec_name(codec: CompressionCostModel) -> &'static str {
    match codec {
        CompressionCostModel::Raw => "raw",
        CompressionCostModel::Gzip => "gzip",
        CompressionCostModel::Brotli => "brotli",
    }
}

/// The manifest of a build's outputs. `modules` names each source module
/// (relative paths); `codec` is the objective's.
pub fn manifest_v3(
    outputs: &[ManifestOutput<'_>],
    modules: &[String],
    codec: CompressionCostModel,
) -> Value {
    let module_name = |module: u32| {
        modules
            .get(module as usize)
            .cloned()
            .unwrap_or_else(|| format!("module-{module}"))
    };
    let outputs = outputs
        .iter()
        .map(|output| {
            let layout = output.layout;
            let name = |index: u32| {
                output
                    .files
                    .get(index as usize)
                    .map_or("", |file| file.name)
            };
            let per_file = output
                .files
                .iter()
                .map(|file| codec_size(&file.sizes, codec))
                .collect::<Option<Vec<_>>>();
            let rows = per_file.as_ref().map(|sizes| layout.rows(sizes));
            let raw_rows = layout.rows(
                &output
                    .files
                    .iter()
                    .map(|file| file.code.len())
                    .collect::<Vec<_>>(),
            );
            let files = layout
                .files
                .iter()
                .zip(&output.files)
                .map(|(file, delivered)| {
                    json!({
                        "file": delivered.name,
                        "role": format!("{:?}", file.role),
                        "label": layout.label_names(&file.label),
                        "modules": file.modules.iter().map(|&module| module_name(module)).collect::<Vec<_>>(),
                        "anchored": file.anchored,
                        "imports": file.imports.iter().map(|&target| name(target)).collect::<Vec<_>>(),
                        "dynamic_imports": file.dynamic.iter().map(|&target| name(target)).collect::<Vec<_>>(),
                        "bytes": delivered.code.len(),
                        "codec_bytes": codec_size(&delivered.sizes, codec),
                        "sha256": format!("{:x}", Sha256::digest(delivered.code.as_bytes())),
                    })
                })
                .collect::<Vec<_>>();
            let entries = layout
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    json!({
                        "name": entry.name,
                        "file": name(entry.file),
                        "dynamic": entry.dynamic,
                        "closure": entry.closure.iter().map(|&file| name(file)).collect::<Vec<_>>(),
                        "row": rows.as_ref().map(|rows| rows[index]),
                        "raw_row": raw_rows[index],
                    })
                })
                .collect::<Vec<_>>();
            let side_effects = layout
                .files
                .iter()
                .zip(&output.files)
                .filter(|(file, _)| file.anchored)
                .map(|(_, delivered)| delivered.name)
                .collect::<Vec<_>>();
            json!({
                "mode": layout.mode.name(),
                "format": layout.format.name(),
                "entries": entries,
                "files": files,
                "side_effects": side_effects,
                "codec_total": per_file.as_ref().map(|sizes| sizes.iter().sum::<usize>()),
                "rows_total": rows.as_ref().map(|rows| rows.iter().sum::<u64>()),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "version": 3,
        "codec": codec_name(codec),
        "outputs": outputs,
    })
}
