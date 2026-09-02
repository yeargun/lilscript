use std::fs;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, ValueEnum};
use sha2::{Digest, Sha256};

use lilscript::config::{
    load_project_config, BundleMode, CandidateSearch, JavaScriptSourceMapMode, ProjectConfig,
};
use lilscript::package::write_lockfile;
use lilscript::{
    compile_path_all_configured, compile_path_all_to_js_bundle_configured, compile_path_configured,
    compile_path_explained_configured, compile_path_to_c_configured,
    compile_path_to_js_bundle_configured, compile_path_to_js_module_configured,
    compile_path_to_js_module_explained_configured, profile_template_path_configured,
    render_module_diagnostic, JavaScriptAnalysisMap, JavaScriptBundle, JavaScriptSourceMap,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Target {
    Js,
    JsModule,
    C,
    Native,
    All,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BuildMode {
    Development,
    Production,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExplainFormat {
    Human,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "lilscript")]
#[command(version)]
#[command(about = "Compile LilScript source to optimized JavaScript, C, or a native executable.")]
struct Args {
    /// LilScript source file to compile.
    input: PathBuf,

    /// Inspect a `.lilmap.json` sidecar instead of compiling the input path.
    #[arg(long)]
    inspect_analysis: bool,

    /// Verify that this JavaScript file has the SHA-256 named by the analysis map.
    #[arg(long, value_name = "JS", requires = "inspect_analysis")]
    verify_artifact: Option<PathBuf>,

    /// Output file, or the base path used by `--target all`.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Compilation target.
    #[arg(long, value_enum, default_value_t = Target::Js)]
    target: Target,

    /// Explicit config path. Otherwise `lilscript.toml` is discovered from the input directory.
    #[arg(long)]
    config: Option<PathBuf>,

    /// JavaScript compiler worker threads. Omit to use RAYON_NUM_THREADS or the host default.
    #[arg(short = 'j', long, value_name = "N")]
    jobs: Option<NonZeroUsize>,

    /// Maximum concurrent terminal Brotli finalizer workers.
    #[arg(long, value_name = "N")]
    codec_jobs: Option<NonZeroUsize>,

    /// Development skips compressor-in-loop candidate search; production uses project policy.
    #[arg(long, value_enum, default_value_t = BuildMode::Production)]
    mode: BuildMode,

    /// Print optimizer pass decisions to stderr without contaminating JavaScript stdout.
    #[arg(long, value_enum)]
    explain: Option<ExplainFormat>,

    /// Resolve all path dependencies and rewrite lilscript.lock before compiling.
    #[arg(long)]
    write_lock: bool,

    /// Write a versioned profile template with stable function/loop keys, then exit.
    #[arg(long)]
    profile_template: Option<PathBuf>,

    /// Force a single ESM artifact for an external bundler such as Lilpack.
    #[arg(long, hide = true)]
    delegate_bundling: bool,

    /// Print a versioned JSON code/source-map artifact for an external bundler.
    #[arg(long, hide = true, requires = "delegate_bundling")]
    print_delegated_artifact: bool,

    /// Print compiler inputs as JSON for an external incremental build graph, then exit.
    #[arg(long, hide = true)]
    print_dependencies: bool,
}

fn main() {
    let started = std::time::Instant::now();
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    if let Some(report) = lilscript::timing::report(started.elapsed().as_nanos()) {
        eprintln!("lilscript-timing {report}");
        if let Some(folds) = lilscript::timing::idle_fold_report(24) {
            eprint!("{folds}");
        }
    }
    report_store_census();
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.inspect_analysis {
        return inspect_analysis_map(&args.input, args.verify_artifact.as_deref());
    }
    let mut loaded = load_project_config(&args.input, args.config.as_deref())
        .map_err(|error| error.to_string())?;
    apply_resource_overrides(&mut loaded.config, args.jobs, args.codec_jobs);
    if args.write_lock {
        let path = write_lockfile(&loaded.config).map_err(|error| error.to_string())?;
        eprintln!("wrote {}", path.display());
    }
    if args.delegate_bundling {
        loaded.config.bundle.mode = BundleMode::Single;
    }
    if matches!(args.mode, BuildMode::Development) {
        loaded.config.javascript.candidate_search = CandidateSearch::Off;
    }
    if args.print_dependencies {
        return print_dependencies(&args.input, &loaded);
    }
    if let Some(output) = &args.profile_template {
        let profile = profile_template_path_configured(&args.input, &loaded.config)
            .map_err(|error| render_module_diagnostic(&error))?;
        let json = serde_json::to_string_pretty(&profile)
            .map_err(|error| format!("failed to serialize profile template: {error}"))?;
        fs::write(output, format!("{json}\n"))
            .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
        return Ok(());
    }
    if args.print_delegated_artifact {
        if !matches!(args.target, Target::JsModule) {
            return Err("--print-delegated-artifact requires --target js-module".to_string());
        }
        if args.output.is_some() || args.explain.is_some() {
            return Err(
                "--print-delegated-artifact cannot be combined with --output or --explain"
                    .to_string(),
            );
        }
        return print_delegated_javascript_artifact(&args.input, &loaded.config);
    }
    if loaded.config.javascript.analysis_map.level.enabled()
        && args.output.is_none()
        && matches!(args.target, Target::Js | Target::JsModule)
    {
        return Err(
            "analysis maps require an explicit --output JavaScript file for the sidecar"
                .to_string(),
        );
    }
    match args.target {
        Target::Js => {
            if loaded.config.bundle.mode == BundleMode::Single {
                if args.explain.is_some()
                    || loaded.config.javascript.source_map.enabled
                    || loaded.config.javascript.analysis_map.level.enabled()
                {
                    let compilation =
                        compile_path_explained_configured(&args.input, &loaded.config)
                            .map_err(|error| render_module_diagnostic(&error))?;
                    if let Some(format) = args.explain {
                        print_explanation(
                            format,
                            &compilation.optimization_reports,
                            &compilation.selection_metrics,
                            &compilation.abi_manifest,
                            compilation.source_map.as_ref(),
                            compilation.analysis_map.as_ref(),
                        )?;
                    }
                    write_javascript_artifact(
                        args.output.as_deref(),
                        &compilation.javascript,
                        compilation.source_map.as_ref(),
                        compilation.analysis_map.as_ref(),
                        loaded.config.javascript.source_map.mode,
                    )?;
                } else {
                    let js = compile_path_configured(&args.input, &loaded.config)
                        .map_err(|error| render_module_diagnostic(&error))?;
                    write_javascript_artifact(
                        args.output.as_deref(),
                        &js,
                        None,
                        None,
                        loaded.config.javascript.source_map.mode,
                    )?;
                }
            } else {
                if args.explain.is_some() {
                    return Err("--explain currently requires bundle.mode=\"single\"".to_string());
                }
                write_configured_bundle(&args.input, args.output.as_deref(), &loaded.config)?;
            }
        }
        Target::JsModule => {
            if loaded.config.bundle.mode == BundleMode::Single {
                if args.explain.is_some()
                    || loaded.config.javascript.source_map.enabled
                    || loaded.config.javascript.analysis_map.level.enabled()
                {
                    let compilation =
                        compile_path_to_js_module_explained_configured(&args.input, &loaded.config)
                            .map_err(|error| render_module_diagnostic(&error))?;
                    if let Some(format) = args.explain {
                        print_explanation(
                            format,
                            &compilation.optimization_reports,
                            &compilation.selection_metrics,
                            &compilation.abi_manifest,
                            compilation.source_map.as_ref(),
                            compilation.analysis_map.as_ref(),
                        )?;
                    }
                    write_javascript_artifact(
                        args.output.as_deref(),
                        &compilation.javascript,
                        compilation.source_map.as_ref(),
                        compilation.analysis_map.as_ref(),
                        loaded.config.javascript.source_map.mode,
                    )?;
                } else {
                    let js = compile_path_to_js_module_configured(&args.input, &loaded.config)
                        .map_err(|error| render_module_diagnostic(&error))?;
                    write_javascript_artifact(
                        args.output.as_deref(),
                        &js,
                        None,
                        None,
                        loaded.config.javascript.source_map.mode,
                    )?;
                }
            } else {
                if args.explain.is_some() {
                    return Err("--explain currently requires bundle.mode=\"single\"".to_string());
                }
                write_configured_bundle(&args.input, args.output.as_deref(), &loaded.config)?;
            }
        }
        Target::C => {
            let c = compile_path_to_c_configured(&args.input, &loaded.config)
                .map_err(|error| render_module_diagnostic(&error))?;
            write_or_print(args.output.as_deref(), &c)?;
        }
        Target::Native => {
            let c = compile_path_to_c_configured(&args.input, &loaded.config)
                .map_err(|error| render_module_diagnostic(&error))?;
            let output = args.output.unwrap_or_else(|| {
                let mut output = args.input.clone();
                output.set_extension("");
                output
            });
            compile_native(&c, &output)?;
        }
        Target::All => {
            let base = args.output.unwrap_or_else(|| {
                let mut output = args.input.clone();
                output.set_extension("");
                output
            });
            let javascript = base.with_extension("js");
            let c = base.with_extension("c");
            if loaded.config.bundle.mode == BundleMode::Single {
                let artifacts = compile_path_all_configured(&args.input, &loaded.config)
                    .map_err(|error| render_module_diagnostic(&error))?;
                ensure_parent(&base)?;
                write_javascript_artifact(
                    Some(&javascript),
                    &artifacts.javascript,
                    artifacts.source_map.as_ref(),
                    artifacts.analysis_map.as_ref(),
                    loaded.config.javascript.source_map.mode,
                )?;
                fs::write(&c, &artifacts.c)
                    .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
                compile_native(&artifacts.c, &base)?;
            } else {
                let entry_file = javascript
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| "bundle output must have a UTF-8 file name".to_string())?;
                let artifacts = compile_path_all_to_js_bundle_configured(
                    &args.input,
                    &loaded.config,
                    entry_file,
                )
                .map_err(|error| render_module_diagnostic(&error))?;
                ensure_parent(&base)?;
                write_javascript_bundle(
                    &javascript,
                    &artifacts.javascript,
                    loaded.config.javascript.source_map.mode,
                )?;
                fs::write(&c, &artifacts.c)
                    .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
                compile_native(&artifacts.c, &base)?;
            }
        }
    }

    Ok(())
}

fn print_delegated_javascript_artifact(input: &Path, config: &ProjectConfig) -> Result<(), String> {
    let (code, source_map, analysis_map) =
        if config.javascript.source_map.enabled || config.javascript.analysis_map.level.enabled() {
            let compilation = compile_path_to_js_module_explained_configured(input, config)
                .map_err(|error| render_module_diagnostic(&error))?;
            (
                compilation.javascript,
                compilation.source_map,
                compilation.analysis_map,
            )
        } else {
            let code = compile_path_to_js_module_configured(input, config)
                .map_err(|error| render_module_diagnostic(&error))?;
            (code, None, None)
        };
    let source_map = source_map
        .as_ref()
        .map(|source_map| {
            serde_json::from_str::<serde_json::Value>(source_map.as_str())
                .map_err(|error| format!("failed to encode delegated source map: {error}"))
        })
        .transpose()?;
    let analysis_map = analysis_map
        .as_ref()
        .map(|analysis_map| {
            serde_json::from_str::<serde_json::Value>(analysis_map.as_str())
                .map_err(|error| format!("failed to encode delegated analysis map: {error}"))
        })
        .transpose()?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "version": 1,
            "code": code,
            "map": source_map,
            "analysisMap": analysis_map,
        }))
        .map_err(|error| format!("failed to serialize delegated JavaScript artifact: {error}"))?
    );
    Ok(())
}

