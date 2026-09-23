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
    compile_plan(source, config, Plan::new(Style::Global))
}

fn compile_plan(source: &str, config: &str, plan: Plan) -> String {
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
            let artifact = output.render(&plan)?;
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
    let javascript = compile_with(
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
        "[javascript]\nstrip_console=false\nkeep_function_names=true\n",
    );
    // Every read of `twice` calls it; `shown` escapes into an exported object,
    // and this contract keeps every name some code could read.
    assert!(!javascript.contains("twice"), "{javascript}");
    assert!(javascript.contains("let shown="), "{javascript}");
    assert_eq!(run(&javascript, ""), "42\nshown\n");
}

#[test]
fn only_published_functions_keep_their_source_names_by_default() {
    let javascript = compile(
        r#"
        extern void inspect(JsValue value);
        JsValue callbackWithALongName(JsValue value) { return JS.add(value, value); }
        export JsValue publishedFunction(JsValue value) {
            inspect(callbackWithALongName);
            return callbackWithALongName(value);
        }
        print(publishedFunction(21));
        inspect(publishedFunction);
        "#,
    );
    // The escaping callback is named by its binding; the export keeps its own.
    assert!(!javascript.contains("callbackWithALongName"), "{javascript}");
    assert_eq!(
        run(&javascript, "globalThis.inspect=f=>console.log(f.name.length<3);"),
        "true\n42\nfalse\n"
    );
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
fn redundant_operators_and_conversions_leave_no_residue() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern JsValue Math;
        extern JsValue Number;
        bool isUndef(JsValue value) { return JS.isUndefined(value); }
        number trunc(number value) { return JS.number(JS.invoke(Math, "trunc", value)); }
        bool isInteger(number value) { return JS.invoke(Number, "isInteger", value).truthy(); }
        string toStr(JsValue value) { return JS.string(value); }
        JsValue orEmpty(JsValue value) {
            if (isUndef(value) || JS.isNullish(value)) { return JS.object(); }
            return value;
        }
        export JsValue probe(JsValue a, number b) {
            bool present = !isUndef(a) && !JS.isNullish(a);
            JsValue label = JS.add("n=", toStr(trunc(b)));
            return JS.array(orEmpty(a), present, trunc(b), isInteger(b), label);
        }
        show(probe(null, 2.5));
        show(probe(JS.object(), -3));
        "#,
        PRISTINE,
    );
    // `isUndef(x)||x==null` is `x==null`; `Math.trunc` is already a number
    // and `Number.isInteger` a boolean; `"n="+(v+"")` converts `v` once.
    assert!(!javascript.contains("===void 0"), "{javascript}");
    assert!(!javascript.contains(",+Math.") && !javascript.contains("+ +Math."), "{javascript}");
    assert!(!javascript.contains("!!Number."), "{javascript}");
    assert!(!javascript.contains("+\"\")"), "{javascript}");
    assert_eq!(
        run(&javascript, SHOW),
        "[{},false,2,false,\"n=2\"]\n[{},true,-3,true,\"n=-3\"]\n"
    );
}

#[test]
fn constant_regex_constructors_become_literals_only_when_valid() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export JsValue probe(string text) {
            Regex slash = new Regex("a/b", "g");
            Regex letters = new Regex("[\\p{L}]+", "u");
            Regex named = new Regex("(?<word>b+)\\k<word>", "");
            return JS.array(slash.test(text), letters.test(text), named.test(text));
        }
        export JsValue broken() {
            Regex never = new Regex("(", "");
            return never.test("x");
        }
        show(probe("xa/bbbé"));
        "#,
        PRISTINE,
    );
    assert!(javascript.contains("/a\\/b/g"), "{javascript}");
    assert!(javascript.contains("/[\\p{L}]+/u"), "{javascript}");
    assert!(javascript.contains("/(?<word>b+)\\k<word>/"), "{javascript}");
    // An invalid pattern stays a constructor: it throws when evaluated,
    // never while the module loads.
    assert!(javascript.contains("RegExp(\"(\",\"\")"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "[true,true,true]\n");
}

