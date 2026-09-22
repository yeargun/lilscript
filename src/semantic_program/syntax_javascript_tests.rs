//! `match`, optional chaining, destructuring and object literals holding
//! value structs on the semantic route, observed by running the output.
use super::publication::*;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain};
use crate::structured_js::selection::{Plan, Style};
use std::process::Command;

fn compile(source: &str) -> String {
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
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let result = compilation
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result)
        .unwrap();
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

fn run(javascript: &str, host: &str) -> String {
    let script = format!(
        "{host}\nawait import('data:text/javascript,'+encodeURIComponent({}));",
        serde_json::to_string(javascript).unwrap()
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for syntax observation tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn match_evaluates_its_scrutinee_once_and_only_the_selected_arm() {
    let javascript = compile(
        r#"
        enum Status { Draft, Active, Sold }
        int calls = 0;
        Status next() { calls++; return Status.Sold; }
        string label(Status status) {
            return match(status) { Status.Draft => "draft", Status.Active => "active", Status.Sold => "sold" };
        }
        string loud(string word) { print("arm " + word); return word; }
        print(label(next()));
        print(calls);
        print(match(-1) { 0 => loud("zero"), -1 => loud("minus"), _ => loud("other") });
        print(match("b") { "a" => 1, _ => 2 });
        print(match(false) { true => "yes", false => "no" });
        "#,
    );
    assert_eq!(run(&javascript, ""), "sold\n1\narm minus\nminus\n2\nno\n");
}

#[test]
fn optional_access_skips_the_index_when_the_receiver_is_absent() {
    let javascript = compile(
        r#"
        class Box { int value; init(int value) { this.value = value; } }
        Box? maybe(bool present) { if (present) { return new Box(4); } return null; }
        int[]? values(bool present) { if (present) { return [7, 8]; } return null; }
        int index() { print("index"); return 1; }
        print(maybe(true)?.value == 4);
        print(maybe(false)?.value == null);
        print(maybe(false)?.value ?? 9);
        print(values(true)?.[index()] == 8);
        print(values(false)?.[index()] == null);
        "#,
    );
    assert_eq!(run(&javascript, ""), "true\ntrue\n9\nindex\ntrue\ntrue\n");
}

#[test]
fn destructuring_reads_absent_positions_and_keys_as_null() {
    let javascript = compile(
        r#"
        int[] values = [1, 2, 3, 4];
        auto [first, , third, ...tail] = values;
        print(first == 1);
        print(third == 3);
        print(tail.length);
        auto [only, absent] = [9];
        print(only == 9);
        print(absent);
        Record<int> listing = record { name: 5, "unit-price": 7, extra: 9, "0": 1 };
        auto {name, "unit-price": price, missing, ...remaining} = listing;
        print(name == 5);
        print(price == 7);
        print(missing);
        print(JSON.stringify(Object.keys(remaining)));
        "#,
    );
    assert_eq!(
        run(&javascript, ""),
        "true\ntrue\n1\ntrue\nnull\ntrue\ntrue\nnull\n[\"0\",\"extra\"]\n"
    );
}

#[test]
fn a_class_extending_a_host_class_is_a_real_subclass() {
    let javascript = compile(
        r#"
        extern class Error { string message; string name; init(string message); }
        extern void inspect(JsValue value);
        class Problem extends Error {
            string code;
            init(string message, string code) { super(message); this.code = code; }
            string describe() { return this.message + "|" + this.code; }
        }
        Problem problem = new Problem("disk full", "E42");
        print(problem.describe());
        print(problem.name);
        inspect(problem);
        "#,
    );
    assert!(javascript.contains("class Problem extends Error"), "{javascript}");
    assert_eq!(
        run(
            &javascript,
            "globalThis.inspect=v=>console.log(v instanceof Error,v.constructor.name,v.stack.startsWith('Error: disk full'),Object.keys(v).join());"
        ),
        "disk full|E42\nError\ntrue Problem true code\n"
    );
}

#[test]
fn generic_classes_inherit_bodies_through_substituted_bases() {
    let javascript = compile(
        r#"
        class Pair<A, B> {
            A first; B second;
            init(A first, B second) { this.first = first; this.second = second; }
            B right() { return this.second; }
            A left() { return this.first; }
        }
        class Named<T> extends Pair<string, T> {
            int uses;
            init(string name, T value) { super(name, value); this.uses = 0; }
            T use() { this.uses += 1; return this.right(); }
        }
        class Counted extends Named<int> {
            init(string name, int count) { super(name, count); }
            int twice() { return this.use() * 2; }
        }
        string describe(Pair<string, int> pair) { return pair.left() + "=" + pair.right(); }
        Counted counted = new Counted("apples", 21);
        print(counted.twice());
        print(counted.uses);
        print(describe(counted));
        Named<bool> flag = new Named("ready", true);
        print(flag.use());
        "#,
    );
    assert_eq!(run(&javascript, ""), "42\n1\napples=21\ntrue\n");
}

#[test]
fn object_literals_give_each_struct_entry_its_public_shape() {
    let javascript = compile(
        r#"
        extern void show(JsValue value);
        struct Point { int x; int y; }
        JsValue holder = object { origin: Point{1, 2}, label: "p" };
        show(holder);
        Point shared = Point{3, 4};
        show(object { a: shared, b: shared });
        "#,
    );
    assert_eq!(
        run(&javascript, "globalThis.show=v=>console.log(JSON.stringify(v),v.a===v.b);"),
        // Each entry of the second object is a fresh object: `a !== b`.
        "{\"origin\":{\"x\":1,\"y\":2},\"label\":\"p\"} true\n{\"a\":{\"x\":3,\"y\":4},\"b\":{\"x\":3,\"y\":4}} false\n"
    );
}