fn inspect_analysis_map(path: &Path, artifact: Option<&Path>) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let document: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("invalid analysis map {}: {error}", path.display()))?;
    let object = document
        .as_object()
        .ok_or_else(|| "analysis map root must be a JSON object".to_string())?;
    if object.get("kind").and_then(serde_json::Value::as_str)
        != Some("lilscript-javascript-analysis-map")
    {
        return Err("unsupported analysis-map kind".to_string());
    }
    if object.get("version").and_then(serde_json::Value::as_u64)
        != Some(u64::from(lilscript::JAVASCRIPT_ANALYSIS_MAP_VERSION))
    {
        return Err(format!(
            "unsupported analysis-map version (expected {})",
            lilscript::JAVASCRIPT_ANALYSIS_MAP_VERSION
        ));
    }
    let level = object
        .get("level")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "analysis map is missing `level`".to_string())?;
    if !matches!(level, "summary" | "full") {
        return Err(format!("unsupported analysis-map level `{level}`"));
    }
    let artifact_identity = object
        .get("artifact")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "analysis map is missing `artifact`".to_string())?;
    let expected_hash = artifact_identity
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "analysis map has an invalid artifact SHA-256".to_string())?;
    let summary = object
        .get("summary")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "analysis map is missing `summary`".to_string())?;
    let decisions = object
        .get("decisions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "analysis map is missing `decisions`".to_string())?;
    let declared_decisions = summary
        .get("decisions")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "analysis map summary is missing `decisions`".to_string())?;
    if usize::try_from(declared_decisions).ok() != Some(decisions.len()) {
        return Err(format!(
            "analysis-map summary declares {declared_decisions} decisions but contains {}",
            decisions.len()
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for decision in decisions {
        let decision = decision
            .as_object()
            .ok_or_else(|| "analysis-map decision must be an object".to_string())?;
        let id = decision
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "analysis-map decision is missing `id`".to_string())?;
        if !ids.insert(id) {
            return Err(format!("duplicate analysis-map decision id `{id}`"));
        }
    }

    // The report is built first and written once: a reader that closes the
    // pipe early (`| head`) must end the inspector quietly, not panic it.
    use std::fmt::Write as _;
    let mut report = String::new();
    writeln!(report, "analysis map        {}", path.display()).expect("String is infallible");
    writeln!(
        report,
        "schema/version      {level}/{}",
        lilscript::JAVASCRIPT_ANALYSIS_MAP_VERSION
    )
    .expect("String is infallible");
    writeln!(report, "artifact sha256     {expected_hash}").expect("String is infallible");
    for (label, key) in [
        ("decisions", "decisions"),
        ("identifiers", "identifiers"),
        ("properties", "properties"),
        ("exports", "exports"),
        ("mangled", "mangled"),
        ("preserved", "preserved"),
        ("coalesced bindings", "coalescedBindings"),
    ] {
        if let Some(value) = summary.get(key).and_then(serde_json::Value::as_u64) {
            writeln!(report, "{label:<20} {value}").expect("String is infallible");
        }
    }
    if let Some(artifact) = artifact {
        let bytes = fs::read(artifact)
            .map_err(|error| format!("failed to read {}: {error}", artifact.display()))?;
        let verification = verify_analysis_artifact(&bytes, expected_hash);
        if verification.is_none() {
            let actual_hash = sha256_hex(&bytes);
            return Err(format!(
                "artifact hash mismatch for {}: expected {expected_hash}, got {actual_hash}",
                artifact.display()
            ));
        }
        let scope = match verification {
            Some(ArtifactHashMatch::Exact) => "",
            Some(ArtifactHashMatch::BeforeSourceMapComment) => {
                " (before source-map publication comment)"
            }
            None => unreachable!("hash mismatch returned above"),
        };
        writeln!(report, "artifact verified   {}{scope}", artifact.display())
            .expect("String is infallible");
    }
    for decision in decisions {
        let kind = decision
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("name");
        let source = decision
            .get("source")
            .and_then(serde_json::Value::as_object);
        let generated = decision
            .get("generated")
            .and_then(serde_json::Value::as_object);
        let original = source
            .and_then(|source| source.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?");
        let emitted = generated
            .and_then(|generated| generated.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?");
        let rule = decision
            .get("primaryRule")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let source_path = source
            .and_then(|source| source.get("path"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?");
        let line = source
            .and_then(|source| source.get("line"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            + 1;
        let column = source
            .and_then(|source| source.get("column"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            + 1;
        writeln!(
            report,
            "{kind:<10} {original} -> {emitted}  {rule}  {source_path}:{line}:{column}"
        )
        .expect("String is infallible");
    }
    write_report(&report)?;
    Ok(())
}

/// Writes to stdout, treating a closed pipe as the reader having seen enough.
fn write_report(report: &str) -> Result<(), String> {
    use std::io::Write as _;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match handle
        .write_all(report.as_bytes())
        .and_then(|()| handle.flush())
    {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(format!("failed to write report: {error}")),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    encoded
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactHashMatch {
    Exact,
    BeforeSourceMapComment,
}

fn verify_analysis_artifact(bytes: &[u8], expected_hash: &str) -> Option<ArtifactHashMatch> {
    if sha256_hex(bytes) == expected_hash {
        return Some(ArtifactHashMatch::Exact);
    }
    let published = std::str::from_utf8(bytes).ok()?;
    let without_terminal_newline = published.strip_suffix('\n').unwrap_or(published);
    let (before_comment, comment) = without_terminal_newline.rsplit_once('\n')?;
    let url = comment.strip_prefix("//# sourceMappingURL=")?;
    if url.is_empty() {
        return None;
    }
    if sha256_hex(before_comment.as_bytes()) == expected_hash {
        return Some(ArtifactHashMatch::BeforeSourceMapComment);
    }
    let mut with_terminal_newline = String::with_capacity(before_comment.len() + 1);
    with_terminal_newline.push_str(before_comment);
    with_terminal_newline.push('\n');
    (sha256_hex(with_terminal_newline.as_bytes()) == expected_hash)
        .then_some(ArtifactHashMatch::BeforeSourceMapComment)
}

fn apply_resource_overrides(
    config: &mut ProjectConfig,
    jobs: Option<NonZeroUsize>,
    codec_jobs: Option<NonZeroUsize>,
) {
    if let Some(jobs) = jobs {
        config.compiler.resources.threads = Some(jobs);
    }
    if let Some(codec_jobs) = codec_jobs {
        config.compiler.resources.codec_workers = codec_jobs;
    }
}

fn print_dependencies(
    input: &Path,
    loaded: &lilscript::config::LoadedConfig,
) -> Result<(), String> {
    let modules = lilscript::module::discover_modules_configured(input, &loaded.config)
        .map_err(|error| render_module_diagnostic(&error))?;
    let mut files = modules
        .modules
        .iter()
        .map(|module| module.path.clone())
        .collect::<Vec<_>>();
    if let Some(path) = &loaded.path {
        files.push(path.canonicalize().unwrap_or_else(|_| path.clone()));
    }
    if let Some(root) = &loaded.config.config_dir {
        let lockfile = root.join("lilscript.lock");
        if lockfile.is_file() {
            files.push(lockfile.canonicalize().unwrap_or(lockfile));
        }
        if let Some(profile) = &loaded.config.profile.path {
            let path = if profile.is_absolute() {
                profile.clone()
            } else {
                root.join(profile)
            };
            if path.is_file() {
                files.push(path.canonicalize().unwrap_or(path));
            }
        }
    }
    files.sort();
    files.dedup();
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "version": 1,
            "entry": modules.modules[modules.root].path,
            "files": files,
        }))
        .map_err(|error| format!("failed to serialize compiler inputs: {error}"))?
    );
    Ok(())
}

fn print_explanation(
    format: ExplainFormat,
    reports: &[lilscript::optimizer::OptimizationReport],
    metrics: &lilscript::JavaScriptSelectionMetrics,
    abi: &lilscript::JavaScriptAbiManifest,
    source_map: Option<&JavaScriptSourceMap>,
    analysis_map: Option<&JavaScriptAnalysisMap>,
) -> Result<(), String> {
    match format {
        ExplainFormat::Human => {
            for report in reports {
                eprintln!(
                    "{:<34} {}",
                    report.pass_name,
                    if report.changed {
                        "changed"
                    } else {
                        "unchanged"
                    }
                );
            }
            let census = lilscript::compiler::store_census();
            if census.iter().any(|count| *count != 0) {
                for (label, count) in [
                    "store: crosses blocks",
                    "store: used more than once",
                    "store: unstable and unfused",
                    "store: single use, fusion refused",
                    "store: other",
                    "  of which: only a fall-through edge",
                ]
                .into_iter()
                .zip(census)
                {
                    eprintln!("{label:<34} {count}");
                }
            }
            eprintln!("{:<34} {}", "javascript codec", metrics.codec);
            eprintln!("{:<34} {}", "ABI world", abi.world);
            eprintln!(
                "{:<34} {}",
                "public aggregate ABI", abi.public_aggregate_abi
            );
            if abi.exports.is_empty() {
                eprintln!("{:<34} {}", "runtime exports", "(none)");
            } else {
                eprintln!(
                    "{:<34} {}",
                    "runtime exports",
                    abi.exports
                        .iter()
                        .map(|export| format!("{}:{:?}", export.name, export.kind))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            eprintln!(
                "{:<34} {}",
                "selected transfer bytes", metrics.transfer_bytes
            );
            eprintln!("{:<34} {}", "syntax tokens", metrics.syntax.tokens);
            eprintln!("{:<34} {}", "syntax AST nodes", metrics.syntax.ast_nodes);
            eprintln!(
                "{:<34} {}",
                "estimated parse cost", metrics.syntax.parse_cost
            );
            eprintln!(
                "{:<34} {}",
                "estimated compile cost", metrics.syntax.compile_cost
            );
            eprintln!(
                "{:<34} {}",
                "estimated startup memory", metrics.syntax.estimated_memory_bytes
            );
            eprintln!(
                "{:<34} {}",
                "JavaScript performance score", metrics.performance.score
            );
            eprintln!(
                "{:<34} {}",
                "deoptimization risk", metrics.performance.deoptimization_risk
            );
            eprintln!(
                "{:<34} {}",
                "allocation pressure", metrics.performance.allocation_pressure
            );
            eprintln!(
                "{:<34} {}",
                "indirect-call pressure", metrics.performance.indirect_call_pressure
            );
            eprintln!(
                "{:<34} {}",
                "monomorphic call weight", metrics.performance.monomorphic_call_sites
            );
            eprintln!(
                "{:<34} {}",
                "candidates evaluated", metrics.candidates_evaluated
            );
            eprintln!("{:<34} {}", "search guarantee", metrics.search_guarantee);
            eprintln!("{:<34} {}", "search stop", metrics.search_stop_reason);
            eprintln!(
                "{:<34} {}",
                "decision registry version", metrics.decision_registry_version
            );
            eprintln!("{:<34} {}", "plans registered", metrics.plans_registered);
            eprintln!(
                "{:<34} {}",
                "optimizer emissions attempted", metrics.optimizer_emissions_attempted
            );
            eprintln!(
                "{:<34} {}",
                "structural emissions attempted", metrics.emissions_attempted
            );
            eprintln!(
                "{:<34} {}/{}{}",
                "structural proposal work",
                metrics.candidate_proposal_work_units,
                metrics.candidate_proposal_limit,
                if metrics.candidate_proposal_limit_reached {
                    " (exhausted)"
                } else {
                    ""
                }
            );
            eprintln!(
                "{:<34} {}/{}{}",
                "terminal work",
                metrics.terminal_work_units,
                metrics.terminal_codec_probe_limit,
                if metrics.terminal_codec_probe_limit_reached {
                    " (exhausted)"
                } else {
                    ""
                }
            );
            eprintln!(
                "{:<34} {}",
                "terminal exact-codec calls", metrics.terminal_codec_probes
            );
            eprintln!("{:<34} {}", "peephole rewrites", metrics.peephole_rewrites);
            eprintln!(
                "{:<34} {}",
                "layout searched",
                if metrics.layout_searched { "yes" } else { "no" }
            );
            if metrics.cartesian_emission_axes.is_empty() {
                eprintln!("{:<34} {}", "cartesian emission axes", "(none)");
            } else {
                eprintln!(
                    "{:<34} {}",
                    "cartesian emission axes",
                    metrics.cartesian_emission_axes.join(", ")
                );
            }
            if metrics.scored_emission_families.is_empty() {
                eprintln!("{:<34} {}", "scored emission families", "(none)");
            } else {
                eprintln!(
                    "{:<34} {}",
                    "scored emission families",
                    metrics.scored_emission_families.join(", ")
                );
            }
            if metrics.starved_emission_families.is_empty() {
                eprintln!("{:<34} {}", "starved emission families", "(none)");
            } else {
                eprintln!(
                    "{:<34} {}",
                    "starved emission families",
                    metrics.starved_emission_families.join(", ")
                );
            }
            if metrics.ir_variants_searched.is_empty() {
                eprintln!("{:<34} {}", "scored ir variants", "(none)");
            } else {
                eprintln!(
                    "{:<34} {}",
                    "scored ir variants",
                    metrics.ir_variants_searched.join(", ")
                );
            }
            if metrics.removed_compression_families.is_empty() {
                eprintln!("{:<34} {}", "compression families removed", "(none)");
            } else {
                eprintln!(
                    "{:<34} {}",
                    "compression families removed",
                    metrics.removed_compression_families.join(", ")
                );
            }
            eprintln!(
                "{:<34} {} source, {} generated",
                "operation origin", metrics.source_operations, metrics.generated_operations
            );
            eprintln!(
                "{:<34} {}",
                "compiler time (microseconds)", metrics.compiler_time_micros
            );
            if let Some(source_map) = source_map {
                eprintln!("{:<34} {}", "source-map sources", source_map.source_count());
                eprintln!(
                    "{:<34} {}",
                    "source-map mappings",
                    source_map.mapping_count()
                );
                eprintln!(
                    "{:<34} {}",
                    "source-map original names",
                    source_map.original_name_count()
                );
            }
            if let Some(analysis_map) = analysis_map {
                eprintln!("{:<34} {:?}", "analysis-map level", analysis_map.level());
                eprintln!(
                    "{:<34} {}",
                    "analysis-map decisions",
                    analysis_map.decision_count()
                );
                eprintln!(
                    "{:<34} {}",
                    "analysis-map mangled names",
                    analysis_map.mangled_count()
                );
                eprintln!(
                    "{:<34} {}",
                    "analysis-map coalesced bindings",
                    analysis_map.coalesced_binding_count()
                );
            }
        }
        ExplainFormat::Json => eprintln!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "optimization_reports": reports,
                "javascript_selection": metrics,
                "abi_manifest": abi,
                "source_map": source_map.map(|map| serde_json::json!({
                    "sources": map.source_count(),
                    "mappings": map.mapping_count(),
                    "original_names": map.original_name_count(),
                })),
                "analysis_map": analysis_map.map(|map| serde_json::json!({
                    "level": map.level(),
                    "decisions": map.decision_count(),
                    "mangled": map.mangled_count(),
                    "coalesced_bindings": map.coalesced_binding_count(),
                    "artifact_sha256": map.artifact_sha256(),
                })),
            }))
            .map_err(|error| format!("failed to serialize optimization report: {error}"))?
        ),
    }
    Ok(())
}

fn write_configured_bundle(
    input: &Path,
    output: Option<&Path>,
    config: &lilscript::config::ProjectConfig,
) -> Result<(), String> {
    let output = output.ok_or_else(|| {
        "split and preserve-modules bundle modes require an explicit --output entry file"
            .to_string()
    })?;
    let entry_file = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "bundle output must have a UTF-8 file name".to_string())?;
    let bundle = compile_path_to_js_bundle_configured(input, config, entry_file)
        .map_err(|error| render_module_diagnostic(&error))?;
    write_javascript_bundle(output, &bundle, config.javascript.source_map.mode)
}

fn write_javascript_bundle(
    output: &Path,
    bundle: &JavaScriptBundle,
    source_map_mode: JavaScriptSourceMapMode,
) -> Result<(), String> {
    ensure_parent(output)?;
    let directory = output.parent().unwrap_or_else(|| Path::new("."));
    let manifest_path = output.with_extension("manifest.json");
    remove_stale_chunks(directory, &manifest_path, bundle)?;
    for file in &bundle.files {
        let path = if file.file_name == bundle.manifest.entry {
            output.to_path_buf()
        } else {
            directory.join(&file.file_name)
        };
        ensure_parent(&path)?;
        write_javascript_artifact(
            Some(&path),
            &file.code,
            file.source_map.as_ref(),
            file.analysis_map.as_ref(),
            source_map_mode,
        )?;
    }
    let manifest = serde_json::to_string_pretty(&bundle.manifest)
        .map_err(|error| format!("failed to serialize bundle manifest: {error}"))?;
    fs::write(&manifest_path, format!("{manifest}\n")).map_err(|error| {
        format!(
            "failed to write bundle manifest {}: {error}",
            manifest_path.display()
        )
    })
}

fn write_javascript_artifact(
    output: Option<&Path>,
    contents: &str,
    source_map: Option<&JavaScriptSourceMap>,
    analysis_map: Option<&JavaScriptAnalysisMap>,
    mode: JavaScriptSourceMapMode,
) -> Result<(), String> {
    if analysis_map.is_some() && output.is_none() {
        return Err(
            "analysis maps require an explicit --output JavaScript file for the sidecar"
                .to_string(),
        );
    }
    let publication = if let Some(source_map) = source_map {
        match (mode, output) {
            (JavaScriptSourceMapMode::Inline, None) => {
                println!(
                    "{}",
                    javascript_with_source_map_url(contents, source_map.data_url())
                );
                Ok(())
            }
            (JavaScriptSourceMapMode::Hidden | JavaScriptSourceMapMode::Linked, None) => Err(
                "hidden and linked source maps require an explicit --output JavaScript file"
                    .to_string(),
            ),
            (JavaScriptSourceMapMode::Inline, Some(output)) => {
                ensure_parent(output)?;
                let published = javascript_with_source_map_url(contents, source_map.data_url());
                fs::write(output, published)
                    .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
                remove_source_map_sidecar(output)
            }
            (JavaScriptSourceMapMode::Hidden, Some(output)) => {
                ensure_parent(output)?;
                let map_path = javascript_source_map_path(output);
                fs::write(&map_path, format!("{}\n", source_map.as_str()))
                    .map_err(|error| format!("failed to write {}: {error}", map_path.display()))?;
                fs::write(output, contents)
                    .map_err(|error| format!("failed to write {}: {error}", output.display()))
            }
            (JavaScriptSourceMapMode::Linked, Some(output)) => {
                ensure_parent(output)?;
                let map_path = javascript_source_map_path(output);
                fs::write(&map_path, format!("{}\n", source_map.as_str()))
                    .map_err(|error| format!("failed to write {}: {error}", map_path.display()))?;
                let map_name = map_path
                    .file_name()
                    .ok_or_else(|| "source-map output has no file name".to_string())?;
                let map_url = url_path_component(&map_name.to_string_lossy());
                let published = javascript_with_source_map_url(contents, &map_url);
                fs::write(output, published)
                    .map_err(|error| format!("failed to write {}: {error}", output.display()))
            }
        }
    } else {
        write_or_print(output, contents)?;
        if let Some(output) = output {
            remove_source_map_sidecar(output)?;
        }
        Ok(())
    };
    publication?;
    if let Some(output) = output {
        publish_analysis_map(output, analysis_map)?;
    }
    Ok(())
}

fn publish_analysis_map(
    output: &Path,
    analysis_map: Option<&JavaScriptAnalysisMap>,
) -> Result<(), String> {
    let path = javascript_analysis_map_path(output);
    if let Some(analysis_map) = analysis_map {
        fs::write(&path, format!("{}\n", analysis_map.as_str()))
            .map_err(|error| format!("failed to write {}: {error}", path.display()))
    } else {
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("failed to remove {}: {error}", path.display())),
        }
    }
}

fn javascript_analysis_map_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".lilmap.json");
    PathBuf::from(path)
}

