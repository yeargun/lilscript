use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use lilscript::config::load_project_config;
use lilscript::lint::{lint_path, DiagnosticSeverity, LintDiagnostic};
use serde_json::json;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
    Sarif,
}

#[derive(Debug, Parser)]
#[command(name = "lilscript-lint")]
#[command(about = "Compiler-aware correctness, performance, and bundle linter for LilScript.")]
struct Args {
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    #[arg(long)]
    fix: bool,

    #[arg(long)]
    deny_warnings: bool,

    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() {
    match run() {
        Ok(false) => {}
        Ok(true) => std::process::exit(1),
        Err(error) => {
            eprintln!("lilscript-lint: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<bool, String> {
    let args = Args::parse();
    let files = collect_files(&args.paths)?;
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        let mut loaded = load_project_config(&file, args.config.as_deref())
            .map_err(|error| error.to_string())?;
        loaded.config.lint.deny_warnings |= args.deny_warnings;
        for diagnostic in lint_path(&file, &loaded.config).map_err(|error| error.to_string())? {
            let key = (diagnostic.path.clone(), diagnostic.span, diagnostic.rule);
            if seen.insert(key) {
                diagnostics.push(diagnostic);
            }
        }
    }
    diagnostics.sort_by(|left, right| {
        (&left.path, left.span.start, left.rule).cmp(&(&right.path, right.span.start, right.rule))
    });
    if args.fix {
        apply_fixes(&diagnostics)?;
    }
    match args.format {
        OutputFormat::Text => print_text(&diagnostics)?,
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&diagnostics)
                .map_err(|error| format!("failed to serialize diagnostics: {error}"))?
        ),
        OutputFormat::Sarif => println!(
            "{}",
            serde_json::to_string_pretty(&sarif(&diagnostics))
                .map_err(|error| format!("failed to serialize SARIF: {error}"))?
        ),
    }
    Ok(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.severity,
            DiagnosticSeverity::Warning | DiagnosticSeverity::Error
        )
    }))
}

fn print_text(diagnostics: &[LintDiagnostic]) -> Result<(), String> {
    for diagnostic in diagnostics {
        let source = fs::read_to_string(&diagnostic.path)
            .map_err(|error| format!("failed to read {}: {error}", diagnostic.path.display()))?;
        let (line, column) = line_column(&source, diagnostic.span.start);
        let severity = match diagnostic.severity {
            DiagnosticSeverity::Hint => "hint",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Error => "error",
        };
        println!(
            "{}:{}:{}: {}[{}]: {}",
            diagnostic.path.display(),
            line,
            column,
            severity,
            diagnostic.rule,
            diagnostic.message
        );
        if let Some(evidence) = &diagnostic.evidence {
            println!("  evidence: {evidence}");
        }
        if let Some(help) = &diagnostic.help {
            println!("  help: {help}");
        }
    }
    Ok(())
}

fn sarif(diagnostics: &[LintDiagnostic]) -> serde_json::Value {
    let mut rules = BTreeMap::new();
    for diagnostic in diagnostics {
        rules.entry(diagnostic.rule).or_insert_with(
            || json!({ "id": diagnostic.rule, "name": diagnostic.rule.replace('/', "_") }),
        );
    }
    let results = diagnostics
        .iter()
        .map(|diagnostic| {
            let source = fs::read_to_string(&diagnostic.path).unwrap_or_default();
            let (line, column) = line_column(&source, diagnostic.span.start);
            json!({
                "ruleId": diagnostic.rule,
                "level": match diagnostic.severity {
                    DiagnosticSeverity::Hint => "note",
                    DiagnosticSeverity::Warning => "warning",
                    DiagnosticSeverity::Error => "error",
                },
                "message": { "text": diagnostic.message },
                "locations": [{ "physicalLocation": {
                    "artifactLocation": { "uri": diagnostic.path.to_string_lossy() },
                    "region": { "startLine": line, "startColumn": column }
                }}]
            })
        })
        .collect::<Vec<_>>();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": { "name": "lilscript-lint", "rules": rules.into_values().collect::<Vec<_>>() }},
            "results": results
        }]
    })
}

fn apply_fixes(diagnostics: &[LintDiagnostic]) -> Result<(), String> {
    let mut by_path = BTreeMap::<PathBuf, Vec<_>>::new();
    for diagnostic in diagnostics {
        if let Some(fix) = &diagnostic.fix {
            by_path
                .entry(diagnostic.path.clone())
                .or_default()
                .extend(fix.edits.clone());
        }
    }
    for (path, mut edits) in by_path {
        let mut source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.span.start));
        for edit in edits {
            source.replace_range(edit.span.start..edit.span.end, &edit.replacement);
        }
        fs::write(&path, source)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, tail)| tail.len())
        + 1;
    (line, column)
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
    for entry in
        fs::read_dir(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?
    {
        let child = entry
            .map_err(|error| format!("failed to read directory entry: {error}"))?
            .path();
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
