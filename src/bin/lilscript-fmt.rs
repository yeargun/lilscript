use std::fs;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Parser};
use lilscript::config::load_project_config;
use lilscript::formatter::format_source;

#[derive(Debug, Parser)]
#[command(name = "lilscript-fmt")]
#[command(about = "Canonical formatter for LilScript source files.")]
struct Args {
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    #[arg(long, conflicts_with_all = ["write", "stdout"])]
    check: bool,

    #[arg(long, action = ArgAction::SetTrue, conflicts_with_all = ["check", "stdout"])]
    write: bool,

    #[arg(long, conflicts_with_all = ["check", "write"])]
    stdout: bool,

    #[arg(long)]
    force: bool,

    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() {
    match run() {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("lilscript-fmt: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<bool, String> {
    let args = Args::parse();
    let files = collect_files(&args.paths)?;
    if args.stdout && files.len() != 1 {
        return Err("--stdout requires exactly one source file".to_string());
    }
    let mut clean = true;
    for file in files {
        let loaded = load_project_config(&file, args.config.as_deref())
            .map_err(|error| error.to_string())?;
        if !loaded.config.format.enabled && !args.force {
            continue;
        }
        let source = fs::read_to_string(&file)
            .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
        let formatted = format_source(&source, &loaded.config.format)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        if args.stdout {
            print!("{formatted}");
        } else if args.check {
            if source != formatted {
                eprintln!("would reformat {}", file.display());
                clean = false;
            }
        } else if source != formatted {
            fs::write(&file, formatted)
                .map_err(|error| format!("failed to write {}: {error}", file.display()))?;
        }
    }
    Ok(clean)
}

fn collect_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for path in paths {
        collect_path(path, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path.extension().and_then(|value| value.to_str()) == Some("lil") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if !path.is_dir() {
        return Err(format!("{} does not exist", path.display()));
    }
    let entries = fs::read_dir(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let child = entry.path();
        if child
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| matches!(name, "target" | "node_modules" | ".git"))
        {
            continue;
        }
        collect_path(&child, files)?;
    }
    Ok(())
}
