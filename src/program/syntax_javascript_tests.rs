//! `match`, optional chaining, destructuring and object literals holding
//! value structs, observed by running the output.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};
use std::process::Command;

fn compile(source: &str) -> String {
    compile_with(source, "[javascript]\n")
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
    assert!(
        javascript.contains("class Problem extends Error"),
        "{javascript}"
    );
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
        "[javascript]\nkeep_function_names=true\n",
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
    assert!(
        !javascript.contains("callbackWithALongName"),
        "{javascript}"
    );
    assert_eq!(
        run(
            &javascript,
            "globalThis.inspect=f=>console.log(f.name.length<3);"
        ),
        "true\n42\nfalse\n"
    );
}

const SHOW: &str = "globalThis.show=v=>console.log(JSON.stringify(v));";
const PRISTINE: &str = "[javascript]\nassume_pristine_builtins=true\n";

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
    assert!(
        !javascript.contains(",+Math.") && !javascript.contains("+ +Math."),
        "{javascript}"
    );
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
    assert!(
        javascript.contains("/(?<word>b+)\\k<word>/"),
        "{javascript}"
    );
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
fn the_raw_seed_spells_statements_in_their_shortest_exact_forms() {
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
    // The raw objective's seed families at level 0 (no walk): since M8.3
    // the statement forms and `+=` are families, not the naming plan's.
    let seeded = |codec: &str| {
        let config: crate::config::ProjectConfig = toml::from_str(&format!(
            "objective.codecs='{codec}'\neffort.level=0\n{PRISTINE}"
        ))
        .unwrap();
        let compiled =
            crate::build::compile_source(source, &config, crate::build::ServiceOptions::default())
                .unwrap();
        let objective = match codec {
            "raw" => crate::js::selection::Objective::Raw,
            _ => crate::js::selection::Objective::Brotli,
        };
        compiled
            .javascript(objective)
            .unwrap()
            .javascript()
            .to_string()
    };
    let raw = seeded("raw");
    let coded = seeded("brotli");
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
        run(
            &javascript,
            "globalThis.show=v=>console.log(v);globalThis.register=(k,h)=>console.log(k,h(41));"
        ),
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
    assert!(
        javascript.contains("{kind:3,raw:\"x\",text:\"\",task:!1}"),
        "{javascript}"
    );
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
    let raw = compile_with(source, &format!("objective.codecs=\"raw\"\n{PRISTINE}"));
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
    let raw = compile_with(source, &format!("objective.codecs=\"raw\"\n{PRISTINE}"));
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
    assert!(
        !javascript.contains("===") && !javascript.contains("!=="),
        "{javascript}"
    );
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
        javascript.contains("{from:{x:1,y:2},to:{x:seed(),y:4},label:\"s\"}")
            && !javascript.contains("function"),
        "{javascript}"
    );
    assert_eq!(
        run(
            &javascript,
            "globalThis.seed=()=>3;globalThis.show=v=>console.log(JSON.stringify(v));"
        ),
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
    let raw = compile_with(source, &format!("objective.codecs=\"raw\"\n{PRISTINE}"));
    let coded = compile_with(source, PRISTINE);
    // `"string"` is spelled once, as a constant; the names are one string.
    assert_eq!(raw.matches("\"string\"").count(), 1, "{raw}");
    assert!(
        raw.contains(".split(\" \")") && !coded.contains(".split("),
        "{raw}\n{coded}"
    );
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
    assert!(
        javascript.contains("{x:1,y:2}") && javascript.contains("{x:seed(),y:0}"),
        "{javascript}"
    );
    assert!(
        !javascript.contains(".x=") && !javascript.contains(".y="),
        "{javascript}"
    );
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
    let raw = compile_with(source, &format!("objective.codecs=\"raw\"\n{PRISTINE}"));
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
    let raw = compile_with(source, &format!("objective.codecs=\"raw\"\n{PRISTINE}"));
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
    // `early` exists before `register` does, but nothing can call it until
    // `show(early(0))`, after `register` is declared (initialization order).
    assert!(javascript.contains("({type:\"early\""), "{javascript}");
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
    let raw = compile_with(source, &format!("objective.codecs=\"raw\"\n{PRISTINE}"));
    let coded = compile_with(source, PRISTINE);
    // Neither the early `return;` nor the `continue` is spelled.
    assert!(
        !raw.contains("return") && !raw.contains("continue"),
        "{raw}"
    );
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
    let pure = compile_with(
        source,
        &format!("{PRISTINE}assume_pure_property_reads=true\n"),
    );
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
    let raw = compile_with(source, &format!("objective.codecs=\"raw\"\n{PRISTINE}"));
    let coded = compile_with(source, PRISTINE);
    // `put`'s body is its calls' statements; the function is gone.
    for javascript in [&raw, &coded] {
        assert!(
            javascript.contains(".opacity=") && javascript.contains(".scale="),
            "{javascript}"
        );
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
    let pure = compile_with(
        source,
        &format!("{PRISTINE}assume_pure_property_reads=true\n"),
    );
    let plain = compile_with(source, PRISTINE);
    // One read of `Point.prototype` needs member reads that run no code.
    assert!(
        pure.contains("Object.assign(") && pure.matches(".prototype").count() == 1,
        "{pure}"
    );
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
    assert!(
        javascript.contains("&&") && javascript.contains("||"),
        "{javascript}"
    );
    assert_eq!(run(&javascript, SHOW), "\"bold\"\n\"normal\"\n\"none\"\n");
}

#[test]
fn defaults_of_a_function_only_ever_called_print_natively() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern int seed();
        float scaled(float x, JsValue factor = 2, JsValue offset = 1) {
            float y = x * JS.number(factor);
            return y + JS.number(offset) + seed();
        }
        show(JS.box(scaled(3.0)));
        show(JS.box(scaled(3.0, 4)));
        show(JS.box(scaled(3.0, 4, 5)));
        "#,
        PRISTINE,
    );
    // Nothing reads `scaled` but its calls, so its `length` is free and the
    // body's default checks become `(a,b=2,c=1)`. (A default of a typed
    // parameter no typed caller can omit goes altogether.)
    assert!(
        javascript.contains("=2,") && !javascript.contains("===void 0"),
        "{javascript}"
    );
    assert_eq!(
        run(&javascript, &format!("globalThis.seed=()=>0;{SHOW}")),
        "7\n13\n17\n"
    );
}