#[test]
fn a_raw_plan_names_functions_themselves_and_keeps_their_names_exact() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue describeTheValue(JsValue value) {
            return JS.array(JS.string(value), JS.isNullish(value));
        }
        export JsValue describeTwice(JsValue value) {
            return JS.array(describeTheValue(value), describeTheValue(value));
        }
        extern JsValue nameOf(JsValue function);
        show(describeTwice(3));
        show(JS.array(nameOf(describeTheValue), nameOf(describeTwice)));
    "#;
    let named = compile_plan(source, PRISTINE, Plan::spelled(Style::Global, true));
    // The binding takes a short name; the exact name is spelled once.
    assert!(named.contains("function describeTheValue("), "{named}");
    assert_eq!(named.matches("describeTheValue").count(), 2, "{named}");
    let host = "globalThis.show=v=>console.log(JSON.stringify(v));globalThis.nameOf=f=>f.name;";
    assert_eq!(
        run(&named, host),
        "[[\"3\",false],[\"3\",false]]\n[\"describeTheValue\",\"describeTwice\"]\n"
    );
    let bound = compile_plan(source, PRISTINE, Plan::new(Style::Global));
    assert!(!bound.contains("function describeTheValue("), "{bound}");
    assert_eq!(run(&bound, host), run(&named, host));
}

#[test]
fn a_raw_plan_spells_statements_in_their_shortest_exact_forms() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue pick(bool flag, int limit) {
            float total = 0.0;
            int index = 0;
            while (index < limit) {
                total = total + 1.5;
                index = index + 1;
            }
            string label = "none";
            if (flag) { label = "yes"; } else { label = "no"; }
            if (total > 3.0) { return JS.array(label, total); }
            return JS.array(label, 0);
        }
        show(pick(true, 3));
        show(pick(false, 1));
    "#;
    let raw = compile_plan(source, PRISTINE, Plan::spelled(Style::Global, true));
    let coded = compile_plan(source, PRISTINE, Plan::new(Style::Global));
    // `total+=1.5`, `label=flag?…:…` and `return total>3?…:…`.
    assert!(raw.contains("+=1.5"), "{raw}");
    assert!(!raw.contains("else"), "{raw}");
    assert!(raw.len() < coded.len(), "{raw}\n{coded}");
    let expected = "[\"yes\",4.5]\n[\"no\",0]\n";
    assert_eq!(run(&raw, SHOW), expected);
    assert_eq!(run(&coded, SHOW), expected);
}

