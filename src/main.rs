use std::fs;
use std::io::Write;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use serde_json::{json, Value};

use lilscript::config::{
    load_project_config, CandidateSearch, DeliveryMode, JavaScriptFormat, LoadedConfig,
    ProjectConfig, CompressionCostModel,
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
enum ProxyPruningArg {
    On,
    Audit,
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DeliveryArg {
    Single,
    Split,
    PreserveModules,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatArg {
    Auto,
    Esm,
    Cjs,
    Iife,
    Umd,
    Bare,
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
    /// written to. Required for several objectives, which get separate
    /// raw/gzip/brotli subdirectories with unchanged relative imports.
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,

    /// How the program is placed in files; overrides `[delivery] mode`.
    #[arg(long, value_enum, value_name = "MODE")]
    delivery: Option<DeliveryArg>,

    /// The container of the primary output; overrides
    /// `[target.javascript] format`.
    #[arg(long, value_enum, value_name = "FORMAT")]
    format: Option<FormatArg>,

    /// Check the requested target without searching, writing files or running a C compiler.
    #[arg(long = "check")]
    check_only: bool,

    /// Link caller-owned C with the configured native toolchain, without LilScript checking.
    #[arg(long, requires = "input", conflicts_with_all = ["check_only", "print_dependencies", "entries", "out_dir", "write_choices", "choices", "write_lock"])]
    link_c: bool,

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

    /// Persist build/codec results in DIR; off disables build, codec and rule reuse.
    /// Overrides [cache]; relative paths start in the current directory.
    #[arg(long, value_name = "DIR|off")]
    cache: Option<String>,

    /// Replay a fingerprinted JavaScript decision lock as the first terminal move.
    /// Overrides [decisions] read; off disables reading. Relative to the current directory.
    #[arg(long, value_name = "FILE|off")]
    choices: Option<String>,

    /// Save selected JavaScript choices; omitted FILE uses lilscript.choices.lock.
    /// Overrides [decisions] write. Relative to the current directory.
    #[arg(long, num_args = 0..=1, default_missing_value = "lilscript.choices.lock", value_name = "FILE")]
    write_choices: Option<PathBuf>,

    /// Scoring workers (1-4), overriding [execution].jobs. Independent
    /// delivered files score in bounded batches; worker count preserves bytes.
    #[arg(short = 'j', long, value_name = "N")]
    jobs: Option<NonZeroUsize>,

    /// Removed: `-j` is the one parallelism flag (plan M3.5). Accepted with
    /// a warning for one release (architecture §14.2), with no effect.
    #[arg(long, hide = true, value_name = "N")]
    codec_jobs: Option<NonZeroUsize>,

    /// Service work ceiling; project policy may restrict it further.
    #[arg(long, value_name = "UNITS")]
    logical_work: Option<NonZeroU64>,

    /// Service memory ceiling; project policy may restrict it further.
    #[arg(long, value_name = "BYTES")]
    retained_bytes: Option<NonZeroU64>,

    /// Terminal proxy rejection: on, exact audit of rejected moves, or off.
    #[arg(long, value_enum)]
    proxy_pruning: Option<ProxyPruningArg>,

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
    if args.link_c {
        if !matches!(args.target, Target::Native) { return Err("--link-c requires --target native".into()); }
        let cc = std::env::var_os("CC");
        let toolchain = lilscript::native_toolchain::NativeToolchain::resolve(&loaded.config, cc.as_deref())?;
        if args.print_policy {
            println!("{}", serde_json::to_string_pretty(&toolchain.receipt()).map_err(|e|e.to_string())?);
            return Ok(());
        }
        let input = args.input.as_ref().unwrap();
        let output = args.output.as_ref().ok_or("--link-c requires -o <executable>")?;
        if input == output { return Err("--link-c input and executable must have different paths".into()); }
        let c = fs::read_to_string(input).map_err(|e|format!("failed to read {}: {e}",input.display()))?;
        let header_path = input.with_extension("h");
        let header = match fs::read_to_string(&header_path) {
            Ok(text) => Some(text), Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("failed to read native header {}: {error}",header_path.display())),
        };
        if header.is_some() && header_path.file_name() != output.with_extension("h").file_name() {
            return Err("--link-c with a provider header must keep its input file stem (main.c -> main or main.exe)".into());
        }
        return compile_native(&c, header.as_deref(), output, &loaded.config);
    }
    if let Some(path) = &args.choices {
        loaded.config.decisions.read = if path == "off" { None } else {
            Some(std::env::current_dir().map_err(|error| error.to_string())?.join(path))
        };
    }
    if let Some(path) = &args.write_choices {
        loaded.config.decisions.write = Some(std::env::current_dir().map_err(|error| error.to_string())?.join(path));
    }
    if let Some(cache) = &args.cache {
        if cache == "off" {
            loaded.config.cache.normalization_reuse = false;
            loaded.config.cache.elaboration_reuse = false;
            loaded.config.cache.formation_reuse = false;
            loaded.config.cache.build_reuse = false;
        }
        loaded.config.cache.codec_reuse = cache != "off";
        loaded.config.cache.directory = if cache == "off" { None } else {
            if cache.is_empty() { return Err("--cache requires a directory or off".into()); }
            Some(std::env::current_dir().map_err(|error| error.to_string())?.join(cache))
        };
    }
    let legacy_audit = std::env::var_os("LILSCRIPT_WALK_AUDIT").is_some();
    apply_proxy_override(&mut loaded.config, args.proxy_pruning, legacy_audit);
    if legacy_audit {
        eprintln!("warning: LILSCRIPT_WALK_AUDIT is deprecated; use --proxy-pruning audit or [policy.search] proxy_pruning = \"audit\"; an explicit --proxy-pruning overrides this adapter");
    }
    if let Some(jobs) = args.jobs { loaded.config.execution.jobs = jobs.get(); }
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
            FormatArg::Auto => JavaScriptFormat::Auto,
            FormatArg::Esm => JavaScriptFormat::Esm,
            FormatArg::Cjs => JavaScriptFormat::Cjs,
            FormatArg::Iife => JavaScriptFormat::Iife,
            FormatArg::Umd => JavaScriptFormat::Umd,
            FormatArg::Bare => JavaScriptFormat::Bare,
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
    if args.out_dir.is_none() {
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
    let options = service_options(&args)?;
    if matches!(args.target, Target::All) && args.out_dir.is_some()
        && options.requested_objectives(&loaded.config)?.iter().count() == 1
        && loaded.config.delivery.also.is_empty() {
        return Err("--target all with one JavaScript output writes FILE.js, FILE.c and the executable beside -o FILE; several objectives/formats use --out-dir DIR".into());
    }
    if std::env::var_os("LILSCRIPT_SEMANTIC_WORK").is_some() {
        eprintln!("warning: LILSCRIPT_SEMANTIC_WORK is deprecated; use --logical-work for the service ceiling or [policy.resources] logical_work to restrict it");
    }
    if args.print_dependencies {
        return print_dependencies(&entries, &loaded, options);
    }
    if args.print_policy {
        return print_policy(&args, &loaded, options);
    }
    if args.check_only {
        return lilscript::check_entries_for_target(&entries, &loaded.config, options)
            .map_err(|error| render_service_error(&error));
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

/// CLI overrides the deprecated environment adapter, which overrides TOML.
/// The library receives the resolved value and never reads the environment.
fn apply_proxy_override(
    config: &mut ProjectConfig,
    explicit: Option<ProxyPruningArg>,
    legacy_audit: bool,
) {
    use lilscript::compilation_policy::ProxyPruning;
    let requested = explicit.or(legacy_audit.then_some(ProxyPruningArg::Audit));
    if let Some(requested) = requested {
        config
            .policy
            .get_or_insert_with(Default::default)
            .search
            .proxy_pruning = match requested {
            ProxyPruningArg::On => ProxyPruning::On,
            ProxyPruningArg::Audit => ProxyPruning::Audit,
            ProxyPruningArg::Off => ProxyPruning::Off,
        };
    }
}

/// The one mapping from the command line to what the compiler builds. The
/// build and `--print-policy` both use it.
fn service_options(args: &Args) -> Result<ServiceOptions, String> {
    service_options_with_environment(args, std::env::var_os("LILSCRIPT_SEMANTIC_WORK").as_deref())
}

fn service_options_with_environment(
    args: &Args,
    legacy_work: Option<&std::ffi::OsStr>,
) -> Result<ServiceOptions, String> {
    // Preserve the existing finite CLI ceiling. A CLI flag has precedence
    // over the deprecated environment adapter; TOML caps both at resolution.
    let logical_work = match args.logical_work {
        Some(work) => work.get(),
        None => match legacy_work {
            Some(value) => value
                .to_str()
                .and_then(|value| value.parse::<NonZeroU64>().ok())
                .map(NonZeroU64::get)
                .ok_or("LILSCRIPT_SEMANTIC_WORK must be a positive 64-bit integer; use --logical-work or [policy.resources] logical_work")?,
            None => 40_000_000_000,
        },
    };
    Ok(ServiceOptions {
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
        logical_work,
        retained_bytes: args
            .retained_bytes
            .map(NonZeroU64::get)
            .unwrap_or(ServiceOptions::default().retained_bytes),
        ..ServiceOptions::default()
    })
}

fn build(
    args: &Args,
    entries: &[EntrySource],
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Result<(), String> {
    let requested = options.requested_objectives(config)?;
    let multiple = options.javascript_request().is_some() && (requested.iter().count() > 1 || !config.delivery.also.is_empty());
    if options.javascript_request().is_some() && config.delivery.directory.is_some() && args.out_dir.is_none() {
        return Err("`delivery.directory` requires --out-dir DIR".into());
    }
    if multiple && (args.out_dir.is_none() || args.output.is_some()) {
        return Err("several objective or format results need --out-dir DIR and no -o FILE".to_string());
    }
    let result = lilscript::compile_entries(entries, config, options)
        .map_err(|error| render_service_error(&error))?;
    if let Some(targets) = result.report()["policy_diagnostics"].as_object() {
        for (target, messages) in targets {
            if let Some(messages) = messages.as_array() {
                for message in messages.iter().filter_map(Value::as_str) {
                    eprintln!("warning: {target}: {message}");
                }
            }
        }
    }
    if let Some(format) = args.explain {
        let text = match format {
            ExplainFormat::Json => {
                serde_json::to_string_pretty(result.report()).map_err(|error| error.to_string())?
            }
            ExplainFormat::Human => explain_human(result.report()),
        };
        eprintln!("{text}");
    }
    let primary = requested
        .iter()
        .find(|codec| *codec == config.objective.codec())
        .unwrap_or_else(|| requested.iter().next().unwrap());
    let javascript = || {
        result
            .javascript(primary)
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
            if multiple {
                return write_objective_deliveries(args, entries, config, &result, requested);
            }
            let selected = javascript()?;
            if selected.layout().is_none() && args.out_dir.is_none() {
                write_or_print(args.output.as_deref(), selected.javascript())
            } else {
                write_delivery(args, entries, config, &result, selected, primary)
            }
        }
        Target::C => {
            if result.native_header().is_some() && args.output.is_none() {
                return Err("native provider C delivery requires -o <file.c> so its matching .h can be written".into());
            }
            if let Some(path) = &args.output { write_native_header(&result, path)?; }
            write_or_print(args.output.as_deref(), native_c()?)
        },
        Target::Native => {
            let base = base();
            ensure_parent(&base)?;
            compile_native(native_c()?, result.native_header(), &base, config)
        }
        Target::All => {
            if multiple {
                write_objective_deliveries(args, entries, config, &result, requested)?;
                let name = entries
                    .first()
                    .and_then(|entry| entry.path.file_stem())
                    .unwrap_or_else(|| std::ffi::OsStr::new("main"));
                let base = args.out_dir.as_ref().unwrap().join("native").join(name);
                ensure_parent(&base)?;
                let c = base.with_extension("c");
                fs::write(&c, native_c()?)
                    .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
                return compile_native(native_c()?, result.native_header(), &base, config);
            }
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
            compile_native(native_c()?, result.native_header(), &base, config)
        }
    }
}

/// A fully checked file plan; every code slice borrows the admitted artifact.
struct PlannedDelivery<'a> {
    directory: PathBuf,
    manifest_path: PathBuf,
    written: Vec<(PathBuf, &'a str)>,
    manifest: Value,
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
    codec: CompressionCostModel,
) -> Result<(), String> {
    let plan = plan_delivery(args, entries, config, result, selected, codec, None)?;
    write_planned_delivery(plan)
}

fn plan_delivery<'a>(
    args: &Args,
    entries: &[EntrySource],
    config: &ProjectConfig,
    result: &ServiceCompilation,
    selected: &'a ServiceJavaScript,
    codec: CompressionCostModel,
    objective_directory: Option<&Path>,
) -> Result<PlannedDelivery<'a>, String> {
    let (directory, entry_file, manifest_path) =
        match (objective_directory, &args.out_dir, &args.output) {
            (Some(directory), _, _) => (
                directory.to_path_buf(),
                None,
                directory.join("lilscript.manifest.json"),
            ),
            (None, Some(directory), _) => (
                directory.join(config.delivery.directory.as_deref().unwrap_or(Path::new(""))),
                None,
                directory.join(config.delivery.directory.as_deref().unwrap_or(Path::new(""))).join("lilscript.manifest.json"),
            ),
            (None, None, Some(output)) => {
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
            (None, None, None) => {
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
    let mut single_layout;
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
                    ext: config.delivery_contract(matches!(args.target, Target::JsModule))?.format.extension(),
                    hash: &hash,
                },
            );
            single_layout = lilscript::js::manifest::one_file_layout(name, modules.len());
            single_layout.format = config
                .delivery_contract(matches!(args.target, Target::JsModule))?
                .format;
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
    let root_index=result.report()["inputs"]["root"].as_u64().unwrap_or(0) as usize;
    let base=modules.get(root_index).and_then(|root|Path::new(root).parent()).unwrap_or(Path::new(""));
    let modules=modules.iter().map(|path| relative_source_name(base,Path::new(path))).collect::<Vec<_>>();
    let mut manifest = lilscript::manifest_v3(&outputs, &modules, codec);
    manifest["source_sha256"] = result.report()["source_sha256"].clone();
    manifest["inputs"] = result.report()["inputs"].clone();
    for output in manifest["outputs"].as_array_mut().unwrap() {
        output["output"] = json!("primary");
        output["codec"] = json!(codec.name());
        output["codec_settings"] = json!(config.objective.settings());
        output["policy_fingerprint"] = selected.details()["policy_fingerprint"].clone();
    }
    Ok(PlannedDelivery {
        directory,
        manifest_path,
        written,
        manifest,
    })
}

fn write_planned_delivery(plan: PlannedDelivery<'_>) -> Result<(), String> {
    let PlannedDelivery {
        directory,
        manifest_path,
        written,
        manifest,
    } = plan;
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
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("failed to serialize the manifest: {error}"))?;
    for (path, code) in &written {
        ensure_parent(path)?;
        fs::write(path, code)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    ensure_parent(&manifest_path)?;
    remove_stale_files(&directory, &manifest_path, &written)?;
    fs::write(&manifest_path, format!("{text}\n"))
        .map_err(|error| format!("failed to write {}: {error}", manifest_path.display()))
}

/// Each objective gets a directory so relative imports keep exactly the names
/// the compiler scored. Only manifest paths acquire the outer codec prefix.
fn write_objective_deliveries(
    args: &Args,
    entries: &[EntrySource],
    config: &ProjectConfig,
    result: &ServiceCompilation,
    objectives: lilscript::js::selection::Objectives,
) -> Result<(), String> {
    if !result.outputs().is_empty() {
        return write_format_deliveries(args, entries, config, result);
    }
    let directory = args.out_dir.as_ref().unwrap();
    let mut written = Vec::new();
    let mut outputs = Vec::new();
    for codec in objectives.iter() {
        let selected = result
            .javascript(codec)
            .ok_or_else(|| format!("missing {} objective result", codec.name()))?;
        let prefix = config.delivery.directory.as_deref().unwrap_or(Path::new("")).join(codec.name());
        let child = directory.join(&prefix);
        let mut plan = plan_delivery(args, entries, config, result, selected, codec, Some(&child))?;
        for (path, _) in &plan.written {
            let relative = path
                .strip_prefix(&child)
                .map_err(|_| "an objective output path escapes its codec directory".to_string())?;
            if relative.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            }) {
                return Err("an objective output path escapes its codec directory".to_string());
            }
        }
        for output in plan.manifest["outputs"].as_array_mut().unwrap().drain(..) {
            let mut output = output;
            prefix_manifest_paths(&mut output, &prefix.to_string_lossy());
            output["codec"] = json!(codec.name());
            output["codec_settings"] = json!(config.objective.settings());
            output["policy_fingerprint"] = selected.details()["policy_fingerprint"].clone();
            outputs.push(output);
        }
        written.append(&mut plan.written);
    }
    write_planned_delivery(PlannedDelivery {
        directory: directory.clone(),
        manifest_path: directory.join("lilscript.manifest.json"),
        written,
        manifest: json!({"version": 4,
            "source_sha256":result.report()["source_sha256"],"inputs":result.report()["inputs"],
            "codecs": objectives.iter().map(CompressionCostModel::name).collect::<Vec<_>>(),
            "outputs": outputs}),
    })
}

fn write_format_deliveries(
    args: &Args, entries: &[EntrySource], config: &ProjectConfig, result: &ServiceCompilation,
) -> Result<(), String> {
    let directory = args.out_dir.as_ref().unwrap();
    let mut written = Vec::new();
    let mut outputs = Vec::new();
    for group in result.outputs() {
        let configured = if group.name()=="primary" { config.clone() } else {
            config.delivery.also.iter().find(|output| output.name==group.name())
                .ok_or("missing configured output")?.configuration(config)
        };
        let multiple = group.objectives().count() > 1;
        for codec in group.objectives() {
            let selected = result.javascript_output(group.name(),codec).ok_or("missing output artifact")?;
            let base = configured.delivery.directory.as_deref().unwrap_or_else(||Path::new(group.name()));
            let prefix = if multiple {base.join(codec.name())} else {base.to_path_buf()};
            let prefix = prefix.components().filter_map(|part| if let std::path::Component::Normal(name)=part {Some(name.to_string_lossy())} else {None}).collect::<Vec<_>>().join("/");
            let child = directory.join(&prefix);
            let mut plan = plan_delivery(args,entries,&configured,result,selected,codec,Some(&child))?;
            for (path,_) in &plan.written {
                let relative = path.strip_prefix(&child).map_err(|_|"an output escapes its directory")?;
                if relative.components().any(|part| !matches!(part,std::path::Component::Normal(_) | std::path::Component::CurDir)) {
                    return Err("an output escapes its directory".into());
                }
            }
            for mut output in plan.manifest["outputs"].as_array_mut().unwrap().drain(..) {
                prefix_manifest_paths(&mut output,&prefix);
                output["output"] = json!(group.name());
                output["codec"] = json!(codec.name());
                output["codec_settings"] = json!(configured.objective.settings());
                output["policy_fingerprint"] = selected.details()["policy_fingerprint"].clone();
                outputs.push(output);
            }
            written.append(&mut plan.written);
        }
    }
    write_planned_delivery(PlannedDelivery {
        directory:directory.clone(), manifest_path:directory.join("lilscript.manifest.json"), written,
        manifest:json!({"version":5,"source_sha256":result.report()["source_sha256"],
            "inputs":result.report()["inputs"],"outputs":outputs}),
    })
}

fn prefix_manifest_paths(output: &mut Value, prefix: &str) {
    if prefix.is_empty() || prefix=="." {return;}
    let path = |value: &mut Value| {
        *value = json!(format!(
            "{prefix}/{}",
            value.as_str().expect("manifest path is a string")
        ));
    };
    let paths = |value: &mut Value| {
        for value in value.as_array_mut().expect("manifest paths are an array") {
            path(value);
        }
    };
    for file in output["files"].as_array_mut().unwrap() {
        path(&mut file["file"]);
        paths(&mut file["imports"]);
        paths(&mut file["dynamic_imports"]);
    }
    for entry in output["entries"].as_array_mut().unwrap() {
        path(&mut entry["file"]);
        paths(&mut entry["closure"]);
    }
    paths(&mut output["side_effects"]);
}

/// Source labels are relative to the primary entry, including sibling trees.
/// They are metadata only; output path validation never accepts these `..`s.
fn relative_source_name(base: &Path, path: &Path) -> String {
    let base=base.components().collect::<Vec<_>>();let parts=path.components().collect::<Vec<_>>();
    let shared=base.iter().zip(&parts).take_while(|(a,b)|a==b).count();
    let mut result=PathBuf::new();
    for _ in shared..base.len(){result.push("..");}
    for part in &parts[shared..]{result.push(part.as_os_str());}
    result.iter().map(|part|part.to_string_lossy()).collect::<Vec<_>>().join("/")
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
    let Ok(root) = fs::canonicalize(directory) else { return Ok(()); };
    let current=previous.get("outputs").and_then(Value::as_array).into_iter().flatten()
        .filter_map(|output| output.get("files")?.as_array()).flatten()
        .filter_map(|file| file.get("file")?.as_str());
    let legacy=previous.get("chunks").filter(|_|previous["version"]==2).and_then(Value::as_array)
        .into_iter().flatten().filter_map(|file|file.get("file")?.as_str());
    for file in current.chain(legacy) {
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
        let objectives = report["javascript_policies"].as_array();
        let codecs = objectives
            .map(|policies| {
                policies
                    .iter()
                    .map(|policy| text(&policy["objective"]["codec"]).to_lowercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| text(&policy["objective"]["codec"]).to_lowercase());
        line(
            if objectives.is_some() {
                "objectives"
            } else {
                "objective"
            },
            format!("{} bytes, effort {}", codecs, text(&policy["effort"])),
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
        for output in report["outputs"].as_array().into_iter().flatten().filter(|output|output["name"]!="primary") {
            for (codec,winner) in ["raw","gzip","brotli"].iter().zip(output["winners"].as_array().into_iter().flatten()) {
                let Some(artifact) = winner.as_u64().and_then(|index|artifacts.get(index as usize)) else {continue;};
                line(&format!("{} {codec}",text(&output["name"])),format!("raw {}, gzip {}, brotli {}; {} file(s)",
                    text(&artifact["raw"]),text(&artifact["gzip9"]),text(&artifact["brotli11"]),
                    artifact["details"]["delivery"]["files"].as_array().map_or(1,Vec::len)));
            }
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
                let kept = kept
                    .into_iter()
                    .chain(joints)
                    .chain(starts)
                    .collect::<Vec<_>>();
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
    let receipt = policy_report(args, loaded, options)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&receipt).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn policy_report(
    args: &Args,
    loaded: &LoadedConfig,
    options: ServiceOptions,
) -> Result<Value, String> {
    let resolve = |request| -> Result<(String, Value, Value), String> {
        let policy = options.resolve_policy(&loaded.config, request)?;
        let fingerprint = policy
            .fingerprint()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok((fingerprint, policy.receipt(), json!(policy.diagnostics())))
    };
    let javascript = options.javascript_request().map(resolve).transpose()?;
    let native = options.native_request().map(resolve).transpose()?;
    let (fingerprint, policy, diagnostics) = javascript
        .clone()
        .or_else(|| native.clone())
        .expect("every target resolves a policy");
    let mut receipt = json!({
        "config": loaded.path.as_ref().map(|path| path.display().to_string()),
        // Retired keys the file set: each has no effect in this compiler.
        "warnings": loaded.warnings,
        "request": {"logical_work": options.logical_work, "retained_bytes": options.retained_bytes},
        // How this run executes, after command-line overrides. Deliberately
        // outside the fingerprint: thread counts must never change the output.
        "execution": {
            "threads": loaded.config.execution.jobs,
            // Removed (architecture §14.2): reported, with no effect, for
            // one release.
            "codec_workers": args.codec_jobs.map(NonZeroUsize::get),
            "cache": loaded.config.cache,
            "mode": format!("{:?}", args.mode),
            "target": format!("{:?}", args.target),
        },
        "fingerprint": fingerprint,
        "policy": policy,
        "diagnostics": diagnostics,
    });
    let primary = options.resolve_policy(&loaded.config,
        options.javascript_request().or_else(|| options.native_request()).unwrap())?;
    receipt["resolution"] = primary.resolution();
    receipt["execution"]["cache"] = serde_json::to_value(primary.cache()).unwrap();
    // Input origins explain CLI precedence without making path spelling or
    // equivalent environment/flag adapters change canonical policy identity.
    receipt["configuration_inputs"] = json!({
        "file": if args.config.is_some() { "explicit" }
            else if loaded.path.is_some() { "discovered" } else { "defaults" },
        "overrides": {
            "cache": args.cache.as_ref().map(|_| "--cache"),
            "delivery": args.delivery.map(|_| "--delivery"),
            "format": args.format.map(|_| "--format"),
            "search_disabled": matches!(args.mode, BuildMode::Development),
            "delegate_bundling": args.delegate_bundling,
            "logical_work": if args.logical_work.is_some() { "--logical-work" }
                else if std::env::var_os("LILSCRIPT_SEMANTIC_WORK").is_some() { "LILSCRIPT_SEMANTIC_WORK" }
                else { "CLI default" },
            "retained_bytes": if args.retained_bytes.is_some() { "--retained-bytes" } else { "service default" },
            "proxy_pruning": if args.proxy_pruning.is_some() { "--proxy-pruning" }
                else if std::env::var_os("LILSCRIPT_WALK_AUDIT").is_some() { "LILSCRIPT_WALK_AUDIT" }
                else { "configuration/default" }
        },
        "resources": "service ceilings intersect TOML limits; the smaller value wins"
    });
    let policies = options.resolve_javascript_policies(&loaded.config)?;
    if policies.len() > 1 {
        receipt["objectives"] = json!(policies
            .iter()
            .map(|policy| policy.objective().unwrap().codec.name())
            .collect::<Vec<_>>());
        receipt["javascript_policies"] = json!(policies.iter().map(|policy| json!({
            "fingerprint": policy.fingerprint().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "policy": policy.receipt(), "diagnostics": policy.diagnostics(),
            "resolution": policy.resolution(),
        })).collect::<Vec<_>>());
        use sha2::{Digest, Sha256};
        let identity = json!({"kind": "independent-objective-policies-v1",
            "javascript": policies.iter().map(|policy| policy.receipt()).collect::<Vec<_>>(),
            "native": native.as_ref().map(|(_, policy, _)| policy)});
        receipt["primary_fingerprint"] = receipt["fingerprint"].clone();
        receipt["fingerprint"] = json!(format!(
            "{:x}",
            Sha256::digest(identity.to_string().as_bytes())
        ));
        receipt["fingerprint_scope"] = json!("requested objective policies");
        receipt["request"]["objectives"] = receipt["objectives"].clone();
    }
    // `--target all` also builds C, under its own policy.
    let additional = options.resolve_additional_outputs(&loaded.config)?;
    if !additional.is_empty() {
        use sha2::{Digest, Sha256};
        receipt["additional_outputs"] = json!(additional.iter().map(|output| output.receipt()).collect::<Vec<_>>());
        let identity = json!({"kind":"independent-delivery-policies-v1","primary":receipt["fingerprint"],
            "additional":receipt["additional_outputs"],
            "directories":std::iter::once(("primary",loaded.config.delivery.directory.as_ref())).chain(loaded.config.delivery.also.iter().map(|output|
                (output.name.as_str(),output.directory.as_ref().or(loaded.config.delivery.directory.as_ref())))).collect::<Vec<_>>()});
        receipt["output_directories"] = identity["directories"].clone();
        receipt["primary_fingerprint"] = receipt["fingerprint"].clone();
        receipt["fingerprint"] = json!(format!("{:x}",Sha256::digest(identity.to_string().as_bytes())));
        receipt["fingerprint_scope"] = json!("requested delivery and objective policies");
    }
    if options.native_request().is_some() {
        receipt["native_capabilities"] = lilscript::native_capabilities::receipt();
        receipt["native_toolchain_controls"] = json!(loaded.config.target.native);
        if matches!(args.target, Target::Native | Target::All) {
            receipt["native_toolchain"] = lilscript::native_toolchain::NativeToolchain::resolve(
                &loaded.config, std::env::var_os("CC").as_deref())?.receipt();
        }
    }
    if let (Some(_), Some((fingerprint, policy, diagnostics))) = (&javascript, native) {
        receipt["native_fingerprint"] = json!(fingerprint);
        receipt["native_policy"] = policy;
        receipt["native_diagnostics"] = diagnostics;
        receipt["native_resolution"] = options.resolve_policy(&loaded.config,
            options.native_request().unwrap())?.resolution();
    }
    Ok(receipt)
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
fn write_native_header(result: &lilscript::build::ServiceCompilation, output: &Path) -> Result<(), String> {
    let Some(header) = result.native_header() else { return Ok(()); };
    let path = output.with_extension("h");
    ensure_parent(&path)?;
    fs::write(&path, header).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn compile_native(c: &str, header: Option<&str>, output: &Path, config: &ProjectConfig) -> Result<(), String> {
    let cc = std::env::var_os("CC");
    let toolchain = lilscript::native_toolchain::NativeToolchain::resolve(config, cc.as_deref())?;
    let receipt = toolchain.compile(c, header, output)?;
    let mut name = output.as_os_str().to_owned();
    name.push(".native.json");
    let path = PathBuf::from(name);
    fs::write(&path, serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?)
        .map_err(|e| format!("failed to write native receipt {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_pruning_cli_adapter_resolves_before_the_shared_policy_receipt() {
        for configured in ["on", "audit", "off"] {
            for legacy_audit in [false, true] {
                for explicit in [None, Some("on"), Some("audit"), Some("off")] {
                    let mut arguments = vec!["lilscript", "input.lil"];
                    if let Some(explicit) = explicit {
                        arguments.extend(["--proxy-pruning", explicit]);
                    }
                    let args = Args::try_parse_from(arguments).unwrap();
                    let mut config: ProjectConfig =
                        toml::from_str(&format!("[policy.search]\nproxy_pruning='{configured}'"))
                            .unwrap();
                    apply_proxy_override(&mut config, args.proxy_pruning, legacy_audit);
                    let expected =
                        explicit.unwrap_or(if legacy_audit { "audit" } else { configured });
                    let loaded = LoadedConfig {
                        config,
                        path: None,
                        warnings: vec![],
                    };
                    let options = service_options_with_environment(&args, None).unwrap();
                    let printed = policy_report(&args, &loaded, options).unwrap();
                    let resolved = options
                        .resolve_policy(&loaded.config, options.javascript_request().unwrap())
                        .unwrap();
                    assert_eq!(printed["policy"], resolved.receipt());
                    assert_eq!(
                        printed["policy"]["objective"]["search"]["proxy_pruning"],
                        expected
                    );
                }
            }
        }
        assert!(
            Args::try_parse_from(["lilscript", "input.lil", "--proxy-pruning", "true"]).is_err()
        );
    }

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
        for flag in ["--logical-work", "--retained-bytes"] {
            for invalid in ["0", "-1", "garbage", "18446744073709551616"] {
                assert!(Args::try_parse_from(["lilscript", "input.lil", flag, invalid]).is_err());
            }
        }
    }

    #[test]
    fn explicit_resource_flags_override_the_legacy_environment_with_toml_caps() {
        let args = Args::try_parse_from([
            "lilscript",
            "input.lil",
            "--logical-work",
            "1000000",
            "--retained-bytes",
            "3000000",
        ])
        .unwrap();
        let options = service_options_with_environment(
            &args,
            Some(std::ffi::OsStr::new("invalid ignored adapter")),
        )
        .unwrap();
        assert_eq!(options.logical_work, 1_000_000);
        assert_eq!(options.retained_bytes, 3_000_000);
        let loaded = LoadedConfig {
            config: toml::from_str(
                "[policy.resources]\nlogical_work=800000\nretained_bytes=2000000",
            )
            .unwrap(),
            path: None,
            warnings: vec![],
        };
        let report = policy_report(&args, &loaded, options).unwrap();
        let expected = options
            .resolve_policy(&loaded.config, options.javascript_request().unwrap())
            .unwrap();
        assert_eq!(report["policy"], expected.receipt());
        assert_eq!(report["policy"]["resources"]["logical_work"], 800_000);
        assert_eq!(report["policy"]["resources"]["retained_bytes"], 2_000_000);
        assert_eq!(
            report["fingerprint"],
            expected
                .fingerprint()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
    }

    #[test]
    fn invalid_legacy_work_never_silently_falls_back_to_the_default() {
        let args = Args::try_parse_from(["lilscript", "input.lil"]).unwrap();
        assert_eq!(
            service_options_with_environment(&args, None)
                .unwrap()
                .logical_work,
            40_000_000_000
        );
        assert_eq!(
            service_options_with_environment(&args, Some(std::ffi::OsStr::new("12345")))
                .unwrap()
                .logical_work,
            12345
        );
        for invalid in ["", "0", "-1", "bad", "18446744073709551616"] {
            assert!(
                service_options_with_environment(&args, Some(std::ffi::OsStr::new(invalid)))
                    .unwrap_err()
                    .contains("LILSCRIPT_SEMANTIC_WORK")
            );
        }
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
            let options = service_options_with_environment(&args, None).unwrap();
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
    fn policy_report_explains_unavailable_controls_for_each_target() {
        let args = Args::try_parse_from([
            "lilscript",
            "input.lil",
            "--target",
            "all",
            "--print-policy",
        ])
        .unwrap();
        let loaded = LoadedConfig {
            config: toml::from_str("[policy.tactics]\nhelper-sharing='on'").unwrap(),
            path: None,
            warnings: vec![],
        };
        let report = policy_report(&args, &loaded, service_options_with_environment(&args, None).unwrap()).unwrap();
        for key in ["diagnostics", "native_diagnostics"] {
            assert!(report[key]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value.as_str().unwrap().contains("helper-sharing")));
        }
        for key in ["policy", "native_policy"] {
            let tactic = report[key]["tactics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|value| value["id"] == "helper-sharing")
                .unwrap();
            assert_eq!(tactic["available"], false);
            assert_eq!(tactic["state"]["enabled"], false);
            assert!(report[key].get("diagnostics").is_none());
        }
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

#[cfg(test)]
#[path = "cli_objective_tests.rs"]
mod objective_tests;
