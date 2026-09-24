//! Source-preserved Marked regex helpers/rules and replaceable constructor/method calls.
//! Host traces and UTF16 vectors are fixed independently of emitted spelling.
//! Concatenating extracted declarations here does not exercise module linking.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Plan, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};

const HELPERS: &str = include_str!("fixtures/regex-calls/marked-helpers.lil");
const RULES: &str = include_str!("fixtures/regex-calls/marked-rules.lil");
const ALL_RULES: &str = include_str!("fixtures/regex-calls/marked-rules-complete.lil");

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 2 },
    )
    .unwrap()
}

fn execute(javascript: &str, setup: &str, host: &str) -> Json {
    let script = format!(
        "const events=[];{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{host}\nprocess.stdout.write(JSON.stringify(events));",
        serde_json::to_string(javascript).unwrap(),
    );
    let mut child = Command::new("node")
        .arg("--input-type=module")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node is required for regex-call execution tests");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn matrix(case: &str, source: &str, setup: &str, host: &str, expected: &Json) {
    for compact in [false, true] {
        let config: crate::config::ProjectConfig = toml::from_str(&format!(
            "[javascript]\nstrip_console=false\n[policy.tactics]\ntarget-compaction='{}'\n",
            if compact { "on" } else { "off" },
        ))
        .unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let mut compilation = compilation();
        let source_id = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let candidate = compilation
            .direct_javascript(source_id, &policy, WorkDomain::Baseline)
            .unwrap();
        compilation
            .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
            .unwrap();
        for style in [Style::Global, Style::Scoped, Style::Source] {
            let retained = compilation.ledger().retained_bytes();
            let (javascript, sizes) = compilation
                .with_javascript_output(candidate, &policy, |output| {
                    let artifact = output.render(&Plan::new(style))?;
                    // Observe the actual admitted complete artifact before any
                    // exact score is accepted. Node is not the size oracle.
                    output.with_artifact(artifact, |view| {
                        assert_eq!(
                            execute(view.javascript, setup, host),
                            *expected,
                            "{case} compact={compact} style={style:?}"
                        );
                    })?;
                    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
                        output.measure(artifact, codec)?;
                    }
                    let sizes = output.with_artifact(artifact, |view| view.sizes)?;
                    Ok::<_, CandidateError>((output.take_artifact(artifact)?, sizes))
                })
                .unwrap()
                .unwrap();
            assert_eq!(compilation.ledger().retained_bytes(), retained);
            assert_eq!(sizes.raw, javascript.len());
            assert!(sizes.gzip9.is_some() && sizes.brotli11.is_some());
            eprintln!(
                "regex-call-artifact {}",
                json!({
                    "case":case,"source_sha256":digest(source),"compact":compact,
                    "style":format!("{style:?}"),"javascript":javascript,
                    "javascript_sha256":digest(&javascript),"raw":sizes.raw,
                    "gzip9":sizes.gzip9,"brotli11":sizes.brotli11,
                    "observation":"fixed constructor/method/UTF16 expectations passed before complete artifact scoring",
                })
            );
        }
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
}

fn marked_source() -> String {
    format!(
        "{RULES}\n{HELPERS}\n{}",
        include_str!("fixtures/regex-calls/marked-harness.lil")
    )
}

