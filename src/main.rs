use std::fs;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, ValueEnum};
use serde_json::{json, Value};

use lilscript::config::{
    load_project_config, CandidateSearch, DeliveryMode, JavaScriptFormat, LoadedConfig,
    ProjectConfig,
};
use lilscript::module::EntrySource;
use lilscript::package::write_lockfile;
use lilscript::{
    render_service_error, ChunkExtension, ServiceCompilation, ServiceJavaScript, ServiceOptions,
    ServiceTarget,
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
enum DeliveryArg {
    Single,
    Split,
    PreserveModules,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatArg {
    Esm,
    Cjs,
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
    /// LilScript source file to compile: an entry named by its file stem.
    /// Optional when the configuration's `[delivery.entries]` or `--entry`
    /// name the entries.
    input: Option<PathBuf>,

    /// Output file, or the base path used by `--target all`. A build with
    /// several entries or files writes a directory instead (`--out-dir`).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// An entry of the program, `NAME=PATH` (repeatable): each is a root of
    /// one module graph and a public surface of its own.
    #[arg(long = "entry", value_name = "NAME=PATH")]
    entries: Vec<String>,

    /// The directory a delivery's files and `lilscript.manifest.json` are
    /// written to.
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,

    /// How the program is placed in files; overrides `[delivery] mode`.
    #[arg(long, value_enum, value_name = "MODE")]
    delivery: Option<DeliveryArg>,

    /// The container of the primary output; overrides
    /// `[target.javascript] format`.
    #[arg(long, value_enum, value_name = "FORMAT")]
    format: Option<FormatArg>,

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

    /// Compiler worker threads, the one parallelism flag. Accepted; the
    /// compiler does not run worker threads yet, so it has no effect. A
    /// thread count never changes the output.
    #[arg(short = 'j', long, value_name = "N")]
    jobs: Option<NonZeroUsize>,

    /// Removed: `-j` is the one parallelism flag (plan M3.5). Accepted with
    /// a warning for one release (architecture §14.2), with no effect.
    #[arg(long, hide = true, value_name = "N")]
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
    if args.codec_jobs.is_some() {
        eprintln!(
            "warning: --codec-jobs was removed: -j is the one parallelism flag; it has no effect and will be refused after this release"
        );
    }
    if args.output.is_some() && args.out_dir.is_some() {
        return Err(
            "-o FILE and --out-dir DIR both name where the delivery goes: give one".to_string(),
        );
    }
    if matches!(args.target, Target::All) && args.out_dir.is_some() {
        return Err(
            "--target all writes FILE.js, FILE.c and the executable beside -o FILE, not a directory"
                .to_string(),
        );
    }
    // The configuration is found from INPUT, else from the entry whose name
    // sorts first: never from the order of the flags (DL10).
    let discovery = args
        .input
        .clone()
        .or_else(|| {
            args.entries
                .iter()
                .filter_map(|entry| entry.split_once('='))
                .min_by(|left, right| left.0.cmp(right.0))
                .map(|(_, path)| PathBuf::from(path))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    let mut loaded = load_project_config(&discovery, args.config.as_deref())
        .map_err(|error| error.to_string())?;
    let config_label = loaded.path.as_ref().map_or_else(
        || "lilscript.toml".to_string(),
        |path| path.display().to_string(),
    );
    for warning in &loaded.warnings {
        eprintln!("warning: {config_label}: {warning}");
    }
    if args.jobs.is_some() {
        eprintln!(
            "warning: --jobs has no effect in this compiler yet: it compiles and encodes on one thread"
        );
    }
    if args.write_lock {
        let path = write_lockfile(&loaded.config).map_err(|error| error.to_string())?;
        eprintln!("wrote {}", path.display());
    }
    if let Some(mode) = args.delivery {
        loaded.config.delivery.mode = match mode {
            DeliveryArg::Single => DeliveryMode::Single,
            DeliveryArg::Split => DeliveryMode::Split,
            DeliveryArg::PreserveModules => DeliveryMode::PreserveModules,
        };
    }
    if let Some(format) = args.format {
        loaded.config.target.javascript.format = match format {
            FormatArg::Esm => JavaScriptFormat::Esm,
            FormatArg::Cjs => JavaScriptFormat::Cjs,
        };
    }
    if args.delegate_bundling {
        loaded.config.delivery.mode = DeliveryMode::Single;
    }
    if matches!(args.mode, BuildMode::Development) {
        loaded.config.javascript.candidate_search = CandidateSearch::Off;
    }
    let entries = if args.delegate_bundling {
        // The external bundler owns the other entries: build INPUT alone.
        match (&args.input, args.entries.is_empty()) {
            (Some(input), true) => vec![EntrySource::of(input)],
            _ => return Err("--delegate-bundling builds one entry: name it as INPUT".to_string()),
        }
    } else {
        entries(&args, &loaded)?
    };
    // Refusals that do not need a compile, before any `--print-*` answer
    // (design §4 "Validity, checked at load").
    if entries.len() > 1 {
        match args.target {
            Target::Js => {
                return Err(format!(
                    "a script build (`--target js`) has one entry, and this build has {}: build the entries as modules with `--target js-module`",
                    entries.len()
                ))
            }
            Target::C | Target::Native | Target::All => {
                return Err(format!(
                    "a native build has one library ABI (plan M11.8), and this build has {} entries",
                    entries.len()
                ))
            }
            Target::JsModule => {}
        }
    }
    // `-o FILE` writes the one entry at FILE: a delivery of several files
    // names that entry's file so, and its other files beside it.
    if args.out_dir.is_none() && loaded.config.delivery.mode != DeliveryMode::Single {
        if let Some(name) = args
            .output
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
        {
            lilscript::js::names::check_template("entry_names", name).map_err(|_| {
                format!(
                    "-o names a file `{name}` a delivered file cannot be called: use --out-dir DIR"
                )
            })?;
            loaded.config.delivery.entry_names = Some(name.to_string());
        }
    }
    let options = service_options(&args);
    if args.print_dependencies {
        return print_dependencies(&entries, &loaded, options);
    }
    if args.print_policy {
        return print_policy(&args, &loaded, options);
    }
    build(&args, &entries, &loaded.config, options)
}

/// The build's entries (plan M3.3, design §4): `[delivery.entries]`
/// (relative to the configuration file), each `--entry NAME=PATH`, and
/// `INPUT`, named by its stem unless a declared entry has that source.
fn entries(args: &Args, loaded: &LoadedConfig) -> Result<Vec<EntrySource>, String> {
    let base = loaded
        .config
        .config_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let mut entries = loaded
        .config
        .delivery
        .entries
        .iter()
        .map(|(name, path)| EntrySource {
            name: name.clone(),
            path: base.join(path),
        })
        .collect::<Vec<_>>();
    for entry in &args.entries {
        let (name, path) = entry
            .split_once('=')
            .ok_or_else(|| format!("--entry `{entry}` must be NAME=PATH"))?;
        entries.push(EntrySource {
            name: name.to_string(),
            path: PathBuf::from(path),
        });
    }
    if let Some(input) = &args.input {
        let same = |path: &Path| match (fs::canonicalize(path), fs::canonicalize(input)) {
            (Ok(left), Ok(right)) => left == right,
            _ => path == input,
        };
        if !entries.iter().any(|entry| same(&entry.path)) {
            entries.push(EntrySource::of(input));
        }
    }
    if entries.is_empty() {
        return Err(
            "no entry: name a source file, `--entry NAME=PATH`, or `[delivery.entries]`"
                .to_string(),
        );
    }
    Ok(entries)
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
            .filter(|_| args.out_dir.is_none())
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

fn build(
    args: &Args,
    entries: &[EntrySource],
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<(), String> {
    let result = lilscript::compile_entries(entries, config, options)
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
        args.output.clone().unwrap_or_else(|| {
            args.input
                .clone()
                .unwrap_or_else(|| entries[0].path.clone())
                .with_extension("")
        })
    };
    match args.target {
        Target::Js | Target::JsModule => {
            let selected = javascript()?;
            if selected.layout().is_none() && args.out_dir.is_none() {
                write_or_print(args.output.as_deref(), selected.javascript())
            } else {
                write_delivery(args, entries, config, &result, selected)
            }
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
            let selected = javascript()?;
            if selected.layout().is_none() {
                fs::write(&entry, selected.javascript())
                    .map_err(|error| format!("failed to write {}: {error}", entry.display()))?;
            } else {
                return Err("--target all delivers one JavaScript file; build a multi-file delivery with --target js-module".to_string());
            }
            let c = base.with_extension("c");
            fs::write(&c, native_c()?)
                .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
            compile_native(native_c()?, &base)
        }
    }
}

/// Where a delivery's files go (design §4): `--out-dir`, or, for one entry
/// written with `-o FILE`, FILE for the entry and its other files beside it.
/// The manifest is `lilscript.manifest.json` in the directory, or
/// `<stem>.manifest.json` beside FILE.
fn write_delivery(
    args: &Args,
    entries: &[EntrySource],
    config: &ProjectConfig,
    result: &ServiceCompilation,
    selected: &ServiceJavaScript,
) -> Result<(), String> {
    let (directory, entry_file, manifest_path) = match (&args.out_dir, &args.output) {
        (Some(directory), _) => (
            directory.clone(),
            None,
            directory.join("lilscript.manifest.json"),
        ),
        (None, Some(output)) => {
            let directory = output
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
            (
                directory,
                Some(output.clone()),
                output.with_extension("manifest.json"),
            )
        }
        (None, None) => {
            return Err(
                "a delivery of several files needs --out-dir DIR (or -o FILE for one entry)"
                    .to_string(),
            )
        }
    };
    let modules = result.report()["inputs"]["modules"]
        .as_array()
        .map(|modules| {
            modules
                .iter()
                .map(|module| module["path"].as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // (name, code) of every file this build writes.
    let mut written: Vec<(PathBuf, &str)> = Vec::new();
    let mut outputs = Vec::new();
    // One entry in one file: a delivery of one file, named and listed like
    // any entry file (design §4, §10).
    let single_name;
    let single_layout;
    match selected.layout() {
        None => {
            let name = entries.first().map_or("main", |entry| entry.name.as_str());
            let template = config
                .delivery
                .entry_names
                .as_deref()
                .unwrap_or("[name].[ext]");
            let hash = if lilscript::js::names::needs_hash(template) {
                use sha2::{Digest, Sha256};
                format!("{:x}", Sha256::digest(selected.javascript().as_bytes()))
            } else {
                String::new()
            };
            single_name = lilscript::js::names::expand(
                template,
                &lilscript::js::names::Fields {
                    name,
                    index: 0,
                    path: name,
                    ext: "js",
                    hash: &hash,
                },
            );
            single_layout = lilscript::js::manifest::one_file_layout(name, modules.len());
            written.push((directory.join(&single_name), selected.javascript()));
            outputs.push(lilscript::ManifestOutput {
                files: vec![lilscript::ManifestFile {
                    name: &single_name,
                    code: selected.javascript(),
                    sizes: selected.sizes(),
                }],
                layout: &single_layout,
            });
        }
        Some(layout) => {
            let statics = layout.entries.iter().filter(|entry| !entry.dynamic).count();
            if entry_file.is_some() && statics != 1 {
                return Err(format!(
                    "-o FILE writes one entry, and this build has {statics}: use --out-dir DIR"
                ));
            }
            for (index, file) in selected.files().iter().enumerate() {
                let facade = layout
                    .entries
                    .first()
                    .is_some_and(|entry| entry.file as usize == index && !entry.dynamic);
                let path = match (&entry_file, facade) {
                    (Some(entry), true) => entry.clone(),
                    _ => directory.join(&file.name),
                };
                written.push((path, file.code.as_str()));
            }
            outputs.push(lilscript::ManifestOutput {
                files: selected
                    .files()
                    .iter()
                    .map(|file| lilscript::ManifestFile {
                        name: &file.name,
                        code: &file.code,
                        sizes: file.sizes,
                    })
                    .collect(),
                layout,
            });
        }
    }
    let mut sorted = written
        .iter()
        .map(|(path, _)| path.clone())
        .chain(std::iter::once(manifest_path.clone()))
        .collect::<Vec<_>>();
    sorted.sort();
    if let Some(pair) = sorted.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(format!(
            "two delivered files are both {}: give the outputs distinct file names",
            pair[0].display()
        ));
    }
    let base = common_directory(&modules);
    let modules = modules
        .iter()
        .map(|path| {
            Path::new(path)
                .strip_prefix(&base)
                .unwrap_or(Path::new(path))
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
    let manifest = lilscript::manifest_v3(&outputs, &modules, config.javascript.cost_model);
    remove_stale_files(&directory, &manifest_path, &written)?;
    for (path, code) in &written {
        ensure_parent(path)?;
        fs::write(path, code)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    ensure_parent(&manifest_path)?;
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("failed to serialize the manifest: {error}"))?;
    fs::write(&manifest_path, format!("{text}\n"))
        .map_err(|error| format!("failed to write {}: {error}", manifest_path.display()))
}

/// The directory every path shares.
fn common_directory(paths: &[String]) -> PathBuf {
    let mut common: Option<Vec<std::ffi::OsString>> = None;
    for path in paths {
        let parts = Path::new(path)
            .parent()
            .map(|parent| {
                parent
                    .iter()
                    .map(|part| part.to_os_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        common = Some(match common {
            None => parts,
            Some(known) => known
                .into_iter()
                .zip(parts)
                .take_while(|(left, right)| left == right)
                .map(|(left, _)| left)
                .collect(),
        });
    }
    common.unwrap_or_default().iter().collect()
}

/// Remove the files the previous manifest listed that this build does not
/// write again, within the output directory.
fn remove_stale_files(
    directory: &Path,
    manifest_path: &Path,
    written: &[(PathBuf, &str)],
) -> Result<(), String> {
    let Ok(previous) = fs::read_to_string(manifest_path) else {
        return Ok(());
    };
    let Ok(previous) = serde_json::from_str::<Value>(&previous) else {
        return Ok(());
    };
    let Some(outputs) = previous.get("outputs").and_then(Value::as_array) else {
        return Ok(());
    };
    let Ok(root) = fs::canonicalize(directory) else {
        return Ok(());
    };
    for file in outputs
        .iter()
        .filter_map(|output| output.get("files")?.as_array())
        .flatten()
        .filter_map(|file| file.get("file")?.as_str())
    {
        let relative = Path::new(file);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            continue;
        }
        let path = directory.join(relative);
        if written.iter().any(|(known, _)| *known == path) {
            continue;
        }
        // Only a regular file inside the output directory: never one a
        // symbolic link leads to, wherever it points.
        let inside = path
            .parent()
            .and_then(|parent| fs::canonicalize(parent).ok())
            .is_some_and(|parent| parent.starts_with(&root));
        let regular = fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_file());
        if !inside || !regular {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to remove stale file {}: {error}",
                    path.display()
                ))
            }
        }
    }
    Ok(())
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
                "{} {}, {}, delivery {}",
                text(&contract["execution"]).to_lowercase(),
                text(&contract["world"]),
                text(&contract["ecmascript"]),
                text(&contract["delivery"]["mode"]).to_lowercase()
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
                    details["delivery"]["files"].as_array().map_or(1, Vec::len),
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
            for stage in search["terminal"]["objectives"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
                let trials = stage["trials"].as_array().map(Vec::as_slice).unwrap_or(&[]);
                let kept = trials
                    .iter()
                    .filter(|trial| trial["outcome"] == "kept")
                    .map(|trial| {
                        format!("{} {}", text(&trial["challenger"]), text(&trial["delta"]))
                    })
                    .collect::<Vec<_>>();
                let choices = stage["choices"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .map(|site| {
                        format!(
                            "{} {} (seed {})",
                            text(&site["site"]),
                            text(&site["delivered"]),
                            text(&site["seed"])
                        )
                    })
                    .collect::<Vec<_>>();
                if !choices.is_empty() {
                    line(
                        &format!("choices {}", text(&stage["codec"])),
                        format!(
                            "{} of {} alternative(s) judged; {}",
                            text(&stage["choices_scored"]),
                            stage["choice_trials"].as_array().map_or(0, Vec::len),
                            choices.join(", ")
                        ),
                    );
                }
                let joints = stage["joint_trials"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .filter(|trial| trial["outcome"] == "kept")
                    .map(|trial| format!("{} {}", text(&trial["name"]), text(&trial["delta"])));
                let starts = stage["starts"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .filter(|start| start["outcome"] == "kept")
                    .map(|start| {
                        format!("start {} {}", text(&start["name"]), text(&start["delta"]))
                    });
                let kept = kept.into_iter().chain(joints).chain(starts).collect::<Vec<_>>();
                let bounded = |value: &Value| match value {
                    Value::Null => "unbounded".to_string(),
                    value => text(value),
                };
                line(
                    &format!("walk {}", text(&stage["codec"])),
                    format!(
                        "{} -> {} (searched {}); {} pass(es) over {} start(s), {} examined (prefix {}), {} judged exactly (budget {}), {} pruned (margin {}); kept: {}",
                        text(&stage["before"]),
                        text(&stage["after"]),
                        text(&stage["searched"]),
                        text(&stage["passes"]),
                        stage["starts"].as_array().map_or(0, Vec::len),
                        text(&stage["examined"]),
                        bounded(&stage["prefix"]),
                        text(&stage["judged"]),
                        bounded(&stage["exact"]),
                        text(&stage["pruned"]),
                        text(&stage["margin"]),
                        if kept.is_empty() {
                            "none".to_string()
                        } else {
                            kept.join(", ")
                        }
                    ),
                );
            }
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
            // Removed (architecture §14.2): reported, with no effect, for
            // one release.
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
    entries: &[EntrySource],
    loaded: &LoadedConfig,
    options: ServiceOptions,
) -> Result<(), String> {
    let inputs = lilscript::build_inputs(entries, &loaded.config, options)
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
            "entries": inputs.entries.iter().map(|(name, path)| json!({"name": name, "path": path})).collect::<Vec<_>>(),
            "files": files,
        }))
        .map_err(|error| format!("failed to serialize compiler inputs: {error}"))?
    );
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
        let args = Args::try_parse_from(["lilscript", "input.lil", "--jobs", "12"]).unwrap();
        assert_eq!(args.jobs.unwrap().get(), 12);
        assert!(Args::try_parse_from(["lilscript", "input.lil", "--jobs", "0"]).is_err());
        // Removed: accepted with a warning for one release, never zero.
        assert_eq!(
            Args::try_parse_from(["lilscript", "input.lil", "--codec-jobs", "8"])
                .unwrap()
                .codec_jobs
                .map(NonZeroUsize::get),
            Some(8)
        );
        assert!(Args::try_parse_from(["lilscript", "input.lil", "--codec-jobs", "0"]).is_err());
    }

    #[test]
    fn entries_parse_from_flags() {
        let args = Args::try_parse_from([
            "lilscript",
            "--entry",
            "a=src/a.lil",
            "--entry",
            "b=src/b.lil",
            "--out-dir",
            "dist",
            "--delivery",
            "split",
        ])
        .unwrap();
        assert!(args.input.is_none());
        assert_eq!(args.entries, ["a=src/a.lil", "b=src/b.lil"]);
        assert_eq!(args.out_dir.as_deref(), Some(Path::new("dist")));
        assert!(matches!(args.delivery, Some(DeliveryArg::Split)));
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
                    "ecmascript": "es2022", "delivery": {"mode": "single"}},
                "tactics": [
                    {"id": "inlining", "state": {"permission": "auto", "enabled": true}},
                    {"id": "property-mangling", "state": {"permission": "off", "enabled": false}},
                ],
            },
            "artifacts": [{"raw": 4786, "gzip9": null, "brotli11": 1646, "details": {
                "style": "Scoped", "output": {"literals": "Original"}, "delivery": null,
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
        assert!(
            summary.contains("tactics enabled      inlining"),
            "{summary}"
        );
        assert!(
            summary.contains("tactics disabled     property-mangling"),
            "{summary}"
        );
        assert!(summary.contains("951.6 ms total"), "{summary}");
        assert!(!summary.contains("raw winner"), "{summary}");
    }
}