#[test]
fn a_function_read_once_takes_its_later_reference() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern void register(JsValue key, JsValue handler);
        JsValue handler = (JsValue value) => { return JS.add(value, 1); };
        show(1);
        show(2);
        register("key", handler);
        "#,
        PRISTINE,
    );
    // Creating the arrow runs nothing and nothing else reads it: it is
    // created where its one reference is, and the binding goes.
    assert!(
        javascript.contains("register(\"key\",") && !javascript.contains("let "),
        "{javascript}"
    );
    assert_eq!(
        run(&javascript, "globalThis.show=v=>console.log(v);globalThis.register=(k,h)=>console.log(k,h(41));"),
        "1\n2\nkey 42\n"
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
    // entries in place, and the constructor, now `()=>({…})` with one call,
    // inlines too: the token is the literal itself.
    assert!(javascript.contains("{kind:3,raw:\"x\",text:\"\",task:!1}"), "{javascript}");
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

#[test]
fn a_raw_objective_inlines_functions_called_once_as_blocks() {
    let source = r#"
        extern void show(JsValue value);
        JsValue classify(int value) {
            show(value);
            if (value < 0) { return "negative"; } else if (value == 0) { return "zero"; } else { return "positive"; }
        }
        JsValue early(int value) {
            if (value < 0) { return "below"; }
            show("early");
            return "above";
        }
        void report(int value) {
            show(value * 2);
            show(value * 3);
        }
        export void run(int value) {
            JsValue label = classify(value);
            show(label);
            report(value);
            show(early(value));
        }
        run(-1);
        run(0);
        run(5);
    "#;
    let raw = compile_with(source, &format!("{PRISTINE}cost_model=\"raw\"\n"));
    let coded = compile_with(source, PRISTINE);
    // `classify` returns only in tail position: its body stores the label
    // in place. `early` would need a loop to leave, so it stays a function.
    // A codec objective keeps both, whose repetition it can match.
    assert_eq!(raw.matches("=>").count(), 1, "{raw}");
    assert!(!raw.contains("for(;;)"), "{raw}");
    assert_eq!(coded.matches("=>").count(), 2, "{coded}");
    assert!(raw.len() < coded.len(), "{raw}\n{coded}");
    let expected = "-1\n\"negative\"\n-2\n-3\n\"below\"\n0\n\"zero\"\n0\n0\n\"early\"\n\"above\"\n5\n\"positive\"\n10\n15\n\"early\"\n\"above\"\n";
    assert_eq!(run(&raw, SHOW), expected);
    assert_eq!(run(&coded, SHOW), expected);
}

#[test]
fn a_function_also_read_as_a_value_keeps_its_declaration() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue pick(bool special) {
            func()->JsValue attempt = () => { show("attempt"); return 1; };
            JsValue chosen = attempt;
            if (!special) { attempt(); }
            return chosen;
        }
        show(JS.typeOf(pick(false)));
        show(JS.typeOf(pick(true)));
    "#;
    let raw = compile_with(source, &format!("{PRISTINE}cost_model=\"raw\"\n"));
    // One call, but `chosen` reads the function too: it stays declared.
    assert!(raw.contains("=>"), "{raw}");
    assert_eq!(run(&raw, SHOW), "\"attempt\"\n\"function\"\n\"function\"\n");
}

#[test]
fn comparisons_between_one_primitive_type_are_loose() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export JsValue kinds(JsValue value) {
            return JS.array(JS.typeOf(value) == "number", JS.typeOf(value) != "string");
        }
        show(kinds(1));
        show(kinds("a"));
        "#,
        PRISTINE,
    );
    // `typeof` is a string, so it compares with a string the same loosely.
    assert!(!javascript.contains("===") && !javascript.contains("!=="), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "[true,true]\n[false,false]\n");
}

#[test]
fn a_fresh_struct_meets_its_public_shape_as_a_literal() {
    let javascript = compile(
        r#"
        extern void show(JsValue value);
        extern int seed();
        struct Point { int x; int y; }
        struct Segment { Point from; Point to; string label; }
        JsValue segment = Segment{Point{1, 2}, Point{seed(), 4}, "s"};
        show(segment);
        "#,
    );
    // Every field is written in place: no encoder is needed.
    assert!(
        javascript.contains("{from:{x:1,y:2},to:{x:seed(),y:4},label:\"s\"}") && !javascript.contains("function"),
        "{javascript}"
    );
    assert_eq!(
        run(&javascript, "globalThis.seed=()=>3;globalThis.show=v=>console.log(JSON.stringify(v));"),
        "{\"from\":{\"x\":1,\"y\":2},\"to\":{\"x\":3,\"y\":4},\"label\":\"s\"}\n"
    );
}

#[test]
fn a_raw_objective_reads_repeated_strings_from_constants_and_packs_string_arrays() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue kinds(JsValue a, JsValue b, JsValue c) {
            return JS.array(JS.typeOf(a) == "string", JS.typeOf(b) == "string", JS.typeOf(c) == "string");
        }
        export JsValue names() {
            return JS.array("alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta");
        }
        show(kinds("x", 1, "y"));
        show(names());
        show(names() == names());
    "#;
    let raw = compile_with(source, &format!("{PRISTINE}cost_model=\"raw\"\n"));
    let coded = compile_with(source, PRISTINE);
    // `"string"` is spelled once, as a constant; the names are one string.
    assert_eq!(raw.matches("\"string\"").count(), 1, "{raw}");
    assert!(raw.contains(".split(\" \")") && !coded.contains(".split("), "{raw}\n{coded}");
    assert!(raw.len() < coded.len(), "{raw}\n{coded}");
    // Each call still creates a fresh array.
    let expected = "[true,false,true]\n[\"alpha\",\"beta\",\"gamma\",\"delta\",\"epsilon\",\"zeta\",\"eta\",\"theta\"]\nfalse\n";
    assert_eq!(run(&raw, SHOW), expected);
    assert_eq!(run(&coded, SHOW), expected);
}