#[test]
fn an_inert_value_is_created_in_the_branch_that_reads_it() {
    let source = r#"
        extern void show(JsValue value);
        extern bool enabled();
        extern JsValue hooks;
        JsValue getter(JsValue element) { return element["selected"]; }
        JsValue setter(JsValue element, JsValue value) { element["selected"] = value; return value; }
        if (!enabled()) {
            hooks["selected"] = JS.object("get", getter, "set", setter);
        }
        show(hooks["selected"]["get"](JS.object("selected", 7)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // Neither function is read anywhere else: each is created in the arm
    // that stores it, and its binding goes.
    assert!(
        javascript.contains("{get:") && !javascript.contains("let "),
        "{javascript}"
    );
    assert_eq!(
        run(&javascript, "globalThis.show=v=>console.log(JSON.stringify(v));globalThis.enabled=()=>false;globalThis.hooks={};"),
        "7\n"
    );
}

#[test]
fn a_tested_value_returned_by_one_arm_is_the_logical_operator() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue kind(JsValue table, JsValue key) {
            JsValue found = table[key];
            if (found.truthy()) {
                return found;
            }
            return "object";
        }
        export JsValue first(JsValue list) {
            JsValue head = list[0];
            if (!(head.truthy())) {
                return head;
            }
            return head["name"];
        }
        show(kind(JS.object("a", "array"), "a"));
        show(kind(JS.object(), "b"));
        show(first(JS.array(JS.object("name", "x"))));
        show(first(JS.array(0)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // `return a[b]||"object"` and `return c&&c.name`: no test, no temporary
    // for the first.
    assert!(!javascript.contains("if("), "{javascript}");
    assert!(javascript.contains("||\"object\""), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "\"array\"\n\"object\"\n\"x\"\n0\n");
}

#[test]
fn an_initializer_store_the_literal_already_holds_goes() {
    let source = r#"
        extern void show(JsValue value);
        extern JsValue seed();
        class Counter {
            int count;
            JsValue label;
            int[] items;
            init(JsValue label) {
                this.count = 0;
                this.items = [];
                show(seed());
                this.label = label;
            }
        }
        export JsValue make(JsValue l) {
            Counter c = new Counter(l);
            c.count = c.count + 1;
            c.items.push(c.count);
            show(JS.box(c.items.length));
            return c.label;
        }
        export JsValue other(JsValue l) {
            Counter c = new Counter(l);
            show(JS.box(c.count));
            return c.label;
        }
        show(make("a"));
        show(other("b"));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // Every construction's literal already holds `count:0` and `items:[]`,
    // and `show(seed())` cannot see the object: those stores go.
    assert!(
        !javascript.contains(".count=0") && !javascript.contains(".items=[]"),
        "{javascript}"
    );
    assert_eq!(
        run(
            &javascript,
            "globalThis.show=v=>console.log(JSON.stringify(v));globalThis.seed=()=>\"s\";"
        ),
        "\"s\"\n1\n\"a\"\n\"s\"\n0\n\"b\"\n"
    );
}

#[test]
fn a_default_only_erased_callers_could_use_is_no_check() {
    let source = r#"
        extern void show(JsValue value);
        extern float noise();
        float scaled(float x, float factor = 2.0) {
            float y = x * factor;
            show(JS.box(y));
            return y + noise();
        }
        show(JS.box(scaled(3.0, noise())));
        show(JS.box(scaled(1.0, noise() + 1.0)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // Typed callers always pass `factor`, and none passes the default: the
    // function opens without it.
    assert!(!javascript.contains("=2"), "{javascript}");
    assert_eq!(
        run(
            &javascript,
            "globalThis.show=v=>console.log(JSON.stringify(v));globalThis.noise=()=>1;"
        ),
        "3\n4\n2\n3\n"
    );
}

#[test]
fn an_object_only_read_through_its_fields_is_its_fields() {
    let source = r#"
        extern void show(JsValue value);
        class Cell {
            bool on;
            int n;
        }
        export func()->int counter(int start) {
            Cell c = new Cell();
            c.n = start;
            return () => {
                c.on = !c.on;
                c.n = c.n + 1;
                if (c.on) { return c.n * 2; }
                return c.n;
            };
        }
        func()->int k = counter(5);
        show(JS.box(k()));
        show(JS.box(k()));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // The cell never leaves its fields: the closure shares two locals.
    assert!(
        !javascript.contains(".on") && !javascript.contains(".n="),
        "{javascript}"
    );
    assert_eq!(run(&javascript, SHOW), "12\n7\n");
}

#[test]
fn a_null_test_of_an_object_is_its_truthiness() {
    let source = r#"
        extern void show(JsValue value);
        class Node { int v; Node? next; init(int v, Node? next) { this.v = v; this.next = next; } }
        export int total(Node? head) {
            int sum = 0;
            Node? at = head;
            while (at != null) { sum = sum + at.v; at = at.next; }
            return sum;
        }
        show(JS.box(total(new Node(2, new Node(3, null)))));
        show(JS.box(total(null)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // An object is never falsy, so `at!=null` is `at`.
    assert!(!javascript.contains("=null)"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "5\n0\n");
}

#[test]
fn an_array_the_program_created_takes_its_own_methods() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue collect(int n) {
            JsValue items = JS.array();
            int i = 0;
            while (i < n) { JS.push(items, JS.box(i * 2)); i = i + 1; }
            JS.pop(items);
            return items;
        }
        show(collect(4));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // Every value `items` holds is an array literal: its own `push` is
    // `Array.prototype.push` under pristine builtins.
    assert!(
        !javascript.contains("prototype") && javascript.contains(".push("),
        "{javascript}"
    );
    assert_eq!(run(&javascript, SHOW), "[0,2,4]\n");
}

#[test]
fn a_method_reached_through_its_receiver_is_a_method_call() {
    let source = r#"
        extern void show(JsValue value);
        export JsValue run(JsValue effects) {
            JsValue step = JS.method1((JsValue _s, JsValue code) => {
                JS.call(effects["enter"], effects, code);
                return code;
            });
            return step;
        }
        JsValue effects = JS.object();
        JS.set(effects, "seen", 0);
        JS.set(effects, "enter", JS.method1((JsValue self, JsValue x) => {
            JS.set(self, "seen", x);
            return x;
        }));
        JS.call(run(effects), JS.undefined(), JS.box(5));
        show(effects["seen"]);
    "#;
    let javascript = compile_with(source, PRISTINE);
    // `effects.enter.call(effects,code)` is `effects.enter(code)`, and the
    // step ignores its receiver, so no adapter wraps it.
    assert!(!javascript.contains(".call("), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "5\n");
}

#[test]
fn a_lambda_that_may_be_constructed_stays_a_function() {
    let source = r#"
        extern void show(JsValue value);
        JsValue M = JS.method1((JsValue self, JsValue a) => JS.get(a, "x"));
        export JsValue P = JS.get(M, "prototype");
        export JsValue make(JsValue v) { return JS.construct(M, v); }
        JsValue direct = JS.method1((JsValue self, JsValue a) => JS.get(a, "y"));
        show(JS.box(JS.typeOf(P)));
        show(make(JS.object("x", JS.object("y", 1))));
        show(JS.call(direct, JS.undefined(), JS.object("y", 2)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // The adapter's result was constructible with a prototype: a lambda that
    // escapes stays a function; one only ever called may be an arrow.
    assert!(javascript.contains("=function("), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "\"object\"\n{\"y\":1}\n2\n");
}

#[test]
fn a_declared_unconstructed_callback_may_be_an_arrow_where_it_escapes() {
    let source = r#"
        extern void show(JsValue value);
        JsValue M = JS.method1((JsValue self, JsValue a) => JS.get(a, "x"));
        export JsValue make(JsValue v) { return JS.construct(M, v); }
        export JsValue escape() { return JS.method1((JsValue self, JsValue a) => JS.get(a, "z")); }
        show(make(JS.object("x", JS.object("y", 1))));
        show(JS.call(escape(), JS.undefined(), JS.object("z", 3)));
    "#;
    let sound = compile_with(source, PRISTINE);
    assert!(
        sound.contains("return function(") && !sound.contains("=>"),
        "{sound}"
    );
    let assumed = compile_with(
        source,
        "[javascript]\nassume_pristine_builtins=true\nassume_unconstructed_callbacks=true\n",
    );
    // The escaping lambda may be an arrow; the one the program constructs
    // through its variable stays a function.
    assert!(
        assumed.contains("=>") && assumed.contains("=function("),
        "{assumed}"
    );
    for javascript in [sound, assumed] {
        assert_eq!(run(&javascript, SHOW), "{\"y\":1}\n3\n");
    }
}

#[test]
fn a_literal_root_constant_is_its_literal_where_it_is_initialized() {
    let source = r#"
        extern void show(JsValue value);
        string tag = "thematicBreak";
        int limit = 3;
        export string label(int n) {
            if (n >= limit) { return tag + "!"; }
            return tag;
        }
        show(JS.box(label(2)));
        show(JS.box(label(limit)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // Nothing runs before the constants hold their values: every read is
    // the literal, and no name is left.
    assert!(
        !javascript.contains("=\"thematicBreak\"") && javascript.contains(">=3"),
        "{javascript}"
    );
    assert_eq!(
        run(&javascript, SHOW),
        "\"thematicBreak\"\n\"thematicBreak!\"\n"
    );
}

#[test]
fn a_repeated_long_number_is_named_once_for_raw_bytes() {
    let source = r#"
        extern void show(JsValue value);
        float big = 281474976710655.0;
        export float a(float x) { return x + big; }
        export float b(float x) { return x - big; }
        export float c(float x) { return x * big; }
        export float d(float x) { return x / big; }
        show(JS.box(a(1.0) + b(1.0) + c(0.0) + d(0.0)));
    "#;
    let raw = "objective.codecs=\"raw\"\n[javascript]\nassume_pristine_builtins=true\n";
    let javascript = compile_with(source, raw);
    // Canonicalized into its reads, then pooled again for the raw objective.
    assert_eq!(
        javascript.matches("281474976710655").count(),
        1,
        "{javascript}"
    );
    assert_eq!(run(&javascript, SHOW), "2\n");
}

#[test]
fn a_loose_null_test_narrowed_by_a_strict_one_is_the_strict_test() {
    let source = r#"
        extern void show(JsValue value);
        export bool isNull(JsValue value) {
            return JS.isNullish(value) && !JS.isUndefined(value);
        }
        show(JS.box(isNull(JS.undefined())));
        show(JS.box(isNull(JS.object())));
        show(JS.box(isNull(null)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    assert!(
        javascript.contains("===null") && !javascript.contains("==null&&"),
        "{javascript}"
    );
    assert_eq!(run(&javascript, SHOW), "false\nfalse\ntrue\n");
}

#[test]
fn a_string_of_double_quotes_prints_in_single_quotes_for_raw_bytes() {
    let source = r#"
        extern void show(JsValue value);
        extern JsValue JSON;
        export JsValue table() { return JS.invoke(JSON, "parse", "{\"a\":\"b\"}"); }
        show(table());
    "#;
    let raw = "objective.codecs=\"raw\"\n[javascript]\nassume_pristine_builtins=true\n";
    let javascript = compile_plan(source, raw, Plan::spelled(Style::Global, true));
    assert!(javascript.contains(r#"'{"a":"b"}'"#), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "{\"a\":\"b\"}\n");
    // A codec objective keeps one delimiter.
    let javascript = compile_with(source, PRISTINE);
    assert!(javascript.contains(r#""{\"a\":\"b\"}""#), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "{\"a\":\"b\"}\n");
}

#[test]
fn a_host_value_assumed_to_be_a_struct_is_read_by_field_name() {
    let source = r#"
        extern void show(JsValue value);
        struct TokenView { string type; bool flag; }
        export bool marked(JsValue token) {
            TokenView view = JS.assume(token);
            view.flag = view.type == "a";
            return view.flag;
        }
        JsValue first = JS.object("type", "a", "flag", false);
        show(JS.box(marked(first)));
        show(JS.box(marked(JS.object("type", "b", "flag", true))));
        show(first["flag"]);
    "#;
    let javascript = compile_with(source, PRISTINE);
    // A struct's storage is private: the host object is decoded by name,
    // and the view is a value, so the store stays in it.
    assert!(
        javascript.contains(".type,") && javascript.contains(".flag]"),
        "{javascript}"
    );
    assert_eq!(run(&javascript, SHOW), "true\nfalse\nfalse\n");
}

#[test]
fn a_large_constant_string_table_is_decoded_from_two_strings() {
    // Sorted keys sharing prefixes, as a named-entity table has; one key a
    // numeric string, which JavaScript enumerates first either way.
    let mut entries = vec!["\"7\": \"seven\"".to_string()];
    for index in 0..80 {
        entries.push(format!("entity{index:02}: \"value {index} ü\""));
    }
    let source = format!(
        r#"
        extern void show(JsValue value);
        extern JsValue Object;
        JsValue table = object {{ {} }};
        export JsValue get(string key) {{ return table[key]; }}
        show(get("entity42"));
        show(get("7"));
        show(JS.invoke(Object, "keys", table));
        "#,
        entries.join(", ")
    );
    let javascript = compile_with(&source, PRISTINE);
    assert!(
        !javascript.contains("entity42:") && javascript.contains(".split("),
        "{javascript}"
    );
    let mut keys = vec!["\"7\"".to_string()];
    keys.extend((0..80).map(|index| format!("\"entity{index:02}\"")));
    assert_eq!(
        run(&javascript, SHOW),
        format!("\"value 42 ü\"\n\"seven\"\n[{}]\n", keys.join(","))
    );
}

#[test]
fn a_number_counts_up_with_the_increment() {
    let source = r#"
        extern void show(JsValue value);
        export float halves(int n) {
            float x = 0.5;
            int i = 0;
            while (i < n) { x = x + 1; i = i + 1; }
            return x;
        }
        show(JS.box(halves(3)));
    "#;
    let javascript = compile_with(source, PRISTINE);
    // A float's `x=x+1` is `++x`. The int counter stays below `n`, an int32
    // by type (R1), so its increment cannot wrap and is `++` too.
    assert!(
        javascript.matches("++").count() >= 2 && !javascript.contains("|0"),
        "{javascript}"
    );
    assert_eq!(run(&javascript, SHOW), "3.5\n");
}

/// R12: an operator on a `JsValue` is the dynamic operation its `JS.*`
/// spelling names, operand for operand, so the two spellings compile to the
/// same program. (Member access, calls and literals keep their own IR forms:
/// the method is read before the arguments are evaluated.)
#[test]
fn dynamic_operators_compile_as_their_js_spelling() {
    let header = r#"
        extern void show(JsValue value);
    "#;
    let syntax = r#"
        export JsValue f(JsValue o, string k, JsValue x, JsValue C) {
            show(x.call(o, 1));
            show(x.apply(o, x));
            show(new C(x));
            show(new (x)(1, 2));
            show(x + 1);
            show(x % 2);
            show(x < o);
            show(x >= o);
            show(x === o);
            show(x !== o);
            show(x || o);
            show(x && o);
            show(typeof x);
            show(k in o);
            delete o.name;
            delete o[k];
            show(string(x));
            show(float(x));
            show(undefined);
            string s = x as string;
            show(s);
            return o;
        }
    "#;
    let spelled = r#"
        export JsValue f(JsValue o, string k, JsValue x, JsValue C) {
            show(JS.call(x, o, 1));
            show(JS.apply(x, o, x));
            show(JS.construct(C, x));
            show(JS.construct(x, 1, 2));
            show(JS.add(x, 1));
            show(JS.mod(x, 2));
            show(JS.lessThan(x, o));
            show(JS.greaterThanOrEqual(x, o));
            show(JS.strictEqual(x, o));
            show(JS.strictNotEqual(x, o));
            show(JS.or(x, o));
            show(JS.and(x, o));
            show(JS.typeOf(x));
            show(JS.in(k, o));
            JS.delete(o, "name");
            JS.delete(o, k);
            show(JS.string(x));
            show(JS.number(x));
            show(JS.undefined());
            string s = JS.assume(x);
            show(s);
            return o;
        }
    "#;
    for config in ["[javascript]\n", PRISTINE] {
        assert_eq!(
            compile_with(&format!("{header}{syntax}"), config),
            compile_with(&format!("{header}{spelled}"), config),
            "{config}"
        );
    }
}

/// `instanceof`, `+=` on a property, an assignment's value, `delete` and
/// `undefined` as a parameter default, observed by running them.
#[test]
fn dynamic_syntax_has_javascripts_meaning() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern JsValue Error;
        extern JsValue Date;
        extern JsValue replace(JsValue o);
        export JsValue f(JsValue o, JsValue fallback = undefined) {
            show(o instanceof Error);
            show(o.when instanceof Date);
            o.count += 2;
            o["count"] += 3;
            JsValue seen = o.last = o.count;
            show(seen);
            delete o.when;
            show(typeof fallback);
            show(fallback === undefined);
            show("k" + o.count);
            // The method is read before its argument redefines it.
            show(o.greet(replace(o)));
            show(o.greet(1));
            show(object { list: [], items: [o.count, 2] });
            return o;
        }
        show(f(object { when: new Date(0), count: 1, greet: (JsValue v) => "old" }));
        "#,
        PRISTINE,
    );
    let host = format!("{SHOW}globalThis.replace=o=>{{o.greet=()=>'new';return 0;}};");
    assert_eq!(
        run(&javascript, &host),
        "false\ntrue\n6\n\"undefined\"\ntrue\n\"k6\"\n\"old\"\n\"new\"\n{\"list\":[],\"items\":[6,2]}\n{\"count\":6,\"last\":6}\n"
    );
}

/// R7: a lambda that names its receiver or its rest is the method its
/// adapter makes, and compiles as the adapter's spelling does.
#[test]
fn receiver_and_rest_lambdas_compile_as_their_adapters() {
    let header = "extern void show(JsValue value);\n";
    let syntax = r#"
        export JsValue methods(JsValue proto) {
            proto.size = (this JsValue self) => self.items;
            proto.add = (this JsValue self, JsValue item, JsValue at) => self.items.splice(at, 0, item);
            proto.all = (this JsValue self, JsValue... items) => self.items.concat(items);
            JsValue named = (this JsValue self) => self;
            proto.named = named;
            return (JsValue... values) => values;
        }
    "#;
    let spelled = r#"
        export JsValue methods(JsValue proto) {
            proto.size = JS.method0((JsValue self) => self.items);
            proto.add = JS.method2((JsValue self, JsValue item, JsValue at) => self.items.splice(at, 0, item));
            proto.all = JS.methodRest((JsValue self, JsValue items) => self.items.concat(items));
            JsValue named = JS.method0((JsValue self) => self);
            proto.named = named;
            return JS.staticRest((JsValue values) => values);
        }
    "#;
    for config in ["[javascript]\n", PRISTINE] {
        assert_eq!(
            compile_with(&format!("{header}{syntax}"), config),
            compile_with(&format!("{header}{spelled}"), config),
            "{config}"
        );
    }
}

/// R12's tests and conversions on a `JsValue`, and its arithmetic: `??`,
/// `bool(v)`, `as?`, `-`, `*`, `/`, observed by running them. Only the
/// source's explicit forms emit a test or a conversion (owner, Y1).
#[test]
fn dynamic_tests_and_conversions_have_javascripts_meaning() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export void f(JsValue a, JsValue b) {
            show(a ?? b);
            show(bool(a));
            show(!bool(b));
            string? s = a as? string;
            show(s ?? "none");
            float? n = b as? float;
            show(n ?? 0.5);
            show(a - b);
            show(a * b);
            show(b / 2);
            show(-b);
        }
        f(null, 4);
        f("7", 2);
        "#,
        PRISTINE,
    );
    assert_eq!(
        run(&javascript, SHOW),
        "4\nfalse\nfalse\n\"none\"\n4\n-4\n0\n2\n-4\n\"7\"\ntrue\nfalse\n\"7\"\n2\n5\n14\n1\n-2\n"
    );
}

/// R14: sets and maps iterate in insertion order, and a `JsValue` by the
/// iterator protocol, observed by running them.
#[test]
fn sets_maps_and_dynamic_iterables_iterate_in_order() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export void f(JsValue letters) {
            Set<string> seen = new Set<string>();
            seen.add("b");
            seen.add("a");
            seen.add("b");
            for (string s of seen) { show(s); }
            Map<string, int> counts = new Map<string, int>();
            counts.set("x", 1);
            counts.set("y", 2);
            for (string k, int v of counts) {
                if (v > 1) { break; }
                show(k + v);
            }
            for (JsValue letter of letters) { show(letter); }
        }
        f("hi");
        "#,
        PRISTINE,
    );
    assert_eq!(
        run(&javascript, SHOW),
        "\"b\"\n\"a\"\n\"x1\"\n\"h\"\n\"i\"\n"
    );
}

/// R7: `...xs` spreads an array or a `JsValue` iterable into a JavaScript
/// call, a method call, `new` and `f.call(t, …)`, observed by running them.
#[test]
fn spread_arguments_reach_javascript_calls() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern JsValue Math;
        extern JsValue Date;
        export void f(JsValue fn, JsValue list) {
            float[] xs = [3.0, 1.0, 2.0];
            show(Math.max(...xs));
            show(Math.min(0, ...list));
            show(fn(...xs, 10));
            show(fn.call(null, ...list));
            show(new Date(...[2000, 1, 2]).getFullYear());
        }
        f((JsValue a, JsValue b, JsValue c, JsValue d) => a + b + c + d, [4, 5, 6, 7]);
        "#,
        PRISTINE,
    );
    assert!(javascript.contains("...") , "{javascript}");
    assert_eq!(run(&javascript, SHOW), "3\n0\n16\n22\n2000\n");
}

/// A LilScript function's parameters are declared: it takes no spread.
#[test]
fn a_spread_into_a_lilscript_function_is_refused() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        "int sum(int a, int b) { return a + b; } int[] xs = [1, 2]; print(sum(...xs));",
    )
    .unwrap();
    let error = crate::analyze(&syntax).unwrap_err();
    assert!(format!("{error:?}").contains("spread argument"), "{error:?}");
}

/// `x as JsValue` views a typed value as the dynamic type with no code
/// (R12): the dynamic operations on it are JavaScript's.
#[test]
fn a_typed_value_viewed_as_js_value_keeps_its_operation() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        float half(float v) { return v / 2.0; }
        export void f(string text) {
            show((text as JsValue).slice(1));
            show((text as JsValue)["length"]);
            show((half as JsValue)(3));
            JsValue view = text as JsValue;
            string back = view as string;
            show(back + "!");
        }
        f("hello");
        "#,
        PRISTINE,
    );
    assert_eq!(run(&javascript, SHOW), "\"ello\"\n5\n1.5\n\"hello!\"\n");
}

/// A typed value is viewed as another type only through `JsValue`.
#[test]
fn a_typed_value_is_viewed_as_another_type_only_through_js_value() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "string s = \"a\"; int n = s as int;").unwrap();
    let error = crate::analyze(&syntax).unwrap_err();
    assert!(format!("{error:?}").contains("x as JsValue as int"), "{error:?}");
}

/// `v is C` and `v as? C` on classes (R13): `instanceof` over a class the
/// test keeps, and over a host class by its name.
#[test]
fn identity_tests_on_classes_are_instanceof() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern class Error { string message; init(string message); }
        extern JsValue globalThis;
        class Shape { float size; init(float size) { this.size = size; } }
        class Circle extends Shape { init(float size) { super(size); } }
        class Square extends Shape { init(float size) { super(size); } }
        string kind(Shape s) {
            if (s is Circle) { return "circle"; }
            if (s is Square) { return "square"; }
            return "shape";
        }
        export void f(JsValue error) {
            show(kind(new Circle(1.0)));
            show(kind(new Square(2.0)));
            show(kind(new Shape(3.0)));
            JsValue v = new Circle(4.0) as JsValue;
            show(v is Circle);
            Circle? c = v as? Circle;
            show(c != null);
            show(("x" as JsValue) is Circle);
            Shape? none = ("x" as JsValue) as? Shape;
            show(none == null);
            show(error is Error);
            show(v is Error);
        }
        f(new (globalThis.Error)("boom"));
        "#,
        PRISTINE,
    );
    assert!(javascript.contains("instanceof"), "{javascript}");
    assert_eq!(
        run(&javascript, SHOW),
        "\"circle\"\n\"square\"\n\"shape\"\ntrue\ntrue\nfalse\ntrue\ntrue\nfalse\n"
    );
}

/// An identity test names a class the value's type can hold.
#[test]
fn identity_tests_are_refused_outside_their_hierarchy() {
    let arena = bumpalo::Bump::new();
    for (source, message) in [
        (
            "class A { int x; init() { this.x = 1; } } class B { int y; init() { this.y = 2; } } bool f(A a) { return a is B; }",
            "neither a `JsValue` test nor a class extending",
        ),
        (
            "class Box<T> { T value; init(T value) { this.value = value; } } bool f(JsValue v) { return v is Box<int>; }",
            "generic class",
        ),
    ] {
        let syntax = crate::parse_source(&arena, source).unwrap();
        let error = crate::analyze(&syntax).unwrap_err();
        assert!(format!("{error:?}").contains(message), "{source}: {error:?}");
    }
}

/// A declared rest parameter (R7) takes the trailing arguments, one by one
/// or spread from an array, in a function, a method and an extern.
#[test]
fn declared_rest_parameters_take_the_trailing_arguments() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        extern void showAll(JsValue... values);
        int sum(int... values) {
            int total = 0;
            for (int value of values) { total = total + value; }
            return total;
        }
        string join(string separator, string... parts) {
            string out = "";
            for (int i = 0; i < parts.length; i = i + 1) {
                if (i > 0) { out = out + separator; }
                out = out + parts[i];
            }
            return out;
        }
        class Bag {
            int total;
            init() { this.total = 0; }
            void add(int... values) { for (int value of values) { this.total = this.total + value; } }
        }
        export void f() {
            show(sum());
            show(sum(1, 2, 3));
            int[] more = [4, 5];
            show(sum(...more));
            show(sum(1, ...more));
            show(join("-", "a", "b", "c"));
            show(join("-"));
            Bag bag = new Bag();
            bag.add(1, 2);
            bag.add();
            bag.add(...more);
            show(bag.total);
            showAll(1, "two", 3);
        }
        f();
        "#,
        PRISTINE,
    );
    let host = "globalThis.show=v=>console.log(JSON.stringify(v));globalThis.showAll=(...v)=>console.log(JSON.stringify(v));";
    assert_eq!(
        run(&javascript, host),
        "0\n6\n9\n10\n\"a-b-c\"\n\"\"\n12\n[1,\"two\",3]\n"
    );
}

