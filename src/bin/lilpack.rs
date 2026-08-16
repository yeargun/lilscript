use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "lilpack")]
#[command(about = "Bundle and serve Lilscript applications with an integrated Vite engine.")]
struct Cli {
    #[command(subcommand)]
    command: LilpackCommand,
}

#[derive(Debug, Subcommand)]
enum LilpackCommand {
    /// Start the Lilpack development server with dependency-aware hot reload.
    Dev(DevArgs),
    /// Create a production bundle containing Lilscript, JS/TS, CSS, and assets.
    Build(BuildArgs),
}

#[derive(Debug, Args)]
struct CommonArgs {
    /// The Lilscript application entry.
    entry: PathBuf,

    /// Application root. Defaults to the entry's directory.
    #[arg(long)]
    root: Option<PathBuf>,

    /// Explicit lilscript.toml path.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Public base path used by development and production asset URLs.
    #[arg(long, default_value = "/")]
    base: String,

    /// Override the Lilscript compiler executable.
    #[arg(long, hide = true)]
    compiler: Option<PathBuf>,

    /// Override Vite's internal module entry.
    #[arg(long, hide = true)]
    vite: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct DevArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Development server address.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Development server port.
    #[arg(long, default_value_t = 5173)]
    port: u16,

    /// Open the application in the default browser.
    #[arg(long)]
    open: bool,
}

#[derive(Debug, Args)]
struct BuildArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Output directory, relative to the application root unless absolute.
    #[arg(long, default_value = "dist")]
    out_dir: PathBuf,

    /// Emit production source maps.
    #[arg(long)]
    sourcemap: bool,

    /// Keep the final Vite output unminified.
    #[arg(long)]
    no_minify: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        LilpackCommand::Dev(args) => {
            let resolved = resolve_common(args.common)?;
            let mut command = engine_command(&resolved)?;
            command
                .arg("dev")
                .arg("--entry")
                .arg(&resolved.entry)
                .arg("--root")
                .arg(&resolved.root)
                .arg("--host")
                .arg(args.host)
                .arg("--port")
                .arg(args.port.to_string());
            append_config(&mut command, resolved.config.as_deref());
            if args.open {
                command.arg("--open");
            }
            execute(command)
        }
        LilpackCommand::Build(args) => {
            let resolved = resolve_common(args.common)?;
            let out_dir = if args.out_dir.is_absolute() {
                args.out_dir
            } else {
                resolved.root.join(args.out_dir)
            };
            let out_dir = safe_output_directory(&resolved.root, &out_dir)?;
            let mut command = engine_command(&resolved)?;
            command
                .arg("build")
                .arg("--entry")
                .arg(&resolved.entry)
                .arg("--root")
                .arg(&resolved.root)
                .arg("--out-dir")
                .arg(out_dir);
            append_config(&mut command, resolved.config.as_deref());
            if args.sourcemap {
                command.arg("--sourcemap");
            }
            if args.no_minify {
                command.arg("--no-minify");
            }
            execute(command)
        }
    }
}

fn execute(mut command: Command) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        Err(format!("failed to start the Lilpack engine: {error}"))
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .map_err(|error| format!("failed to start the Lilpack engine: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            std::process::exit(status.code().unwrap_or(1));
        }
    }
}

struct ResolvedCommon {
    entry: PathBuf,
    root: PathBuf,
    config: Option<PathBuf>,
    compiler: PathBuf,
    vite: PathBuf,
    base: String,
}

