//! `match`, optional chaining, destructuring and object literals holding
//! value structs on the semantic route, observed by running the output.
use super::publication::*;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain};
use crate::structured_js::selection::{Plan, Style};
use std::process::Command;

fn compile(source: &str) -> String {
    compile_with(source, "[javascript]\nstrip_console=false\n")
}

fn compile_with(source: &str, config: &str) -> String {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let checked =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    let program = from_checked_source(&syntax, &checked)
        .unwrap_or_else(|error| panic!("convert: {error:?}\n{source}"));
    program.verify().unwrap();
    let config: crate::config::ProjectConfig = toml::from_str(config).unwrap();
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

#[test]
fn undefined_calls_and_unreachable_statements_leave_no_residue() {
    let javascript = compile(
        r#"
        JsValue nothing() { return JS.undefined(); }
        int count = 0;
        JsValue pick(JsValue a, JsValue b) {
            if (JS.isNullish(a)) { return b; }
            return a;
            return nothing();
        }
        JsValue bump() {
            JsValue seen = nothing();
            count = count + 1;
            if (count > 1) { seen = "many"; }
            print(seen);
            return nothing();
        }
        export JsValue run(JsValue a) { bump(); bump(); return pick(a, "fallback"); }
        print(run(null));
        print(run(5));
        "#,
    );
    // `let seen=void 0` is `let seen`, `return void 0` at a body's end is
    // nothing, and `return nothing()` after `return a` is never lowered.
    assert!(!javascript.contains("void 0"), "{javascript}");
    assert!(
        !javascript.contains("return;") && !javascript.contains("return}"),
        "{javascript}"
    );
    assert_eq!(
        run(&javascript, ""),
        "undefined\nmany\nfallback\nmany\nmany\n5\n"
    );
}

#[test]
fn a_closure_only_invoked_through_its_cell_loses_its_name() {
    let javascript = compile(
        r#"
        JsValue nothing() { return JS.undefined(); }
        JsValue twice = nothing();
        twice = (JsValue value) => { return JS.add(value, value); };
        JsValue shown = nothing();
        shown = (JsValue value) => { return value; };
        JsValue table = object { shown: shown };
        export JsValue run(JsValue value) {
            print(JS.call(twice, nothing(), value));
            return table;
        }
        JsValue result = run(21);
        print(result["shown"]["name"]);
        "#,
    );
    // Every read of `twice` calls it; `shown` escapes into an exported object.
    assert!(!javascript.contains("twice"), "{javascript}");
    assert!(javascript.contains("let shown="), "{javascript}");
    assert_eq!(run(&javascript, ""), "42\nshown\n");
}

const SHOW: &str = "globalThis.show=v=>console.log(JSON.stringify(v));";
const PRISTINE: &str = "[javascript]\nstrip_console=false\nassume_pristine_builtins=true\n";

#[test]
fn forwarding_wrappers_become_their_builtins_and_stores_fold_into_the_literal() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        JsValue emptyObject() { return JS.object(); }
        string toStr(JsValue value) { return JS.string(value); }
        number toNum(JsValue value) { return JS.number(value); }
        bool isNullish(JsValue value) { return JS.isNullish(value); }
        export JsValue build(JsValue a) {
            JsValue o = emptyObject();
            o["kind"] = toStr("tag");
            o["size"] = toNum(a) * 2;
            o["text"] = toStr(a);
            if (isNullish(a)) { o["none"] = true; }
            return o;
        }
        show(build(null));
        show(build(4));
        "#,
        PRISTINE,
    );
    // `toStr("tag")` is the literal; the stores define the object's entries.
    assert!(javascript.contains("{kind:\"tag\",size:"), "{javascript}");
    assert!(javascript.contains("==null"), "{javascript}");
    assert_eq!(
        run(&javascript, SHOW),
        "{\"kind\":\"tag\",\"size\":0,\"text\":\"null\",\"none\":true}\n{\"kind\":\"tag\",\"size\":8,\"text\":\"4\"}\n"
    );
}

#[test]
fn a_store_whose_value_may_read_the_object_stays_a_store() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        JsValue emptyObject() { return JS.object(); }
        JsValue o = emptyObject();
        JsValue seen() { return o["y"]; }
        o["x"] = seen();
        o["y"] = 2;
        show(o);
        "#,
        PRISTINE,
    );
    // `seen` reads `o` while `o.x` is evaluated: a literal would still be
    // in its temporal dead zone.
    assert_eq!(run(&javascript, SHOW), "{\"y\":2}\n");
}