#[test]
fn a_contract_may_publish_names_without_reflecting_them() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue describeTheValue(JsValue value) {
            return JS.array(JS.string(value), JS.isNullish(value));
        }
        show(describeTheValue(3));
    "#;
    let kept = compile_plan(source, PRISTINE, Plan::spelled(Style::Global, true));
    let config = format!("{PRISTINE}keep_published_function_names=false\n");
    let dropped = compile_plan(source, &config, Plan::spelled(Style::Global, true));
    // Kept, the name is the function's and the export's; dropped, only the
    // export's.
    assert_eq!(kept.matches("describeTheValue").count(), 2, "{kept}");
    assert_eq!(dropped.matches("describeTheValue").count(), 1, "{dropped}");
    assert_eq!(run(&kept, SHOW), "[\"3\",false]\n");
    assert_eq!(run(&dropped, SHOW), "[\"3\",false]\n");
}

#[test]
fn constructions_through_a_field_initializer_are_their_literals() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern float seed();
        class Point {
            float x;
            float y;
            init(float x, float y = 0.0) { this.x = x; this.y = y; }
        }
        JsValue describe(Point p) { return JS.array(JS.box(p.x), JS.box(p.y)); }
        Point a = new Point(1.0, 2.0);
        Point b = new Point(seed());
        Point c = new Point(seed(), seed() + 1.0);
        show(describe(a));
        show(describe(b));
        show(describe(c));
        "#,
        PRISTINE,
    );
    // Every construction is its literal, and the initializer is gone.
    assert!(javascript.contains("{x:1,y:2}") && javascript.contains("{x:seed(),y:0}"), "{javascript}");
    assert!(!javascript.contains(".x=") && !javascript.contains(".y="), "{javascript}");
    assert_eq!(
        run(&javascript, "globalThis.seed=(()=>{let n=4;return()=>n++})();globalThis.show=v=>console.log(JSON.stringify(v));"),
        "[1,2]\n[4,0]\n[5,7]\n"
    );
}

#[test]
fn a_raw_objective_writes_statements_as_expressions() {
    let source = r#"
        extern void show(JsValue value);
        extern void note(JsValue value);
        export JsValue classify(int n) {
            if (n < 0) { note("negative"); note(n); } else { note("non-negative"); }
            if (n > 100) { return "big"; }
            note("small");
            return "small";
        }
        show(classify(-1));
        show(classify(5));
        show(classify(500));
    "#;
    let raw = compile_with(source, &format!("{PRISTINE}cost_model=\"raw\"\n"));
    let coded = compile_with(source, PRISTINE);
    // `n<0?(note(…),note(n)):note(…)` and `return n>100?"big":(note(…),…)`.
    assert!(!raw.contains("if("), "{raw}");
    assert!(coded.contains("if("), "{coded}");
    let host = "globalThis.show=v=>console.log(JSON.stringify(v));globalThis.note=v=>console.log('note',v);";
    let expected = "note negative\nnote -1\nnote small\n\"small\"\nnote non-negative\nnote small\n\"small\"\nnote non-negative\n\"big\"\n";
    assert_eq!(run(&raw, host), expected);
    assert_eq!(run(&coded, host), expected);
}