fn resolve_common(args: CommonArgs) -> Result<ResolvedCommon, String> {
    let entry = canonical_file(&args.entry, "Lilscript entry")?;
    let root = match args.root {
        Some(root) => canonical_directory(&root, "application root")?,
        None => entry
            .parent()
            .ok_or_else(|| "Lilscript entry has no parent directory".to_string())?
            .to_path_buf(),
    };
    if !entry.starts_with(&root) {
        return Err(format!(
            "Lilscript entry {} must be inside application root {}",
            entry.display(),
            root.display()
        ));
    }
    let index = root.join("index.html");
    if !index.is_file() {
        return Err(format!(
            "Lilpack needs {} as the application HTML shell",
            index.display()
        ));
    }
    let config = args
        .config
        .map(|path| canonical_file(&path, "Lilscript config"))
        .transpose()?;
    let compiler = resolve_compiler(
        args.compiler
            .or_else(|| std::env::var_os("LILSCRIPT_COMPILER").map(PathBuf::from)),
    )?;
    let vite = resolve_vite(
        &root,
        args.vite
            .or_else(|| std::env::var_os("LILPACK_VITE").map(PathBuf::from)),
    )?;
    Ok(ResolvedCommon {
        entry,
        root,
        config,
        compiler,
        vite,
        base: args.base,
    })
}

fn engine_command(resolved: &ResolvedCommon) -> Result<Command, String> {
    let node = resolve_nvm_node()?;
    let mut command = Command::new(node);
    command
        .arg("--input-type=module")
        .arg("--eval")
        .arg(include_str!("../../tooling/lilpack/vite-runtime.mjs"))
        .arg("--")
        .arg("lilpack")
        .arg("--compiler")
        .arg(&resolved.compiler)
        .arg("--vite")
        .arg(&resolved.vite)
        .arg("--base")
        .arg(&resolved.base);
    Ok(command)
}

#[cfg(unix)]
fn resolve_nvm_node() -> Result<PathBuf, String> {
    let nvm_dir = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".nvm")))
        .ok_or_else(|| "cannot locate nvm; set NVM_DIR and run `nvm install`".to_string())?;
    let nvm_script = nvm_dir.join("nvm.sh");
    if !nvm_script.is_file() {
        return Err(format!(
            "cannot find {}; install nvm or set NVM_DIR",
            nvm_script.display()
        ));
    }
    let requested = include_str!("../../.nvmrc").trim();
    let output = Command::new("bash")
        .arg("-c")
        .arg("set -e; . \"$1\"; nvm use --silent \"$2\" >/dev/null; nvm which --silent current")
        .arg("lilpack-nvm")
        .arg(&nvm_script)
        .arg(requested)
        .output()
        .map_err(|error| format!("failed to run nvm: {error}"))?;
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "nvm cannot select Node {requested}; run `nvm install {requested}`{}{}",
            if diagnostic.trim().is_empty() {
                ""
            } else {
                ": "
            },
            diagnostic.trim()
        ));
    }
    let path = String::from_utf8(output.stdout)
        .map_err(|error| format!("nvm returned a non-UTF-8 Node path: {error}"))?;
    validate_node(canonical_file(
        Path::new(path.trim()),
        "nvm Node executable",
    )?)
}

#[cfg(windows)]
fn resolve_nvm_node() -> Result<PathBuf, String> {
    let directory = std::env::var_os("NVM_SYMLINK")
        .map(PathBuf::from)
        .ok_or_else(|| "cannot locate nvm-windows; run `nvm use` first".to_string())?;
    validate_node(canonical_file(
        &directory.join("node.exe"),
        "nvm Node executable",
    )?)
}

fn validate_node(path: PathBuf) -> Result<PathBuf, String> {
    let output = Command::new(&path)
        .arg("--version")
        .output()
        .map_err(|error| format!("failed to inspect nvm Node {}: {error}", path.display()))?;
    let version = String::from_utf8(output.stdout)
        .map_err(|error| format!("Node returned a non-UTF-8 version: {error}"))?;
    let version = semver::Version::parse(version.trim().trim_start_matches('v'))
        .map_err(|error| format!("cannot parse Node version `{}`: {error}", version.trim()))?;
    if !vite_node_compatible(&version) {
        return Err(format!(
            "nvm selected Node {version}, but Vite requires Node 20.19+ or 22.12+"
        ));
    }
    Ok(path)
}