#[test]
fn single_expression_helpers_inline_only_where_the_arguments_keep_their_order() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        int count = 0;
        JsValue next() { count = count + 1; return count; }
        int len(JsValue value) { return JS.number(value["length"]).toInt(); }
        JsValue twice(JsValue x) { return JS.add(x, x); }
        JsValue pair(JsValue a, JsValue b) { return JS.array(a, b); }
        JsValue lengthFirst(JsValue a, JsValue b) { return JS.add(a["length"], b); }
        show(len("abcd"));
        show(twice(next()));
        show(pair(next(), next()));
        show(lengthFirst("xy", next()));
        show(count);
        "#,
        PRISTINE,
    );
    // `len` and `pair` take their arguments in place. `twice` reads its
    // parameter twice, and `lengthFirst` reads `a.length` (a getter) before
    // `b`, which the call evaluated first: both stay calls.
    assert!(javascript.contains("\"abcd\".length"), "{javascript}");
    assert!(javascript.contains("show(["), "{javascript}");
    assert!(javascript.matches("=>").count() >= 3, "{javascript}");
    assert_eq!(run(&javascript, SHOW), "4\n2\n[2,3]\n6\n4\n");
}

#[test]
fn inlined_bodies_repeat_only_arguments_whose_value_cannot_change() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        JsValue path(JsValue a, JsValue b) {
            return JS.add(JS.add(JS.add(JS.add("M", JS.add(a, b)), " l"), a), JS.add(" h", a));
        }
        JsValue build(JsValue kind, JsValue size) {
            size = JS.add(size, 1);
            JsValue d = "";
            if (JS.strictEqual(kind, "main")) { d = path(size, 80); }
            return d;
        }
        int calls = 0;
        JsValue next() { calls = calls + 1; return calls; }
        JsValue twice(JsValue x) { return JS.add(x, x); }
        show(build("main", 2));
        show(twice(next()));
        show(twice(7));
        show(twice(next()));
        "#,
        PRISTINE,
    );
    // `path` reads `a` three times: its argument is a parameter no other
    // function reaches. `twice(7)` copies the literal and folds; a call
    // argument is never repeated, so `twice(next())` stays a call.
    assert!(javascript.contains("+80)+\" l\"+"), "{javascript}");
    assert!(javascript.contains("show(14)"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "\"M83 l3 h3\"\n2\n14\n4\n");
}

#[test]
fn a_reset_method_and_its_stores_fold_into_the_constructed_literal() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        class Token {
            int kind;
            string raw;
            string text;
            bool task;
            init(int kind, string raw) { this.reset(); this.kind = kind; this.raw = raw; }
            void reset() { this.kind = 0; this.raw = ""; this.text = ""; this.task = false; }
        }
        Token make(int kind, string raw) { return new Token(kind, raw); }
        Token token = make(3, "x");
        show(JS.box(token.kind));
        show(JS.box(token.raw));
        show(JS.box(token.task));
        "#,
        PRISTINE,
    );
    // `reset` inlines at its one call, its stores replace the literal's own
    // entries in place, and the literal is returned directly.
    assert!(javascript.contains("=>({kind:"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "3\n\"x\"\nfalse\n");
}

#[test]
fn inlined_statements_never_repeat_an_argument_with_effects() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        int count = 0;
        JsValue next() { count = count + 1; return JS.box(count); }
        void fill(JsValue target, JsValue value) { target["a"] = value; target["b"] = value; }
        JsValue holder = JS.object();
        fill(holder, next());
        show(holder);
        JsValue first = JS.object("x", 1);
        JsValue alias = first;
        first = JS.object("x", 2);
        show(alias);
        "#,
        PRISTINE,
    );
    // `next()` runs once, so `fill` stays a call; `alias` keeps the value
    // `first` had, since `first` is assigned again.
    assert_eq!(run(&javascript, SHOW), "{\"a\":1,\"b\":1}\n{\"x\":1}\n");
}

#[test]
fn a_constant_namespace_object_reads_as_its_members() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        JsValue helper = JS.undefined();
        helper = (JsValue x) => { return JS.add(x, 1); };
        JsValue other = JS.undefined();
        other = (JsValue x) => { return JS.add(x, 2); };
        JsValue ns = JS.object("helper", helper, "other", other, "scale", 10);
        JsValue useIt = JS.undefined();
        useIt = (JsValue v) => { return JS.invoke(ns, "helper", v); };
        show(JS.call(useIt, JS.undefined(), 1));
        show(JS.invoke(ns, "other", 5));
        show(ns["scale"]);
        "#,
        PRISTINE,
    );
    // Every use of `ns` reads a literal key of a literal nothing assigns: the
    // reads become the members, the object goes, and the arrows called only
    // directly lose their observable names.
    assert!(!javascript.contains("helper:"), "{javascript}");
    assert!(javascript.contains("show(10)"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "2\n7\n10\n");
}

#[test]
fn an_inert_single_use_literal_moves_into_the_store_that_reads_it() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        JsValue schema = JS.object();
        JsValue output = JS.object("type", "string", "cli", "-F");
        schema["output"] = output;
        JsValue strict = JS.object("type", "boolean");
        schema["strict"] = strict;
        show(schema);
        "#,
        PRISTINE,
    );
    // Creating an inert literal later is unobservable: each takes its one
    // reader's place, and the stores then fold into `schema`'s literal.
    assert!(javascript.contains("{output:{type:"), "{javascript}");
    assert_eq!(
        run(&javascript, SHOW),
        "{\"output\":{\"type\":\"string\",\"cli\":\"-F\"},\"strict\":{\"type\":\"boolean\"}}\n"
    );
}