#[test]
fn a_raw_objective_stores_a_conditional_and_moves_a_loop_increment_into_its_update() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue fill(JsValue source, JsValue flag) {
            JsValue out = JS.object();
            if (flag.truthy()) { out["payload"] = JS.add("p", source); } else { out["payload"] = null; }
            JsValue items = JS.array();
            int i = 0;
            int n = 3;
            while (i < n) {
                JS.invoke(items, "push", i);
                i = i + 1;
            }
            int j = 0;
            while (j < n) {
                j = j + 1;
                if (j > 1) {
                    if (j == 2) { continue; }
                    JS.invoke(items, "push", j * 10);
                }
                JS.invoke(items, "push", j);
            }
            int k = 0;
            while (k < n) {
                JsValue kept = JS.array(k);
                k = k + 1;
                JS.invoke(items, "push", kept);
            }
            out["items"] = items;
            return out;
            return source;
        }
        show(fill("x", true));
        show(fill("y", false));
    "#;
    let raw = compile_with(source, &format!("{PRISTINE}cost_model=\"raw\"\n"));
    let coded = compile_with(source, PRISTINE);
    // One store of a conditional, which then folds into the literal; the
    // first loop's increment is its update.
    assert_eq!(raw.matches("payload").count(), 1, "{raw}");
    assert!(raw.contains("{payload:b?"), "{raw}");
    assert_eq!(raw.matches("for(;").count(), 1, "{raw}");
    // A `continue` would skip a moved increment, and `kept` is the body's.
    assert!(raw.contains("continue"), "{raw}");
    assert_eq!(raw.matches("while(").count(), 2, "{raw}");
    // Nothing runs after the first return.
    assert_eq!(raw.matches("return").count(), 1, "{raw}");
    let expected = "{\"payload\":\"px\",\"items\":[0,1,2,1,30,3,[0],[1],[2]]}\n{\"payload\":null,\"items\":[0,1,2,1,30,3,[0],[1],[2]]}\n";
    assert_eq!(run(&raw, SHOW), expected);
    assert_eq!(run(&coded, SHOW), expected);
}

#[test]
fn a_value_moves_past_a_quiet_start_of_its_statement() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern JsValue seed();
        JsValue registry = JS.array();
        JsValue early(JsValue p) {
            JsValue b = JS.object("type", "early", "handler", wrap(p));
            return register(b);
        }
        JsValue register(JsValue spec) {
            JS.invoke(registry, "push", spec);
            float n = JS.number(registry["length"]);
            if (n > 3.0) { show("many"); }
            return spec;
        }
        JsValue wrap(JsValue f) {
            JsValue out = JS.array(f, seed());
            JS.invoke(out, "push", f);
            return out;
        }
        export JsValue later(JsValue p) {
            JsValue b = JS.object("type", "later", "handler", wrap(p));
            return register(b);
        }
        export bool maybe(bool flag, JsValue p) {
            JsValue b = JS.object("type", "maybe", "handler", wrap(p));
            return flag && register(b).truthy();
        }
        show(early(0));
        show(later(1));
        show(maybe(false, 2));
        show(maybe(true, 3));
        show(JS.number(registry["length"]));
        "#,
        PRISTINE,
    );
    // `register` is declared before `later` is created, so reading it there
    // is quiet and the literal is created in the call.
    assert!(javascript.contains("({type:\"later\""), "{javascript}");
    // `early` exists before `register` does: the read might meet its TDZ.
    assert!(javascript.contains("={type:\"early\""), "{javascript}");
    // Behind `&&`, the call and the literal might not run.
    assert!(javascript.contains("={type:\"maybe\""), "{javascript}");
    assert_eq!(
        run(&javascript, "globalThis.seed=()=>7;globalThis.show=v=>console.log(JSON.stringify(v));"),
        "{\"type\":\"early\",\"handler\":[0,7,0]}\n{\"type\":\"later\",\"handler\":[1,7,1]}\nfalse\ntrue\n3\n"
    );
}