/// A rest parameter is last and alone with no defaults before it.
#[test]
fn declared_rest_parameters_are_refused_with_defaults() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int f(int a = 1, int... rest) { return a; }").unwrap();
    let error = crate::analyze(&syntax).unwrap_err();
    assert!(format!("{error:?}").contains("takes no parameter defaults"), "{error:?}");
}

/// A method lambda's parameters may be typed (R7): trusted views of what
/// JavaScript passes (Y1).
#[test]
fn method_lambdas_take_typed_parameters() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        class Counter { int count; init(int count) { this.count = count; } }
        export void f(JsValue o) {
            o["next"] = (this JsValue self, int by) => by + 1;
            show(o.next(41));
            Counter counter = new Counter(5);
            JsValue view = counter as JsValue;
            view["read"] = (this Counter self, int add) => self.count + add;
            show(view.read(2));
        }
        f(object {});
        "#,
        PRISTINE,
    );
    assert_eq!(run(&javascript, SHOW), "42\n7\n");
}

/// `unknown` takes the tests and the ways out, and lowers as `JsValue`
/// (R12).
#[test]
fn unknown_takes_its_tests_and_ways_out() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export void f(unknown u) {
            show(u === null);
            show(u == null);
            show(typeof u);
            if (u is string) { show(u + "!"); }
            string? s = u as? string;
            show(s != null);
            show(float(u) + 1.0);
            show(bool(u));
            JsValue j = u as JsValue;
            show(j);
        }
        f("5" as JsValue as unknown);
        "#,
        PRISTINE,
    );
    assert_eq!(
        run(&javascript, SHOW),
        "false\nfalse\n\"string\"\n\"5!\"\ntrue\n6\ntrue\n\"5\"\n"
    );
}

