use std::fs;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, ValueEnum};
use serde_json::{json, Value};

use lilscript::config::{
    load_project_config, BundleMode, CandidateSearch, LoadedConfig, ProjectConfig,
};
use lilscript::package::write_lockfile;
use lilscript::{
    render_service_error, ChunkExtension, JavaScriptBundle, ServiceCompilation, ServiceJavaScript,
    ServiceOptions, ServiceTarget,
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

    /// Output file, or the base path used by `--target all`.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Compilation target.
    #[arg(long, value_enum, default_value_t = Target::Js)]
    target: Target,

    /// Removed: there is one compiler. Present only to refuse with a clear
    /// message.
    #[arg(long, hide = true, num_args = 0..=1, value_name = "ROUTE")]
    backend: Option<Option<String>>,

    /// Explicit config path. Otherwise `lilscript.toml` is discovered from the input directory.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Compiler worker threads. Accepted; the compiler does not run worker
    /// threads yet, so it has no effect.
    #[arg(short = 'j', long, value_name = "N")]
    jobs: Option<NonZeroUsize>,

    /// Concurrent codec workers. Accepted; codec work runs on one thread
    /// yet, so it has no effect.
    #[arg(long, value_name = "N")]
    codec_jobs: Option<NonZeroUsize>,

    /// Development skips the candidate search; production uses project policy.
    #[arg(long, value_enum, default_value_t = BuildMode::Production)]
    mode: BuildMode,

    /// Print the compiler's report to stderr: a readable summary (`human`)
    /// or the full report (`json`).
    #[arg(long, value_enum)]
    explain: Option<ExplainFormat>,

    /// Resolve all path dependencies and rewrite lilscript.lock before compiling.
    #[arg(long)]
    write_lock: bool,

    /// Force a single ESM artifact for an external bundler such as Lilpack.
    #[arg(long, hide = true)]
    delegate_bundling: bool,

    /// Print compiler inputs as JSON for an external incremental build graph, then exit.
    #[arg(long, hide = true)]
    print_dependencies: bool,

    /// Print the fully resolved compilation policy for this input, target and
    /// configuration — after defaults, the TOML file and command-line overrides
    /// are combined — as a versioned JSON receipt with its fingerprint, then exit.
    #[arg(long)]
    print_policy: bool,
}

