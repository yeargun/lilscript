//! Async bodies and generators on the semantic route: `await` and `yield`
//! are target operations, `for...of` over a generator runs its iterator
//! protocol, and callbacks returning `JsValue` on some paths return it.
use super::publication::*;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain};
use crate::structured_js::selection::{Plan, Style};
use std::process::Command;

fn compile(source: &str, style: Style) -> String {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let checked =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    let program = from_checked_source(&syntax, &checked)
        .unwrap_or_else(|error| panic!("convert: {error:?}\n{source}"));
    program.verify().unwrap();
    let config: crate::config::ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
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
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source_id = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compilation
        .direct_javascript(source_id, &policy, WorkDomain::Baseline)
        .unwrap();
    let result = compilation
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(style))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result);
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result.unwrap()
}

fn run(javascript: &str, host: &str, after: &str) -> String {
    let script = format!(
        "{host}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{after}",
        serde_json::to_string(javascript).unwrap()
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for suspension tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn generators_yield_delegate_and_drive_for_of() {
    let source = r#"
        generator int upTo(int limit) {
            for (int i = 0; i < limit; i++) {
                yield i;
            }
        }
        generator int both() {
            yield* [7, 8];
            yield* upTo(2);
        }
        extern void note(int value);
        int total = 0;
        for (int value of upTo(4)) { total += value; }
        print(total);
        string seen = "";
        for (int value of both()) { seen = seen + value + ","; }
        print(seen);
        // Leaving early closes the generator: its `finally` would run.
        for (int value of upTo(10)) { if (value == 3) { break; } note(value); }
    "#;
    for style in [Style::Global, Style::Scoped] {
        let javascript = compile(source, style);
        assert!(javascript.contains("function*"), "{javascript}");
        assert_eq!(
            run(&javascript, "globalThis.note=v=>console.log('note',v);", ""),
            "6\n7,8,0,1,\nnote 0\nnote 1\nnote 2\n"
        );
    }
}

#[test]
fn async_bodies_await_tasks_and_resolve_with_their_result() {
    let source = r#"
        async int loadCount(int base) {
            int got = await Task.resolve(base + 1);
            return got * 2;
        }
        async string fails() {
            string never = await Task.reject(JS.object("reason", "no"));
            return never;
        }
        export Task<int> start(int base) { return loadCount(base); }
        export Task<string> broken() { return fails(); }
    "#;
    let javascript = compile(source, Style::Scoped);
    assert!(javascript.contains("async function") && javascript.contains("await "), "{javascript}");
    assert_eq!(
        run(
            &javascript,
            "",
            "console.log(await library.start(20));try{await library.broken()}catch(e){console.log(e.reason)}"
        ),
        "42\nno\n"
    );
}

#[test]
fn a_callback_returning_jsvalue_on_some_paths_returns_it() {
    // Falling off the end yields `undefined`, itself a `JsValue`, so the
    // checker types such a callback as returning `JsValue` and host callers
    // read what it returns.
    let source = r#"
        extern JsValue apply(JsValue callback, JsValue value);
        JsValue pick = (JsValue value) => {
            if (value.truthy()) { return JS.array(value); }
            if (JS.isNullish(value)) { print("nullish"); }
        };
        print(apply(pick, 3));
        print(apply(pick, null));
    "#;
    assert_eq!(
        run(
            &compile(source, Style::Scoped),
            "globalThis.apply=(f,v)=>JSON.stringify(f(v)??'undefined');",
            ""
        ),
        "[3]\nnullish\n\"undefined\"\n"
    );
}

#[test]
fn a_record_typed_by_a_jsvalue_context_is_a_null_prototype_record() {
    let source = r#"
        extern void inspect(JsValue value);
        inspect(record{line: 1, column: 2});
    "#;
    assert_eq!(
        run(
            &compile(source, Style::Scoped),
            "globalThis.inspect=v=>console.log(JSON.stringify(v),Object.getPrototypeOf(v));",
            ""
        ),
        "{\"line\":1,\"column\":2} null\n"
    );
}