/// Every other operation needs `unknown` narrowed first, and leaving it is
/// written, even to `JsValue`.
#[test]
fn unknown_refuses_other_operations() {
    let arena = bumpalo::Bump::new();
    for source in [
        "void f(unknown u) { JsValue j = u; }",
        "void f(unknown u, JsValue v) { bool b = u in v; }",
        "void f(unknown u) { unknown w = u; string s = w.name; }",
        "void f(unknown u) { float x = u + 1.0; }",
    ] {
        let syntax = crate::parse_source(&arena, source).unwrap();
        assert!(crate::analyze(&syntax).is_err(), "{source} was accepted");
    }
    let syntax = crate::parse_source(&arena, "void f(unknown u, JsValue v) { bool b = u in v; }").unwrap();
    let error = crate::analyze(&syntax).unwrap_err();
    assert!(format!("{error:?}").contains("narrowed before other operations"), "{error:?}");
}

/// Trusted crossings (R1): a typed `int` field is an int32 by type, so no
/// `|0` normalizes its load; arithmetic still wraps. An element read is in
/// range by R11's precondition, so it carries none either.
#[test]
fn typed_int_loads_are_not_normalized() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        class Counter { int count; init(int count) { this.count = count; } }
        export int read(Counter c) { return c.count; }
        export int next(int[] values, int i) { return values[i] + 1; }
        show(read(new Counter(41)));
        show(next([1, 2, 3], 1));
        "#,
        PRISTINE,
    );
    let read = javascript
        .split("read=function")
        .nth(1)
        .unwrap_or_default()
        .split('}')
        .next()
        .unwrap_or_default();
    assert!(read.contains("return") && !read.contains("|0"), "{javascript}");
    let next = javascript
        .split("next=function")
        .nth(1)
        .unwrap_or_default()
        .split('}')
        .next()
        .unwrap_or_default();
    assert_eq!(next.matches("|0").count(), 1, "the sum's wrap alone: {javascript}");
    assert_eq!(run(&javascript, SHOW), "41\n3\n");
}