fn javascript_source_map_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".map");
    PathBuf::from(path)
}

fn remove_source_map_sidecar(output: &Path) -> Result<(), String> {
    let map_path = javascript_source_map_path(output);
    match fs::remove_file(&map_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to remove {}: {error}", map_path.display())),
    }
}

fn javascript_with_source_map_url(contents: &str, url: &str) -> String {
    let mut published = String::with_capacity(contents.len() + url.len() + 24);
    published.push_str(contents);
    if !published.ends_with('\n') {
        published.push('\n');
    }
    published.push_str("//# sourceMappingURL=");
    published.push_str(url);
    published.push('\n');
    published
}

fn url_path_component(component: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(component.len());
    for byte in component.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

fn remove_stale_chunks(
    directory: &Path,
    manifest_path: &Path,
    bundle: &JavaScriptBundle,
) -> Result<(), String> {
    let Ok(previous) = fs::read_to_string(manifest_path) else {
        return Ok(());
    };
    let Ok(previous) = serde_json::from_str::<serde_json::Value>(&previous) else {
        return Ok(());
    };
    let current = bundle
        .manifest
        .chunks
        .iter()
        .map(|chunk| chunk.file.as_str())
        .collect::<std::collections::HashSet<_>>();
    let Some(chunks) = previous.get("chunks").and_then(|chunks| chunks.as_array()) else {
        return Ok(());
    };
    for file in chunks
        .iter()
        .filter_map(|chunk| chunk.get("file")?.as_str())
    {
        let flat_chunk = (file.starts_with("chunk-") || file.starts_with("lil-chunk-"))
            && Path::new(file).file_name().and_then(|name| name.to_str()) == Some(file);
        if current.contains(file) || !flat_chunk {
            continue;
        }
        let path = directory.join(file);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to remove stale bundle chunk {}: {error}",
                    path.display()
                ));
            }
        }
        remove_source_map_sidecar(&path)?;
        publish_analysis_map(&path, None)?;
    }
    Ok(())
}