fn vite_node_compatible(version: &semver::Version) -> bool {
    (version.major == 20 && version.minor >= 19)
        || (version.major == 22 && version.minor >= 12)
        || version.major > 22
}

fn append_config(command: &mut Command, config: Option<&Path>) {
    if let Some(config) = config {
        command.arg("--config").arg(config);
    }
}

fn resolve_compiler(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return canonical_file(&path, "Lilscript compiler");
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to locate the Lilpack executable: {error}"))?;
    let sibling = executable
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(executable_name("lilscript"));
    if sibling.is_file() {
        return sibling
            .canonicalize()
            .map_err(|error| format!("cannot resolve {}: {error}", sibling.display()));
    }
    Err("cannot find the Lilscript compiler beside Lilpack; set LILSCRIPT_COMPILER".to_string())
}

fn resolve_vite(root: &Path, explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return vite_entry(&path);
    }
    let integrated = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tooling/lilpack/node_modules/vite/dist/node/index.js");
    if integrated.is_file() {
        return canonical_file(&integrated, "integrated Vite module");
    }
    let mut directory = Some(root);
    while let Some(current) = directory {
        let candidate = current.join("node_modules/vite/dist/node/index.js");
        if candidate.is_file() {
            return canonical_file(&candidate, "integrated Vite module");
        }
        directory = current.parent();
    }
    Err("Lilpack's Vite engine is not installed; run `npm install` in tooling/lilpack".to_string())
}

fn vite_entry(path: &Path) -> Result<PathBuf, String> {
    let path = if path.is_dir() {
        path.join("dist/node/index.js")
    } else {
        path.to_path_buf()
    };
    canonical_file(&path, "integrated Vite module")
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!("{label} {} is not a file", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("cannot resolve {label} {}: {error}", path.display()))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_dir() {
        return Err(format!("{label} {} is not a directory", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("cannot resolve {label} {}: {error}", path.display()))
}

fn safe_output_directory(root: &Path, requested: &Path) -> Result<PathBuf, String> {
    let output = resolve_output_path(requested)?;
    if output == root || root.starts_with(&output) {
        return Err(format!(
            "refusing to empty broad output directory {}; choose a dedicated directory such as `dist`",
            output.display()
        ));
    }
    Ok(output)
}

fn resolve_output_path(requested: &Path) -> Result<PathBuf, String> {
    let absolute = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to read current directory: {error}"))?
            .join(requested)
    };
    let mut cursor = absolute.as_path();
    let mut suffix = Vec::new();
    while !cursor.exists() {
        let name = cursor
            .file_name()
            .ok_or_else(|| format!("cannot resolve output directory {}", absolute.display()))?;
        suffix.push(name.to_os_string());
        cursor = cursor
            .parent()
            .ok_or_else(|| format!("cannot resolve output directory {}", absolute.display()))?;
    }
    let mut resolved = cursor.canonicalize().map_err(|error| {
        format!(
            "cannot resolve output directory {}: {error}",
            absolute.display()
        )
    })?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(normalize_path(&resolved))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_directory_must_not_collapse_to_the_application_root() {
        let directory =
            std::env::temp_dir().join(format!("lilpack-output-safety-test-{}", std::process::id()));
        let root = directory.join("app");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();

        assert!(safe_output_directory(&root, &root.join("dist")).is_ok());
        assert!(safe_output_directory(&root, &root).is_err());
        assert!(safe_output_directory(&root, &root.join("missing/..")).is_err());
        assert!(safe_output_directory(&root, &root.join("..")).is_err());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn recognizes_vites_supported_node_ranges() {
        assert!(!vite_node_compatible(&semver::Version::new(20, 18, 0)));
        assert!(vite_node_compatible(&semver::Version::new(20, 19, 0)));
        assert!(!vite_node_compatible(&semver::Version::new(22, 11, 0)));
        assert!(vite_node_compatible(&semver::Version::new(22, 12, 0)));
        assert!(vite_node_compatible(&semver::Version::new(24, 0, 0)));
    }
}