/// Definite assignment (R3): a local declared without a value is read only
/// where every path has assigned it.
#[test]
fn definite_assignment_accepts_and_refuses_reads() {
    let arena = bumpalo::Bump::new();
    let accepted = [
        "int f(bool b) { int x; if (b) { x = 1; } else { x = 2; } return x; }",
        "int f(bool b) { int x; if (b) { return 0; } else { x = 2; } return x; }",
        "int f(bool b) { int x; x = 1; if (b) { x = 2; } return x; }",
        "int f() { int x; try { x = 1; } finally { } return 0; }",
        "int f() { int x; x = 3; auto g = () => x; return g(); }",
    ];
    for source in accepted {
        let syntax = crate::parse_source(&arena, source).unwrap();
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    }
    let refused = [
        "int f(bool b) { int x; if (b) { x = 1; } return x; }",
        "int f(int n) { int x; while (n > 0) { x = n; n = n - 1; } return x; }",
        "int f() { int x; x += 1; return x; }",
        "int f(bool b) { int x; bool ok = b && ((x = 1) > 0); return x; }",
        "int f() { int x; auto g = () => x; x = 1; return g(); }",
        "int f() { int x; try { x = 1; } catch (auto e) { } return x; }",
    ];
    for source in refused {
        let syntax = crate::parse_source(&arena, source).unwrap();
        let error = crate::analyze(&syntax).expect_err(source);
        assert!(
            format!("{error:?}").contains("read before it is assigned"),
            "{source}: {error:?}"
        );
    }
    let syntax = crate::parse_source(&arena, "int x; print(1);").unwrap();
    let error = crate::analyze(&syntax).unwrap_err();
    assert!(format!("{error:?}").contains("module-level"), "{error:?}");
}