fn write_or_print(output: Option<&Path>, contents: &str) -> Result<(), String> {
    if let Some(output) = output {
        ensure_parent(output)?;
        fs::write(output, contents)
            .map_err(|error| format!("failed to write {}: {error}", output.display()))
    } else {
        println!("{contents}");
        Ok(())
    }
}

fn ensure_parent(output: &Path) -> Result<(), String> {
    let Some(parent) = output.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))
}

fn compile_native(c: &str, output: &Path) -> Result<(), String> {
    let compiler = std::env::var("CC").unwrap_or_else(|_| "clang".to_string());
    let mut command = Command::new(&compiler);
    command.args(["-x", "c", "-std=c11", "-O3"]);
    #[cfg(target_os = "macos")]
    command.arg("-Wl,-no_uuid");
    command.arg("-o").arg(output).arg("-");
    #[cfg(not(target_os = "windows"))]
    command.arg("-lm");
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start native compiler `{compiler}`: {error}"))?;
    child
        .stdin
        .take()
        .expect("native compiler stdin was piped")
        .write_all(c.as_bytes())
        .map_err(|error| format!("failed to send C source to `{compiler}`: {error}"))?;
    let result = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for `{compiler}`: {error}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "native compiler failed:\n{}",
            String::from_utf8_lossy(&result.stderr)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_resource_limits_are_nonzero_and_override_project_values() {
        let args = Args::try_parse_from([
            "lilscript",
            "input.lil",
            "--jobs",
            "12",
            "--codec-jobs",
            "8",
        ])
        .unwrap();
        let mut config = ProjectConfig::default();
        config.compiler.resources.threads = NonZeroUsize::new(2);
        config.compiler.resources.codec_workers = NonZeroUsize::new(3).unwrap();

        apply_resource_overrides(&mut config, args.jobs, args.codec_jobs);

        assert_eq!(config.compiler.resources.threads.unwrap().get(), 12);
        assert_eq!(config.compiler.resources.codec_workers.get(), 8);
        assert!(Args::try_parse_from(["lilscript", "input.lil", "--jobs", "0"]).is_err());
        assert!(Args::try_parse_from(["lilscript", "input.lil", "--codec-jobs", "0"]).is_err());
    }

    #[test]
    fn analysis_hash_verification_accepts_source_map_publication_comments() {
        for selected in ["let a=1", "let a=1\n", ""] {
            let expected = sha256_hex(selected.as_bytes());
            assert_eq!(
                verify_analysis_artifact(selected.as_bytes(), &expected),
                Some(ArtifactHashMatch::Exact)
            );
            let linked = javascript_with_source_map_url(selected, "app.js.map");
            assert_eq!(
                verify_analysis_artifact(linked.as_bytes(), &expected),
                Some(ArtifactHashMatch::BeforeSourceMapComment)
            );
            let inline = javascript_with_source_map_url(
                selected,
                "data:application/json;charset=utf-8;base64,e30=",
            );
            assert_eq!(
                verify_analysis_artifact(inline.as_bytes(), &expected),
                Some(ArtifactHashMatch::BeforeSourceMapComment)
            );
        }
        assert_eq!(
            verify_analysis_artifact(b"let a=2", &sha256_hex(b"let a=1")),
            None
        );
    }

    #[test]
    fn source_map_publication_paths_and_comments_are_unambiguous() {
        assert_eq!(
            javascript_source_map_path(Path::new("dist/app.min.js")),
            PathBuf::from("dist/app.min.js.map")
        );
        assert_eq!(
            javascript_analysis_map_path(Path::new("dist/app.min.js")),
            PathBuf::from("dist/app.min.js.lilmap.json")
        );
        assert_eq!(url_path_component("app build.js.map"), "app%20build.js.map");
        assert_eq!(
            javascript_with_source_map_url("let a=1", "app.js.map"),
            "let a=1\n//# sourceMappingURL=app.js.map\n"
        );
        assert_eq!(
            javascript_with_source_map_url("let a=1\n", "app.js.map"),
            "let a=1\n//# sourceMappingURL=app.js.map\n"
        );

        let directory = std::env::temp_dir().join(format!(
            "lilscript-source-map-publication-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let output = directory.join("app.js");
        let sidecar = javascript_source_map_path(&output);
        let analysis_sidecar = javascript_analysis_map_path(&output);
        fs::write(&sidecar, "stale map").unwrap();
        fs::write(&analysis_sidecar, "stale analysis").unwrap();
        write_javascript_artifact(
            Some(&output),
            "let a=1",
            None,
            None,
            JavaScriptSourceMapMode::Hidden,
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&output).unwrap(), "let a=1");
        assert!(!sidecar.exists());
        assert!(!analysis_sidecar.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn publishes_inspects_and_removes_a_real_analysis_sidecar() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let input = root.join("tests/analysis-map/main.lil");
        let config_path = root.join("tests/analysis-map/lilscript.toml");
        let mut config = load_project_config(&input, Some(&config_path))
            .unwrap()
            .config;
        config.javascript.source_map.enabled = true;
        config.javascript.source_map.mode = JavaScriptSourceMapMode::Linked;
        let compilation = compile_path_to_js_module_explained_configured(&input, &config).unwrap();
        assert!(!compilation.javascript.is_empty());

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "lilscript-analysis-map-publication-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let output = directory.join("app.js");
        write_javascript_artifact(
            Some(&output),
            &compilation.javascript,
            compilation.source_map.as_ref(),
            compilation.analysis_map.as_ref(),
            config.javascript.source_map.mode,
        )
        .unwrap();
        let analysis_sidecar = javascript_analysis_map_path(&output);
        assert!(analysis_sidecar.exists());
        inspect_analysis_map(&analysis_sidecar, Some(&output)).unwrap();

        write_javascript_artifact(
            Some(&output),
            &compilation.javascript,
            None,
            None,
            JavaScriptSourceMapMode::Hidden,
        )
        .unwrap();
        assert!(!javascript_source_map_path(&output).exists());
        assert!(!analysis_sidecar.exists());
        fs::remove_dir_all(directory).unwrap();
    }
}

// STORE_CENSUS report (temporary)
fn report_store_census() {
    if std::env::var_os("LILSCRIPT_STORE_CENSUS").is_none() {
        return;
    }
    let names = [
        "cross_block",
        "use_count>1",
        "unstable",
        "single_use",
        "other",
        "fallthrough_only",
    ];
    eprint!("CENSUS");
    for (i, n) in names.iter().enumerate() {
        eprint!(
            " {n}={}",
            lilscript::codegen_ir_js::STORE_REASONS[i].load(std::sync::atomic::Ordering::Relaxed)
        );
    }
    eprintln!();
}
