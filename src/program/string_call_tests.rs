//! Source-preserved Marked helper slices and replaceable string-method calls.
//! Host traces and UTF16 vectors are fixed independently of emitted spelling.
//! Concatenating extracted declarations here does not exercise module linking.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Plan, Style};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::process::Command;

const HELPERS: &str = include_str!("fixtures/string-calls/marked-helpers.lil");
const JOIN_BODY: &str = include_str!("fixtures/string-calls/marked-join-lines-body.lil");
const JOIN_ARCHIVE: &str = include_str!("fixtures/string-calls/marked-join-lines.lil");
const METHODS: &str = include_str!("fixtures/string-calls/methods.lil");

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
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for string-call execution tests");
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
        for style in [Style::Global, Style::Scoped, Style::Source] {
            compilation
                .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
                .unwrap();
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
                "string-call-artifact {}",
                json!({
                    "case":case,"source_sha256":digest(source),"compact":compact,
                    "style":format!("{style:?}"),"javascript":javascript,
                    "javascript_sha256":digest(&javascript),"raw":sizes.raw,
                    "gzip9":sizes.gzip9,"brotli11":sizes.brotli11,
                    "observation":"fixed host/UTF16 expectations passed before complete artifact scoring",
                })
            );
        }
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
}

#[test]
fn marked_archives_preserve_complete_helpers_and_existing_join_fixture_bytes() {
    let origin: Json =
        serde_json::from_str(include_str!("fixtures/string-calls/marked.origin.json")).unwrap();
    assert_eq!(digest(HELPERS), origin["helpersSha256"]);
    let mut start = 0usize;
    for function in origin["functions"].as_array().unwrap() {
        let length =
            (function["end"].as_u64().unwrap() - function["start"].as_u64().unwrap()) as usize;
        assert_eq!(
            digest(&HELPERS.as_bytes()[start..start + length]),
            function["sha256"]
        );
        start += length + 1;
    }
    assert_eq!(start - 1, HELPERS.len());
    assert_eq!(
        JOIN_ARCHIVE,
        include_str!("../../finer/tools/fixtures/marked-join-lines.lil")
    );
    assert_eq!(digest(JOIN_ARCHIVE), origin["joinLines"]["archiveSha256"]);
    assert_eq!(digest(JOIN_BODY), origin["joinLines"]["bodySha256"]);
    let span = &origin["joinLines"]["bodySpan"];
    assert_eq!(
        JOIN_BODY,
        &JOIN_ARCHIVE
            [span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize]
    );
}

#[test]
fn unchanged_marked_helpers_cover_slice_split_first_line_and_utf16_round_trips() {
    let source = format!(
        "{HELPERS}\n{JOIN_BODY}\n{}",
        include_str!("fixtures/string-calls/marked-harness.lil")
    );
    matrix(
        "marked-string-helpers",
        &source,
        "",
        include_str!("fixtures/string-calls/marked.host.js"),
        &serde_json::from_str(include_str!("fixtures/string-calls/marked.expected.json")).unwrap(),
    );
}

#[test]
fn archived_join_lines_fixture_remains_unchanged_in_the_new_core() {
    matrix(
        "archived-marked-join-lines",
        JOIN_ARCHIVE,
        "globalThis.lines=()=>[\"first\",\"\",\"last\"];console.log=value=>events.push(String(value));",
        "",
        &json!(["", "only", "first\n\nlast"]),
    );
}

#[test]
fn replaceable_slice_split_preserve_lookup_arity_arguments_throws_and_reentry() {
    matrix(
        "string-method-host-schedule",
        METHODS,
        "",
        include_str!("fixtures/string-calls/methods.host.js"),
        &serde_json::from_str(include_str!("fixtures/string-calls/methods.expected.json")).unwrap(),
    );
}

#[test]
fn split_result_identity_comes_from_the_selected_method_and_unused_calls_execute() {
    matrix(
        "split-result-identity",
        METHODS,
        "",
        include_str!("fixtures/string-calls/identity.host.js"),
        &serde_json::from_str(include_str!("fixtures/string-calls/identity.expected.json"))
            .unwrap(),
    );
}

#[test]
fn checked_string_array_types_do_not_erase_raw_host_result_effects() {
    matrix(
        "string-method-raw-results",
        METHODS,
        "",
        include_str!("fixtures/string-calls/raw-results.host.js"),
        &serde_json::from_str(include_str!(
            "fixtures/string-calls/raw-results.expected.json"
        ))
        .unwrap(),
    );
}

#[test]
fn javascript_slice_split_repeat_and_trim_execute_natively() {
    let policy = crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap();
    for (source, expected) in [
        ("string value=\"abcdef\".slice(1,3);print(value);", "bc\n"),
        ("string[] values=\"a/b\".split(\"/\");print(values.length);", "2\n"),
        ("string[] units=\"ab\".split(\"\");print(units[1]);", "b\n"),
        ("string value=\"x\".repeat(2);print(value.length);", "2\n"),
        ("string value=\" x \".trim();print(value.length);", "1\n"),
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let mut compilation = compilation();
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compilation.ledger().retained_bytes();
        let c = compilation
            .with_native_c(source, &policy, WorkDomain::Baseline, |output| output.take_c())
            .unwrap();
        super::native_tests::compile_and_execute(&c, expected, "native-string-methods");
        assert_eq!(compilation.ledger().retained_bytes(), retained);
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
}