/// A declared local runs as JavaScript's `let x;` (R3).
#[test]
fn declared_locals_run() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export int pick(bool first, int a, int b) {
            int chosen;
            if (first) { chosen = a * 3 + b; } else { chosen = b * 5 - a; }
            return chosen;
        }
        show(pick(true, 4, 5));
        show(pick(false, 4, 5));
        "#,
        PRISTINE,
    );
    assert_eq!(run(&javascript, SHOW), "17\n21\n");
}


/// Field initializers (R3): each construction evaluates a field's own
/// initializer before `init`, a fresh value every time and the base's first,
/// for a generic class and for a derived class kept as JavaScript's (its
/// constructor is exported), where they run after `super()`.
#[test]
fn field_initializers_run_at_construction() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        int made = 0;
        int serial() { made = made + 1; return made; }
        class Bag<T> {
            T[] items = [];
            int limit = 3;
        }
        class Base {
            int base = 40 + 2;
            string[] names = ["a"];
            int id = serial();
            init() { }
        }
        class Counter extends Base {
            int count = 5;
            int later;
            init(int start) {
                super();
                this.later = this.count + start;
            }
        }
        export constructor Counter;
        export int total(int start) {
            Counter c = new Counter(start);
            c.names.push("b");
            Counter d = new Counter(1);
            Bag<int> bag = new Bag<int>();
            bag.items.push(start);
            Bag<int> other = new Bag<int>();
            return c.base + c.count + c.later + c.names.length * 100 + d.names.length * 1000
                + bag.items.length * 10000 + other.items.length * 100000 + bag.limit * 1000000
                + (c.id * 10 + d.id) * 10000000;
        }
        show(total(3));
        show(made);
        "#,
        PRISTINE,
    );
    assert_eq!(run(&javascript, SHOW), "123011255\n2\n");
}

