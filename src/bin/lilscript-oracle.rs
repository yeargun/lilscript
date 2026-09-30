//! Produce or verify conformance stdout with the independent checked-AST
//! evaluator. Compiler output is never an input to oracle generation.
use bumpalo::Bump;
use clap::Parser;
use lilscript::{analyze, interpret_program_with_limits, parse_source, InterpreterLimits};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(about = "Check independent conformance oracles and retain measured feature coverage")]
struct Args {
    /// Source files or directories of .lil cases. Directories are not recursive.
    #[arg(required = true)]
    sources: Vec<PathBuf>,
    #[arg(long)]
    json: PathBuf,
    /// Create missing .out files after all existing covered outputs agree.
    /// Existing outputs are never overwritten, even with this option.
    #[arg(long)]
    write_missing: bool,
    /// A previous receipt: losing a covered case is a gate failure.
    #[arg(long)]
    baseline: Option<PathBuf>,
    #[arg(long, default_value_t = 10_000_000)]
    steps: u64,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.json.exists() {
        return Err("receipt exists; choose a new path".into());
    }
    let mut sources = Vec::new();
    for path in args.sources {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let path = entry?.path();
                if path.extension().is_some_and(|extension| extension == "lil") {
                    sources.push(path.canonicalize()?);
                }
            }
        } else {
            sources.push(path.canonicalize()?);
        }
    }
    sources.sort();
    sources.dedup();
    if sources.is_empty() {
        return Err("no source files".into());
    }
    let mut rows = Vec::new();
    let mut pending = Vec::new();
    let mut failed = false;
    let mut names = std::collections::HashSet::new();
    for path in sources {
        if !names.insert(path.file_stem().unwrap().to_owned()) {
            return Err(format!("ambiguous case name: {}", path.display()).into());
        }
        let source = fs::read_to_string(&path)?;
        let expected_path = path.with_extension("out");
        let hosts = [
            path.with_extension("host.js"),
            path.with_extension("module-probe.mjs"),
        ]
        .into_iter()
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
        let arena = Bump::new();
        let evaluated: Result<String, String> = (|| {
            if !hosts.is_empty() {
                return Err("host-model: this case requires its declared JavaScript prelude or module observer; checked-AST evaluation alone is not its behavior oracle".into());
            }
            let program =
                parse_source(&arena, &source).map_err(|error| format!("parse: {error}"))?;
            let semantics = analyze(&program).map_err(|error| format!("check: {error}"))?;
            interpret_program_with_limits(
                &program,
                &semantics,
                InterpreterLimits {
                    steps: args.steps,
                    recursion_depth: 256,
                },
            )
            .map_err(|error| format!("evaluate: {error}"))
        })();
        let mut row = json!({"case": path.file_stem().unwrap().to_string_lossy(), "source":path,
            "source_sha256": hash(source.as_bytes()), "status":"uncovered"});
        row["host_inputs"] = json!(hosts
            .iter()
            .map(|path| fs::read(path).map(|bytes| json!({"path":path,"sha256":hash(&bytes)})))
            .collect::<Result<Vec<_>, _>>()?);
        match evaluated {
            Err(error) => {
                row["reason"] = json!(error);
            }
            Ok(output) => {
                row["oracle_sha256"] = json!(hash(output.as_bytes()));
                if expected_path.exists() {
                    let expected = fs::read(&expected_path)?;
                    row["expected_sha256"] = json!(hash(&expected));
                    if expected == output.as_bytes() {
                        row["status"] = json!("covered");
                    } else {
                        row["status"] = json!("disagreement");
                        row["independent_output"] = json!(output);
                        failed = true;
                    }
                } else {
                    row["status"] = json!(if args.write_missing {
                        "pending"
                    } else {
                        "missing"
                    });
                    pending.push((expected_path, output));
                    if !args.write_missing {
                        failed = true;
                    }
                }
            }
        }
        rows.push(row);
    }
    let mut lost = Vec::new();
    if let Some(baseline) = args.baseline {
        let baseline: Value = serde_json::from_slice(&fs::read(baseline)?)?;
        let cases = baseline["cases"]
            .as_array()
            .ok_or("invalid baseline: cases missing")?;
        for before in cases.iter().filter(|case| case["status"] == "covered") {
            if !rows
                .iter()
                .any(|after| after["case"] == before["case"] && after["status"] == "covered")
            {
                lost.push(before["case"].clone());
                failed = true;
            }
        }
    }
    if args.write_missing && !failed {
        for (path, output) in pending {
            // create_new also refuses a file created concurrently after audit.
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(output.as_bytes())?;
        }
        for row in &mut rows {
            if row["status"] == "pending" {
                row["status"] = json!("covered");
            }
        }
    }
    let covered = rows.iter().filter(|row| row["status"] == "covered").count();
    let executable = std::env::current_exe()?;
    let report = json!({"schema":1, "complete":!failed, "oracle":"checked-AST interpreter",
        "interpreter":{"path":executable,"sha256":hash(&fs::read(&executable)?)},
        "limits":{"steps":args.steps,"recursion_depth":256}, "covered":covered,
        "total":rows.len(), "lost_coverage":lost, "cases":rows,
        "limitations":"Coverage means this execution agrees. Unexecuted branches and unsupported host/effect operations require their declared independent behavior oracle."});
    if let Some(parent) = args.json.parent().filter(|path| *path != Path::new("")) {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args.json,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "{covered}/{} cases independently covered; receipt {}",
        rows.len(),
        args.json.display()
    );
    if failed {
        return Err(
            "oracle disagreement, missing output or coverage loss; no existing oracle was changed"
                .into(),
        );
    }
    Ok(())
}