#[test]
fn a_raw_objective_ends_a_body_with_an_else_instead_of_an_exit() {
    let source = r#"
        extern void show(JsValue value);
        extern void note(JsValue value);
        export void visit(JsValue[] items, bool quiet) {
            if (quiet) {
                note("quiet");
                return;
            }
            JsValue seen = JS.array();
            int i = 0;
            int n = items.length;
            while (i < n) {
                JsValue item = items[i];
                i = i + 1;
                if (i == 2) {
                    note(item);
                    continue;
                }
                JsValue doubled = JS.array(item, item);
                JS.invoke(seen, "push", doubled);
            }
            show(seen);
        }
        visit([JS.box(1.0), JS.box("a"), JS.box(2.0)], false);
        visit([JS.box(3.0)], true);
    "#;
    let raw = compile_with(source, &format!("{PRISTINE}cost_model=\"raw\"\n"));
    let coded = compile_with(source, PRISTINE);
    // Neither the early `return;` nor the `continue` is spelled.
    assert!(!raw.contains("return") && !raw.contains("continue"), "{raw}");
    assert!(coded.contains("continue"), "{coded}");
    let host = "globalThis.show=v=>console.log(JSON.stringify(v));globalThis.note=v=>console.log('note',v);";
    let expected = "note [String: 'a']\n[[1,1],[2,2]]\nnote quiet\n";
    assert_eq!(run(&raw, host), expected);
    assert_eq!(run(&coded, host), expected);
}

#[test]
fn a_value_that_only_reads_moves_past_member_reads_under_pure_property_reads() {
    let source = r#"
        extern void show(JsValue value);
        JsValue log = JS.array();
        JsValue track(JsValue parser, JsValue options, string mode) {
            JS.invoke(log, "push", mode);
            float n = JS.number(log["length"]);
            if (n > 5.0) { show("many"); }
            return JS.array(parser["name"], options, mode);
        }
        export JsValue describe(JsValue context) {
            JsValue options = JS.object("cols", context["cols"], "leqno", context["parser"]["settings"]["leqno"]);
            return track(context["parser"], options, "display");
        }
        show(describe(JS.object("cols", 2, "parser", JS.object("name", "p", "settings", JS.object("leqno", true)))));
    "#;
    let pure = compile_with(source, &format!("{PRISTINE}assume_pure_property_reads=true\n"));
    let plain = compile_with(source, PRISTINE);
    // Reads commute with reads: the literal is created in the call.
    assert!(pure.contains(",{cols:"), "{pure}");
    // A read might run a getter that the other reads would observe.
    assert!(plain.contains("={cols:"), "{plain}");
    let expected = "[\"p\",{\"cols\":2,\"leqno\":true},\"display\"]\n";
    assert_eq!(run(&pure, SHOW), expected);
    assert_eq!(run(&plain, SHOW), expected);
}

#[test]
fn a_function_of_one_statement_is_inlined_where_its_value_is_discarded() {
    let source = r#"
        extern void show(JsValue value);
        void put(JsValue target, string key, JsValue value) { target[key] = value; }
        JsValue lookup(JsValue table, JsValue key) {
            JsValue found = table[key];
            if (JS.strictEqual(found, JS.undefined())) { return null; }
            return found;
        }
        export JsValue fill(JsValue o, JsValue t) {
            put(o, "opacity", JS.box(1.0));
            put(o, "scale", lookup(t, "s"));
            put(o, "x", lookup(t, "x"));
            return o;
        }
        show(fill(JS.object(), JS.object("s", 2)));
    "#;
    let raw = compile_with(source, &format!("{PRISTINE}cost_model=\"raw\"\n"));
    let coded = compile_with(source, PRISTINE);
    // `put`'s body is its calls' statements; the function is gone.
    for javascript in [&raw, &coded] {
        assert!(javascript.contains(".opacity=") && javascript.contains(".scale="), "{javascript}");
    }
    // `if(v===void 0)return null;return v` compresses to `return v??null`.
    assert!(raw.contains("??null"), "{raw}");
    let expected = "{\"opacity\":1,\"scale\":2,\"x\":null}\n";
    assert_eq!(run(&raw, SHOW), expected);
    assert_eq!(run(&coded, SHOW), expected);
}

