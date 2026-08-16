use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "lilscript-codec")]
#[command(version)]
#[command(about = "Measure artifacts with LilScript's canonical transfer codecs.")]
struct Args {
    /// Emit the schema-versioned JSON measurement document.
    #[arg(long, required = true)]
    json: bool,

    /// Artifact paths to measure, in output order.
    #[arg(required = true)]
    paths: Vec<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u32,
    codecs: CodecProvenance,
    artifacts: Vec<ArtifactMeasurement>,
}

#[derive(Serialize)]
struct CodecProvenance {
    gzip9: GzipProvenance,
    brotli11: BrotliProvenance,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GzipProvenance {
    encoder: &'static str,
    library_version: &'static str,
    cargo_package: &'static str,
    cargo_package_version: &'static str,
    level: u8,
    mtime: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrotliProvenance {
    encoder: &'static str,
    library_version: &'static str,
    cargo_package: &'static str,
    cargo_package_version: &'static str,
    quality: u8,
    lgwin: u8,
    mode: &'static str,
}

#[derive(Serialize)]
struct ArtifactMeasurement {
    path: String,
    raw: usize,
    gzip9: usize,
    brotli11: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("lilscript-codec: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    debug_assert!(args.json);

    let zlib_version = lilscript::canonical_zlib_version()?;
    if zlib_version != lilscript::CANONICAL_ZLIB_LIBRARY_VERSION {
        return Err(format!(
            "expected bundled zlib {}, linked {zlib_version}",
            lilscript::CANONICAL_ZLIB_LIBRARY_VERSION
        ));
    }
    let brotli_version = lilscript::canonical_brotli_version();
    if brotli_version != lilscript::CANONICAL_BROTLI_LIBRARY_VERSION {
        return Err(format!(
            "expected bundled Brotli {:#010x}, linked {brotli_version:#010x}",
            lilscript::CANONICAL_BROTLI_LIBRARY_VERSION
        ));
    }

    let mut artifacts = Vec::with_capacity(args.paths.len());
    for path in args.paths {
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let sizes = lilscript::measure_javascript_transfer_sizes(&bytes)
            .map_err(|error| format!("failed to measure {}: {error}", path.display()))?;
        artifacts.push(ArtifactMeasurement {
            path: path.to_string_lossy().into_owned(),
            raw: sizes.raw,
            gzip9: sizes.gzip9,
            brotli11: sizes.brotli11,
        });
    }

    let report = Report {
        schema_version: 1,
        codecs: CodecProvenance {
            gzip9: GzipProvenance {
                encoder: "upstream-stock-zlib-c",
                library_version: lilscript::CANONICAL_ZLIB_LIBRARY_VERSION,
                cargo_package: "libz-sys",
                cargo_package_version: lilscript::CANONICAL_ZLIB_PACKAGE_VERSION,
                level: 9,
                mtime: 0,
            },
            brotli11: BrotliProvenance {
                encoder: "official-google-brotli-c",
                library_version: "1.1.0",
                cargo_package: "compu-brotli-sys",
                cargo_package_version: lilscript::CANONICAL_BROTLI_PACKAGE_VERSION,
                quality: 11,
                lgwin: 22,
                mode: "generic",
            },
        },
        artifacts,
    };
    println!(
        "{}",
        serde_json::to_string(&report)
            .map_err(|error| format!("failed to serialize measurements: {error}"))?
    );
    Ok(())
}
