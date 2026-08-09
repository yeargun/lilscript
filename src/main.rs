use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, ValueEnum};

use lilscript::config::{load_project_config, BundleMode, CandidateSearch};
use lilscript::package::write_lockfile;
use lilscript::{
    compile_path_all_configured, compile_path_configured, compile_path_explained_configured,
    compile_path_to_c_configured, compile_path_to_js_bundle_configured,
    compile_path_to_js_module_configured, compile_path_to_js_module_explained_configured,
    profile_template_path_configured, render_module_diagnostic, JavaScriptBundle,
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
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let mut loaded = load_project_config(&args.input, args.config.as_deref())
        .map_err(|error| error.to_string())?;
    if args.write_lock {
        let path = write_lockfile(&loaded.config).map_err(|error| error.to_string())?;
        eprintln!("wrote {}", path.display());
    }
    if matches!(args.mode, BuildMode::Development) {
        loaded.config.javascript.candidate_search = CandidateSearch::Off;
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
            let artifacts = compile_path_all_configured(&args.input, &loaded.config)
                .map_err(|error| render_module_diagnostic(&error))?;
            let base = args.output.unwrap_or_else(|| {
                let mut output = args.input.clone();
                output.set_extension("");
                output
            });
            let javascript = base.with_extension("js");
            let c = base.with_extension("c");
            ensure_parent(&base)?;
            if loaded.config.bundle.mode == BundleMode::Single {
                fs::write(&javascript, &artifacts.javascript).map_err(|error| {
                    format!("failed to write {}: {error}", javascript.display())
                })?;
            } else {
                write_configured_bundle(&args.input, Some(&javascript), &loaded.config)?;
            }
            fs::write(&c, &artifacts.c)
                .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
            compile_native(&artifacts.c, &base)?;
        }
    }

    Ok(())
}

fn print_explanation(
    format: ExplainFormat,
    reports: &[lilscript::optimizer::OptimizationReport],
    metrics: &lilscript::JavaScriptSelectionMetrics,
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
            eprintln!("{:<34} {}", "javascript codec", metrics.codec);
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
            eprintln!("{:<34} {}", "peephole rewrites", metrics.peephole_rewrites);
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
        if current.contains(file)
            || !(file.starts_with("chunk-") || file.starts_with("lil-chunk-"))
            || Path::new(file).file_name().and_then(|name| name.to_str()) != Some(file)
        {
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
