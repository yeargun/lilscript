use std::fs;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, ValueEnum};

use lilscript::config::{load_project_config, BundleMode, CandidateSearch, ProjectConfig};
use lilscript::package::write_lockfile;
use lilscript::{
    compile_path_all_configured, compile_path_all_to_js_bundle_configured, compile_path_configured,
    compile_path_explained_configured, compile_path_to_c_configured,
    compile_path_to_js_bundle_configured, compile_path_to_js_module_configured,
    compile_path_to_js_module_explained_configured, profile_template_path_configured,
    render_module_diagnostic, JavaScriptBundle,
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
    match args.target {
        Target::Js => {
            if loaded.config.bundle.mode == BundleMode::Single {
                if let Some(format) = args.explain {
                    let compilation =
                        compile_path_explained_configured(&args.input, &loaded.config)
                            .map_err(|error| render_module_diagnostic(&error))?;
                    print_explanation(
                        format,
                        &compilation.optimization_reports,
                        &compilation.selection_metrics,
                        &compilation.abi_manifest,
                    )?;
                    write_or_print(args.output.as_deref(), &compilation.javascript)?;
                } else {
                    let js = compile_path_configured(&args.input, &loaded.config)
                        .map_err(|error| render_module_diagnostic(&error))?;
                    write_or_print(args.output.as_deref(), &js)?;
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
                if let Some(format) = args.explain {
                    let compilation =
                        compile_path_to_js_module_explained_configured(&args.input, &loaded.config)
                            .map_err(|error| render_module_diagnostic(&error))?;
                    print_explanation(
                        format,
                        &compilation.optimization_reports,
                        &compilation.selection_metrics,
                        &compilation.abi_manifest,
                    )?;
                    write_or_print(args.output.as_deref(), &compilation.javascript)?;
                } else {
                    let js = compile_path_to_js_module_configured(&args.input, &loaded.config)
                        .map_err(|error| render_module_diagnostic(&error))?;
                    write_or_print(args.output.as_deref(), &js)?;
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
                fs::write(&javascript, &artifacts.javascript).map_err(|error| {
                    format!("failed to write {}: {error}", javascript.display())
                })?;
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
                write_javascript_bundle(&javascript, &artifacts.javascript)?;
                fs::write(&c, &artifacts.c)
                    .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
                compile_native(&artifacts.c, &base)?;
            }
        }
    }

    Ok(())
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
        }
        ExplainFormat::Json => eprintln!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "optimization_reports": reports,
                "javascript_selection": metrics,
                "abi_manifest": abi,
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
    write_javascript_bundle(output, &bundle)
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