#[test]
fn marked_regex_archives_preserve_complete_original_declarations_and_rules_module() {
    let origin: Json =
        serde_json::from_str(include_str!("fixtures/regex-calls/marked.origin.json")).unwrap();
    for (fixture, text) in [("marked-helpers.lil", HELPERS), ("marked-rules.lil", RULES)] {
        let archive = origin["archives"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["fixture"] == fixture)
            .unwrap();
        assert_eq!(digest(text), archive["fixtureSha256"]);
        let mut start = 0usize;
        for declaration in archive["declarations"].as_array().unwrap() {
            let length = (declaration["end"].as_u64().unwrap()
                - declaration["start"].as_u64().unwrap()) as usize;
            assert_eq!(
                digest(&text.as_bytes()[start..start + length]),
                declaration["sha256"]
            );
            start += length + 1;
        }
        assert_eq!(start - 1, text.len());
    }
    let complete: Json = serde_json::from_str(include_str!(
        "fixtures/regex-calls/marked-rules-complete.origin.json"
    ))
    .unwrap();
    assert_eq!(digest(ALL_RULES), complete["sourceSha256"]);
    assert_eq!(
        ALL_RULES.lines().count() as u64,
        complete["lineCount"].as_u64().unwrap()
    );
    let oracle: Json = serde_json::from_str(include_str!(
        "fixtures/regex-calls/rules-oracle-origin.json"
    ))
    .unwrap();
    let table: Json =
        serde_json::from_str(include_str!("fixtures/regex-calls/rules-oracle-table.json")).unwrap();
    assert_eq!(oracle["sourceSha256"], complete["sourceSha256"]);
    assert_eq!(table.as_array().unwrap().len(), 117);
    assert_eq!(
        digest(serde_json::to_string(&table).unwrap()),
        oracle["tableSha256"]
    );
    assert_eq!(
        digest(include_str!("fixtures/regex-calls/rules-oracle.mjs")),
        oracle["oracleScriptSha256"]
    );
}

#[test]
fn unchanged_marked_regex_helpers_preserve_utf16_last_index_and_replacement() {
    matrix(
        "marked-regex-helpers",
        &marked_source(),
        "",
        include_str!("fixtures/regex-calls/marked.host.js"),
        &serde_json::from_str(include_str!("fixtures/regex-calls/marked.expected.json")).unwrap(),
    );
}

#[test]
fn marked_indent_rule_keeps_host_constructor_alias_and_last_index_setter() {
    matrix(
        "marked-regex-constructor-alias",
        &marked_source(),
        include_str!("fixtures/regex-calls/marked-alias.setup.js"),
        include_str!("fixtures/regex-calls/marked-alias.host.js"),
        &serde_json::from_str(include_str!(
            "fixtures/regex-calls/marked-alias.expected.json"
        ))
        .unwrap(),
    );
}

#[test]
fn constructors_prepare_before_arguments_preserve_new_target_arity_aliases_and_errors() {
    matrix(
        "regex-constructors",
        include_str!("fixtures/regex-calls/constructors.lil"),
        "",
        include_str!("fixtures/regex-calls/constructors.host.js"),
        &serde_json::from_str(include_str!(
            "fixtures/regex-calls/constructors.expected.json"
        ))
        .unwrap(),
    );
}

#[test]
fn regex_methods_preserve_getters_raw_results_reentry_and_symbol_replace() {
    matrix(
        "regex-methods",
        include_str!("fixtures/regex-calls/methods.lil"),
        "",
        include_str!("fixtures/regex-calls/methods.host.js"),
        &serde_json::from_str(include_str!("fixtures/regex-calls/methods.expected.json")).unwrap(),
    );
}

#[test]
fn repeat_and_trim_preserve_utf16_raw_results_coercions_and_unused_errors() {
    matrix(
        "string-repeat-trim",
        include_str!("fixtures/regex-calls/strings.lil"),
        "",
        include_str!("fixtures/regex-calls/strings.host.js"),
        &serde_json::from_str(include_str!("fixtures/regex-calls/strings.expected.json")).unwrap(),
    );
}

#[test]
fn complete_unchanged_marked_rules_module_preserves_patterns_factories_and_identities() {
    matrix(
        "marked-rules-complete",
        ALL_RULES,
        "",
        include_str!("fixtures/regex-calls/marked-rules-complete.host.js"),
        &serde_json::from_str(include_str!(
            "fixtures/regex-calls/marked-rules-complete.expected.json"
        ))
        .unwrap(),
    );
}

#[test]
fn regex_constructor_and_new_methods_remain_explicitly_unsupported_by_native() {
    let policy = crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap();
    for text in [
        "Regex value=new Regex(\"a\");",
        "extern Regex opaque();opaque().test(\"a\");",
        // A string method taking a pattern needs ECMAScript regular expressions.
        "string value=\"a-b\".replace(new Regex(\"-\"),\"+\");print(value.length);",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, text).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.with_native_c(source, &policy, WorkDomain::Baseline, |_| panic!(
                "unsupported regex/string recipe reached native output"
            )),
            Err(NativeError::Unsupported { .. })
        ));
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    }
}