/// A field initializer is a value of the field's type; an extern class's
/// field is the host's and a struct's comes from its literal, so neither
/// takes one.
#[test]
fn field_initializers_are_checked_and_refused_where_they_mean_nothing() {
    let arena = bumpalo::Bump::new();
    let syntax =
        crate::parse_source(&arena, "class C { int n = \"one\"; } C c = new C(); print(c.n);")
            .unwrap();
    let error = crate::analyze(&syntax).unwrap_err();
    assert!(format!("{error:?}").contains("string"), "{error:?}");
    let syntax = crate::parse_source(&arena, "extern class Host { int n = 1; } print(1);").unwrap();
    let error = crate::analyze(&syntax).unwrap_err();
    assert!(format!("{error:?}").contains("the host's"), "{error:?}");
    let error = crate::parse_source(&arena, "struct P { int x = 1; } print(1);").unwrap_err();
    assert!(format!("{error:?}").contains("construction literal"), "{error:?}");
}

/// `debug` (R15) is a modifier only before `void` or `extern void`; it
/// refuses `pure`, suspension, a result, and values and classes, and `debug`
/// stays an identifier everywhere else.
#[test]
fn debug_declarations_parse_and_refuse() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        "debug void trace(string m) { print(m); }\ndebug extern void invariant(bool ok, string m);\nint debug = 3;\nprint(debug);\n",
    )
    .unwrap();
    let debug = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            crate::ast::Item::Function(function) => Some(function.declared_debug),
            crate::ast::Item::Extern(declaration) => Some(declaration.declared_debug),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(debug, [true, true]);
    crate::analyze(&syntax).unwrap();
    for (source, message) in [
        ("pure debug void f() { }", "cannot be `pure`"),
        ("async debug void f() { }", "runs to completion"),
        ("debug extern int f();", "returns `void`"),
        ("debug extern JsValue x;", "not values"),
        ("debug extern class C { }", "not classes"),
    ] {
        let error = crate::parse_source(&arena, source).expect_err(source);
        assert!(format!("{error:?}").contains(message), "{source}: {error:?}");
    }
}

