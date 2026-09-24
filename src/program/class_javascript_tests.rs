//! Internal classes: an instance is a shared mutable
//! object, methods and `init` are statically dispatched functions taking it
//! first, and nothing that could hand an instance to host code is admitted.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};
use std::process::Command;

fn compile(source: &str) -> Result<String, CandidateError> {
    compile_named(source, Style::Global)
}

fn compile_named(source: &str, style: Style) -> Result<String, CandidateError> {
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
            let artifact = output.render(&Plan::new(style))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result);
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
        .expect("Node is required for class observation tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn instances_alias_fields_default_and_methods_dispatch_statically() {
    // Two references to one instance observe each other's writes; fields
    // start at their type's default before `init`; a derived class runs its
    // base's `init` through `super` and calls inherited methods statically.
    let source = r#"
        class Counter {
            int value;
            string label;
            int[] history;
            init(int start) { this.value = start; }
            int add(int amount) { this.value += amount; this.history.push(amount); return this.value; }
        }
        class Named extends Counter {
            string name;
            init(string name, int start) { super(start); this.name = name; }
            string describe() { return this.name + ":" + this.value + ":" + this.history.length; }
        }
        Counter first = new Counter(10);
        Counter alias = first;
        alias.add(5);
        print(first.value);
        print(first.label == "");
        Named named = new Named("n", 1);
        named.add(2);
        named.add(3);
        print(named.describe());
        Counter upcast = named;
        print(upcast.add(4));
    "#;
    assert_eq!(run(&compile(source).unwrap(), ""), "15\ntrue\nn:6:2\n10\n");
}

#[test]
fn generic_classes_defaults_and_field_held_functions_execute() {
    let source = r#"
        class Box<T> {
            T value;
            func(T)->T step;
            init(T value, func(T)->T step = (T current) => current) {
                this.value = value;
                this.step = step;
            }
            T next() { this.value = this.step(this.value); return this.value; }
        }
        class Settings { int retries; init(int retries = 3) { this.retries = retries; } }
        int budget(Settings settings = new Settings()) { return settings.retries * 2; }
        Box<int> counting = new Box(1, (int value) => value + 1);
        Box<string> fixed = new Box("same");
        print(counting.next());
        print(counting.next());
        print(fixed.next());
        print(budget());
        print(budget(new Settings(5)));
    "#;
    assert_eq!(run(&compile(source).unwrap(), ""), "2\n3\nsame\n6\n10\n");
}

#[test]
fn a_class_instance_reaches_host_code_as_its_data_object() {
    // An instance is exactly its fields as own data properties; methods are
    // statically dispatched and never members, as on the old route. Hosts
    // see and share the same object, and program code sees their writes.
    let source = r#"
        class Handle {
            string type;
            JsValue data;
            int count;
            init(string type) { this.type = type; }
            int bump() { this.count = this.count + 1; return this.count; }
        }
        class Tagged extends Handle { string tag; init(string tag) { super("tagged"); this.tag = tag; } }
        extern void keep(JsValue value);
        extern JsValue fetch();
        Handle handle = new Handle("click");
        JsValue shared = handle;
        keep(shared);
        handle.bump();
        print(JS.strictEqual(fetch(), shared));
        print(handle.count);
        print(handle.data);
        keep(new Tagged("t"));
        export Handle make() { return new Handle("made"); }
    "#;
    let host = "let held;globalThis.keep=v=>{held=v;v.data=7;console.log(JSON.stringify(v),typeof v.bump)};globalThis.fetch=()=>held;";
    assert_eq!(
        run(&compile(source).unwrap(), host),
        "{\"type\":\"click\",\"data\":7,\"count\":0} undefined\ntrue\n1\n7\n{\"type\":\"tagged\",\"data\":7,\"count\":0,\"tag\":\"t\"} undefined\n"
    );
}

#[test]
fn class_instance_allocations_verify_their_declared_fields() {
    let arena = bumpalo::Bump::new();
    let source = "class C{int a;int b;}C c=new C();print(c.a+c.b);";
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    // Dropping a declared field from the table leaves the allocation with a
    // key the class no longer declares.
    let mut broken = program.clone();
    std::sync::Arc::make_mut(&mut broken.classes)[0]
        .fields
        .pop();
    assert!(broken.verify().is_err());
}

#[test]
fn methods_and_functions_taking_host_values_are_not_boundaries() {
    // A method's own cell holds its exact type, and a function that takes a
    // `JsValue` beside an instance hands the instance to no host code.
    let source = r#"
        extern JsValue hostValue();
        class State {
            JsValue effects;
            int count;
            init(JsValue effects) { this.effects = effects; }
            void bump(JsValue label) { this.effects = label; this.count = this.count + 1; }
        }
        void touch(JsValue label, State state) { state.bump(label); }
        State state = new State(hostValue());
        touch(hostValue(), state);
        touch(hostValue(), state);
        print(state.count);
        print(state.effects);
    "#;
    assert_eq!(
        run(
            &compile(source).unwrap(),
            "let n=0;globalThis.hostValue=()=>++n;"
        ),
        "2\n3\n"
    );
}

#[test]
fn extern_class_members_are_exact_host_properties_and_methods() {
    // An extern class names a host object: a field is an exact property
    // read or write (a getter runs once per source read), a method is a host
    // call on the receiver, and the value crosses to `JsValue` unchanged.
    let source = r#"
        extern class Point {
            float x;
            float y;
            int reads;
            float length();
        }
        extern JsValue makePoint(float x, float y);
        void shift(Point p, float by) { p.x = p.x + by; }
        Point p = JS.assume(makePoint(1.0, 2.0));
        shift(p, 3.0);
        print(p.x);
        print(p.length());
        print(p.reads);
        print(p.reads);
        JsValue back = p;
        JsValue again = p;
        print(JS.strictEqual(back, again));
    "#;
    let host = "let reads=0;globalThis.makePoint=(x,y)=>({x,y,get reads(){return ++reads},length(){return Math.hypot(this.x,this.y)}});";
    assert_eq!(
        run(&compile(source).unwrap(), host),
        "4\n4.47213595499958\n1\n2\ntrue\n"
    );
}

#[test]
fn an_object_key_converts_once_per_source_access() {
    let source = r#"
        extern JsValue table;
        extern JsValue key;
        JsValue first = table[key];
        print(first);
    "#;
    let host =
        "globalThis.key={toString(){console.log('convert');return 'a'}};globalThis.table={a:5};";
    assert_eq!(run(&compile(source).unwrap(), host), "convert\n5\n");
}

#[test]
fn a_for_in_key_never_shadows_a_binding_its_object_reads() {
    // `for (let k in object)` evaluates `object` while `k` is in its TDZ.
    // Scoped naming once reused the parameter's short name for the key,
    // printing `for(let a in a)`, which throws.
    let source = r#"
        bool plain(JsValue object) {
            string last = "";
            bool seen = false;
            for (string key in object) { last = key; seen = true; }
            return !seen || last != "";
        }
        extern JsValue probe;
        print(plain(probe));
    "#;
    for style in [Style::Global, Style::Scoped] {
        assert_eq!(
            run(
                &compile_named(source, style).unwrap(),
                "globalThis.probe={a:1};"
            ),
            "true\n"
        );
    }
}
