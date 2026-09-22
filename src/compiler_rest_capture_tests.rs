use super::*;
use crate::codegen_ir_js::{
    emit_optimized_ir_js_module_with_options, FunctionSpelling, IrJsOptions,
};
use crate::config::CandidateSearch;

const SOURCE: &str = r#"
extern JsValue invoke(JsValue callback);
extern int slot();
export JsValue make(JsValue enclosing) {
    JsValue method = JS.methodRest((JsValue _self, JsValue incoming) => {
        JsValue saved = incoming;
        return invoke(() => {
            JsValue selected = invoke(() => saved[slot()]);
            saved = null;
            return [selected, () => saved];
        });
    });
    return [method, () => enclosing];
}
"#;

const REFERENCE: &str = r#"
export function make(enclosing) {
    const method = function() {
        let saved = arguments;
        return invoke(() => {
            const selected = invoke(() => saved[slot()]);
            saved = null;
            return [selected, () => saved];
        });
    };
    return [method, () => enclosing];
}
"#;

const CLEARED_SOURCE: &str = r#"
extern JsValue invoke(JsValue callback);
extern int slot();
export JsValue make(JsValue enclosing) {
    func(JsValue, JsValue)->JsValue method = (JsValue _self, JsValue incoming) => {
        JsValue saved = incoming;
        return invoke(() => {
            JsValue selected = invoke(() => saved[slot()]);
            saved = null;
            return selected;
        });
    };
    return [JS.methodRest(method), () => enclosing];
}
"#;

const CLEARED_REFERENCE: &str = r#"
export function make(enclosing) {
    const method = function() {
        let saved = arguments;
        return invoke(() => {
            const selected = invoke(() => saved[slot()]);
            saved = null;
            return selected;
        });
    };
    return [method, () => enclosing];
}
"#;

fn observe(javascript: &str, retained_reader: bool) -> Result<(), String> {
    let source = serde_json::to_string(javascript).unwrap();
    let script = format!(
        r#"
let calls = 0, slots = 0, previous, duringSecond;
globalThis.slot = () => {{ slots++; return 1 }};
globalThis.invoke = callback => {{
    calls++;
    if ({retained_reader} && calls === 3) duringSecond = previous();
    return callback();
}};
const {{ make }} = await import('data:text/javascript;base64,' + Buffer.from({source}).toString('base64'));
const outer = {{ 1: 'outer' }}, first = {{ id: 1 }}, second = {{ id: 2 }};
const [method, enclosing] = make(outer);
const a = method('ignored', first);
if ({retained_reader}) previous = a[1];
const b = method('ignored', second);
const observations = {retained_reader}
    ? [a[0] === first, b[0] === second, a[1]() === null, b[1]() === null, duringSecond === null]
    : [a === first, b === second];
console.log(JSON.stringify([...observations, enclosing() === outer, calls, slots]));
"#
    );
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node.js is required for captured-argument runtime tests");
    let actual = String::from_utf8(output.stdout).unwrap();
    let expected = if retained_reader {
        "[true,true,true,true,true,true,4,2]\n"
    } else {
        "[true,true,true,4,2]\n"
    };
    if output.status.success() && actual == expected {
        Ok(())
    } else {
        Err(format!(
            "status={} stdout={actual:?}\nstderr={}\nJavaScript:\n{javascript}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

#[test]
fn rest_argument_alias_keeps_nested_mutation_and_invocation_identity_at_emission() {
    let variants = [
        (
            "unmangled",
            IrJsOptions {
                mangle_identifiers: false,
                function_spelling: FunctionSpelling::Function,
                ..IrJsOptions::default()
            },
        ),
        ("default", IrJsOptions::default()),
        (
            "nested-name-reuse",
            IrJsOptions {
                mangle_identifiers: true,
                cross_scope_name_reuse: true,
                precise_cross_scope_shadowing: true,
                transitive_nested_shadowing: true,
                local_name_reserve: 0,
                function_spelling: FunctionSpelling::Arrow,
                public_function_arrows: true,
                ..IrJsOptions::default()
            },
        ),
    ];
    let mut failures = Vec::new();
    for (source, reference, retained_reader) in [
        (SOURCE, REFERENCE, true),
        (CLEARED_SOURCE, CLEARED_REFERENCE, false),
    ] {
        observe(reference, retained_reader).unwrap();
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = analyze(&syntax).unwrap();
        let mut ir = lower_to_control_flow(&syntax, &semantics).unwrap();
        crate::optimizer::optimize_control_flow_for_module(&mut ir).unwrap();
        for (name, options) in &variants {
            let output = emit_optimized_ir_js_module_with_options(&ir, options).unwrap();
            if let Err(error) = observe(&output, retained_reader) {
                failures.push(format!(
                    "{name}, retained_reader={retained_reader}: {error}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn rest_argument_alias_keeps_nested_mutation_and_invocation_identity_in_configured_pipeline() {
    let mut failures = Vec::new();
    for (source, retained_reader) in [(SOURCE, true), (CLEARED_SOURCE, false)] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        for search in [CandidateSearch::Off, CandidateSearch::Production] {
            let mut config = javascript_oracle_config();
            config.javascript.candidate_search = search;
            config.mangle.identifiers = Some(true);
            let output = compile_program_to_js_module_configured(&syntax, &config).unwrap();
            if let Err(error) = observe(&output, retained_reader) {
                failures.push(format!(
                    "{search:?}, retained_reader={retained_reader}: {error}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