#[test]
fn a_namespace_is_flattened_where_its_reads_run_after_it() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        JsValue ns = JS.undefined();
        JsValue early = JS.undefined();
        early = (JsValue x) => JS.invoke(ns, "twice", x);
        JsValue twice = JS.undefined();
        twice = (JsValue x) => JS.add(JS.add(x, x), JS.array(x)["length"]);
        JsValue lit = JS.object("twice", twice, "name", "ns");
        ns = lit;
        JsValue late = JS.undefined();
        late = (JsValue x) => JS.array(JS.invoke(ns, "twice", x), JS.invoke(ns, "twice", JS.add(x, 1)));
        show(JS.call(early, JS.undefined(), 1));
        show(JS.call(late, JS.undefined(), 2));
        "#,
        PRISTINE,
    );
    // `late` is created after `ns` holds the literal: its reads are the
    // member itself. `early` exists before then, so its read stays.
    assert_eq!(javascript.matches(".twice(").count(), 1, "{javascript}");
    assert_eq!(run(&javascript, SHOW), "3\n[5,7]\n");
}

#[test]
fn method_stores_into_one_prototype_become_one_assign() {
    let source = r#"
        extern void show(JsValue value);
        JsValue Point = JS.undefined();
        Point = JS.methodRest((JsValue self, JsValue argv) => {
            self["x"] = argv[0];
            self["y"] = argv[1];
            return self;
        });
        Point["prototype"]["sum"] = JS.method0((JsValue self) => JS.add(self["x"], self["y"]));
        Point["prototype"]["scaled"] = JS.method1((JsValue self, JsValue k) => JS.construct(Point, JS.add(self["x"], k), JS.add(self["y"], k)));
        Point["prototype"]["label"] = "point";
        export JsValue make(JsValue a, JsValue b) {
            JsValue p = JS.construct(Point, a, b);
            return JS.array(JS.invoke(p, "sum"), JS.invoke(JS.invoke(p, "scaled", 2), "sum"), p["label"]);
        }
        show(make(1, 2));
    "#;
    let pure = compile_with(source, &format!("{PRISTINE}assume_pure_property_reads=true\n"));
    let plain = compile_with(source, PRISTINE);
    // One read of `Point.prototype` needs member reads that run no code.
    assert!(pure.contains("Object.assign(") && pure.matches(".prototype").count() == 1, "{pure}");
    assert!(!plain.contains("Object.assign("), "{plain}");
    let expected = "[3,7,\"point\"]\n";
    assert_eq!(run(&pure, SHOW), expected);
    assert_eq!(run(&plain, SHOW), expected);
}

#[test]
fn a_temporary_and_its_test_are_the_logical_operator() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue pick(JsValue options, JsValue fallback) {
            JsValue and1 = options;
            if (and1.truthy()) {
                and1 = options["font"];
            }
            JsValue or1 = and1;
            if (!(or1.truthy())) {
                or1 = fallback;
            }
            return or1;
        }
        show(pick(JS.object("font", "bold"), "normal"));
        show(pick(JS.object(), "normal"));
        show(pick(null, "none"));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // `options&&options.font||fallback`: no temporary is tested.
    assert!(!javascript.contains("if("), "{javascript}");
    assert!(javascript.contains("&&") && javascript.contains("||"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "\"bold\"\n\"normal\"\n\"none\"\n");
}

#[test]
fn defaults_of_a_function_only_ever_called_print_natively() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern int seed();
        int scaled(int x, int factor = 2, int offset = 1) {
            int y = x * factor;
            return y + offset + seed();
        }
        show(JS.box(scaled(3)));
        show(JS.box(scaled(3, 4)));
        show(JS.box(scaled(3, 4, 5)));
        "#,
        PRISTINE,
    );
    // Nothing reads `scaled` but its calls, so its `length` is free and the
    // body's default checks become `(a,b=2,c=1)`.
    assert!(javascript.contains("=2,") && !javascript.contains("===void 0"), "{javascript}");
    assert_eq!(run(&javascript, &format!("globalThis.seed=()=>0;{SHOW}")), "7\n13\n17\n");
}