fn main() {
    let started = std::time::Instant::now();
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    if let Some(report) = lilscript::timing::report(started.elapsed().as_nanos()) {
        eprintln!("lilscript-timing {report}");
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.backend.is_some() {
        return Err(
            "error: --backend was removed: there is one compiler; remove the flag".to_string(),
        );
    }
    let mut loaded = load_project_config(&args.input, args.config.as_deref())
        .map_err(|error| error.to_string())?;
    let config_label = loaded
        .path
        .as_ref()
        .map_or_else(|| "lilscript.toml".to_string(), |path| path.display().to_string());
    for warning in &loaded.warnings {
        eprintln!("warning: {config_label}: {warning}");
    }
    if args.jobs.is_some() || args.codec_jobs.is_some() {
        eprintln!(
            "warning: --jobs and --codec-jobs have no effect in this compiler yet: it compiles and encodes on one thread"
        );
    }
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
    let options = service_options(&args);
    if args.print_dependencies {
        return print_dependencies(&args.input, &loaded, options);
    }
    if args.print_policy {
        return print_policy(&args, &loaded, options);
    }
    build(&args, &loaded.config, options)
}

/// The one mapping from the command line to what the compiler builds. The
/// build and `--print-policy` both use it.
fn service_options(args: &Args) -> ServiceOptions {
    ServiceOptions {
        target: match args.target {
            Target::Js | Target::JsModule => ServiceTarget::JavaScript,
            Target::C | Target::Native => ServiceTarget::Native,
            Target::All => ServiceTarget::All,
        },
        preserve_root_exports: matches!(args.target, Target::JsModule),
        chunk_extension: args
            .output
            .as_deref()
            .map(ChunkExtension::of)
            .unwrap_or_default(),
        // Whole ports exceed the library default: Micromark's 303 KB of
        // source uses 354M units, and motionlil's full entry 4.4G, most of it
        // in conversion (3 s). This interim CLI ceiling only stops a runaway
        // compile; 012 sets the cost policy. `LILSCRIPT_SEMANTIC_WORK`
        // overrides it for measurement, and policy resources still restrict it.
        logical_work: std::env::var("LILSCRIPT_SEMANTIC_WORK")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(40_000_000_000),
        ..ServiceOptions::default()
    }
}

fn build(args: &Args, config: &ProjectConfig, options: ServiceOptions) -> Result<(), String> {
    let result = lilscript::compile_path_semantic(&args.input, config, options)
        .map_err(|error| render_service_error(&error))?;
    if let Some(format) = args.explain {
        let text = match format {
            ExplainFormat::Json => {
                serde_json::to_string_pretty(result.report()).map_err(|error| error.to_string())?
            }
            ExplainFormat::Human => explain_human(result.report()),
        };
        eprintln!("{text}");
    }
    let javascript = || {
        result
            .javascript(config.javascript.cost_model)
            .ok_or_else(|| "missing selected JavaScript artifact".to_string())
    };
    let native_c = || {
        result
            .native_c()
            .ok_or_else(|| "missing native C artifact".to_string())
    };
    let base = || {
        args.output
            .clone()
            .unwrap_or_else(|| args.input.with_extension(""))
    };
    match args.target {
        Target::Js | Target::JsModule if config.bundle.mode == BundleMode::Single => {
            write_or_print(args.output.as_deref(), javascript()?.javascript())
        }
        Target::Js | Target::JsModule => {
            let output = args.output.as_deref().ok_or_else(|| {
                "split and preserve-modules bundle modes require an explicit --output entry file"
                    .to_string()
            })?;
            let bundle = javascript_bundle(args, config, &result, javascript()?, output)?;
            write_javascript_bundle(output, &bundle)
        }
        Target::C => write_or_print(args.output.as_deref(), native_c()?),
        Target::Native => {
            let base = base();
            ensure_parent(&base)?;
            compile_native(native_c()?, &base)
        }
        Target::All => {
            let base = base();
            ensure_parent(&base)?;
            let entry = base.with_extension("js");
            if config.bundle.mode == BundleMode::Single {
                fs::write(&entry, javascript()?.javascript())
                    .map_err(|error| format!("failed to write {}: {error}", entry.display()))?;
            } else {
                let bundle = javascript_bundle(args, config, &result, javascript()?, &entry)?;
                write_javascript_bundle(&entry, &bundle)?;
            }
            let c = base.with_extension("c");
            fs::write(&c, native_c()?)
                .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
            compile_native(native_c()?, &base)
        }
    }
}

/// The delivered files of a multi-file build and their manifest, with the
/// entry written at `output`.
fn javascript_bundle(
    args: &Args,
    config: &ProjectConfig,
    result: &ServiceCompilation,
    selected: &ServiceJavaScript,
    output: &Path,
) -> Result<JavaScriptBundle, String> {
    let entry_file = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "bundle output must have a UTF-8 file name".to_string())?;
    // Module names relative to the entry module's directory.
    let paths = result.report()["inputs"]["modules"]
        .as_array()
        .map(|modules| {
            modules
                .iter()
                .map(|module| module["path"].as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let base = args
        .input
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base = fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    let name = |index: u32| {
        let path = Path::new(paths.get(index as usize).map_or("", String::as_str));
        path.strip_prefix(&base)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    let entry = lilscript::ManifestFile {
        file_name: entry_file.to_string(),
        modules: Vec::new(),
        dependencies: selected.entry_links().dependencies.clone(),
        dynamic_dependencies: selected.entry_links().dynamic_dependencies.clone(),
        lazy: false,
        importers: 0,
        code: selected.javascript().to_string(),
    };
    let chunks = selected
        .chunks()
        .iter()
        .map(|chunk| lilscript::ManifestFile {
            file_name: chunk.name.clone(),
            modules: chunk.modules.iter().map(|&module| name(module)).collect(),
            dependencies: chunk.dependencies.clone(),
            dynamic_dependencies: chunk.dynamic_dependencies.clone(),
            lazy: chunk.lazy,
            importers: chunk.importers,
            code: chunk.code.clone(),
        })
        .collect();
    lilscript::javascript_bundle(
        entry,
        chunks,
        selected.entry_links().preload.clone(),
        config.bundle.mode,
        config.javascript.cost_model,
        &config.bundle.cost,
    )
}

/// The compiler's report as a person reads it: the objective, each codec's
/// winner and its sizes, how much the search tried, the tactics the policy
/// enabled, and time.
fn explain_human(report: &Value) -> String {
    use std::fmt::Write as _;
    let text = |value: &Value| match value {
        Value::String(text) => text.clone(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    };
    let mut out = String::new();
    let mut line = |label: &str, value: String| {
        let _ = writeln!(out, "{label:<20} {value}");
    };
    let policy = &report["javascript_policy"];
    if !policy.is_null() {
        let objective = &policy["objective"];
        line(
            "objective",
            format!(
                "{} bytes, effort {}",
                text(&objective["codec"]).to_lowercase(),
                text(&policy["effort"])
            ),
        );
        let contract = &policy["contract"];
        line(
            "contract",
            format!(
                "{} {}, {}, bundle {}",
                text(&contract["execution"]).to_lowercase(),
                text(&contract["world"]),
                text(&contract["ecmascript"]),
                text(&contract["bundle_mode"]).to_lowercase()
            ),
        );
        let artifacts = report["artifacts"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let winners = report["winners"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for (codec, winner) in ["raw", "gzip", "brotli"].iter().zip(winners) {
            let Some(artifact) = winner
                .as_u64()
                .and_then(|index| artifacts.get(index as usize))
            else {
                continue;
            };
            let details = &artifact["details"];
            line(
                &format!("{codec} winner"),
                format!(
                    "raw {}, gzip {}, brotli {}; {} file(s), style {}, literals {}, {} rewrite(s)",
                    text(&artifact["raw"]),
                    text(&artifact["gzip9"]),
                    text(&artifact["brotli11"]),
                    1 + details["chunks"].as_array().map_or(0, Vec::len),
                    text(&details["style"]).to_lowercase(),
                    text(&details["output"]["literals"]).to_lowercase(),
                    details["semantic"]["rewrites"]
                        .as_array()
                        .map_or(0, Vec::len),
                ),
            );
        }
        let search = &report["search"];
        if !search.is_null() {
            line(
                "search",
                format!(
                    "{} proposals, {} structures, {} renders, {} codec probes, {} admitted; {}",
                    text(&search["proposals"]),
                    text(&search["structures"]),
                    text(&search["renders"]),
                    text(&search["codec_probes"]),
                    text(&search["admitted_artifacts"]),
                    match &search["stop"] {
                        Value::Null => "completed".to_string(),
                        stop => format!("stopped: {}", text(stop)),
                    }
                ),
            );
        }
        let tactics = policy["tactics"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for (label, enabled) in [("tactics enabled", true), ("tactics disabled", false)] {
            let names = tactics
                .iter()
                .filter(|tactic| tactic["state"]["enabled"].as_bool() == Some(enabled))
                .map(|tactic| text(&tactic["id"]))
                .collect::<Vec<_>>();
            line(
                label,
                if names.is_empty() {
                    "(none)".to_string()
                } else {
                    names.join(", ")
                },
            );
        }
    }
    let native = &report["native_delivery"];
    if !native.is_null() {
        line("native C", format!("{} bytes", text(&native["c_bytes"])));
    }
    let milliseconds = |value: &Value| {
        value.as_u64().map_or_else(
            || "-".to_string(),
            |nanos| format!("{:.1} ms", nanos as f64 / 1e6),
        )
    };
    line(
        "time",
        format!(
            "{} total, first artifact {}",
            milliseconds(&report["total_ns"]),
            milliseconds(&report["first_artifact_ns"])
        ),
    );
    out.truncate(out.trim_end().len());
    out
}

/// The resolved policy as the compiler will use it, so a port author can see
/// every axis — contract, objective, effort, tactic permissions, resources and
/// constraints — without reading source, and a receipt can pin its fingerprint.
fn print_policy(args: &Args, loaded: &LoadedConfig, options: ServiceOptions) -> Result<(), String> {
    let resolve = |request| -> Result<(String, Value), String> {
        let policy = loaded.config.resolve_policy(request)?;
        let fingerprint = policy
            .fingerprint()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok((fingerprint, policy.receipt()))
    };
    let javascript = options.javascript_request().map(resolve).transpose()?;
    let native = options.native_request().map(resolve).transpose()?;
    let (fingerprint, policy) = javascript
        .clone()
        .or_else(|| native.clone())
        .expect("every target resolves a policy");
    let mut receipt = json!({
        "config": loaded.path.as_ref().map(|path| path.display().to_string()),
        // Retired keys the file set: each has no effect in this compiler.
        "warnings": loaded.warnings,
        // How this run executes, after command-line overrides. Deliberately
        // outside the fingerprint: thread counts must never change the output.
        "execution": {
            "threads": args.jobs.map(NonZeroUsize::get),
            "codec_workers": args.codec_jobs.map(NonZeroUsize::get),
            "mode": format!("{:?}", args.mode),
            "target": format!("{:?}", args.target),
        },
        "fingerprint": fingerprint,
        "policy": policy,
    });
    // `--target all` also builds C, under its own policy.
    if let (Some(_), Some((fingerprint, policy))) = (&javascript, native) {
        receipt["native_fingerprint"] = json!(fingerprint);
        receipt["native_policy"] = policy;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&receipt).map_err(|error| error.to_string())?
    );
    Ok(())
}

/// Every file the build reads — source modules, delivered host modules, the
/// configuration and the lockfile — for an external incremental build graph.
fn print_dependencies(
    input: &Path,
    loaded: &LoadedConfig,
    options: ServiceOptions,
) -> Result<(), String> {
    let inputs = lilscript::build_inputs(input, &loaded.config, options)
        .map_err(|error| render_service_error(&error))?;
    let mut files = inputs.files;
    if let Some(path) = &loaded.path {
        files.push(path.canonicalize().unwrap_or_else(|_| path.clone()));
    }
    if let Some(root) = &loaded.config.config_dir {
        let lockfile = root.join("lilscript.lock");
        if lockfile.is_file() {
            files.push(lockfile.canonicalize().unwrap_or(lockfile));
        }
    }
    files.sort();
    files.dedup();
    println!(
        "{}",
        serde_json::to_string(&json!({
            "version": 1,
            "entry": inputs.entry,
            "files": files,
        }))
        .map_err(|error| format!("failed to serialize compiler inputs: {error}"))?
    );
    Ok(())
}

fn write_javascript_bundle(output: &Path, bundle: &JavaScriptBundle) -> Result<(), String> {
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
        fs::write(&path, &file.code)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
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

fn remove_stale_chunks(
    directory: &Path,
    manifest_path: &Path,
    bundle: &JavaScriptBundle,
) -> Result<(), String> {
    let Ok(previous) = fs::read_to_string(manifest_path) else {
        return Ok(());
    };
    let Ok(previous) = serde_json::from_str::<Value>(&previous) else {
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
    }
    Ok(())
}

fn write_or_print(output: Option<&Path>, contents: &str) -> Result<(), String> {
    if let Some(output) = output {
        ensure_parent(output)?;
        fs::write(output, contents)
            .map_err(|error| format!("failed to write {}: {error}", output.display()))
    } else {
        std::io::stdout()
            .lock()
            .write_all(contents.as_bytes())
            .map_err(|error| format!("failed to write stdout: {error}"))
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

/// Compile C to an executable with the strict numerics the C target
/// assumes: no fast math and no floating-point contraction.
fn compile_native(c: &str, output: &Path) -> Result<(), String> {
    // The platform's C compiler unless `CC` names one.
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let mut command = Command::new(&compiler);
    command.args([
        "-x",
        "c",
        "-std=c11",
        "-O3",
        "-fno-fast-math",
        "-ffp-contract=off",
    ]);
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
    // Feed the source from its own thread while this one drains stdout and
    // stderr: a compiler that fills a pipe before reading all of its input
    // would otherwise block both processes forever.
    let mut stdin = child.stdin.take().expect("native compiler stdin was piped");
    let source = c.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(source.as_bytes()));
    let result = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for `{compiler}`: {error}"))?;
    writer
        .join()
        .expect("the source writer does not panic")
        .map_err(|error| format!("failed to send C source to `{compiler}`: {error}"))?;
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
    fn resource_flags_are_nonzero() {
        let args = Args::try_parse_from([
            "lilscript",
            "input.lil",
            "--jobs",
            "12",
            "--codec-jobs",
            "8",
        ])
        .unwrap();
        assert_eq!(args.jobs.unwrap().get(), 12);
        assert_eq!(args.codec_jobs.unwrap().get(), 8);
        assert!(Args::try_parse_from(["lilscript", "input.lil", "--jobs", "0"]).is_err());
        assert!(Args::try_parse_from(["lilscript", "input.lil", "--codec-jobs", "0"]).is_err());
    }

    #[test]
    fn the_backend_flag_parses_only_to_be_refused() {
        for argv in [
            &["lilscript", "input.lil", "--backend"][..],
            &["lilscript", "input.lil", "--backend", "semantic"][..],
            &["lilscript", "input.lil", "--backend=legacy"][..],
        ] {
            assert!(
                Args::try_parse_from(argv).unwrap().backend.is_some(),
                "{argv:?}"
            );
        }
        assert!(Args::try_parse_from(["lilscript", "input.lil"])
            .unwrap()
            .backend
            .is_none());
        assert!(
            Args::try_parse_from(["lilscript", "input.lil", "--profile-template", "p.json"])
                .is_err()
        );
    }

    /// `--print-policy` and the build resolve one request per target; `all`
    /// builds a closed script and C, like `js` and `c`.
    #[test]
    fn every_target_maps_to_one_request() {
        use lilscript::compilation_policy::CompilationRequest as R;
        let requests = |target: &str| {
            let args =
                Args::try_parse_from(["lilscript", "input.lil", "--target", target]).unwrap();
            let options = service_options(&args);
            (options.javascript_request(), options.native_request())
        };
        let script = Some(R::JavaScript {
            preserve_root_exports: false,
        });
        let module = Some(R::JavaScript {
            preserve_root_exports: true,
        });
        assert_eq!(requests("js"), (script, None));
        assert_eq!(requests("js-module"), (module, None));
        assert_eq!(requests("c"), (None, Some(R::Native)));
        assert_eq!(requests("native"), (None, Some(R::Native)));
        assert_eq!(requests("all"), (script, Some(R::Native)));
    }

    #[test]
    fn the_human_summary_names_the_objective_winner_and_tactics() {
        let report = json!({
            "javascript_policy": {
                "effort": 13,
                "objective": {"codec": "Brotli"},
                "contract": {"execution": "Module", "world": "ReusableLibrary",
                    "ecmascript": "es2022", "bundle_mode": "Single"},
                "tactics": [
                    {"id": "inlining", "state": {"permission": "auto", "enabled": true}},
                    {"id": "property-mangling", "state": {"permission": "off", "enabled": false}},
                ],
            },
            "artifacts": [{"raw": 4786, "gzip9": null, "brotli11": 1646, "details": {
                "style": "Scoped", "output": {"literals": "Original"}, "chunks": [],
                "semantic": {"rewrites": []}}}],
            "winners": [null, null, 0],
            "search": {"proposals": 104, "structures": 35, "renders": 105, "codec_probes": 102,
                "admitted_artifacts": 105, "stop": null},
            "native_delivery": null,
            "total_ns": 951_596_411u64,
            "first_artifact_ns": 19_736_025u64,
        });
        let summary = explain_human(&report);
        assert!(
            summary.contains("objective            brotli bytes, effort 13"),
            "{summary}"
        );
        assert!(
            summary.contains("brotli winner        raw 4786, gzip -, brotli 1646"),
            "{summary}"
        );
        assert!(summary.contains("104 proposals"), "{summary}");
        assert!(summary.contains("tactics enabled      inlining"), "{summary}");
        assert!(
            summary.contains("tactics disabled     property-mangling"),
            "{summary}"
        );
        assert!(summary.contains("951.6 ms total"), "{summary}");
        assert!(!summary.contains("raw winner"), "{summary}");
    }
}
