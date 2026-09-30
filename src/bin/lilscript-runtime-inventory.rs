//! Independent syntax inventory of the delivered artifact, outside compilation.
//! Counts are static sites, never claimed dynamic allocations or materialized
//! arguments objects. Engine profiles and workload counters measure execution.
use clap::Parser;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    file: PathBuf,
}

fn visit(node: &Value, function_depth: usize, counts: &mut BTreeMap<&'static str, u64>) {
    match node {
        Value::Array(items) => {
            for item in items {
                visit(item, function_depth, counts);
            }
        }
        Value::Object(object) => {
            let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
            let function = matches!(
                kind,
                "FunctionExpression" | "FunctionDeclaration" | "ArrowFunctionExpression"
            );
            let mut count = |key| {
                *counts.entry(key).or_default() += 1;
            };
            match kind {
                "NewExpression" => count("constructor_sites"),
                "ArrayExpression" => count("array_literal_sites"),
                "ObjectExpression" => count("object_literal_sites"),
                "CallExpression" => count("call_sites"),
                "RestElement" => count("rest_binding_sites"),
                "Identifier" if object.get("name").and_then(Value::as_str) == Some("arguments") => {
                    count("arguments_identifier_sites")
                }
                _ => {}
            }
            if function {
                count("function_sites");
            }
            if function_depth == 0 && (kind.ends_with("Statement") || kind == "VariableDeclaration")
            {
                count("outer_statement_nodes");
            }
            // A syntactic forwarding body is a useful audit counter, not proof
            // that a particular emitted function is an ABI adapter/trampoline.
            if function {
                if let Some(body) = object
                    .get("body")
                    .and_then(|body| body.get("body"))
                    .and_then(Value::as_array)
                {
                    if body.len() == 1
                        && body[0]["type"] == "ReturnStatement"
                        && body[0]["argument"]["type"] == "CallExpression"
                    {
                        count("single_return_call_functions");
                    }
                }
            }
            for (key, value) in object {
                if !matches!(key.as_str(), "start" | "end" | "loc" | "raw" | "comments") {
                    visit(value, function_depth + usize::from(function), counts);
                }
            }
        }
        _ => {}
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let source = fs::read_to_string(&args.file)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::mjs()).parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        return Err(format!(
            "delivered artifact does not parse: {:?}",
            parsed.diagnostics
        )
        .into());
    }
    let ast: Value = serde_json::from_str(&parsed.program.to_estree_json(false, false))?;
    let mut counts = BTreeMap::new();
    for key in [
        "constructor_sites",
        "array_literal_sites",
        "object_literal_sites",
        "call_sites",
        "rest_binding_sites",
        "arguments_identifier_sites",
        "function_sites",
        "outer_statement_nodes",
        "single_return_call_functions",
    ] {
        counts.insert(key, 0);
    }
    visit(&ast, 0, &mut counts);
    println!(
        "{}",
        json!({"schema":1,"artifact":{"path":args.file,"sha256":format!("{:x}",Sha256::digest(source.as_bytes()))},
        "static_sites":counts,"limitations":"Syntax sites, not dynamic counts. Function bodies include IIFEs; outer statement nodes do not assert startup execution frequency. Adapter identity, allocation totals and engine argument materialization require compiler provenance or engine profiles."})
    );
    Ok(())
}
