use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, ValueEnum};

use lilscript::config::{load_project_config, BundleMode};
use lilscript::{
    compile_path_all_configured, compile_path_configured, compile_path_to_c_configured,
    compile_path_to_js_module_configured, render_module_diagnostic,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Target {
    Js,
    JsModule,
    C,
    Native,
    All,
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
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let loaded = load_project_config(&args.input, args.config.as_deref())
        .map_err(|error| error.to_string())?;
    if loaded.config.bundle.mode != BundleMode::Single {
        return Err(format!(
            "bundle mode `{:?}` is configured, but this target currently emits only a single whole-program artifact",
            loaded.config.bundle.mode
        ));
    }
    match args.target {
        Target::Js => {
            let js = compile_path_configured(&args.input, &loaded.config)
                .map_err(|error| render_module_diagnostic(&error))?;
            write_or_print(args.output.as_deref(), &js)?;
        }
        Target::JsModule => {
            let js = compile_path_to_js_module_configured(&args.input, &loaded.config)
                .map_err(|error| render_module_diagnostic(&error))?;
            write_or_print(args.output.as_deref(), &js)?;
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
            fs::write(&javascript, &artifacts.javascript)
                .map_err(|error| format!("failed to write {}: {error}", javascript.display()))?;
            fs::write(&c, &artifacts.c)
                .map_err(|error| format!("failed to write {}: {error}", c.display()))?;
            compile_native(&artifacts.c, &base)?;
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
    let mut child = Command::new(&compiler)
        .args(["-x", "c", "-std=c11", "-O3", "-o"])
        .arg(output)
        .arg("-")
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
