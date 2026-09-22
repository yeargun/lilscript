//! Generic target scope is selected by original concrete calls, never by an
//! unrelated schema declaration or an unsealed-body-as-primitive assumption.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Plan, Style};
use serde_json::{json, Value as Json};
use std::process::Command;

fn output(source: &str, compact: bool, module: bool) -> Result<String, CandidateError> {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\n[policy.tactics]\ntarget-compaction='{}'\n",
        if compact { "on" } else { "off" },
    ))
    .unwrap();
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: module,
        })
        .unwrap();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 20_000_000,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: 32_000_000,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let result = compiler
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result);
    if result.is_err() {
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        // A semantic refusal cannot corrupt a retry or silently become success.
        let retry = compiler
            .with_javascript_output(candidate, &policy, |_| ())
            .unwrap_err();
        assert!(matches!(retry, CandidateError::Unsupported(_)), "{retry:?}");
        assert_eq!(compiler.ledger().retained_bytes(), retained);
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
    result
}
fn execute(javascript: &str, host: &str, observations: &str, module: bool) -> Json {
    let body = if module {
        format!(
            "const library=await import('data:text/javascript,'+encodeURIComponent({}));",
            serde_json::to_string(javascript).unwrap()
        )
    } else {
        format!(
            "import vm from 'node:vm';vm.runInThisContext({});",
            serde_json::to_string(javascript).unwrap()
        )
    };
    let script = format!("const events=[];{host}\n{body}\n{observations}\nprocess.stdout.write(JSON.stringify(events));");
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node required for generic target-scope execution");
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    serde_json::from_slice(&result.stdout).unwrap()
}
fn reject(source: &str, feature: &str) {
    for compact in [false, true] {
        match output(source, compact, true) {
            Err(CandidateError::Unsupported(error)) => {
                assert!(error.feature.contains(feature), "{error:?}")
            }
            result => panic!("expected {feature}, got {result:?}\n{source}"),
        }
    }
}

#[test]
fn unrelated_schemas_do_not_restrict_public_or_observed_primitive_generic_calls() {
    for schema in ["", "struct Unrelated { int value; }"] {
        for compact in [false, true] {
            let source = format!("{schema}export T forward<T>(T value){{return value;}}export int run(int value){{return forward(value);}}");
            let javascript = output(&source, compact, true).unwrap();
            let observed = execute(
                &javascript,
                "",
                r#"
                let conversions=0;const opaque={ [Symbol.toPrimitive](){conversions++;return 7} };
                const symbol=Symbol('same');
                events.push(library.run(8),library.run(opaque)===opaque,
                    library.run(symbol)===symbol,library.run(1n)===1n,
                    library.forward(opaque)===opaque,conversions);
            "#,
                true,
            );
            assert_eq!(observed, json!([8, true, true, true, true, 0]));
            // An observed callable also prevents a complete private seal.
            // Its concrete JsValue call adds no private nominal transport.
            let source = format!("{schema}extern JsValue input;extern void observe(JsValue value);T forward<T>(T value){{return value;}}observe(forward);observe(forward(input));");
            let javascript = output(&source, compact, false).unwrap();
            let observed=execute(&javascript,
                "globalThis.input=Symbol('input');globalThis.observe=value=>events.push(typeof value==='function'?'callable':value===input);",
                "",false);
            assert_eq!(observed, json!(["callable", true]));
        }
    }
}

#[test]
fn nominal_generic_calls_still_require_private_producers_and_closed_raw_forwarding() {
    // Public visibility of the same body prevents its full input seal, even
    // though a separate call instantiates T as a primitive.
    reject("struct P{int x;}export T forward<T>(T value){return value;}export int scalar(){return forward(3);}export int run(){P p=P{7};P saved=forward(p);return saved.x;}",
        "complete private interface");
    // Mere generic annotations cannot authorize an opaque or captured value
    // transport. All source calls still execute through the original ABI.
    reject("struct P{int x;}extern void observe(JsValue value);T forward<T>(T value){observe(value);return value;}export int run(){P p=P{7};P saved=forward(p);return saved.x;}",
        "closed value forwarding");
    reject("struct P{int x;}bool choose=true;T forward<T>(T value){if(choose){return value;}return value;}export int run(){P p=P{7};P saved=forward(p);return saved.x;}",
        "closed value forwarding");
}