/// Typed operations mean ECMAScript's originals (R10), whatever
/// `assume_pristine_builtins` says: a literal `Regex` construction is the
/// regex literal, and a length is an int32 with no `|0`.
#[test]
fn typed_operations_mean_the_originals_without_pristine_builtins() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export int size(string s) { return s.length; }
        export bool digits(string s) { Regex pattern = new Regex("^[0-9]+$"); return pattern.test(s); }
        show(size("abc"));
        show(digits("123"));
        show(digits("12a"));
        "#,
        "",
    );
    assert!(javascript.contains("/^[0-9]+$/"), "{javascript}");
    assert!(!javascript.contains("RegExp"), "{javascript}");
    assert!(!javascript.contains("length|0"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "3\ntrue\nfalse\n");
}

/// `checks = "development"` (the contract axis; R11's precondition): an index
/// read out of range throws a RangeError through one hoisted helper, where a
/// production build reads past the end.
#[test]
fn development_checks_index_reads() {
    let source = r#"
        extern void show(JsValue value);
        export int at(int[] values, int i) { return values[i]; }
        export string letter(string s, int i) { return s[i]; }
        show(at([4, 5], 1));
        try { show(at([4, 5], 2)); } catch { show("range"); }
        try { show(letter("ab", -1)); } catch { show("range"); }
        show(letter("ab", 0));
    "#;
    let development = compile_with(source, "[javascript]\nchecks = \"development\"\n");
    assert_eq!(development.matches("RangeError").count(), 1, "{development}");
    assert_eq!(run(&development, SHOW), "5\n\"range\"\n\"range\"\n\"a\"\n");
    // Production checks nothing: past the end is unspecified (R11), and an
    // element read is the plain read.
    let production = compile_with(source, "");
    assert!(!production.contains("RangeError"), "{production}");
    assert!(!production.contains("??\"\""), "{production}");
}

/// R11: `%` with a `float` operand is a `float`; a bitwise operator takes a
/// `float` operand through ToInt32 and gives an `int`; `codeUnitAt` is a
/// code unit with no `|0`.
#[test]
fn float_remainder_bitwise_conversion_and_code_units() {
    let javascript = compile_with(
        r#"
        extern void show(JsValue value);
        export float rem(float a, float b) { return a % b; }
        export int bits(float a) { return a | 0; }
        export int unit(string s, int i) { return s.codeUnitAt(i); }
        show(rem(5.5, 2.0));
        show(bits(-1.75));
        show(bits(4294967297.5));
        show(unit("héllo", 1));
        "#,
        PRISTINE,
    );
    let unit = javascript
        .split("unit=function")
        .nth(1)
        .unwrap_or_default()
        .split('}')
        .next()
        .unwrap_or_default();
    assert!(unit.contains("charCodeAt") && !unit.contains("|0"), "{javascript}");
    assert_eq!(run(&javascript, SHOW), "1.5\n-1\n1\n233\n");
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int x = 5.5 % 2;").unwrap();
    assert!(crate::analyze(&syntax).is_err(), "a float remainder is a float");
}

/// Development checks at crossings (R1): an extern's result, a typed host
/// binding and a trusted view throw a TypeError where the host breaks the
/// declared type; well-typed values pass; a production build checks nothing.
#[test]
fn development_checks_crossings() {
    let source = r#"
        extern void show(JsValue value);
        extern int count();
        extern string? label;
        extern JsValue raw();
        export void run() {
            try { show(count() + 1); } catch { show("count"); }
            try { show(label ?? "none"); } catch { show("label"); }
            try { int n = raw() as int; show(n); } catch { show("view"); }
        }
        run();
    "#;
    let development = compile_with(source, "[javascript]\nchecks = \"development\"\n");
    let show = "globalThis.show=v=>console.log(JSON.stringify(v));";
    assert_eq!(
        run(&development, &format!("{show}globalThis.count=()=>'three';globalThis.label=7;globalThis.raw=()=>1.5;")),
        "\"count\"\n\"label\"\n\"view\"\n"
    );
    assert_eq!(
        run(&development, &format!("{show}globalThis.count=()=>3;globalThis.label=undefined;globalThis.raw=()=>2;")),
        "4\n\"none\"\n2\n"
    );
    let production = compile_with(source, "");
    assert!(!production.contains("TypeError"), "{production}");
}

/// Development checks at an export's entry (R1): a caller that passes the
/// wrong type meets a TypeError; a parameter with a default is left to it.
#[test]
fn development_checks_export_parameters() {
    let javascript = compile_with(
        "export int twice(int n, int step = 1) { return n * 2 + step; }\n",
        "[javascript]\nchecks = \"development\"\n",
    );
    let script = format!(
        "const library=await import('data:text/javascript,'+encodeURIComponent({}));\nconst seen=[library.twice(4)];\ntry{{library.twice('x')}}catch(e){{seen.push(e.name)}}\nconsole.log(JSON.stringify(seen));",
        serde_json::to_string(&javascript).unwrap()
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "[9,\"TypeError\"]\n", "{javascript}");
}

